use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::ops::Deref;

use chirp_worker::prelude::*;
use proto::backend::{self, pkg::*};
use redis::AsyncCommands;
use serde_json::json;

mod nomad_job;
mod oci_config;
mod seccomp;

lazy_static::lazy_static! {
	static ref NOMAD_CONFIG: nomad_client::apis::configuration::Configuration =
		nomad_util::config_from_env().unwrap();

	static ref REDIS_SCRIPT: redis::Script = redis::Script::new(include_str!("../../../redis-scripts/lobby_create.lua"));
}

/// Send a lobby create fail message and cleanup the lobby if needed.
#[tracing::instrument]
async fn fail(
	client: &chirp_client::Client,
	lobby_id: Uuid,
	preemptively_created: bool,
	error_code: mm::msg::lobby_create_fail::ErrorCode,
) -> GlobalResult<()> {
	tracing::warn!(%lobby_id, %preemptively_created, ?error_code, "lobby create failed");

	// Cleanup preemptively inserted lobby.
	//
	// We have to perform a full cleanup instead of just deleting the row since
	// players may have been inserted while waiting for the lobby creation.
	if preemptively_created {
		msg!([client] mm::msg::lobby_cleanup(lobby_id) {
			lobby_id: Some(lobby_id.into()),
		})
		.await?;
	}

	// Send failure message
	msg!([client] mm::msg::lobby_create_fail(lobby_id) {
		lobby_id: Some(lobby_id.into()),
		error_code: error_code as i32,
	})
	.await?;

	Ok(())
}

