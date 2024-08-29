use std::time::Duration;

use chirp_worker::prelude::*;
use rivet_operation::prelude::proto::backend::pkg::*;
use serde::Deserialize;

use crate::util::NEW_NOMAD_CONFIG;

// TODO:
const TRAEFIK_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlanResult {
	allocation: nomad_client::models::Allocation,
}

#[derive(Debug, sqlx::FromRow)]
struct RunRow {
	server_id: Uuid,
	datacenter_id: Uuid,
	connectable_ts: Option<i64>,
	nomad_alloc_plan_ts: Option<i64>, // this was nomad_plan_ts
}

#[derive(Clone)]
struct RunData {
	job_id: String,
	alloc_id: String,
	nomad_node_id: String,
	nomad_node_name: String,
	nomad_node_public_ipv4: String,
	nomad_node_vlan_ipv4: String,
	ports: Vec<Port>,
}

#[derive(Clone, Debug)]
struct Port {
	label: String,
	source: u32,
	target: u32,
	ip: String,
}

#[worker(name = "ds-nomad-monitor-alloc-plan")]
async fn worker(
	ctx: &OperationContext<nomad::msg::monitor_alloc_plan::Message>,
) -> GlobalResult<()> {
	let PlanResult { allocation: alloc } = serde_json::from_str(&ctx.payload_json)?;
	tracing::info!(?alloc, "from nomad");

	let job_id = unwrap_ref!(alloc.job_id, "alloc has no job id");
	let alloc_id = unwrap_ref!(alloc.ID);
	let nomad_node_id = unwrap_ref!(alloc.node_id, "alloc has no node id");
	let _nomad_node_name = unwrap_ref!(alloc.node_id, "alloc has no node name");

	// Fetch node metadata
	let node = nomad_client::apis::nodes_api::get_node(
		&NEW_NOMAD_CONFIG,
		nomad_node_id,
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
	let mut meta = unwrap!(node.meta);

	// Read ports
	let mut ports = Vec::new();
	let alloc_resources = unwrap_ref!(alloc.resources);
	if let Some(networks) = &alloc_resources.networks {
		for network in networks {
			let network_ip = unwrap_ref!(network.IP);

			if let Some(dynamic_ports) = &network.dynamic_ports {
				for port in dynamic_ports {
					// Don't share connect proxy ports
					let label = unwrap_ref!(port.label);
					ports.push(Port {
						label: label.clone(),
						source: *unwrap_ref!(port.value) as u32,
						target: *unwrap_ref!(port.to) as u32,
						ip: network_ip.clone(),
					});
				}
			}
		}
	} else {
		tracing::info!("no network returned");
	}

	// Wait for Traefik to be ready
	tokio::time::sleep(TRAEFIK_GRACE_PERIOD).await;

	// Fetch the run
	//
	// Backoff mitigates race condition with job-run-create not having inserted
	// the dispatched_job_id yet.
	let run_data = RunData {
		job_id: job_id.clone(),
		alloc_id: alloc_id.clone(),
		nomad_node_id: nomad_node_id.clone(),
		nomad_node_name: unwrap!(node.name),
		nomad_node_public_ipv4: unwrap!(meta.remove("network-public-ipv4")),
		nomad_node_vlan_ipv4: unwrap!(meta.remove("network-vlan-ipv4")),
		ports: ports.clone(),
	};
	let db_output = rivet_pools::utils::crdb::tx(&ctx.crdb().await?, |tx| {
		let ctx = ctx.clone();
		let now = ctx.ts();
		let run_data = run_data.clone();
		Box::pin(update_db(ctx, tx, now, run_data))
	})
	.await?;

	// Check if run found
	let Some(DbOutput { server_id }) = db_output else {
		tracing::error!(
			?job_id,
			"run not found, may be race condition with insertion"
		);
		return Ok(());
	};

	tracing::info!(%job_id, %server_id, "updated run");

	Ok(())
}

#[derive(Debug)]
struct DbOutput {
	server_id: Uuid,
}

/// Returns `None` if the run could not be found.
#[tracing::instrument(skip_all)]
async fn update_db(
	ctx: OperationContext<nomad::msg::monitor_alloc_plan::Message>,
	tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
	now: i64,
	RunData {
		job_id,
		alloc_id,
		nomad_node_id,
		nomad_node_name,
		nomad_node_public_ipv4,
		nomad_node_vlan_ipv4,
		ports,
	}: RunData,
) -> GlobalResult<Option<DbOutput>> {
	let run_row = sql_fetch_optional!(
		[ctx, RunRow, @tx tx]
		"
		SELECT
			s.server_id,
			s.datacenter_id,
			s.connectable_ts,
			s.stop_ts,
			sn.nomad_alloc_plan_ts
		FROM db_ds.server_nomad AS sn
		INNER JOIN db_ds.servers AS s
		ON s.server_id = sn.server_id
		WHERE sn.nomad_dispatched_job_id = $1
		FOR UPDATE OF sn
		",
		&job_id,
	)
	.await?;

	// Check if run found
	let run_row = if let Some(run_row) = run_row {
		run_row
	} else {
		tracing::info!("caught race condition with ds-server-create");
		return Ok(None);
	};
	let server_id = run_row.server_id;

	if run_row.connectable_ts.is_some() {
		tracing::warn!("connectable ts already set");
	} else {
		sql_execute!(
			[ctx, @tx tx]
			"
			UPDATE db_ds.servers
			SET connectable_ts = $2
			WHERE server_id = $1
			",
			server_id,
			now,
		)
		.await?;
	}

	// Write run meta on first plan
	if run_row.nomad_alloc_plan_ts.is_none() {
		// Write alloc information
		sql_execute!(
			[ctx, @tx tx]
			"
			UPDATE
				db_ds.server_nomad
			SET
				nomad_alloc_id = $2,
				nomad_alloc_plan_ts = $3,
				nomad_node_id = $4,
				nomad_node_name = $5,
				nomad_node_public_ipv4 = $6,
				nomad_node_vlan_ipv4 = $7
			WHERE server_id = $1
			",
			server_id,
			&alloc_id,
			now,
			&nomad_node_id,
			&nomad_node_name,
			&nomad_node_public_ipv4,
			&nomad_node_vlan_ipv4,
		)
		.await?;

		tracing::info!(?ports, "got ds ports");

		// Save the ports to the db
		for port in &ports {
			tracing::info!(%server_id, label = %port.label, source = port.source, target = port.target, ip = %port.ip, "inserting ds port");
			sql_execute!(
				[ctx, @tx tx]
				"
				INSERT INTO db_ds.internal_ports (
					server_id,
					nomad_label,
					nomad_source,
					nomad_ip
				)
				VALUES ($1, $2, $3, $4)
				",
				server_id,
				&port.label,
				port.source as i64,
				&port.ip,
			)
			.await?;
		}

		// Invalidate cache when ports are updated
		if !ports.is_empty() {
			ctx.cache()
				.purge("servers_ports", [run_row.datacenter_id])
				.await?;
		}
	}

	Ok(Some(DbOutput { server_id }))
}
