use api_helper::{anchor::WatchIndexQuery, ctx::Ctx};
use proto::backend::pkg::*;
use rivet_api::{
	apis::{configuration::Configuration, *},
	models,
};
use rivet_operation::prelude::*;
use serde::{Deserialize, Serialize};

use crate::auth::Auth;

// MARK: GET /matchmaker
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusQuery {
	region: String,
}

pub async fn status(
	ctx: Ctx<Auth>,
	_watch_index: WatchIndexQuery,
	query: StatusQuery,
) -> GlobalResult<serde_json::Value> {
	let _domain_cdn = unwrap!(util::env::domain_cdn());

	// Find namespace ID
	let game_res = op!([ctx] game_resolve_name_id {
		name_ids: vec!["sandbox".into()],
	})
	.await?;
	let game_id = unwrap_with!(
		game_res.games.first().and_then(|x| x.game_id),
		INTERNAL_STATUS_CHECK_FAILED,
		error = "missing sandbox game"
	);
	let ns_resolve = op!([ctx] game_namespace_resolve_name_id {
		game_id: Some(game_id),
		name_ids: vec!["prod".into()],
	})
	.await?;
	let ns_id = unwrap_with!(
		ns_resolve.namespaces.first().and_then(|x| x.namespace_id),
		INTERNAL_STATUS_CHECK_FAILED,
		error = "missing prod namespace"
	);

	// Create bypass token
	let ns_token_res = op!([ctx] token_create {
		token_config: Some(token::create::request::TokenConfig {
			ttl: util::duration::minutes(15),
		}),
		refresh_token_config: None,
		issuer: "api-status".to_owned(),
		client: Some(ctx.client_info()),
		kind: Some(token::create::request::Kind::New(token::create::request::KindNew {
			entitlements: vec![
				proto::claims::Entitlement {
					kind: Some(proto::claims::entitlement::Kind::GameNamespacePublic(
						proto::claims::entitlement::GameNamespacePublic {
							namespace_id: Some(ns_id),
						},
					))
				},
				proto::claims::Entitlement {
					kind: Some(
						proto::claims::entitlement::Kind::Bypass(proto::claims::entitlement::Bypass {})
					)
				}
			],
		})),
		label: Some("ns_pub".to_owned()),
		..Default::default()
	})
	.await?;
	let ns_token = unwrap_ref!(ns_token_res.token).token.clone();

	// Bypass token
	let bypass_token_res = op!([ctx] token_create {
		token_config: Some(token::create::request::TokenConfig {
			ttl: util::duration::minutes(15),
		}),
		refresh_token_config: None,
		issuer: "api-status".to_owned(),
		client: Some(ctx.client_info()),
		kind: Some(token::create::request::Kind::New(token::create::request::KindNew {
			entitlements: vec![
				proto::claims::Entitlement {
					kind: Some(
						proto::claims::entitlement::Kind::Bypass(proto::claims::entitlement::Bypass {})
					)
				}
			],
		})),
		label: Some("byp".to_owned()),
		..Default::default()
	})
	.await?;
	let bypass_token = unwrap_ref!(bypass_token_res.token).token.clone();

	// Create client
	let mut headers = reqwest::header::HeaderMap::new();
	headers.insert("host", util::env::host_api().parse()?);
	headers.insert(
		"cf-connecting-ip",
		reqwest::header::HeaderValue::from_str("127.0.0.1")?,
	);
	headers.insert(
		"x-coords",
		reqwest::header::HeaderValue::from_str("0.0,0.0")?,
	);
	headers.insert(
		"x-bypass-token",
		reqwest::header::HeaderValue::from_str(&bypass_token)?,
	);

	let client = reqwest::ClientBuilder::new()
		.default_headers(headers)
		.build()?;
	let config = Configuration {
		base_path: "http://traefik.traefik.svc.cluster.local:80".into(),
		bearer_access_token: Some(ns_token),
		client,
		..Default::default()
	};

	tracing::info!("finding lobby");
	let res = matchmaker_lobbies_api::matchmaker_lobbies_create(
		&config,
		models::MatchmakerLobbiesCreateRequest {
			game_mode: "custom".into(),
			region: Some(query.region.clone()),
			..Default::default()
		},
	)
	.await;
	let res = match res {
		Ok(x) => x,
		Err(err) => {
			bail_with!(
				INTERNAL_STATUS_CHECK_FAILED,
				error = format!("find lobby: {:?}", err)
			)
		}
	};

	// Make HTTP request through the socket
	let port_default = unwrap!(res.lobby.ports.get("default"));
	let port_host = unwrap_ref!(port_default.host);
	let res = reqwest::get(format!("https://{port_host}/health")).await;
	let res = match res {
		Ok(x) => x,
		Err(err) => {
			bail_with!(
				INTERNAL_STATUS_CHECK_FAILED,
				error = format!("connect to lobby: {:?}", err)
			)
		}
	};
	let _res = match res.error_for_status() {
		Ok(x) => x,
		Err(err) => {
			bail_with!(
				INTERNAL_STATUS_CHECK_FAILED,
				error = format!("connect to lobby status: {:?}", err)
			)
		}
	};

	Ok(serde_json::json!({}))
}
