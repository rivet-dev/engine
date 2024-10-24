use chirp_worker::prelude::*;
use proto::backend::pkg::*;
use serde_json::json;

#[worker(name = "team-join-request-resolve")]
async fn worker(
	ctx: &OperationContext<team::msg::join_request_resolve::Message>,
) -> GlobalResult<()> {
	let team_id: Uuid = unwrap_ref!(ctx.team_id).as_uuid();
	let user_id: Uuid = unwrap_ref!(ctx.user_id).as_uuid();

	sql_execute!(
		[ctx]
		"DELETE FROM db_team.join_requests WHERE team_id = $1 AND user_id = $2",
		team_id,
		user_id,
	)
	.await?;

	if ctx.resolution {
		// Create the team member
		msg!([ctx] team::msg::member_create(team_id, user_id) -> team::msg::member_create_complete {
			team_id: Some(team_id.into()),
			user_id: Some(user_id.into()),
			invitation: None,
		})
		.await?;
	}

	// Dispatch events
	msg!([ctx] team::msg::join_request_resolve_complete(team_id, user_id) {
		team_id: Some(team_id.into()),
		user_id: Some(user_id.into()),
	})
	.await?;

	msg!([ctx] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "team.join_request.resolve".into(),
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
