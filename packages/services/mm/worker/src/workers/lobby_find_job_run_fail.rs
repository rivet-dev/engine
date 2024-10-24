use chirp_worker::prelude::*;
use proto::backend::{self, pkg::*};

#[derive(Debug, sqlx::FromRow)]
struct LobbyRow {
	lobby_id: Uuid,
	namespace_id: Uuid,
}

#[worker(name = "mm-lobby-find-job-run-fail")]
async fn worker(ctx: &OperationContext<job_run::msg::fail::Message>) -> GlobalResult<()> {
	let run_id = unwrap_ref!(ctx.run_id).as_uuid();

	// Find the associated lobby
	let query_rows = sql_fetch_all!(
		[ctx, (Uuid,)]
		"
		SELECT find_queries.query_id
		FROM db_mm_state.lobbies
		INNER JOIN db_mm_state.find_queries ON find_queries.lobby_id = lobbies.lobby_id
		WHERE lobbies.run_id = $1
		",
		run_id,
	)
	.await?;
	if query_rows.is_empty() {
		tracing::info!(?run_id, "no find queries for run id");
		return Ok(());
	}

	// Match the error
	let error_code = match job_run::msg::fail::ErrorCode::from_i32(ctx.error_code) {
		Some(
			job_run::msg::fail::ErrorCode::NomadEvalPlanFailed
			| job_run::msg::fail::ErrorCode::NomadDispatchFailed,
		) => {
			// We make this the same error as the one dispatched in
			// mm-lobby-find-lobby-cleanup since there is a race condition
			// between the two.
			backend::matchmaker::lobby_find::ErrorCode::LobbyStoppedPrematurely
		}
		Some(job_run::msg::fail::ErrorCode::StaleMessage) => {
			backend::matchmaker::lobby_find::ErrorCode::StaleMessage
		}
		Some(job_run::msg::fail::ErrorCode::Unknown) | None => {
			tracing::warn!("unknown job run fail error code");
			backend::matchmaker::lobby_find::ErrorCode::Unknown
		}
	};

	// Fail the queries
	let query_ids = query_rows
		.iter()
		.map(|x| x.0)
		.map(common::Uuid::from)
		.collect::<Vec<_>>();
	op!([ctx] mm_lobby_find_fail {
		query_ids: query_ids,
		error_code: error_code as i32,
		..Default::default()
	})
	.await?;

	Ok(())
}
