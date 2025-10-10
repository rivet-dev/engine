use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::TryStreamExt;
use gas::prelude::*;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, StatusCode, header::HeaderName};
use rivet_guard_core::{
	WebSocketHandle, custom_serve::CustomServeTrait, errors::WebSocketServiceUnavailable,
	proxy_service::ResponseBody, request_context::RequestContext,
};
use rivet_runner_protocol as protocol;
use rivet_util::serde::HashableMap;
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Message, protocol::frame::coding::CloseCode};

use crate::shared_state::{SharedState, TunnelMessageData};

pub mod shared_state;

const TUNNEL_ACK_TIMEOUT: Duration = Duration::from_secs(2);
const SEC_WEBSOCKET_PROTOCOL: HeaderName = HeaderName::from_static("sec-websocket-protocol");
const WS_PROTOCOL_ACTOR: &str = "rivet_actor.";

pub struct PegboardGateway {
	shared_state: SharedState,
	runner_id: Id,
	actor_id: Id,
}

impl PegboardGateway {
	#[tracing::instrument(skip_all, fields(?actor_id, ?runner_id))]
	pub fn new(shared_state: SharedState, runner_id: Id, actor_id: Id) -> Self {
		Self {
			shared_state,
			runner_id,
			actor_id,
		}
	}
}

#[async_trait]
impl CustomServeTrait for PegboardGateway {
	#[tracing::instrument(skip_all, fields(actor_id=?self.actor_id, runner_id=?self.runner_id))]
	async fn handle_request(
		&self,
		req: Request<Full<Bytes>>,
		_request_context: &mut RequestContext,
	) -> Result<Response<ResponseBody>> {
		// Extract actor ID for the message (HTTP requests use x-rivet-actor header)
		let actor_id = req
			.headers()
			.get("x-rivet-actor")
			.context("missing x-rivet-actor header")?
			.to_str()
			.context("invalid x-rivet-actor header")?
			.to_string();

		// Extract request parts
		let mut headers = HashableMap::new();
		for (name, value) in req.headers() {
			if let Result::Ok(value_str) = value.to_str() {
				headers.insert(name.to_string(), value_str.to_string());
			}
		}

		// Extract method and path before consuming the request
		let method = req.method().to_string();
		let path = req
			.uri()
			.path_and_query()
			.map_or_else(|| "/".to_string(), |x| x.to_string());

		let body_bytes = req
			.into_body()
			.collect()
			.await
			.context("failed to read body")?
			.to_bytes();

		// Build subject to publish to
		let tunnel_subject =
			pegboard::pubsub_subjects::RunnerReceiverSubject::new(self.runner_id).to_string();

		// Start listening for request responses
		let (request_id, mut msg_rx) = self
			.shared_state
			.start_in_flight_request(tunnel_subject)
			.await;

		// Start request
		let message = protocol::ToClientTunnelMessageKind::ToClientRequestStart(
			protocol::ToClientRequestStart {
				actor_id: actor_id.clone(),
				method,
				path,
				headers,
				body: if body_bytes.is_empty() {
					None
				} else {
					Some(body_bytes.to_vec())
				},
				stream: false,
			},
		);
		self.shared_state.send_message(request_id, message).await?;

		// Wait for response
		tracing::debug!("gateway waiting for response from tunnel");
		let fut = async {
			while let Some(msg) = msg_rx.recv().await {
				match msg {
					TunnelMessageData::Message(msg) => match msg {
						protocol::ToServerTunnelMessageKind::ToServerResponseStart(
							response_start,
						) => {
							return anyhow::Ok(response_start);
						}
						_ => {
							tracing::warn!("received non-response message from pubsub");
						}
					},
					TunnelMessageData::Timeout => {
						tracing::warn!("tunnel message timeout");
						return Err(WebSocketServiceUnavailable.build());
					}
				}
			}

			tracing::warn!("received no message response");
			Err(WebSocketServiceUnavailable.build())
		};
		let response_start = tokio::time::timeout(TUNNEL_ACK_TIMEOUT, fut)
			.await
			.map_err(|_| {
				tracing::warn!("timed out waiting for tunnel ack");

				WebSocketServiceUnavailable.build()
			})??;
		tracing::debug!("response handler task ended");

		// Build HTTP response
		let mut response_builder =
			Response::builder().status(StatusCode::from_u16(response_start.status)?);

		// Add headers
		for (key, value) in response_start.headers {
			response_builder = response_builder.header(key, value);
		}

		// Add body
		let body = response_start.body.unwrap_or_default();
		let response = response_builder.body(ResponseBody::Full(Full::new(Bytes::from(body))))?;

		Ok(response)
	}