#[worker(name = "mm-lobby-create")]
async fn worker(ctx: &OperationContext<mm::msg::lobby_create::Message>) -> GlobalResult<()> {
	let lobby_id = unwrap_ref!(ctx.lobby_id).as_uuid();
	let namespace_id = unwrap_ref!(ctx.namespace_id).as_uuid();
	let lobby_group_id = unwrap_ref!(ctx.lobby_group_id).as_uuid();
	let region_id = unwrap_ref!(ctx.region_id).as_uuid();
	let create_ray_id = ctx.region_id.as_ref().map(common::Uuid::as_uuid);
	let creator_user_id = ctx.creator_user_id.as_ref().map(common::Uuid::as_uuid);

	// Check for stale message
	if ctx.req_dt() > util::duration::seconds(60) {
		tracing::warn!("discarding stale message");
		return fail(
			ctx.chirp(),
			lobby_id,
			ctx.preemptively_created,
			mm::msg::lobby_create_fail::ErrorCode::StaleMessage,
		)
		.await;
	}

	let (
		(mm_game_config, namespace),
		mm_ns_config,
		(lobby_group, lobby_group_meta, version_id),
		region,
		tiers,
	) = tokio::try_join!(
		fetch_namespace(ctx, namespace_id),
		fetch_mm_namespace_config(ctx, namespace_id),
		fetch_lobby_group_config(ctx, lobby_group_id),
		fetch_region(ctx, region_id),
		fetch_tiers(ctx, region_id),
	)?;
	let version = fetch_version(ctx, version_id).await?;

	// Make assertions about the fetched data
	{
		ensure_eq!(
			namespace.game_id,
			version.game_id,
			"namespace and version do not belong to the same game"
		);

		// Check if the versions match. If this is not true, then this lobby was
		// likely created while a version was being published. Continue anyway.
		if namespace.version_id != version.version_id {
			tracing::warn!(
				ns_version_id = ?namespace.version_id,
				version_id = ?version.version_id,
				"namespace version is not the same as the given version, likely due to a race condition"
			);
		}
	}

	// Override max player count
	let max_players_normal = ctx
		.dynamic_max_players
		.unwrap_or(lobby_group.max_players_normal);
	let max_players_direct = ctx
		.dynamic_max_players
		.unwrap_or(lobby_group.max_players_direct);

	// Get the relevant lobby group region
	let lobby_group_region = if let Some(x) = lobby_group
		.regions
		.iter()
		.find(|r| r.region_id == ctx.region_id)
	{
		x
	} else {
		return fail(
			ctx.chirp(),
			lobby_id,
			ctx.preemptively_created,
			mm::msg::lobby_create_fail::ErrorCode::RegionNotEnabled,
		)
		.await;
	};

	// Find the relevant tier
	let tier = unwrap!(tiers
		.iter()
		.find(|x| x.tier_name_id == lobby_group_region.tier_name_id));

	let runtime = unwrap_ref!(lobby_group.runtime);
	let runtime = unwrap_ref!(runtime.runtime);
	let runtime_meta = unwrap_ref!(lobby_group_meta.runtime);
	let runtime_meta = unwrap_ref!(runtime_meta.runtime);

	let validate_lobby_count_perf = ctx.perf().start("validate-lobby-count").await;
	if !validate_lobby_count(
		ctx,
		ctx.redis_mm().await?,
		lobby_id,
		&mm_ns_config,
		namespace_id,
	)
	.await?
	{
		return fail(
			ctx.chirp(),
			lobby_id,
			ctx.preemptively_created,
			mm::msg::lobby_create_fail::ErrorCode::LobbyCountOverMax,
		)
		.await;
	}
	validate_lobby_count_perf.end();

	// Create lobby token
	let (lobby_token, token_session_id) = gen_lobby_token(ctx, lobby_id).await?;

	// Insert to database
	let run_id = Uuid::new_v4();
	let insert_opts = UpdateDbOpts {
		lobby_id,
		namespace_id,
		region_id,
		lobby_group_id,
		token_session_id,
		run_id,
		create_ray_id: ctx.ray_id(),
		lobby_group: lobby_group.clone(),
		creator_user_id,
		is_custom: ctx.is_custom,
		publicity: ctx
			.publicity
			.and_then(backend::matchmaker::lobby::Publicity::from_i32),
		max_players_normal,
		max_players_direct,
	};
	rivet_pools::utils::crdb::tx(&ctx.crdb().await?, |tx| {
		let ctx = ctx.clone();
		Box::pin(update_db(ctx, tx, insert_opts.clone()))
	})
	.await?;

	{
		use util_mm::key;

		let write_perf = ctx.perf().start("write-lobby-redis").await;
		REDIS_SCRIPT
			.arg(ctx.ts())
			.arg(lobby_id.to_string())
			.arg(serde_json::to_string(&key::lobby_config::Config {
				namespace_id,
				region_id,
				lobby_group_id,
				max_players_normal,
				max_players_direct,
				max_players_party: lobby_group.max_players_party,
				preemptive: false,
				is_closed: false,
				ready_ts: None,
				is_custom: ctx.is_custom,
				state_json: None,
				is_node_closed: false,
			})?)
			.arg(ctx.ts() + util_mm::consts::LOBBY_READY_TIMEOUT)
			.key(key::lobby_config(lobby_id))
			.key(key::ns_lobby_ids(namespace_id))
			.key(key::lobby_available_spots(
				namespace_id,
				region_id,
				lobby_group_id,
				util_mm::JoinKind::Normal,
			))
			.key(key::lobby_available_spots(
				namespace_id,
				region_id,
				lobby_group_id,
				util_mm::JoinKind::Party,
			))
			.key(key::lobby_unready())
			.key(key::idle_lobby_ids(namespace_id, region_id, lobby_group_id))
			.key(key::idle_lobby_lobby_group_ids(namespace_id, region_id))
			.key(key::lobby_player_ids(lobby_id))
			.invoke_async(&mut ctx.redis_mm().await?)
			.await?;
		write_perf.end();
	}

	// TODO: Handle this failure case
	// Start the runtime
	match (runtime, runtime_meta) {
		(
			backend::matchmaker::lobby_runtime::Runtime::Docker(runtime),
			backend::matchmaker::lobby_runtime_meta::Runtime::Docker(runtime_meta),
		) => {
			create_docker_job(
				ctx,
				runtime,
				runtime_meta,
				&namespace,
				&version,
				&mm_game_config,
				&lobby_group,
				&lobby_group_meta,
				&region,
				tier,
				max_players_normal,
				max_players_direct,
				run_id,
				lobby_id,
				&lobby_token,
			)
			.await?
		}
	};

	msg!([ctx] mm::msg::lobby_create_complete(lobby_id) {
		lobby_id: Some(lobby_id.into()),
		run_id: Some(run_id.into()),
	})
	.await?;

	msg!([ctx] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "mm.lobby.create".into(),
				namespace_id: ctx.namespace_id,
				properties_json: Some(serde_json::to_string(&json!({
					"lobby_id": lobby_id,
					"lobby_group_id": lobby_group_id,
					"region_id": region_id,
					"create_ray_id": create_ray_id,
					"preemptively_created": ctx.preemptively_created,
					"tier": tier.tier_name_id,
					"max_players": {
						"normal": max_players_normal,
						"direct": max_players_direct,
						"party": lobby_group.max_players_party,
					},
					"run_id": run_id,
				}))?),
				..Default::default()
			}
		],
	})
	.await?;

	Ok(())
}

