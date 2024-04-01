use chirp_worker::prelude::*;

use ::linode_gc::run_from_env;

#[tokio::test(flavor = "multi_thread")]
async fn basic() {
	tracing_subscriber::fmt()
		.json()
		.with_max_level(tracing::Level::INFO)
		.with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
		.init();

	let pools = rivet_pools::from_env("linode-gc-test").await.unwrap();

	run_from_env(util::timestamp::now(), pools).await.unwrap();
}
