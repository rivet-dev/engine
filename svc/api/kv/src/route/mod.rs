use api_helper::{define_router, util::CorsConfigBuilder};
use hyper::{Body, Request, Response};
use rivet_api::models;

mod batch_operations;
mod operations;

pub async fn handle(
	shared_client: chirp_client::SharedClientHandle,
	config: rivet_config::Config,
	pools: rivet_pools::Pools,
	cache: rivet_cache::Cache,
	ray_id: uuid::Uuid,
	request: Request<Body>,
) -> Result<Response<Body>, http::Error> {
	let response = Response::builder();

	// Handle route
	Router::handle(
		shared_client,
		config,
		pools,
		cache,
		ray_id,
		request,
		response,
	)
	.await
}

define_router! {
	cors: |config| CorsConfigBuilder::public().build(),
	routes: {
		"entries": {
			GET: operations::get(
				query: operations::SingleQuery,
				rate_limit: {
					buckets: [
						{ count: 1_000_000 },
					],
				},
			),
			PUT: operations::put(
				body: models::KvPutRequest,
				rate_limit: {
					buckets: [
						{ count: 100_000 },
					],
				},
			),
			DELETE: operations::delete(
				query: operations::SingleQuery,
				rate_limit: {
					buckets: [
						{ count: 100_000 },
					],
				},
			),
		},
		"entries" / "list": {
			GET: operations::list(query: operations::ListQuery),
		},
		"entries" / "batch": {
			GET: batch_operations::get_batch(
				query: batch_operations::BatchQuery,
				rate_limit: {
					buckets: [
						{ count: 1_000_000 },
					],
				},
			),
			PUT: batch_operations::put_batch(
				body: models::KvPutBatchRequest,
				rate_limit: {
					buckets: [
						{ count: 100_000 },
					],
				},
			),
			DELETE: batch_operations::delete_batch(
				query: batch_operations::BatchQuery,
				rate_limit: {
					buckets: [
						{ count: 100_000 },
					],
				},
			),
		},
	},
}
