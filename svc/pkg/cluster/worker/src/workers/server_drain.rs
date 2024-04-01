use chirp_worker::prelude::*;
use nomad_client::{
	apis::{configuration::Configuration, nodes_api},
	models,
};
use proto::backend::{self, pkg::*};

lazy_static::lazy_static! {
	static ref NOMAD_CONFIG: Configuration =
		nomad_util::new_config_from_env().unwrap();
}

#[derive(sqlx::FromRow)]
struct Server {
	datacenter_id: Uuid,
	pool_type: i64,
	nomad_node_id: Option<String>,
}

#[worker(name = "cluster-server-drain")]
async fn worker(ctx: &OperationContext<cluster::msg::server_drain::Message>) -> GlobalResult<()> {
	let server_id = unwrap_ref!(ctx.server_id).as_uuid();

	let server = sql_fetch_one!(
		[ctx, Server]
		"
		SELECT
			datacenter_id, pool_type, nomad_node_id
		FROM db_cluster.servers
		WHERE server_id = $1
		",
		server_id,
	)
	.await?;

	// Fetch datacenter config
	let datacenter_res = op!([ctx] cluster_datacenter_get {
		datacenter_ids: vec![server.datacenter_id.into()],
	})
	.await?;
	let datacenter = unwrap!(datacenter_res.datacenters.first());

	let pool_type = unwrap!(backend::cluster::PoolType::from_i32(
		server.pool_type as i32
	));
	match pool_type {
		backend::cluster::PoolType::Job => {
			let Some(nomad_node_id) = server.nomad_node_id else {
				tracing::error!("server does not have nomad running, cannot drain");
				return Ok(());
			};

			nodes_api::update_node_drain(
				&NOMAD_CONFIG,
				&nomad_node_id,
				models::NodeUpdateDrainRequest {
					drain_spec: Some(Box::new(models::DrainSpec {
						deadline: Some(datacenter.drain_timeout as i64),
						ignore_system_jobs: None,
					})),
					mark_eligible: None,
					meta: None,
					node_id: Some(nomad_node_id.clone()),
				},
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
			)
			.await?;

			// Prevent new matchmaker requests to the node running on this server
			msg!([ctx] mm::msg::nomad_node_closed_set(&nomad_node_id) {
				datacenter_id: Some(server.datacenter_id.into()),
				nomad_node_id: nomad_node_id.clone(),
				is_closed: true,
			})
			.await?;
		}
		backend::cluster::PoolType::Gg => {
			// Delete DNS record
			msg!([ctx] cluster::msg::server_dns_delete(server_id) {
				server_id: ctx.server_id,
			})
			.await?;
		}
		_ => {
			// Gracefully fail
			tracing::error!("cannot undrain this pool type: {:?}", pool_type);
		}
	}

	Ok(())
}
