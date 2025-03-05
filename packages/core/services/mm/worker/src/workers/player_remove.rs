use chirp_worker::prelude::*;
use proto::backend::pkg::*;
use redis::AsyncCommands;
use serde_json::json;
use tracing::Instrument;

lazy_static::lazy_static! {
	static ref REDIS_SCRIPT: redis::Script = redis::Script::new(include_str!("../../redis-scripts/player_remove.lua"));
}

#[derive(Debug, sqlx::FromRow)]
struct PlayerRow {
	// Lobby may not exist for this ID if mm-lobby-create didn't succeed
	lobby_id: Uuid,
	create_ts: Option<i64>,
	register_ts: Option<i64>,
	remote_address: Option<String>,
	remove_ts: Option<i64>,

	// Lobby
	namespace_id: Uuid,
	region_id: Uuid,
	lobby_group_id: Uuid,
	lobby_stop_ts: Option<i64>,
	max_players_normal: i64,
	max_players_party: i64,
}

#[worker(name = "mm-player-remove")]
async fn worker(ctx: &OperationContext<mm::msg::player_remove::Message>) -> GlobalResult<()> {
	// NOTE: Idempotent

	let player_id = unwrap_ref!(ctx.player_id).as_uuid();
	let lobby_id = ctx.lobby_id.map(|x| x.as_uuid());

	// Fetch player
	let player_row = sql_fetch_optional!(
		[ctx, PlayerRow]
		"
		WITH
			select_player AS (
				SELECT
					players.lobby_id,
					players.create_ts,
					players.register_ts,
					players.remote_address,
					players.remove_ts,
					lobbies.namespace_id,
					lobbies.region_id,
					lobbies.lobby_group_id,
					lobbies.stop_ts AS lobby_stop_ts,
					lobbies.max_players_normal,
					lobbies.max_players_party
				FROM db_mm_state.players
				INNER JOIN db_mm_state.lobbies ON lobbies.lobby_id = players.lobby_id
				WHERE players.player_id = $1
			),
			_update AS (
				UPDATE db_mm_state.players
				SET remove_ts = $3
				WHERE
					player_id = $1 AND
					-- Validate lobby (if lobby provided)
					($2 IS NULL OR lobby_id = $2) AND
					-- Not already removed
					remove_ts IS NULL
				RETURNING 1
			)
		SELECT * FROM select_player
		",
		player_id,
		lobby_id,
		ctx.ts(),
	)
	.await?;
	tracing::info!(?player_row, "player row");

	let player_row = if let Some(player_row) = player_row {
		player_row
	} else if let Some(player_row) = fallback_fetch_player_from_redis(ctx, player_id).await? {
		player_row
	} else if ctx.req_dt() > util::duration::minutes(5) {
		tracing::error!("discarding stale message");
		return Ok(());
	} else {
		// Delete what we can. Remove from GC so this message doesn't get published again.
		tracing::warn!(
			?player_id,
			"player not found in sql or redis, cleaning up & discarding message"
		);
		let player_id_str = player_id.to_string();
		redis::pipe()
			.del(util_mm::key::player_config(player_id))
			.ignore()
			.zrem(util_mm::key::player_unregistered(), &player_id_str)
			.ignore()
			.zrem(util_mm::key::player_auto_remove(), &player_id_str)
			.ignore()
			.query_async(&mut ctx.redis_mm().await?)
			.await?;

		return Ok(());
	};

	// Validate lobby
	if let Some(lobby_id) = lobby_id {
		if player_row.lobby_id != lobby_id {
			tracing::info!("player not in lobby");

			return fail(
				ctx.chirp(),
				player_id,
				player_row,
				mm::msg::player_remove_fail::ErrorCode::PlayerInDifferentLobby,
			)
			.await;
		}
	} else {
		tracing::warn!("lobby id not provided");
	}

	// Player removed successfully
	if player_row.remove_ts.is_none() {
		tracing::info!("player removed");

		msg!([ctx] analytics::msg::event_create() {
			events: vec![
				analytics::msg::event_create::Event {
					event_id: Some(Uuid::new_v4().into()),
					name: "mm.player.remove".into(),
					properties_json: Some(serde_json::to_string(&json!({
						"namespace_id": player_row.namespace_id,
						"player_id": player_id,
						"lobby_id": player_row.lobby_id,
						"create_ts": player_row.create_ts,
						"register_ts": player_row.register_ts,
						"region_id": player_row.region_id,
						"lobby_group_id": player_row.lobby_group_id,
					}))?),
					..Default::default()
				}
			],
		})
		.await?;
	}

	// Remove the player from Redis (idempotent)
	let remove_perf = ctx.perf().start("remove-player-redis").await;
	let (should_remove_lobby, should_update_idle) = REDIS_SCRIPT
		.arg(ctx.ts())
		.arg(player_row.lobby_id.to_string())
		.arg(player_id.to_string())
		.arg(player_row.lobby_group_id.to_string())
		.arg(player_row.max_players_normal)
		.arg(player_row.max_players_party)
		.arg(
			// TODO: Make this configurable, but we need to write tests for this
			// TODO: Enable for custom games
			// Set to 1 for remove lobby if empty
			0i64,
		)
		.key(util_mm::key::player_config(player_id))
		.key(util_mm::key::ns_player_ids(player_row.namespace_id))
		.key(util_mm::key::lobby_player_ids(player_row.lobby_id))
		.key(util_mm::key::lobby_registered_player_ids(
			player_row.lobby_id,
		))
		.key(util_mm::key::player_unregistered())
		.key(util_mm::key::player_auto_remove())
		.key(util_mm::key::lobby_available_spots(
			player_row.namespace_id,
			player_row.region_id,
			player_row.lobby_group_id,
			util_mm::JoinKind::Normal,
		))
		.key(util_mm::key::lobby_available_spots(
			player_row.namespace_id,
			player_row.region_id,
			player_row.lobby_group_id,
			util_mm::JoinKind::Party,
		))
		.key(if let Some(remote_address) = &player_row.remote_address {
			if remote_address.is_empty() {
				util_mm::key::empty()
			} else {
				util_mm::key::ns_remote_address_player_ids(player_row.namespace_id, remote_address)
			}
		} else {
			util_mm::key::empty()
		})
		.key(util_mm::key::idle_lobby_ids(
			player_row.namespace_id,
			player_row.region_id,
			player_row.lobby_group_id,
		))
		.key(util_mm::key::idle_lobby_lobby_group_ids(
			player_row.namespace_id,
			player_row.region_id,
		))
		.key(util_mm::key::lobby_config(player_row.lobby_id))
		.invoke_async::<_, (bool, bool)>(&mut ctx.redis_mm().await?)
		.await?;
	remove_perf.end();

	// Remove the lobby if needed.
	//
	// This causes a race condition in mm-lobby-seek, but mm-lobby-seek will
	// retry seeking a lobby if the lobby is stopped.
	if should_remove_lobby {
		// Don't stop lobby if already stopped
		if player_row.lobby_stop_ts.is_none() {
			tracing::info!("removing empty lobby");
			msg!([ctx] mm::msg::lobby_stop(player_row.lobby_id) {
				lobby_id: Some(player_row.lobby_id.into()),
			})
			.await?;
		}
	} else if should_update_idle {
		tracing::info!("updating idle lobbies");
		let ctx = ctx.base();
		let namespace_id = player_row.namespace_id;
		let region_id = player_row.region_id;
		tokio::task::Builder::new()
			.name("mm::msg::player_remove::update_idle_lobbies")
			.spawn(
				async move {
					let res = op!([ctx] mm_lobby_idle_update {
						namespace_id: Some(namespace_id.into()),
						region_id: Some(region_id.into()),
					})
					.await;
					match res {
						Ok(_) => {
							tracing::info!("lobby idle updated successfully");
						}
						Err(err) => {
							tracing::error!(?err, "failed to update idle lobbies");
						}
					}
				}
				.instrument(tracing::info_span!("lobby_idle_update")),
			)?;
	}

	// Handle error
	msg!([ctx] mm::msg::player_remove_complete(player_id) {
		player_id: Some(player_id.into()),
		from_lobby_destroy: ctx.from_lobby_destroy,
	})
	.await?;

	Ok(())
}

