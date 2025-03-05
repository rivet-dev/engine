use chirp_worker::prelude::*;
use proto::backend::pkg::*;
use serde_json::json;

async fn fail(
	client: &chirp_client::Client,
	team_id: Uuid,
	user_id: Uuid,
	error_code: team::msg::join_request_create_fail::ErrorCode,
) -> GlobalResult<()> {
	msg!([client] team::msg::join_request_create_fail(team_id, user_id) {
		team_id: Some(team_id.into()),
		user_id: Some(user_id.into()),
		error_code: error_code as i32,
	})
	.await?;

	msg!([client] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "team.join_request.create_fail".into(),
				properties_json: Some(serde_json::to_string(&json!({
					"team_id": team_id,
					"error": error_code as i32,
					"user_id": user_id,
				}))?),
				..Default::default()
			}
		],
	})
	.await?;

	Ok(())
}

#[worker(name = "team-join-request-create")]
async fn worker(
	ctx: &OperationContext<team::msg::join_request_create::Message>,
) -> GlobalResult<()> {
	let team_id: Uuid = unwrap_ref!(ctx.team_id).as_uuid();
	let user_id: Uuid = unwrap_ref!(ctx.user_id).as_uuid();

	let (sql_exists,) = sql_fetch_one!(
		[ctx, (bool,)]
		"
		SELECT EXISTS (
			SELECT 1
			FROM db_team.join_requests
			WHERE
				team_id = $1 AND
				user_id = $2
		)
		",
		team_id,
		user_id,
	)
	.await?;

	if sql_exists {
		return fail(
			ctx.chirp(),
			team_id,
			user_id,
			team::msg::join_request_create_fail::ErrorCode::RequestAlreadyExists,
		)
		.await;
	}

	sql_execute!(
		[ctx]
		"INSERT INTO db_team.join_requests (team_id, user_id, ts) VALUES ($1, $2, $3)",
		team_id,
		user_id,
		ctx.ts(),
	)
	.await?;

	msg!([ctx] team::msg::join_request_create_complete(team_id, user_id) {
		team_id: Some(team_id.into()),
		user_id: Some(user_id.into()),
	})
	.await?;

	msg!([ctx] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "team.join_request.create".into(),
				properties_json: Some(serde_json::to_string(&json!({
					"team_id": team_id,
					"user_id": user_id,
				}))?),
				..Default::default()
			}
		],
	})
	.await?;

	Ok(())
}
