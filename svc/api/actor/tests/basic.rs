use proto::backend::pkg::*;
use rivet_api::{apis::*, models};
use rivet_operation::prelude::*;
use std::{collections::HashMap, sync::Once};

static GLOBAL_INIT: Once = Once::new();

struct Ctx {
	pub op_ctx: OperationContext<()>,
	pub service_token: String,
	pub game_id: Uuid,
	pub env_id: Uuid,
	pub game_id_str: String,
	pub env_id_str: String,
	pub datacenter_id: Uuid,
	pub image_id: Uuid,
}

impl Ctx {
	async fn init() -> GlobalResult<Ctx> {
		GLOBAL_INIT.call_once(|| {
			tracing_subscriber::fmt()
				.pretty()
				.with_max_level(tracing::Level::INFO)
				.with_target(false)
				.init();
		});

		let pools = rivet_pools::from_env().await?;
		let cache = rivet_cache::CacheInner::new(
			"api-actor-test".to_string(),
			util::env::var("RIVET_SOURCE_HASH")?,
			pools.redis_cache()?,
		);
		let client = chirp_client::SharedClient::from_env(pools.clone())
			.expect("create client")
			.wrap_new("api-actor-test");
		let conn = rivet_connection::Connection::new(client, pools, cache);
		let op_ctx = OperationContext::new(
			"api-actor-test".to_string(),
			std::time::Duration::from_secs(60),
			conn,
			Uuid::new_v4(),
			Uuid::new_v4(),
			util::timestamp::now(),
			util::timestamp::now(),
			(),
		);

		let (datacenter_id, _primary_datacenter_name_id) = Self::setup_datacenter(&op_ctx).await?;
		let (game_id, env_id, image_id) = Self::setup_game(&op_ctx, datacenter_id).await?;
		let service_token = Self::setup_token(&op_ctx, env_id).await?;

		Ok(Ctx {
			op_ctx,
			service_token,
			game_id,
			env_id,
			game_id_str: game_id.to_string(),
			env_id_str: env_id.to_string(),
			datacenter_id,
			image_id,
		})
	}

	fn chirp(&self) -> &chirp_client::Client {
		self.op_ctx.chirp()
	}

	fn op_ctx(&self) -> &OperationContext<()> {
		&self.op_ctx
	}

	pub fn config(
		&self,
		bearer_token: String,
	) -> GlobalResult<rivet_api::apis::configuration::Configuration> {
		Ok(rivet_api::apis::configuration::Configuration {
			base_path: "http://traefik.traefik.svc.cluster.local:80".into(),
			bearer_access_token: Some(bearer_token),
			client: {
				let mut headers = http::header::HeaderMap::new();
				headers.insert(
					http::header::HOST,
					unwrap!(http::header::HeaderValue::from_str(unwrap!(
						util::env::domain_main_api()
					))),
				);
				headers.insert(
					"cf-connecting-ip",
					http::header::HeaderValue::from_static("127.0.0.1"),
				);
				unwrap!(reqwest::Client::builder().default_headers(headers).build())
			},
			..Default::default()
		})
	}

	pub async fn setup_datacenter(ctx: &OperationContext<()>) -> GlobalResult<(Uuid, String)> {
		tracing::info!("setup region");

		let region_res = op!([ctx] faker_region {}).await?;
		let region_id = unwrap!(region_res.region_id).as_uuid();

		let get_res = op!([ctx] region_get {
			region_ids: vec![region_id.into()],
		})
		.await?;
		let region_data = unwrap!(get_res.regions.first());

		Ok((region_id, region_data.name_id.clone()))
	}

	pub async fn setup_game(
		ctx: &OperationContext<()>,
		region_id: Uuid,
	) -> GlobalResult<(Uuid, Uuid, Uuid)> {
		let game_res = op!([ctx] faker_game {
			..Default::default()
		})
		.await?;
		let game_id = unwrap!(game_res.game_id);
		let env_id = unwrap!(game_res.prod_env_id);

		let build_res = op!([ctx] faker_build {
			env_id: Some(env_id.clone()),
			image: proto::backend::faker::Image::DsEcho as i32,
		})
		.await?;

		Ok((
			unwrap!(game_res.game_id).as_uuid(),
			unwrap!(game_res.prod_env_id).as_uuid(),
			unwrap!(build_res.build_id).as_uuid(),
		))
	}

