use chirp_worker::prelude::*;

#[worker_test]
async fn empty(ctx: TestCtx) {
	let namespace_id = Uuid::new_v4();
	let auth_user = util::faker::ident();
	let (_, auth_password) = util::faker::bcrypt();

	op!([ctx] cdn_namespace_create {
		namespace_id: Some(namespace_id.into()),
	})
	.await
	.unwrap();

	op!([ctx] cdn_namespace_auth_user_update {
		namespace_id: Some(namespace_id.into()),
		user: auth_user.clone(),
		password: auth_password.clone(),
	})
	.await
	.unwrap();

	op!([ctx] cdn_namespace_auth_user_remove {
		namespace_id: Some(namespace_id.into()),
		user: auth_user.clone(),
	})
	.await
	.unwrap();

	let (sql_exists,) = sql_fetch_one!(
		[ctx, (bool,)]
		"
		SELECT EXISTS (
			SELECT 1
			FROM db_cdn.game_namespace_auth_users
			WHERE
				namespace_id = $1 AND
				user_name = $2
		)
		",
		namespace_id,
		auth_user,
	)
	.await
	.unwrap();
	assert!(!sql_exists);
}