	#[tracing::instrument(skip_all, fields(actor_id=?self.actor_id, runner_id=?self.runner_id))]
	async fn handle_websocket(
		&self,
		client_ws: WebSocketHandle,
		headers: &hyper::HeaderMap,
		path: &str,
		_request_context: &mut RequestContext,
	) -> Result<()> {
		// Extract actor ID from WebSocket protocol
		let actor_id = headers
			.get(SEC_WEBSOCKET_PROTOCOL)
			.and_then(|protocols| protocols.to_str().ok())
			.and_then(|protocols| {
				// Parse protocols to find actor.{id}
				protocols
					.split(',')
					.map(|p| p.trim())
					.find_map(|p| p.strip_prefix(WS_PROTOCOL_ACTOR))
			})
			.context("missing actor protocol in sec-websocket-protocol")?
			.to_string();

		// Extract headers
		let mut request_headers = HashableMap::new();
		for (name, value) in headers {
			if let Result::Ok(value_str) = value.to_str() {
				request_headers.insert(name.to_string(), value_str.to_string());
			}
		}

		// Build subject to publish to
		let tunnel_subject =
			pegboard::pubsub_subjects::RunnerReceiverSubject::new(self.runner_id).to_string();

		// Start listening for WebSocket messages
		let (request_id, mut msg_rx) = self
			.shared_state
			.start_in_flight_request(tunnel_subject.clone())
			.await;

		// Send WebSocket open message
		let open_message = protocol::ToClientTunnelMessageKind::ToClientWebSocketOpen(
			protocol::ToClientWebSocketOpen {
				actor_id: actor_id.clone(),
				path: path.to_string(),
				headers: request_headers,
			},
		);

		self.shared_state
			.send_message(request_id, open_message)
			.await?;

		tracing::debug!("gateway waiting for websocket open from tunnel");

		// Wait for WebSocket open acknowledgment
		let fut = async {
			while let Some(msg) = msg_rx.recv().await {
				match msg {
					TunnelMessageData::Message(
						protocol::ToServerTunnelMessageKind::ToServerWebSocketOpen,
					) => {
						return anyhow::Ok(());
					}
					TunnelMessageData::Message(
						protocol::ToServerTunnelMessageKind::ToServerWebSocketClose(close),
					) => {
						tracing::warn!(?close, "websocket closed before opening");
						return Err(WebSocketServiceUnavailable.build());
					}
					TunnelMessageData::Timeout => {
						tracing::warn!("websocket open timeout");
						return Err(WebSocketServiceUnavailable.build());
					}
					_ => {
						tracing::warn!(
							"received unexpected message while waiting for websocket open"
						);
					}
				}
			}

			tracing::warn!("received no message response");
			Err(WebSocketServiceUnavailable.build())
		};
		tokio::time::timeout(TUNNEL_ACK_TIMEOUT, fut)
			.await
			.map_err(|_| {
				tracing::warn!("timed out waiting for tunnel ack");

				WebSocketServiceUnavailable.build()
			})??;

		// Accept the WebSocket
		let mut ws_rx = client_ws.accept().await?;

		// Spawn task to forward messages from server to client
		let mut server_to_client = tokio::spawn(
			async move {
				while let Some(msg) = msg_rx.recv().await {
					match msg {
						TunnelMessageData::Message(
							protocol::ToServerTunnelMessageKind::ToServerWebSocketMessage(ws_msg),
						) => {
							let msg = if ws_msg.binary {
								Message::Binary(ws_msg.data.into())
							} else {
								Message::Text(
									String::from_utf8_lossy(&ws_msg.data).into_owned().into(),
								)
							};
							client_ws.send(msg).await?;
						}
						TunnelMessageData::Message(
							protocol::ToServerTunnelMessageKind::ToServerWebSocketClose(close),
						) => {
							tracing::debug!(?close, "server closed websocket");
							return Err(WebSocketServiceUnavailable.build());
						}
						TunnelMessageData::Timeout => {
							tracing::warn!("websocket message timeout");
							return Err(WebSocketServiceUnavailable.build());
						}
						_ => {}
					}
				}

				tracing::debug!("sub closed");

				Err(WebSocketServiceUnavailable.build())
			}
			.instrument(tracing::info_span!("server_to_client_task")),
		);

		// Spawn task to forward messages from client to server
		let shared_state_clone = self.shared_state.clone();
		let mut client_to_server = tokio::spawn(
			async move {
				while let Some(msg) = ws_rx.try_next().await? {
					match msg {
						Message::Binary(data) => {
							let ws_message =
								protocol::ToClientTunnelMessageKind::ToClientWebSocketMessage(
									protocol::ToClientWebSocketMessage {
										data: data.into(),
										binary: true,
									},
								);
							shared_state_clone
								.send_message(request_id, ws_message)
								.await?;
						}
						Message::Text(text) => {
							let ws_message =
								protocol::ToClientTunnelMessageKind::ToClientWebSocketMessage(
									protocol::ToClientWebSocketMessage {
										data: text.as_bytes().to_vec(),
										binary: false,
									},
								);
							shared_state_clone
								.send_message(request_id, ws_message)
								.await?;
						}
						Message::Close(_) => {
							return Ok(());
						}
						_ => {}
					}
				}

				tracing::debug!("websocket stream closed");

				Ok(())
			}
			.instrument(tracing::info_span!("client_to_server_task")),
		);

		// Wait for either task to complete
		let lifecycle_res = tokio::select! {
			res = &mut server_to_client => {
				let res = res?;
				tracing::info!(?res, "server to client task completed");
				res
			}
			res = &mut client_to_server => {
				let res = res?;
				tracing::info!(?res, "client to server task completed");
				res
			}
		};

		// Abort remaining tasks
		server_to_client.abort();
		client_to_server.abort();

		let (close_code, close_reason) = if lifecycle_res.is_ok() {
			(CloseCode::Normal.into(), None)
		} else {
			(CloseCode::Error.into(), Some("ws.downstream_closed".into()))
		};

		// Send WebSocket close message to runner
		let close_message = protocol::ToClientTunnelMessageKind::ToClientWebSocketClose(
			protocol::ToClientWebSocketClose {
				code: Some(close_code),
				reason: close_reason,
			},
		);

		if let Err(err) = self
			.shared_state
			.send_message(request_id, close_message)
			.await
		{
			tracing::error!(?err, "error sending close message");
		}

		lifecycle_res
	}
}
