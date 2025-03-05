use chirp_worker::prelude::*;
use proto::backend::pkg::*;
use serde_json::json;

#[derive(Debug, sqlx::FromRow)]
struct InvitationRow {
	team_id: Uuid,
	expire_ts: Option<i64>,
	max_use_count: Option<i64>,
	use_counter: i64,
	revoke_ts: Option<i64>,
}

#[worker(name = "team-invite-consume")]
async fn worker(ctx: &OperationContext<team_invite::msg::consume::Message>) -> GlobalResult<()> {
	let user_id = unwrap_ref!(ctx.user_id).as_uuid();

	let db_output = rivet_pools::utils::crdb::tx(&ctx.crdb().await?, |tx| {
		let code = ctx.code.clone();
		Box::pin(update_db(ctx.base(), tx, ctx.ts(), code, user_id))
	})
	.await?;

	match db_output {
		DbOutput::Success { invitation_row } => {
			msg!([ctx] team::msg::member_create(invitation_row.team_id, user_id) {
				team_id: Some(invitation_row.team_id.into()),
				user_id: Some(user_id.into()),
				invitation: Some(team::msg::member_create::Invitation {
					code: ctx.code.clone(),
				}),
			})
			.await?;
		}
		DbOutput::Fail {
			team_id,
			error_code,
		} => {
			fail(ctx.chirp(), user_id, &ctx.code, team_id, error_code).await?;
		}
	}

	Ok(())
}

#[tracing::instrument]
async fn fail(
	client: &chirp_client::Client,
	user_id: Uuid,
	code: &str,
	team_id: Option<Uuid>,
	error_code: team_invite::msg::consume_fail::ErrorCode,
) -> GlobalResult<()> {
	tracing::warn!(%user_id, %code, ?error_code, "consume fail");

	msg!([client] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "team.invite.consume_fail".into(),
				properties_json: Some(serde_json::to_string(&json!({
					"user_id": user_id,
					"team_id": team_id,
					"code": code,
					"error": error_code as i32,
				}))?),
				..Default::default()
			}
		],
	})
	.await?;

	msg!([client] team_invite::msg::consume_fail(&code, user_id) {
		user_id: Some(user_id.into()),
		code: code.to_string(),
		team_id: team_id.map(Into::into),
		error_code: error_code as i32,
	})
	.await?;

	Ok(())
}

enum DbOutput {
	Success {
		invitation_row: InvitationRow,
	},
	Fail {
		team_id: Option<Uuid>,
		error_code: team_invite::msg::consume_fail::ErrorCode,
	},
}

// TODO: Speed this up by using a `WHERE` clause or CTE
#[tracing::instrument(skip_all)]
async fn update_db(
	ctx: OperationContext<()>,
	tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
	now: i64,
	code: String,
	user_id: Uuid,
) -> GlobalResult<DbOutput> {
	// Find the invitation
	let invitation_row = sql_fetch_optional!(
		[ctx, InvitationRow, @tx tx]
		"
		SELECT team_id, expire_ts, max_use_count, use_counter, revoke_ts
		FROM db_team_invite.invitations
		WHERE code = $1
		FOR UPDATE
		",
		&code,
	)
	.await?;
	let invitation_row = if let Some(invitation) = invitation_row {
		tracing::info!(?invitation, "found invitation");
		invitation
	} else {
		return Ok(DbOutput::Fail {
			team_id: None,
			error_code: team_invite::msg::consume_fail::ErrorCode::InviteCodeInvalid,
		});
	};

	// TODO: Create `team-member-get` service and replace this
	// Check the user is not part of the team
	let members_res = op!([ctx] team_member_list {
		team_ids: vec![invitation_row.team_id.into()],
		limit: None,
		anchor: None,
	})
	.await?;
	let team = unwrap!(members_res.teams.first());
	let is_member = team
		.members
		.iter()
		.filter_map(|x| x.user_id)
		.any(|x| x.as_uuid() == user_id);
	if is_member {
		return Ok(DbOutput::Fail {
			team_id: Some(invitation_row.team_id),
			error_code: team_invite::msg::consume_fail::ErrorCode::UserAlreadyTeamMember,
		});
	}

	// Check if the user is banned
	let banned_users_res = op!([ctx] team_user_ban_get {
		members: vec![team::user_ban_get::request::Member {
			team_id: Some(invitation_row.team_id.into()),
			user_id: Some(user_id.into()),
		}],
	})
	.await?;
	if !banned_users_res.banned_users.is_empty() {
		return Ok(DbOutput::Fail {
			team_id: Some(invitation_row.team_id),
			error_code: team_invite::msg::consume_fail::ErrorCode::UserBanned,
		});
	}

	// Check if the code is revoked
	if invitation_row.revoke_ts.is_some() {
		return Ok(DbOutput::Fail {
			team_id: Some(invitation_row.team_id),
			error_code: team_invite::msg::consume_fail::ErrorCode::InviteRevoked,
		});
	}

	// Check if the code is expired
	if invitation_row.expire_ts.map_or(false, |x| x < now) {
		return Ok(DbOutput::Fail {
			team_id: Some(invitation_row.team_id),
			error_code: team_invite::msg::consume_fail::ErrorCode::InviteExpired,
		});
	}

	// Check the member count
	if let Some(max_use_count) = invitation_row.max_use_count {
		if invitation_row.use_counter >= max_use_count {
			return Ok(DbOutput::Fail {
				team_id: Some(invitation_row.team_id),
				error_code: team_invite::msg::consume_fail::ErrorCode::InviteAlreadyUsed,
			});
		}
	}

	// Insert consumption
	sql_execute!(
		[ctx, @tx tx]
		"UPDATE db_team_invite.invitations SET use_counter = use_counter + 1 WHERE code = $1",
		&code,
	)
	.await?;
	sql_execute!(
		[ctx, @tx tx]
		"INSERT INTO db_team_invite.invitation_uses (code, user_id, create_ts) VALUES ($1, $2, $3)",
		&code,
		user_id,
		now,
	)
	.await?;

	Ok(DbOutput::Success { invitation_row })
}