#[tracing::instrument]
async fn fetch_region(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	region_id: Uuid,
) -> GlobalResult<backend::region::Region> {
	tracing::info!(?region_id, "fetching primary region");
	let primary_get_res = op!([ctx] region_get {
		region_ids: vec![region_id.into()],
	})
	.await?;
	let region = unwrap!(primary_get_res.regions.first(), "region not found");

	Ok(region.clone())
}

#[tracing::instrument]
async fn fetch_tiers(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	region_id: Uuid,
) -> GlobalResult<Vec<backend::region::Tier>> {
	let tier_res = op!([ctx] tier_list {
		region_ids: vec![region_id.into()],
	})
	.await?;
	let tier_region = unwrap!(tier_res.regions.first());

	Ok(tier_region.tiers.clone())
}

#[tracing::instrument]
async fn fetch_namespace(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	namespace_id: Uuid,
) -> GlobalResult<(backend::matchmaker::GameConfig, backend::game::Namespace)> {
	let get_res = op!([ctx] game_namespace_get {
		namespace_ids: vec![namespace_id.into()],
	})
	.await?;

	let namespace = unwrap!(get_res.namespaces.first(), "namespace not found").clone();
	let game_id = unwrap!(namespace.game_id);

	let get_res = op!([ctx] mm_config_game_get {
		game_ids: vec![game_id],
	})
	.await?;
	let game_config = unwrap_ref!(unwrap!(get_res.games.first()).config).clone();

	Ok((game_config, namespace))
}

#[tracing::instrument]
async fn fetch_mm_namespace_config(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	namespace_id: Uuid,
) -> GlobalResult<backend::matchmaker::NamespaceConfig> {
	let get_res = op!([ctx] mm_config_namespace_get {
		namespace_ids: vec![namespace_id.into()],
	})
	.await?;

	let namespace = unwrap!(get_res.namespaces.first(), "namespace not found")
		.deref()
		.clone();
	let namespace_config = unwrap_ref!(namespace.config).clone();

	Ok(namespace_config)
}

#[tracing::instrument]
async fn fetch_version(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	version_id: Uuid,
) -> GlobalResult<backend::game::Version> {
	let get_res = op!([ctx] game_version_get {
		version_ids: vec![version_id.into()],
	})
	.await?;

	let version = unwrap!(get_res.versions.first(), "version not found").clone();

	Ok(version)
}

