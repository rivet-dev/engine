use std::convert::{TryFrom, TryInto};

use backend::cluster::PoolType::*;
use proto::backend;
use rivet_operation::prelude::*;
use util_cluster::metrics;

#[derive(sqlx::FromRow)]
struct ServerRow {
	datacenter_id: Uuid,
	pool_type: i64,
	is_provisioned: bool,
	is_installed: bool,
	has_nomad_node: bool,
	is_draining: bool,
	is_drained: bool,
	is_tainted: bool,
}

struct Server {
	datacenter_id: Uuid,
	pool_type: backend::cluster::PoolType,
	is_provisioned: bool,
	is_installed: bool,
	has_nomad_node: bool,
	is_draining: bool,
	is_tainted: bool,
}

impl TryFrom<ServerRow> for Server {
	type Error = GlobalError;

	fn try_from(value: ServerRow) -> GlobalResult<Self> {
		Ok(Server {
			datacenter_id: value.datacenter_id,
			pool_type: unwrap!(backend::cluster::PoolType::from_i32(value.pool_type as i32)),
			is_provisioned: value.is_provisioned,
			is_installed: value.is_installed,
			has_nomad_node: value.has_nomad_node,
			is_tainted: value.is_tainted,
			is_draining: value.is_draining && !value.is_drained,
		})
	}
}

#[tracing::instrument(skip_all)]
pub async fn run_from_env(_ts: i64, pools: rivet_pools::Pools) -> GlobalResult<()> {
	let client =
		chirp_client::SharedClient::from_env(pools.clone())?.wrap_new("cluster-metrics-publish");
	let cache = rivet_cache::CacheInner::from_env(pools.clone())?;
	let ctx = OperationContext::new(
		"cluster-metrics-publish".into(),
		std::time::Duration::from_secs(60),
		rivet_connection::Connection::new(client, pools, cache),
		Uuid::new_v4(),
		Uuid::new_v4(),
		util::timestamp::now(),
		util::timestamp::now(),
		(),
		Vec::new(),
	);

	let servers = select_servers(&ctx).await?;

	let datacenters_res = op!([ctx] cluster_datacenter_get {
		datacenter_ids: servers
			.iter()
			.map(|s| s.datacenter_id.into())
			.collect::<Vec<_>>(),
	})
	.await?;

	for dc in &datacenters_res.datacenters {
		insert_metrics(dc, &servers)?;
	}

	Ok(())
}

async fn select_servers(ctx: &OperationContext<()>) -> GlobalResult<Vec<Server>> {
	let servers = sql_fetch_all!(
		[ctx, ServerRow]
		"
		SELECT
			datacenter_id, pool_type,
			(provider_server_id IS NOT NULL) AS is_provisioned,
			(install_complete_ts IS NOT NULL) AS is_installed,
			(nomad_node_id IS NOT NULL) AS has_nomad_node,
			(drain_ts IS NOT NULL) AS is_draining,
			(drain_complete_ts IS NOT NULL) AS is_drained,
			(taint_ts IS NOT NULL) AS is_tainted
		FROM db_cluster.servers AS OF SYSTEM TIME '-5s'
		WHERE
			-- Filters out servers that are being destroyed/already destroyed
			cloud_destroy_ts IS NULL
		",
	)
	.await?;

	servers
		.into_iter()
		.map(TryInto::try_into)
		.collect::<GlobalResult<Vec<_>>>()
}

fn insert_metrics(dc: &backend::cluster::Datacenter, servers: &[Server]) -> GlobalResult<()> {
	let datacenter_id = unwrap_ref!(dc.datacenter_id).as_uuid();
	let servers_in_dc = servers.iter().filter(|s| s.datacenter_id == datacenter_id);

	let datacenter_id = datacenter_id.to_string();
	let cluster_id = unwrap_ref!(dc.cluster_id).as_uuid().to_string();

	let servers_per_pool = [
		(
			Job,
			servers_in_dc
				.clone()
				.filter(|s| matches!(s.pool_type, Job))
				.collect::<Vec<_>>(),
		),
		(
			Gg,
			servers_in_dc
				.clone()
				.filter(|s| matches!(s.pool_type, Gg))
				.collect::<Vec<_>>(),
		),
		(
			Ats,
			servers_in_dc
				.clone()
				.filter(|s| matches!(s.pool_type, Ats))
				.collect::<Vec<_>>(),
		),
	];

	// Aggregate all states per pool type
	for (pool_type, servers) in servers_per_pool {
		let mut provisioning = 0;
		let mut installing = 0;
		let mut active = 0;
		let mut nomad = 0;
		let mut draining = 0;
		let mut tainted = 0;

		for server in servers {
			if server.is_draining {
				draining += 1;
			} else if server.is_provisioned {
				if server.is_installed {
					active += 1;

					if server.has_nomad_node {
						nomad += 1;
					}
				} else {
					installing += 1;
				}
			} else {
				provisioning += 1;
			}

			if server.is_tainted {
				tainted += 1;
			}
		}

		let labels = [
			cluster_id.as_str(),
			datacenter_id.as_str(),
			&dc.provider_datacenter_id,
			&dc.name_id,
			match pool_type {
				Job => "job",
				Gg => "gg",
				Ats => "ats",
			},
		];

		metrics::PROVISIONING_SERVERS
			.with_label_values(&labels)
			.set(provisioning);
		metrics::INSTALLING_SERVERS
			.with_label_values(&labels)
			.set(installing);
		metrics::ACTIVE_SERVERS
			.with_label_values(&labels)
			.set(active);
		metrics::DRAINING_SERVERS
			.with_label_values(&labels)
			.set(draining);
		metrics::TAINTED_SERVERS
			.with_label_values(&labels)
			.set(tainted);

		if let Job = pool_type {
			metrics::NOMAD_SERVERS
				.with_label_values(&[
					&cluster_id,
					&datacenter_id,
					&dc.provider_datacenter_id,
					&dc.name_id,
				])
				.set(nomad);
		}
	}

	Ok(())
}
