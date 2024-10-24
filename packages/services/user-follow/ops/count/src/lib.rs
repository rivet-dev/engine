use proto::backend::pkg::{user_follow::count::request::Kind as RequestKind, *};
use rivet_operation::prelude::*;

#[derive(Debug, sqlx::FromRow)]
struct FollowCount {
	user_id: Uuid,
	count: i64,
}

#[operation(name = "user-follow-count")]
async fn handle(
	ctx: OperationContext<user_follow::count::Request>,
) -> GlobalResult<user_follow::count::Response> {
	let user_ids = ctx
		.user_ids
		.iter()
		.map(common::Uuid::as_uuid)
		.collect::<Vec<_>>();

	let req_kind = unwrap!(RequestKind::from_i32(ctx.kind));

	let follows = match req_kind {
		RequestKind::Mutual => {
			sql_fetch_all!(
				[ctx, FollowCount]
				"
				SELECT follower_user_id as user_id, COUNT(*)
				FROM (
				SELECT
					uf.follower_user_id,
					EXISTS(
						SELECT 1
						FROM db_user_follow.user_follows AS uf2
						WHERE
							uf2.follower_user_id = uf.following_user_id AND
							uf2.following_user_id = uf.follower_user_id
					) AS is_mutual
				FROM UNNEST($1::UUID[]) AS q
				INNER JOIN db_user_follow.user_follows AS uf
				ON uf.follower_user_id = q
				) as f
				WHERE is_mutual
				GROUP BY follower_user_id
				",
				&user_ids,
			)
			.await?
		}
		_ => match req_kind {
			RequestKind::Follower => {
				sql_fetch_all!(
					[ctx, FollowCount]
					"
						SELECT following_user_id as user_id, COUNT(*)
						FROM db_user_follow.user_follows
						WHERE following_user_id = ANY($1)
						GROUP BY following_user_id
						",
					&user_ids,
				)
				.await?
			}
			RequestKind::Following => {
				sql_fetch_all!(
					[ctx, FollowCount]
					"
						SELECT follower_user_id as user_id, COUNT(*)
						FROM db_user_follow.user_follows
						WHERE follower_user_id = ANY($1)
						GROUP BY follower_user_id
						",
					&user_ids,
				)
				.await?
			}
			RequestKind::Mutual => unreachable!(),
		},
	};

	let follows = user_ids
		.iter()
		.cloned()
		.map(|user_id| {
			let count = follows
				.iter()
				.find(|f| f.user_id == user_id)
				.map(|f| f.count)
				.unwrap_or_default();

			user_follow::count::response::Follows {
				user_id: Some(user_id.into()),
				count,
			}
		})
		.collect::<Vec<_>>();

	Ok(user_follow::count::Response { follows })
}
