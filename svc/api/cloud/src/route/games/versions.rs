use api_helper::{anchor::WatchIndexQuery, ctx::Ctx};
use proto::backend::pkg::*;
use rivet_api::models;
use rivet_claims::ClaimsDecode;
use rivet_convert::{ApiInto, ApiTryFrom};
use rivet_operation::prelude::*;

use crate::{assert, auth::Auth};

// MARK: GET /games/{}/versions/{}
pub async fn get(
	ctx: Ctx<Auth>,
	game_id: Uuid,
	version_id: Uuid,
	_watch_index: WatchIndexQuery,
) -> GlobalResult<models::CloudGamesGetGameVersionByIdResponse> {
	ctx.auth()
		.check_game_read_or_admin(ctx.op_ctx(), game_id)
		.await?;
	let game_version = assert::version_for_game(&ctx, game_id, version_id).await?;

	let cloud_version_res = op!([ctx] cloud_version_get {
		version_ids: vec![version_id.into()],
	})
	.await?;
	let cloud_version = unwrap!(cloud_version_res.versions.first());
	let cloud_version_config = unwrap_ref!(cloud_version.config);

	let summary = models::CloudVersionSummary::api_try_from(game_version)?;
	let openapi_version = rivet_convert::cloud::version::config_to_openapi(
		ctx.op_ctx(),
		cloud_version_config.clone(),
	)
	.await?;
	let version = models::CloudVersionFull {
		version_id: summary.version_id,
		create_ts: summary.create_ts,
		display_name: summary.display_name,
		config: Box::new(openapi_version),
	};

	Ok(models::CloudGamesGetGameVersionByIdResponse {
		version: Box::new(version),
	})
}

// MARK: POST /games/{}/versions
pub async fn create(
	ctx: Ctx<Auth>,
	game_id: Uuid,
	body: models::CloudGamesCreateGameVersionRequest,
) -> GlobalResult<models::CloudGamesCreateGameVersionResponse> {
	ctx.auth()
		.check_game_write_or_admin(ctx.op_ctx(), game_id)
		.await?;

	let user_id = ctx.auth().claims()?.as_user().ok();

	let proto_config =
		rivet_convert::cloud::version::config_to_proto(ctx.op_ctx(), *body.config).await?;
	let publish_res = op!([ctx] cloud_version_publish {
		game_id: Some(game_id.into()),
		display_name: body.display_name,
		config: Some(proto_config),
		creator_user_id: user_id.as_ref().map(|x| x.user_id.into()),
	})
	.await?;
	let version_id = unwrap_ref!(publish_res.version_id).as_uuid();

	Ok(models::CloudGamesCreateGameVersionResponse { version_id })
}

// MARK: POST /games/{}/versions/reserve-name
pub async fn reserve_name(
	ctx: Ctx<Auth>,
	game_id: Uuid,
	_body: serde_json::Value,
) -> GlobalResult<models::CloudGamesReserveVersionNameResponse> {
	ctx.auth()
		.check_game_write_or_admin(ctx.op_ctx(), game_id)
		.await?;

	let request_id = uuid::Uuid::new_v4();
	let res = msg!([ctx] cloud::msg::version_name_reserve(game_id, request_id) -> cloud::msg::version_name_reserve_complete {
		game_id: Some(game_id.into()),
		request_id: Some(request_id.into()),
	}).await?;

	Ok(models::CloudGamesReserveVersionNameResponse {
		version_display_name: res.version_display_name.clone(),
	})
}

// MARK: POST /games/{}/versions/validate
pub async fn validate(
	ctx: Ctx<Auth>,
	game_id: Uuid,
	body: models::CloudGamesValidateGameVersionRequest,
) -> GlobalResult<models::CloudGamesValidateGameVersionResponse> {
	let proto_config =
		rivet_convert::cloud::version::config_to_proto(ctx.op_ctx(), *body.config).await?;
	let res = op!([ctx] game_version_validate {
		game_id: Some(game_id.into()),
		display_name: body.display_name,
		config: Some(proto_config),
	})
	.await?;

	Ok(models::CloudGamesValidateGameVersionResponse {
		errors: res
			.errors
			.into_iter()
			.map(ApiInto::api_into)
			.collect::<Vec<_>>(),
	})
}
