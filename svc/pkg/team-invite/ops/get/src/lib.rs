use proto::backend::pkg::*;
use rivet_operation::prelude::*;

#[derive(Debug, sqlx::FromRow)]
struct InvitationRow {
	code: String,
	team_id: Uuid,
	create_ts: i64,
	expire_ts: Option<i64>,
	max_use_count: Option<i64>,
	revoke_ts: Option<i64>,
}

#[operation(name = "team-invite-get")]
async fn handle(
	ctx: OperationContext<team_invite::get::Request>,
) -> GlobalResult<team_invite::get::Response> {
	let _crdb = ctx.crdb().await?;

	// Find the invitation
	let invitations = sql_fetch_all!(
		[ctx, InvitationRow]
		"
		SELECT
			code, team_id, create_ts, expire_ts, max_use_count, revoke_ts
		FROM db_team_invite.invitations
		WHERE code = ANY($1)
		",
		&ctx.codes,
	)
	.await?;

	Ok(team_invite::get::Response {
		invites: invitations
			.into_iter()
			.map(|invite| team_invite::get::response::Invite {
				code: invite.code,
				team_id: Some(invite.team_id.into()),
				create_ts: invite.create_ts,
				expire_ts: invite.expire_ts,
				use_count: invite.max_use_count,
				revoke_ts: invite.revoke_ts,
			})
			.collect::<Vec<_>>(),
	})
}