#[tracing::instrument]
async fn fetch_lobby_group_config(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	lobby_group_id: Uuid,
) -> GlobalResult<(
	backend::matchmaker::LobbyGroup,
	backend::matchmaker::LobbyGroupMeta,
	Uuid,
)> {
	let lobby_group_id_proto = Some(common::Uuid::from(lobby_group_id));

	// Resolve the version ID
	let resolve_version_res = op!([ctx] mm_config_lobby_group_resolve_version {
		lobby_group_ids: vec![lobby_group_id.into()],
	})
	.await?;
	let version_id = unwrap_ref!(
		unwrap_ref!(
			resolve_version_res.versions.first(),
			"lobby group not found"
		)
		.version_id
	)
	.as_uuid();

	// Fetch the config data
	let config_get_res = op!([ctx] mm_config_version_get {
		version_ids: vec![version_id.into()],
	})
	.await?;

	let version = config_get_res.versions.first();
	let version = unwrap_ref!(version, "version config not found");
	let version_config = unwrap_ref!(version.config);
	let version_config_meta = unwrap_ref!(version.config_meta);

	// Find the matching lobby group
	let lobby_group_meta = version_config_meta
		.lobby_groups
		.iter()
		.enumerate()
		.find(|(_, lg)| lg.lobby_group_id == lobby_group_id_proto);
	let (lg_idx, lobby_group_meta) = unwrap_ref!(lobby_group_meta, "lobby group not found");
	let lobby_group = version_config.lobby_groups.get(*lg_idx);
	let lobby_group = unwrap_ref!(lobby_group);

	Ok((
		(*lobby_group).clone(),
		(*lobby_group_meta).clone(),
		version_id,
	))
}

/// Validates that there is room to create one more lobby without going over the lobby count cap.
#[tracing::instrument(skip(redis_mm))]
async fn validate_lobby_count(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	mut redis_mm: RedisPool,
	lobby_id: Uuid,
	mm_ns_config: &backend::matchmaker::NamespaceConfig,
	namespace_id: Uuid,
) -> GlobalResult<bool> {
	let lobby_count = redis_mm
		.zcard::<_, u64>(util_mm::key::ns_lobby_ids(namespace_id))
		.await?;
	tracing::info!(?lobby_count, lobby_count_max = ?mm_ns_config.lobby_count_max, "current lobby count");

	Ok(lobby_count < mm_ns_config.lobby_count_max as u64)
}

#[tracing::instrument]
async fn gen_lobby_token(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	lobby_id: Uuid,
) -> GlobalResult<(String, Uuid)> {
	let token_res = op!([ctx] token_create {
		issuer: "mm-lobby-create".into(),
		token_config: Some(token::create::request::TokenConfig {
			ttl: util::duration::days(365),
		}),
		refresh_token_config: None,
		client: None,
		kind: Some(token::create::request::Kind::New(token::create::request::KindNew {
			entitlements: vec![
				proto::claims::Entitlement {
					kind: Some(
						proto::claims::entitlement::Kind::MatchmakerLobby(proto::claims::entitlement::MatchmakerLobby {
							lobby_id: Some(lobby_id.into()),
						})
					)
				}
			],
		})),
		label: Some("lobby".into()),
		..Default::default()
	})
	.await?;

	let token = unwrap_ref!(token_res.token);
	let token_session_id = unwrap_ref!(token_res.session_id).as_uuid();

	Ok((token.token.clone(), token_session_id))
}

#[derive(Clone)]
struct UpdateDbOpts {
	lobby_id: Uuid,
	namespace_id: Uuid,
	region_id: Uuid,
	lobby_group_id: Uuid,
	token_session_id: Uuid,
	run_id: Uuid,
	create_ray_id: Uuid,
	lobby_group: backend::matchmaker::LobbyGroup,
	creator_user_id: Option<Uuid>,
	is_custom: bool,
	publicity: Option<backend::matchmaker::lobby::Publicity>,
	max_players_normal: u32,
	max_players_direct: u32,
}

