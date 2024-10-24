use proto::backend::pkg::*;
use rivet_operation::prelude::*;

#[operation(name = "team-member-count")]
async fn handle(
	ctx: OperationContext<team::member_count::Request>,
) -> GlobalResult<team::member_count::Response> {
	let team_ids = ctx
		.team_ids
		.iter()
		.map(|id| id.as_uuid())
		.collect::<Vec<_>>();

	let member_counts = sql_fetch_all!(
		[ctx, (Uuid, i64)]
		"
		SELECT team_id, COUNT(*)
		FROM db_team.team_members
		WHERE team_id = ANY($1::UUID[])
		GROUP BY team_id
		",
		&team_ids,
	)
	.await?;

	Ok(team::member_count::Response {
		teams: team_ids
			.iter()
			.map(|team_id| {
				let member_count = member_counts
					.iter()
					.find(|(tid, _)| tid == team_id)
					.map(|(_, count)| *count)
					.unwrap_or_default();

				team::member_count::response::Team {
					team_id: Some((*team_id).into()),
					member_count: member_count as u32,
				}
			})
			.collect::<Vec<_>>(),
	})
}
