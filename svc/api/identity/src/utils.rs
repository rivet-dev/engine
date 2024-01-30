use api_helper::ctx::Ctx;
use proto::common;

use rivet_operation::prelude::*;

use crate::auth::Auth;

// Used to get the game id when the game user has not been made yet
pub async fn get_game_id(ctx: &Ctx<Auth>) -> GlobalResult<common::Uuid> {
	let namespace_id = if let Some(ns_dev_ent) = ctx.auth().game_ns_dev_option()? {
		ns_dev_ent.namespace_id
	} else {
		ctx.auth().game_ns(ctx).await?.namespace_id
	};

	let namespace_res = op!([ctx] game_namespace_get {
		namespace_ids: vec![namespace_id.into()]
	})
	.await?;

	let namespace = unwrap!(namespace_res.namespaces.first()).clone();

	Ok(unwrap!(namespace.game_id))
}

// Used to get the game id when the game user has not been made yet
pub async fn get_namespace_id(ctx: &Ctx<Auth>) -> GlobalResult<common::Uuid> {
	let namespace_id = if let Some(ns_dev_ent) = ctx.auth().game_ns_dev_option()? {
		ns_dev_ent.namespace_id
	} else {
		ctx.auth().game_ns(ctx).await?.namespace_id
	};

	Ok(namespace_id.into())
}

// Returns the user id
pub async fn resolve_user_with_game_user_id(
	ctx: &Ctx<Auth>,
	game_user_id: Uuid,
) -> GlobalResult<Option<Uuid>> {
	let game_user_res = op!([ctx] game_user_get {
		game_user_ids: vec![game_user_id.into()]
	})
	.await?;
	let Some(game_user) = game_user_res.game_users.first().clone() else {
		return Ok(None);
	};

	Ok(Some(unwrap_ref!(game_user.user_id).as_uuid()))
}

pub fn touch_user_presence(ctx: OperationContext<()>, user_id: Uuid, silent: bool) {
	let spawn_res = tokio::task::Builder::new()
		.name("api_identity::user_presence_touch")
		.spawn(async move {
			let res = op!([ctx] user_presence_touch {
				user_id: Some(user_id.into()),
				silent: silent,
			})
			.await;
			match res {
				Ok(_) => {}
				Err(err) => tracing::error!(?err, "failed to touch user presence"),
			}
		});
	if let Err(err) = spawn_res {
		tracing::error!(?err, "failed to spawn user_presence_touch task");
	}
}

pub async fn validate_config(
	ctx: &OperationContext<()>,
	namespace_id: common::Uuid,
) -> GlobalResult<()> {
	let namespaces_res = op!([ctx] game_namespace_get {
		namespace_ids: vec![namespace_id],
	})
	.await?;
	let namespace = unwrap!(namespaces_res.namespaces.first());

	let version_id = unwrap_ref!(namespace.version_id);
	let config_res = op!([ctx] identity_config_version_get {
		version_ids: vec![*version_id],
	})
	.await?;

	ensure_with!(
		!config_res.versions.is_empty(),
		API_FORBIDDEN,
		reason = "Identity service not enabled for this namespace"
	);

	Ok(())
}