#[tracing::instrument(skip_all)]
async fn update_db(
	ctx: OperationContext<mm::msg::lobby_create::Message>,
	tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
	opts: UpdateDbOpts,
) -> GlobalResult<()> {
	let now = ctx.ts();

	// Check the lobby was created preemptively created and already stopped.
	//
	// This can happen when preemptively created in mm-lobby-find then
	// mm-lobby-cleanup is called.
	//
	// This will lock the lobby for the duration of the transaction
	let lobby_row = sql_fetch_optional!(
		[ctx, (Option<i64>, Option<i64>), @tx tx]
		"SELECT stop_ts, preemptive_create_ts FROM db_mm_state.lobbies WHERE lobby_id = $1 FOR UPDATE",
		opts.lobby_id,
	)
	.await?;
	if let Some((stop_ts, preemptive_create_ts)) = lobby_row {
		if preemptive_create_ts.is_none() {
			tracing::error!("lobby row exists but is not preemptively created");
			return Ok(());
		}
		if stop_ts.is_some() {
			tracing::info!("lobby already stopped");
			return Ok(());
		}
	}

	// Upsert lobby. May have already been inserted preemptively in
	// mm-lobby-find.
	sql_execute!(
		[ctx, @tx tx]
		"
		UPSERT INTO db_mm_state.lobbies (
			lobby_id,
			namespace_id,
			region_id,
			lobby_group_id,
			token_session_id,
			create_ts,
			run_id,
			create_ray_id,
			
			max_players_normal,
			max_players_direct,
			max_players_party,

			is_closed,
			creator_user_id,
			is_custom,
			publicity
		)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, false, $12, $13, $14)
		",
		opts.lobby_id,
		opts.namespace_id,
		opts.region_id,
		opts.lobby_group_id,
		opts.token_session_id,
		now,
		opts.run_id,
		opts.create_ray_id,
		opts.max_players_normal as i64,
		opts.max_players_direct as i64,
		opts.lobby_group.max_players_party as i64,
		opts.creator_user_id,
		opts.is_custom,
		opts.publicity
			.unwrap_or(backend::matchmaker::lobby::Publicity::Public) as i64,
	)
	.await?;

	Ok(())
}