#[tracing::instrument]
async fn fail(
	client: &chirp_client::Client,
	player_id: Uuid,
	player_row: PlayerRow,
	error_code: mm::msg::player_remove_fail::ErrorCode,
) -> GlobalResult<()> {
	tracing::warn!(?player_id, ?player_row, ?error_code, "player remove failed");

	msg!([client] mm::msg::player_remove_fail(player_id) {
		player_id: Some(player_id.into()),
		error_code: error_code as i32,
	})
	.await?;

	msg!([client] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "mm.player.remove_fail".into(),
				properties_json: Some(serde_json::to_string(&json!({
					"namespace_id": player_row.namespace_id,
					"player_id": player_id,
					"lobby_id": player_row.lobby_id,
					"error": error_code as i32,
				}))?),
				..Default::default()
			}
		],
	})
	.await?;

	Ok(())
}

/// If the player does not exist in Cockraoch, then attempt to fetch it from Redis.
///
/// This happens when the SQL query to insert in to the database fails after the player was already
/// preemptively inserted in to Redis. It's a rare edge case, but has cascading implications for
/// player counts.
#[tracing::instrument(skip(ctx))]
async fn fallback_fetch_player_from_redis(
	ctx: &OperationContext<mm::msg::player_remove::Message>,
	player_id: Uuid,
) -> GlobalResult<Option<PlayerRow>> {
	tracing::warn!("player not found in cockroach, falling back to fetching from redis");

	let mut redis = ctx.redis_mm().await?;

	// Read player config
	let mut player_config_iter = redis
		.hget::<_, _, Vec<Option<String>>>(
			util_mm::key::player_config(player_id),
			&[
				util_mm::key::player_config::LOBBY_ID,
				util_mm::key::player_config::REMOTE_ADDRESS,
			],
		)
		.await?
		.into_iter();

	// If the first value is nil, then there is no player config in Redis
	let Some(lobby_id_str) = unwrap!(player_config_iter.next()) else {
		tracing::warn!(?player_id, "player does not exist in redis");
		return Ok(None);
	};

	let lobby_id = lobby_id_str.parse::<Uuid>()?;
	let remote_address = unwrap!(player_config_iter.next());

	// Read lobby config
	let mut lobby_config_iter = redis
		.hget::<_, _, Vec<Option<String>>>(
			util_mm::key::lobby_config(lobby_id),
			&[
				util_mm::key::lobby_config::NAMESPACE_ID,
				util_mm::key::lobby_config::REGION_ID,
				util_mm::key::lobby_config::LOBBY_GROUP_ID,
				util_mm::key::lobby_config::MAX_PLAYERS_NORMAL,
			],
		)
		.await?
		.into_iter();
	let Some(namespace_id_str) = unwrap!(lobby_config_iter.next()) else {
		tracing::warn!(?lobby_id, "lobby does not exist in redis");
		return Ok(None);
	};
	let namespace_id = namespace_id_str.parse::<Uuid>()?;
	let region_id = unwrap!(unwrap!(lobby_config_iter.next())).parse::<Uuid>()?;
	let lobby_group_id = unwrap!(unwrap!(lobby_config_iter.next())).parse::<Uuid>()?;
	let max_players_normal = unwrap!(unwrap!(lobby_config_iter.next())).parse::<i64>()?;

	// Build row with the information we have
	let row = PlayerRow {
		// Lobby may not exist for this ID if mm-lobby-create didn't succeed
		lobby_id,
		create_ts: None,
		register_ts: None,
		remote_address,
		remove_ts: None,

		// Lobby
		namespace_id,
		region_id,
		lobby_group_id,
		lobby_stop_ts: None,
		max_players_normal,
		max_players_party: max_players_normal,
	};

	Ok(Some(row))
}
