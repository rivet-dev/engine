use proto::backend::pkg::*;
use rivet_operation::prelude::*;
use serde_json::json;

#[operation(name = "cdn-namespace-domain-create")]
async fn handle(
	ctx: OperationContext<cdn::namespace_domain_create::Request>,
) -> GlobalResult<cdn::namespace_domain_create::Response> {
	ensure!(ctx.config().server()?.cloudflare()?.zone.game.is_some());

	let namespace_id = unwrap_ref!(ctx.namespace_id).as_uuid();
	ensure_with!(
		util::check::domain(ctx.config(), &ctx.domain, true),
		CDN_INVALID_DOMAIN
	);

	let game_res = op!([ctx] game_resolve_namespace_id {
		namespace_ids: vec![namespace_id.into()],
	})
	.await?;
	let game = unwrap!(game_res.games.first());
	let game_id = unwrap_ref!(game.game_id).as_uuid();

	let game_res = op!([ctx] game_get {
		game_ids: vec![game_id.into()],
	})
	.await?;
	let game = unwrap!(game_res.games.first());
	let developer_team_id = unwrap_ref!(game.developer_team_id).as_uuid();

	let (domain_count,) = sql_fetch_one!(
		[ctx, (i64,)]
		"SELECT COUNT(*) FROM db_cdn.game_namespace_domains WHERE namespace_id = $1",
		namespace_id,
	)
	.await?;

	ensure_with!(domain_count < 10, CDN_TOO_MANY_DOMAINS);

	sql_execute!(
		[ctx]
		"
		INSERT INTO db_cdn.game_namespace_domains (namespace_id, domain, create_ts)
		VALUES ($1, $2, $3)
		",
		namespace_id,
		&ctx.domain,
		ctx.ts(),
	)
	.await?;

	// Create a cloudflare custom hostname
	{
		let custom_hostname_res = msg!([ctx] cf_custom_hostname::msg::create(namespace_id, &ctx.domain) -> Result<cf_custom_hostname::msg::create_complete, cf_custom_hostname::msg::create_fail> {
			namespace_id: ctx.namespace_id,
			hostname: ctx.domain.clone(),
			bypass_pending_cap: false,
		}).await?;

		match custom_hostname_res {
			Ok(_) => {}
			Err(msg) => {
				use cf_custom_hostname::msg::create_fail::ErrorCode::*;

				let code =
					cf_custom_hostname::msg::create_fail::ErrorCode::from_i32(msg.error_code);
				match unwrap!(code) {
					Unknown => bail!("unknown custom hostname create error code"),
					AlreadyExists => {
						rollback(&ctx, namespace_id, &ctx.domain).await?;
						bail_with!(CLOUD_HOSTNAME_TAKEN)
					}
					TooManyPendingHostnames => {
						rollback(&ctx, namespace_id, &ctx.domain).await?;
						bail_with!(CLOUD_TOO_MANY_PENDING_HOSTNAMES_FOR_GROUP)
					}
				}
			}
		};
	}

	msg!([ctx] cdn::msg::ns_config_update(namespace_id) {
		namespace_id: Some(namespace_id.into()),
	})
	.await?;

	msg!([ctx] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "cdn.domain.update".into(),
				properties_json: Some(serde_json::to_string(&json!({
					"developer_team_id": developer_team_id,
					"game_id": game_id,
					"namespace_id": namespace_id,
					"domain": ctx.domain,
				}))?),
				..Default::default()
			}
		],
	})
	.await?;

	Ok(cdn::namespace_domain_create::Response {})
}

async fn rollback(
	ctx: &OperationContext<cdn::namespace_domain_create::Request>,
	namespace_id: Uuid,
	domain: &str,
) -> GlobalResult<()> {
	// Rollback
	sql_execute!(
		[ctx]
		"DELETE FROM db_cdn.game_namespace_domains WHERE namespace_id = $1 AND domain = $2",
		namespace_id,
		domain,
	)
	.await?;

	Ok(())
}