#[tracing::instrument]
async fn create_docker_job(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	runtime: &backend::matchmaker::lobby_runtime::Docker,
	runtime_meta: &backend::matchmaker::lobby_runtime_meta::Docker,
	namespace: &backend::game::Namespace,
	version: &backend::game::Version,
	mm_game_config: &backend::matchmaker::GameConfig,
	lobby_group: &backend::matchmaker::LobbyGroup,
	lobby_group_meta: &backend::matchmaker::LobbyGroupMeta,
	region: &backend::region::Region,
	tier: &backend::region::Tier,
	max_players_normal: u32,
	max_players_direct: u32,
	run_id: Uuid,
	lobby_id: Uuid,
	lobby_token: &str,
) -> GlobalResult<()> {
	let namespace_id = unwrap_ref!(namespace.namespace_id).as_uuid();
	let version_id = unwrap_ref!(version.version_id).as_uuid();
	let lobby_group_id = unwrap_ref!(lobby_group_meta.lobby_group_id).as_uuid();
	let region_id = unwrap_ref!(region.region_id).as_uuid();

	let job_runner_binary_url = resolve_job_runner_binary_url(ctx).await?;

	let resolve_perf = ctx.perf().start("resolve-image-artifact-url").await;
	let build_id = unwrap_ref!(runtime.build_id).as_uuid();
	let image_artifact_url = resolve_image_artifact_url(ctx, build_id, region).await?;
	resolve_perf.end();

	// Validate build exists and belongs to this game
	let build_id = unwrap_ref!(runtime.build_id).as_uuid();
	let build_get = op!([ctx] build_get {
		build_ids: vec![build_id.into()],
	})
	.await?;
	let build = unwrap!(build_get.builds.first());
	let build_kind = unwrap!(backend::build::BuildKind::from_i32(build.kind));
	let build_compression = unwrap!(backend::build::BuildCompression::from_i32(
		build.compression
	));

	// Generate the Docker job
	let job_spec = nomad_job::gen_lobby_docker_job(
		runtime,
		&build.image_tag,
		tier,
		ctx.lobby_config_json.is_some(),
		!ctx.tags.is_empty(),
		build_kind,
		build_compression,
	)?;
	let job_spec_json = serde_json::to_string(&job_spec)?;

	// Build proxied ports for each exposed port
	let proxied_ports = runtime
		.ports
		.iter()
		.filter(|port| {
			port.proxy_kind == backend::matchmaker::lobby_runtime::ProxyKind::GameGuard as i32
				&& port.port_range.is_none()
		})
		.flat_map(|port| {
			let mut ports = vec![direct_proxied_port(lobby_id, region_id, port)];
			match backend::matchmaker::lobby_runtime::ProxyProtocol::from_i32(port.proxy_protocol) {
				Some(
					backend::matchmaker::lobby_runtime::ProxyProtocol::Http
					| backend::matchmaker::lobby_runtime::ProxyProtocol::Https,
				) => {
					ports.push(path_proxied_port(lobby_id, region_id, port));
				}
				Some(
					backend::matchmaker::lobby_runtime::ProxyProtocol::Udp
					| backend::matchmaker::lobby_runtime::ProxyProtocol::Tcp
					| backend::matchmaker::lobby_runtime::ProxyProtocol::TcpTls,
				)
				| None => {}
			}
			ports
		})
		.collect::<GlobalResult<Vec<_>>>()?;

	msg!([ctx] job_run::msg::create(run_id) {
		run_id: Some(run_id.into()),
		region_id: Some(region_id.into()),
		parameters: vec![
			job_run::msg::create::Parameter {
				key: "job_runner_binary_url".into(),
				value: job_runner_binary_url,
			},
			job_run::msg::create::Parameter {
				key: "vector_socket_addr".into(),
				value: "127.0.0.1:5021".to_string(),
			},
			job_run::msg::create::Parameter {
				key: "image_artifact_url".into(),
				value: image_artifact_url.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "namespace_id".into(),
				value: namespace_id.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "namespace_name".into(),
				value: namespace.name_id.to_owned(),
			},
			job_run::msg::create::Parameter {
				key: "version_id".into(),
				value: version_id.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "version_name".into(),
				value: version.display_name.to_owned(),
			},
			job_run::msg::create::Parameter {
				key: "lobby_group_id".into(),
				value: lobby_group_id.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "lobby_group_name".into(),
				value: lobby_group.name_id.clone(),
			},
			job_run::msg::create::Parameter {
				key: "lobby_id".into(),
				value: lobby_id.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "lobby_token".into(),
				value: lobby_token.to_owned(),
			},
			job_run::msg::create::Parameter {
				key: "lobby_config".into(),
				value: ctx.lobby_config_json.clone().unwrap_or_default(),
			},
			job_run::msg::create::Parameter {
				key: "lobby_tags".into(),
				value: serde_json::to_string(&ctx.tags)?,
			},
			job_run::msg::create::Parameter {
				key: "region_id".into(),
				value: region_id.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "region_name".into(),
				value: region.name_id.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "max_players_normal".into(),
				value: max_players_normal.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "max_players_direct".into(),
				value: max_players_direct.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "max_players_party".into(),
				value: lobby_group.max_players_party.to_string(),
			},
			job_run::msg::create::Parameter {
				key: "root_user_enabled".into(),
				value: if mm_game_config.root_user_enabled { "1" } else { "0" }.into()
			},
		],
		job_spec_json: job_spec_json,
		proxied_ports: proxied_ports,
		..Default::default()
	})
	.await?;

	Ok(())
}

