use std::{convert::Infallible, future::Future, net::SocketAddr, time::Instant};

use hyper::{
	body::HttpBody,
	server::conn::AddrStream,
	service::{make_service_fn, service_fn},
	Body, Request, Response, Server,
};
use tracing::Instrument;
use uuid::Uuid;

#[tracing::instrument(skip_all)]
pub fn start<T: 'static, Fut>(handle: T)
where
	T: Fn(
			chirp_client::SharedClientHandle,
			rivet_pools::Pools,
			rivet_cache::Cache,
			Uuid,
			Request<Body>,
		) -> Fut
		+ Send
		+ Sync
		+ Copy,
	Fut: Future<Output = Result<Response<Body>, http::Error>> + Send,
{
	rivet_runtime::run(start_fut(handle)).unwrap()
}

#[tracing::instrument(skip_all)]
async fn start_fut<T: 'static, Fut>(handle: T)
where
	T: Fn(
			chirp_client::SharedClientHandle,
			rivet_pools::Pools,
			rivet_cache::Cache,
			Uuid,
			Request<Body>,
		) -> Fut
		+ Send
		+ Sync
		+ Copy,
	Fut: Future<Output = Result<Response<Body>, http::Error>> + Send,
{
	let pools = rivet_pools::from_env("api").await.expect("create pool");
	let shared_client = chirp_client::SharedClient::from_env(pools.clone()).expect("create client");

	let cache = rivet_cache::CacheInner::from_env(pools.clone()).expect("create cache");

	let health_check_config = rivet_health_checks::Config {
		pools: Some(pools.clone()),
	};
	tokio::task::Builder::new()
		.name("api_helper::run_standalone")
		.spawn(rivet_health_checks::run_standalone(
			health_check_config.clone(),
		))
		.unwrap();

	tokio::task::Builder::new()
		.name("rivet_metrics::run_standalone")
		.spawn(rivet_metrics::run_standalone())
		.unwrap();

	let port: u16 = std::env::var("PORT")
		.ok()
		.and_then(|v| v.parse::<u16>().ok())
		.unwrap();

	// A `MakeService` that produces a `Service` to handle each connection
	let make_service = make_service_fn(move |conn: &AddrStream| {
		let shared_client = shared_client.clone();
		let pools = pools.clone();
		let cache = cache.clone();

		// Create a `Service` for responding to the request
		let remote_addr = conn.remote_addr();
		let service = service_fn(move |req: Request<Body>| {
			let start = Instant::now();

			let shared_client = shared_client.clone();
			let pools = pools.clone();
			let cache = cache.clone();

			// Handle request
			let ray_id = Uuid::new_v4();
			let req_span = tracing::info_span!("http request", method = %req.method(), uri = %req.uri(), %ray_id);
			async move {
				tracing::info!(
					method = %req.method(),
					uri = %req.uri(),
					headers = ?req.headers(),
					body_size_hint = ?req.body().size_hint(),
					remote_addr = %remote_addr,
					"http request meta"
				);

				let res = tokio::task::Builder::new()
					.name("api_helper::handle")
					.spawn(
						async move {
							let mut res = handle(shared_client, pools, cache, ray_id, req).await?;
							res.headers_mut()
								.insert("rvt-ray-id", ray_id.to_string().parse()?);
							Result::<Response<Body>, http::Error>::Ok(res)
						}
						.in_current_span(),
					);
				let res = match res {
					Ok(res) => match res.await {
						Ok(res) => match res {
							Ok(res) => res,
							Err(err) => {
								tracing::error!(?err, "http error");
								return Err(err);
							}
						},
						Err(_) => {
							tracing::error!("http error");
							return Ok(Response::builder()
								.status(http::StatusCode::INTERNAL_SERVER_ERROR)
								.body(Body::empty())?);
						}
					},
					Err(err) => {
						tracing::error!(?err, "tokio spawn error");
						return Ok(Response::builder()
							.status(http::StatusCode::INTERNAL_SERVER_ERROR)
							.body(Body::empty())?);
					}
				};

				if res.status().is_server_error() {
					tracing::error!(status = ?res.status().as_u16(), "http server error");
				} else if res.status().is_client_error() {
					tracing::warn!(status = ?res.status().as_u16(), "http client error");
				} else if res.status().is_redirection() {
					tracing::info!(status = ?res.status().as_u16(), "http redirection");
				} else if res.status().is_informational() {
					tracing::info!(status = ?res.status().as_u16(), "http informational");
				}

				let duration = start.elapsed().as_secs_f64();
				tracing::info!(
					status = %res.status().as_u16(),
					headers = ?res.headers(),
					body_size_hint = ?res.body().size_hint(),
					duration = duration,
					"http response meta"
				);

				Ok::<_, http::Error>(res)
			}
			.instrument(req_span)
		});

		// Return the service to hyper
		async move { Ok::<_, Infallible>(service) }
	});

	let addr = SocketAddr::from(([0, 0, 0, 0], port));
	let server = Server::bind(&addr).serve(make_service);

	tracing::info!(?port, "server listening");
	if let Err(e) = server.await {
		eprintln!("server error: {}", e);
	}
}
