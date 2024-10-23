use chirp_workflow::prelude::*;

mod monitors;
use monitors::*;

pub async fn start(config: rivet_config::Config, pools: rivet_pools::Pools) -> GlobalResult<()> {
	run_from_env(config, pools).await
}

#[tracing::instrument(skip_all)]
pub async fn run_from_env(
	config: rivet_config::Config,
	pools: rivet_pools::Pools,
) -> GlobalResult<()> {
	let client = chirp_client::SharedClient::from_env(pools.clone())?.wrap_new("nomad-monitor");
	let cache = rivet_cache::CacheInner::from_env(pools.clone())?;
	let redis_job = pools.redis("persistent")?;
	let ctx = StandaloneCtx::new(
		chirp_workflow::compat::db_from_pools(&pools).await?,
		config,
		rivet_connection::Connection::new(client, pools, cache),
		"nomad-monitor",
	)
	.await?;

	// Start nomad event monitor
	let redis_index_key = "nomad:monitor_index";
	let configuration = nomad_util::new_build_config(ctx.config())?;

	nomad_util::monitor::Monitor::run(
		configuration,
		redis_job,
		redis_index_key,
		&["Allocation", "Evaluation", "Node"],
		|event| {
			let ctx = ctx.clone();
			async move {
				match handle(ctx.clone(), event).await {
					Ok(_) => {}
					Err(err) => {
						tracing::error!(?err, "error handling nomad event");
					}
				}
			}
		},
	)
	.await?;

	Ok(())
}

async fn handle(ctx: StandaloneCtx, event: nomad_util::monitor::NomadEvent) -> GlobalResult<()> {
	// TODO: Figure out how to abstract the branches
	if let Some(payload) = event.decode::<alloc_plan::PlanResult>("Allocation", "PlanResult")? {
		// let client = shared_client.wrap_new("nomad-alloc-plan-monitor");
		let spawn_res = tokio::task::Builder::new()
			.name("nomad_alloc_plan_monitor::handle_event")
			.spawn(async move {
				match alloc_plan::handle(ctx, &payload, event.payload.to_string()).await {
					Ok(_) => {}
					Err(err) => {
						tracing::error!(?err, ?payload, "error handling event");
					}
				}
			});
		if let Err(err) = spawn_res {
			tracing::error!(?err, "failed to spawn handle_event task");
		}
	} else if let Some(payload) =
		event.decode::<alloc_update::AllocationUpdated>("Allocation", "AllocationUpdated")?
	{
		// let client = shared_client.wrap_new("nomad-alloc-updated-monitor");
		let spawn_res = tokio::task::Builder::new()
			.name("nomad_alloc_update_monitor::handle_event")
			.spawn(async move {
				match alloc_update::handle(ctx, &payload, event.payload.to_string()).await {
					Ok(_) => {}
					Err(err) => {
						tracing::error!(?err, ?payload, "error handling event");
					}
				}
			});
		if let Err(err) = spawn_res {
			tracing::error!(?err, "failed to spawn handle_event task");
		}
	} else if let Some(payload) =
		event.decode::<eval_update::PlanResult>("Evaluation", "EvaluationUpdated")?
	{
		// let client = shared_client.wrap_new("nomad-eval-update-monitor");
		let spawn_res = tokio::task::Builder::new()
			.name("nomad_eval_update_monitor::handle_event")
			.spawn(async move {
				match eval_update::handle(ctx, &payload, event.payload.to_string()).await {
					Ok(_) => {}
					Err(err) => {
						tracing::error!(?err, ?payload, "error handling event");
					}
				}
			});
		if let Err(err) = spawn_res {
			tracing::error!(?err, "failed to spawn handle_event task");
		}
	} else if let Some(payload) =
		event.decode::<node_registration::NodeRegistration>("Node", "NodeRegistration")?
	{
		// let client = shared_client.wrap_new("nomad-node-registration-monitor");
		let spawn_res = tokio::task::Builder::new()
			.name("nomad_node_registration_monitor::handle")
			.spawn(async move {
				match node_registration::handle(ctx, &payload).await {
					Ok(_) => {}
					Err(err) => {
						tracing::error!(?err, ?payload, "error handling event");
					}
				}
			});
		if let Err(err) = spawn_res {
			tracing::error!(?err, "failed to spawn handle_event task");
		}
	}

	Ok(())
}