/// Generates a presigned URL for the job runner binary.
#[tracing::instrument]
async fn resolve_job_runner_binary_url(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
) -> GlobalResult<String> {
	// Build client
	let s3_client = s3_util::Client::from_env_opt(
		"bucket-infra-artifacts",
		s3_util::Provider::default()?,
		s3_util::EndpointKind::External,
	)
	.await?;
	let presigned_req = s3_client
		.get_object()
		.bucket(s3_client.bucket())
		.key("job-runner/job-runner")
		.presigned(
			s3_util::aws_sdk_s3::presigning::config::PresigningConfig::builder()
				.expires_in(std::time::Duration::from_secs(15 * 60))
				.build()?,
		)
		.await?;

	let addr = presigned_req.uri().clone();

	let addr_str = addr.to_string();
	tracing::info!(addr = %addr_str, "resolved job runner presigned request");

	Ok(addr_str)
}

#[tracing::instrument]
async fn resolve_image_artifact_url(
	ctx: &OperationContext<mm::msg::lobby_create::Message>,
	build_id: Uuid,
	region: &backend::region::Region,
) -> GlobalResult<String> {
	let build_res = op!([ctx] build_get {
		build_ids: vec![build_id.into()],
	})
	.await?;
	let build = build_res.builds.first();
	let build = unwrap_ref!(build);
	let build_kind = unwrap!(backend::build::BuildKind::from_i32(build.kind));
	let build_compression = unwrap!(backend::build::BuildCompression::from_i32(
		build.compression
	));
	let upload_id_proto = unwrap!(build.upload_id);

	let upload_res = op!([ctx] upload_get {
		upload_ids: vec![upload_id_proto],
	})
	.await?;
	let upload = unwrap!(upload_res.uploads.first());

	// Get provider
	let proto_provider = unwrap!(
		backend::upload::Provider::from_i32(upload.provider),
		"invalid upload provider"
	);
	let provider = match proto_provider {
		backend::upload::Provider::Minio => s3_util::Provider::Minio,
		backend::upload::Provider::Backblaze => s3_util::Provider::Backblaze,
		backend::upload::Provider::Aws => s3_util::Provider::Aws,
	};

	let file_name = util_build::file_name(build_kind, build_compression);

	let mm_lobby_delivery_method = unwrap!(
		backend::cluster::BuildDeliveryMethod::from_i32(region.build_delivery_method),
		"invalid datacenter build delivery method"
	);
	match mm_lobby_delivery_method {
		backend::cluster::BuildDeliveryMethod::S3Direct => {
			tracing::info!("using s3 direct delivery");

			let bucket = "bucket-build";

			// Build client
			let s3_client =
				s3_util::Client::from_env_opt(bucket, provider, s3_util::EndpointKind::External)
					.await?;

			let upload_id = unwrap_ref!(upload.upload_id).as_uuid();
			let presigned_req = s3_client
				.get_object()
				.bucket(s3_client.bucket())
				.key(format!("{upload_id}/{file_name}"))
				.presigned(
					s3_util::aws_sdk_s3::presigning::config::PresigningConfig::builder()
						.expires_in(std::time::Duration::from_secs(15 * 60))
						.build()?,
				)
				.await?;

			let addr = presigned_req.uri().clone();

			let addr_str = addr.to_string();
			tracing::info!(addr = %addr_str, "resolved artifact s3 presigned request");

			Ok(addr_str)
		}
		backend::cluster::BuildDeliveryMethod::TrafficServer => {
			tracing::info!("using traffic server delivery");

			let region_id = unwrap_ref!(region.region_id).as_uuid();

			// Hash build id
			let build_id = unwrap_ref!(build.build_id).as_uuid();
			let mut hasher = DefaultHasher::new();
			hasher.write(build_id.as_bytes());
			let hash = hasher.finish() as i64;

			// Get vlan ip from build id hash for consistent routing
			let (ats_vlan_ip,) = sql_fetch_one!(
				[ctx, (String,)]
				"
				WITH sel AS (
					-- Select candidate vlan ips
					SELECT
						vlan_ip
					FROM db_cluster.servers
					WHERE
						datacenter_id = $1 AND
						pool_type = $2 AND
						vlan_ip IS NOT NULL AND
						cloud_destroy_ts IS NULL	
				)
				SELECT vlan_ip
				FROM sel
				-- Use mod to make sure the hash stays within bounds
				OFFSET abs($3 % (SELECT COUNT(*) from sel))
				LIMIT 1
				",
				// NOTE: region_id is just the old name for datacenter_id
				&region_id,
				backend::cluster::PoolType::Ats as i64,
				hash,
			)
			.await?;

			let upload_id = unwrap_ref!(upload.upload_id).as_uuid();
			let addr = format!(
				"http://{vlan_ip}:8080/s3-cache/{provider}/{namespace}-bucket-build/{upload_id}/{file_name}",
				vlan_ip = ats_vlan_ip,
				provider = heck::KebabCase::to_kebab_case(provider.as_str()),
				namespace = util::env::namespace(),
				upload_id = upload_id,
			);

			tracing::info!(%addr, "resolved artifact s3 url");

			Ok(addr)
		}
	}
}

