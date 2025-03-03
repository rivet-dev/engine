use proto::backend::pkg::*;
use rivet_operation::prelude::*;

#[operation(name = "mm-lobby-history")]
async fn handle(
	ctx: OperationContext<mm::lobby_history::Request>,
) -> GlobalResult<mm::lobby_history::Response> {
	let namespace_id = unwrap_ref!(ctx.namespace_id).as_uuid();

	// Use AS OF SYSTEM TIME to reduce contention.
	// https://www.cockroachlabs.com/docs/v22.2/performance-best-practices-overview#use-as-of-system-time-to-decrease-conflicts-with-long-running-queries
	let lobby_ids = sql_fetch_all!(
		[ctx, (Uuid,)]
		"
		SELECT lobby_id
		FROM db_mm_state.lobbies AS OF SYSTEM TIME '-5s'
		WHERE namespace_id = $1 AND create_ts < $2
		ORDER BY create_ts DESC
		LIMIT $3
		",
		namespace_id,
		ctx.before_create_ts,
		ctx.count as i32,
	)
	.await?
	.into_iter()
	.map(|x| x.0)
	.map(Into::<common::Uuid>::into)
	.collect::<Vec<_>>();

	Ok(mm::lobby_history::Response { lobby_ids })
}
