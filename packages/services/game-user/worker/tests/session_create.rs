use std::time::Duration;

use chirp_worker::prelude::*;
use proto::backend::pkg::*;

#[worker_test]
async fn empty(ctx: TestCtx) {
	let namespace_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	let res = op!([ctx] game_user_create {
		namespace_id: Some(namespace_id.into()),
		user_id: Some(user_id.into())
	})
	.await
	.unwrap();

	let game_user_id = res.game_user_id.unwrap().as_uuid();

	msg!([ctx] game_user::msg::session_create(game_user_id) {
		game_user_id: Some(game_user_id.into()),
		refresh_jti: Some(Uuid::new_v4().into()),
	})
	.await
	.unwrap();

	// TODO: Hack
	loop {
		let (crdb_exists,) = sql_fetch_one!(
			[ctx, (bool,)]
			"
			SELECT EXISTS (
				SELECT 1
				FROM db_game_user.sessions
				WHERE game_user_id = $1
			)
			",
			game_user_id,
		)
		.await
		.unwrap();
		if crdb_exists {
			break;
		} else {
			tracing::warn!("game user session not created yet");
			tokio::time::sleep(Duration::from_millis(250)).await;
		}
	}
}
