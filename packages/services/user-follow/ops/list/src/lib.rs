use proto::backend::pkg::{user_follow::list::request::Kind as RequestKind, *};
use rivet_operation::prelude::*;

#[derive(Debug, sqlx::FromRow)]
struct Follow {
	follower_user_id: Uuid,
	following_user_id: Uuid,
	create_ts: i64,
	is_mutual: bool,
}

impl Follow {
	fn group_user_id(&self, kind: &RequestKind) -> Uuid {
		match kind {
			RequestKind::Follower => self.follower_user_id,
			RequestKind::Mutual | RequestKind::Following => self.following_user_id,
		}
	}

	fn entry_user_id(&self, kind: &RequestKind) -> Uuid {
		match kind {
			RequestKind::Follower => self.following_user_id,
			RequestKind::Mutual | RequestKind::Following => self.follower_user_id,
		}
	}
}

#[operation(name = "user-follow-list")]
async fn handle(
	ctx: OperationContext<user_follow::list::Request>,
) -> GlobalResult<user_follow::list::Response> {
	let user_ids = ctx
		.user_ids
		.iter()
		.map(|id| id.as_uuid())
		.collect::<Vec<_>>();

	let limit = ctx.limit;

	ensure!(limit != 0, "limit too low");
	ensure!(limit <= 32, "limit too high");

	let req_kind = unwrap!(RequestKind::from_i32(ctx.kind));

	let follows = match req_kind {
		RequestKind::Mutual => {
			sql_fetch_all!(
				[ctx, Follow]
				"
				SELECT follower_user_id, following_user_id, create_ts, is_mutual
				FROM (
					SELECT
						uf.follower_user_id, uf.following_user_id, uf.create_ts,
						EXISTS(
							SELECT 1
							FROM db_user_follow.user_follows AS uf2
							WHERE
								uf2.follower_user_id = uf.following_user_id AND
								uf2.following_user_id = uf.follower_user_id
						) AS is_mutual
					FROM UNNEST($1::UUID[]) AS q
					INNER JOIN db_user_follow.user_follows AS uf
					ON uf.following_user_id = q
				)
				WHERE is_mutual AND create_ts > $2
				ORDER BY create_ts DESC
				LIMIT $3
				",
				&user_ids,
				ctx.anchor.unwrap_or_default(),
				limit as i64,
			)
			.await?
		}
		RequestKind::Follower => {
			sql_fetch_all!(
				[ctx, Follow]
				"
				SELECT follower_user_id, following_user_id, create_ts, is_mutual
				FROM (
					SELECT
						uf.follower_user_id, uf.following_user_id, uf.create_ts,
						exists(
							SELECT 1
							FROM db_user_follow.user_follows AS uf2
							WHERE
								uf2.follower_user_id = uf.following_user_id AND
								uf2.following_user_id = uf.follower_user_id
						) AS is_mutual
					FROM unnest($1::UUID[]) AS q
					INNER JOIN db_user_follow.user_follows AS uf
					ON uf.follower_user_id = q
				)
				WHERE create_ts > $2
				ORDER BY is_mutual DESC, create_ts DESC
				LIMIT $3
				",
				&user_ids,
				ctx.anchor.unwrap_or_default(),
				limit as i64,
			)
			.await?
		}
		RequestKind::Following => {
			sql_fetch_all!(
				[ctx, Follow]
				"
				SELECT follower_user_id, following_user_id, create_ts, is_mutual
				FROM (
					SELECT
						uf.follower_user_id, uf.following_user_id, uf.create_ts,
						exists(
							SELECT 1
							FROM db_user_follow.user_follows AS uf2
							WHERE
								uf2.follower_user_id = uf.following_user_id AND
								uf2.following_user_id = uf.follower_user_id
						) AS is_mutual
					FROM unnest($1::UUID[]) AS q
					INNER JOIN db_user_follow.user_follows AS uf
					ON uf.following_user_id = q
				)
				WHERE create_ts > $2
				ORDER BY is_mutual DESC, create_ts DESC
				LIMIT $3
				",
				&user_ids,
				ctx.anchor.unwrap_or_default(),
				limit as i64,
			)
			.await?
		}
	};

	let follows = user_ids
		.iter()
		.cloned()
		.map(|user_id| {
			let follows = follows
				.iter()
				.filter(|f| f.group_user_id(&req_kind) == user_id)
				.map(|follow| user_follow::list::response::Follow {
					user_id: Some(follow.entry_user_id(&req_kind).into()),
					create_ts: follow.create_ts,
					is_mutual: follow.is_mutual,
				})
				.collect::<Vec<_>>();

			let anchor = follows
				.last()
				.and_then(|follow| (follows.len() >= limit as usize).then_some(follow.create_ts));

			user_follow::list::response::Follows {
				user_id: Some(user_id.into()),
				follows,
				anchor,
			}
		})
		.collect::<Vec<_>>();

	Ok(user_follow::list::Response { follows })
}
