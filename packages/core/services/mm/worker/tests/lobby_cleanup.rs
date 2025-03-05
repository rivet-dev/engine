use chirp_worker::prelude::*;
use proto::backend::pkg::*;

#[worker_test]
async fn lobby_cleanup(ctx: TestCtx) {
	if !util::feature::job_run() {
		return;
	}

	let lobby_res = op!([ctx] faker_mm_lobby {
		..Default::default()
	})
	.await
	.unwrap();
	let lobby_id = lobby_res.lobby_id.as_ref().unwrap().as_uuid();

	msg!([ctx] mm::msg::lobby_cleanup(lobby_id) -> mm::msg::lobby_cleanup_complete {
		lobby_id: Some(lobby_id.into()),
	})
	.await
	.unwrap();

	let (stop_ts,) = sqlx::query_as::<_, (Option<i64>,)>(
		"SELECT stop_ts FROM db_mm_state.lobbies WHERE lobby_id = $1",
	)
	.bind(lobby_id)
	.fetch_one(&ctx.crdb().await.unwrap())
	.await
	.unwrap();
	assert!(stop_ts.is_some(), "lobby not removed");

	let players = sqlx::query_as::<_, (Option<i64>,)>(
		"SELECT remove_ts FROM db_mm_state.players WHERE lobby_id = $1",
	)
	.bind(lobby_id)
	.fetch_all(&ctx.crdb().await.unwrap())
	.await
	.unwrap();
	for (remove_ts,) in players {
		assert!(remove_ts.is_some(), "player not removed");
	}
}
