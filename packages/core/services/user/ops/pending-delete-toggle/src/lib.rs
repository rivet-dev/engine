use proto::backend::pkg::*;
use rivet_operation::prelude::*;

#[operation(name = "pending-delete-toggle")]
async fn handle(
	ctx: OperationContext<user::pending_delete_toggle::Request>,
) -> GlobalResult<user::pending_delete_toggle::Response> {
	let user_id = unwrap_ref!(ctx.user_id).as_uuid();

	// Verify the user is registered
	let identity = op!([ctx] user_identity_get {
		user_ids: vec![user_id.into()],
	})
	.await?;
	let identities = &unwrap_ref!(identity.users.first()).identities;
	ensure_with!(!identities.is_empty(), IDENTITY_NOT_REGISTERED);

	sql_execute!(
		[ctx]
		"UPDATE db_user.users SET delete_request_ts = $2 WHERE user_id = $1",
		user_id,
		ctx.active.then(util::timestamp::now),
	)
	.await?;

	ctx.cache().purge("user", [user_id]).await?;

	msg!([ctx] user::msg::update(user_id) {
		user_id: ctx.user_id,
	})
	.await?;

	Ok(user::pending_delete_toggle::Response {})
}