fn direct_proxied_port(
	lobby_id: Uuid,
	region_id: Uuid,
	port: &backend::matchmaker::lobby_runtime::Port,
) -> GlobalResult<backend::job::ProxiedPortConfig> {
	Ok(backend::job::ProxiedPortConfig {
		// Match the port label generated in mm-config-version-prepare
		// and in api-matchmaker
		target_nomad_port_label: Some(util_mm::format_nomad_port_label(&port.label)),
		ingress_port: None,
		ingress_hostnames: vec![format!(
			"{}-{}.lobby.{}.{}",
			lobby_id,
			port.label,
			region_id,
			unwrap!(util::env::domain_job()),
		)],
		proxy_protocol: job_proxy_protocol(port.proxy_protocol)? as i32,
		ssl_domain_mode: backend::job::SslDomainMode::ParentWildcard as i32,
	})
}

fn path_proxied_port(
	lobby_id: Uuid,
	region_id: Uuid,
	port: &backend::matchmaker::lobby_runtime::Port,
) -> GlobalResult<backend::job::ProxiedPortConfig> {
	Ok(backend::job::ProxiedPortConfig {
		// Match the port label generated in mm-config-version-prepare
		// and in api-matchmaker
		target_nomad_port_label: Some(util_mm::format_nomad_port_label(&port.label)),
		ingress_port: None,
		// TODO: Not just for hostnames anymore, change name?
		ingress_hostnames: vec![format!(
			"lobby.{}.{}/{}-{}",
			region_id,
			unwrap!(util::env::domain_job()),
			lobby_id,
			port.label,
		)],
		proxy_protocol: job_proxy_protocol(port.proxy_protocol)? as i32,
		ssl_domain_mode: backend::job::SslDomainMode::Exact as i32,
	})
}

fn job_proxy_protocol(proxy_protocol: i32) -> GlobalResult<backend::job::ProxyProtocol> {
	let proxy_protocol = unwrap!(backend::matchmaker::lobby_runtime::ProxyProtocol::from_i32(
		proxy_protocol
	));
	let job_proxy_protocol = match proxy_protocol {
		backend::matchmaker::lobby_runtime::ProxyProtocol::Http => {
			backend::job::ProxyProtocol::Http
		}
		backend::matchmaker::lobby_runtime::ProxyProtocol::Https => {
			backend::job::ProxyProtocol::Https
		}
		backend::matchmaker::lobby_runtime::ProxyProtocol::Tcp => backend::job::ProxyProtocol::Tcp,
		backend::matchmaker::lobby_runtime::ProxyProtocol::TcpTls => {
			backend::job::ProxyProtocol::TcpTls
		}
		backend::matchmaker::lobby_runtime::ProxyProtocol::Udp => backend::job::ProxyProtocol::Udp,
	};

	Ok(job_proxy_protocol)
}
