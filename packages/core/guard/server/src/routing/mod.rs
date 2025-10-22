use std::sync::Arc;

use anyhow::*;
use gas::prelude::*;
use hyper::header::HeaderName;
use rivet_guard_core::RoutingFn;

use crate::{errors, shared_state::SharedState};

mod api_public;
pub mod pegboard_gateway;
mod runner;

pub(crate) const X_RIVET_TARGET: HeaderName = HeaderName::from_static("x-rivet-target");
pub(crate) const X_RIVET_TOKEN: HeaderName = HeaderName::from_static("x-rivet-token");
pub(crate) const SEC_WEBSOCKET_PROTOCOL: HeaderName =
	HeaderName::from_static("sec-websocket-protocol");
pub(crate) const WS_PROTOCOL_TARGET: &str = "rivet_target.";

/// Creates the main routing function that handles all incoming requests
#[tracing::instrument(skip_all)]
pub fn create_routing_function(ctx: StandaloneCtx, shared_state: SharedState) -> RoutingFn {
	Arc::new(
		move |hostname: &str,
		      path: &str,
		      port_type: rivet_guard_core::proxy_service::PortType,
		      headers: &hyper::HeaderMap| {
			let ctx = ctx.clone();
			let shared_state = shared_state.clone();

			Box::pin(
				async move {
					// Extract just the host, stripping the port if present
					let host = hostname.split(':').next().unwrap_or(hostname);

					tracing::debug!("Routing request for hostname: {host}, path: {path}");

					// Parse query parameters
					let query_params = parse_query_params(path);

					// Check if this is a WebSocket upgrade request
					let is_websocket = headers
						.get("upgrade")
						.and_then(|v| v.to_str().ok())
						.map(|v| v.eq_ignore_ascii_case("websocket"))
						.unwrap_or(false);

					// Extract target from WebSocket protocol, HTTP header, or query param
					let target = if is_websocket {
						// For WebSocket, parse the sec-websocket-protocol header
						headers
							.get(SEC_WEBSOCKET_PROTOCOL)
							.and_then(|protocols| protocols.to_str().ok())
							.and_then(|protocols| {
								// Parse protocols to find target.{value}
								protocols
									.split(',')
									.map(|p| p.trim())
									.find_map(|p| p.strip_prefix(WS_PROTOCOL_TARGET))
							})
							// Fallback to query parameter if protocol not provided
							.or_else(|| query_params.get("x_rivet_target").map(|s| s.as_str()))
					} else {
						// For HTTP, use the x-rivet-target header, fallback to query param
						headers
							.get(X_RIVET_TARGET)
							.and_then(|x| x.to_str().ok())
							.or_else(|| query_params.get("x_rivet_target").map(|s| s.as_str()))
					};

					// Read target
					if let Some(target) = target {
						if let Some(routing_output) =
							runner::route_request(&ctx, target, host, path, headers, &query_params)
								.await?
						{
							return Ok(routing_output);
						}

						if let Some(routing_output) = pegboard_gateway::route_request(
							&ctx,
							&shared_state,
							target,
							host,
							path,
							headers,
							is_websocket,
							&query_params,
						)
						.await?
						{
							return Ok(routing_output);
						}

						if let Some(routing_output) =
							api_public::route_request(&ctx, target, host, path).await?
						{
							return Ok(routing_output);
						}
					} else {
						// No x-rivet-target header, try routing to api-public by default
						if let Some(routing_output) =
							api_public::route_request(&ctx, "api-public", host, path).await?
						{
							return Ok(routing_output);
						}
					}

					// No matching route found
					tracing::debug!("No route found for: {host} {path}");
					Err(errors::NoRoute {
						host: host.to_string(),
						path: path.to_string(),
					}
					.build())
				}
				.instrument(tracing::info_span!("routing_fn", %hostname, %path, ?port_type)),
			)
		},
	)
}

/// Parse query parameters from a path string
fn parse_query_params(path: &str) -> std::collections::HashMap<String, String> {
	let mut params = std::collections::HashMap::new();

	if let Some(query_start) = path.find('?') {
		// Strip fragment if present
		let query = &path[query_start + 1..].split('#').next().unwrap_or("");
		// Use url::form_urlencoded to properly decode query parameters
		for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
			params.insert(key.into_owned(), value.into_owned());
		}
	}

	params
}
