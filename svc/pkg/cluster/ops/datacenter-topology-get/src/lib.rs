use std::collections::HashMap;

use nomad_client::apis::{allocations_api, configuration::Configuration, nodes_api};
use proto::backend::pkg::*;
use rivet_operation::prelude::*;

lazy_static::lazy_static! {
	static ref NOMAD_CONFIG: Configuration =
	nomad_util::new_config_from_env().unwrap();
}

#[derive(sqlx::FromRow)]
struct Server {
	server_id: Uuid,
	datacenter_id: Uuid,
	nomad_node_id: String,
}

#[operation(name = "cluster-datacenter-topology-get")]
pub async fn handle(
	ctx: OperationContext<cluster::datacenter_topology_get::Request>,
) -> GlobalResult<cluster::datacenter_topology_get::Response> {
	let datacenter_ids = ctx
		.datacenter_ids
		.iter()
		.map(common::Uuid::as_uuid)
		.collect::<Vec<_>>();

	let servers = sql_fetch_all!(
		[ctx, Server]
		"
		SELECT
			server_id, datacenter_id, nomad_node_id
		FROM db_cluster.servers
		WHERE
			datacenter_id = ANY($1) AND
			nomad_node_id IS NOT NULL AND
			cloud_destroy_ts IS NULL AND
			taint_ts IS NULL
		",
		&datacenter_ids,
	)
	.await?;

	// Fetch batch data from nomad
	let (allocation_info, node_info) = tokio::try_join!(
		async {
			allocations_api::get_allocations(
				&NOMAD_CONFIG,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				Some(true),
				None,
			)
			.await
			.map_err(Into::<GlobalError>::into)
		},
		async {
			nodes_api::get_nodes(
				&NOMAD_CONFIG,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				Some(true),
			)
			.await
			.map_err(Into::<GlobalError>::into)
		},
	)?;

	// Fill in empty datacenters
	let mut datacenters = datacenter_ids
		.iter()
		.map(|datacenter_id| {
			(
				*datacenter_id,
				cluster::datacenter_topology_get::response::Datacenter {
					datacenter_id: Some((*datacenter_id).into()),
					servers: Vec::new(),
				},
			)
		})
		.collect::<HashMap<_, _>>();

	for server in servers {
		let mut usage = cluster::datacenter_topology_get::response::Stats {
			cpu: 0,
			memory: 0,
			disk: 0,
		};

		// Aggregate all allocated resources for this node
		for alloc in &allocation_info {
			let alloc_node_id = unwrap_ref!(alloc.node_id);

			if alloc_node_id == &server.nomad_node_id {
				let resources = unwrap_ref!(alloc.allocated_resources);
				let shared_resources = unwrap_ref!(resources.shared);

				// Task states don't exist until a task starts
				if let Some(task_states) = &alloc.task_states {
					let tasks = unwrap_ref!(resources.tasks);

					for (task_name, task) in tasks {
						let task_state = unwrap!(task_states.get(task_name));
						let state = unwrap_ref!(task_state.state);

						// Only count pending, running, or failed tasks
						if state != "pending" && state != "running" && state != "failed" {
							continue;
						}

						let cpu = unwrap_ref!(task.cpu);
						let memory = unwrap_ref!(task.memory);

						usage.cpu += unwrap!(cpu.cpu_shares) as u64;
						usage.memory += unwrap!(memory.memory_mb) as u64;
					}
				}

				usage.disk += unwrap!(shared_resources.disk_mb) as u64;
			}
		}

		// Get node resource limits
		let node = unwrap!(
			node_info.iter().find(|node| node
				.ID
				.as_ref()
				.map_or(false, |node_id| node_id == &server.nomad_node_id)),
			format!("node not found {}", server.nomad_node_id)
		);
		let resources = unwrap_ref!(node.node_resources);
		let limits = cluster::datacenter_topology_get::response::Stats {
			cpu: unwrap!(unwrap_ref!(resources.cpu).cpu_shares) as u64,
			memory: unwrap!(unwrap_ref!(resources.memory).memory_mb) as u64,
			disk: unwrap!(unwrap_ref!(resources.disk).disk_mb) as u64,
		};

		let datacenter = datacenters.entry(server.datacenter_id).or_insert_with(|| {
			cluster::datacenter_topology_get::response::Datacenter {
				datacenter_id: Some(server.datacenter_id.into()),
				servers: Vec::new(),
			}
		});

		datacenter
			.servers
			.push(cluster::datacenter_topology_get::response::Server {
				server_id: Some(server.server_id.into()),
				node_id: server.nomad_node_id,
				usage: Some(usage),
				limits: Some(limits),
			});
	}

	Ok(cluster::datacenter_topology_get::Response {
		datacenters: datacenters.into_values().collect::<Vec<_>>(),
	})
}
