use chirp_workflow::prelude::*;

pub async fn start() -> GlobalResult<()> {
	let pools = rivet_pools::from_env().await?;

	run_from_env(pools).await?;

	Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn run_from_env(pools: rivet_pools::Pools) -> GlobalResult<()> {
	let reg = cluster::registry()?
		.merge(linode::registry()?)?
		.merge(ds::registry()?)?
		.merge(job_run::registry()?)?
		.merge(pegboard::registry()?)?;

	let db = db::DatabasePgNats::from_pools(pools.crdb()?, pools.nats()?);
	let worker = Worker::new(reg.handle(), db);

	// Start worker
	worker.wake_start(pools).await?;
	bail!("worker exited unexpectedly");
}