	pub async fn setup_token(ctx: &OperationContext<()>, env_id: Uuid) -> GlobalResult<String> {
		let token_res = op!([ctx] token_create {
			token_config: Some(token::create::request::TokenConfig {
				ttl: util::duration::days(15 * 365)
			}),
			refresh_token_config: None,
			issuer: "test".to_owned(),
			client: None,
			kind: Some(token::create::request::Kind::New(
				token::create::request::KindNew { entitlements: vec![proto::claims::Entitlement {
					kind: Some(proto::claims::entitlement::Kind::EnvService(
						proto::claims::entitlement::EnvService {
							env_id: Some(env_id.into()),
						}
					)),
				}]},
			)),
			label: Some("env_svc".to_owned()),
			..Default::default()
		})
		.await?;

		Ok(unwrap!(token_res.token).token.clone())
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn create_http() -> GlobalResult<()> {
	let ctx = Ctx::init().await?;

	let ctx_config = ctx.config(ctx.service_token.clone())?;

	servers_api::servers_create(
		&ctx_config,
		&ctx.game_id_str,
		&ctx.env_id_str,
		models::ServersCreateServerRequest {
			datacenter: ctx.datacenter_id,
			tags: None,
			runtime: Box::new(models::ServersCreateServerRuntimeRequest {
				build: ctx.image_id,
				environment: Some(HashMap::new()),
				arguments: None,
			}),
			network: Box::new(models::ServersCreateServerNetworkRequest {
				mode: Some(models::ServersNetworkMode::Bridge),
				ports: vec![(
					"testing2".to_string(),
					models::ServersCreateServerPortRequest {
						protocol: models::ServersPortProtocol::Http,
						routing: Some(Box::new(models::ServersPortRouting {
							game_guard: Some(serde_json::Value::Object(serde_json::Map::new())),
							host: None,
						})),
						internal_port: Some(12523),
					},
				)]
				// Collect into hashmap
				.into_iter()
				.collect(),
			}),
			lifecycle: Some(Box::new(models::ServersLifecycle {
				kill_timeout: Some(0),
			})),
			resources: Box::new(models::ServersResources {
				cpu: 100,
				memory: 200,
			}),
		},
	)
	.await?;

	Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_builds_with_tags() -> GlobalResult<()> {
	let ctx = Ctx::init().await?;

	let ctx_config = ctx.config(ctx.service_token.clone())?;

	servers_api::servers_create(
		&ctx_config,
		&ctx.game_id_str,
		&ctx.env_id_str,
		models::ServersCreateServerRequest {
			datacenter: ctx.datacenter_id,
			tags: None,
			runtime: Box::new(models::ServersCreateServerRuntimeRequest {
				build: ctx.image_id,
				arguments: None,
				environment: Some(HashMap::new()),
			}),
			lifecycle: Some(Box::new(models::ServersLifecycle {
				kill_timeout: Some(0),
			})),
			network: Box::new(models::ServersCreateServerNetworkRequest {
				mode: Some(models::ServersNetworkMode::Bridge),
				ports: vec![(
					"testing2".to_string(),
					models::ServersCreateServerPortRequest {
						protocol: models::ServersPortProtocol::Http,
						routing: Some(Box::new(models::ServersPortRouting {
							game_guard: Some(serde_json::Value::Object(serde_json::Map::new())),
							host: None,
						})),
						internal_port: Some(12523),
					},
				)]
				// Collect into hashmap
				.into_iter()
				.collect(),
			}),
			resources: Box::new(models::ServersResources {
				cpu: 100,
				memory: 200,
			}),
		},
	)
	.await?;

	Ok(())
}
