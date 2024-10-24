use std::collections::HashMap;

use chirp_worker::prelude::*;
use proto::backend::pkg::*;

#[worker_test]
async fn empty(ctx: TestCtx) {
	let user_a = Uuid::new_v4();
	let user_b = Uuid::new_v4();
	let user_c = Uuid::new_v4();

	let rows = vec![
		(user_a, user_b),
		(user_a, user_c),
		(user_b, user_c),
		(user_c, user_a),
		(user_c, user_b),
	];
	for (follower, following) in &rows {
		op!([ctx] user_follow_toggle {
			follower_user_id: Some((*follower).into()),
			following_user_id: Some((*following).into()),
			active: true,
		})
		.await
		.unwrap();
	}

	let res = op!([ctx] user_follow_list {
		kind: user_follow::list::request::Kind::Follower as i32,
		user_ids: vec![user_a.into(), user_b.into(), user_c.into()],
		limit: 10,
		anchor: None,
	})
	.await
	.unwrap();

	// Create follower map for easy lookup
	let following_map = rows.iter().fold(
		HashMap::<Uuid, Vec<Uuid>>::new(),
		|mut acc, (user_id_a, user_id_b)| {
			acc.entry(*user_id_a).or_default().push(*user_id_b);
			acc
		},
	);

	// Validate the follows match
	assert_eq!(following_map.len(), res.follows.len());
	for follows in &res.follows {
		let follower_user_id = follows.user_id.unwrap().as_uuid();
		let following_list = following_map
			.get(&follower_user_id)
			.expect("invalid follower id");

		assert_eq!(following_list.len(), follows.follows.len());

		for follow in &follows.follows {
			let following_user_id = follow.user_id.unwrap().as_uuid();
			assert!(
				following_list.contains(&following_user_id),
				"missing following"
			);
		}
	}
}

#[worker_test]
async fn mutual(ctx: TestCtx) {
	let user_a = Uuid::new_v4();
	let user_b = Uuid::new_v4();
	let user_c = Uuid::new_v4();

	let rows = vec![
		(user_a, user_b),
		(user_a, user_c),
		(user_b, user_c),
		(user_c, user_a),
		(user_c, user_b),
	];
	for (follower, following) in &rows {
		op!([ctx] user_follow_toggle {
			follower_user_id: Some((*follower).into()),
			following_user_id: Some((*following).into()),
			active: true,
		})
		.await
		.unwrap();
	}

	let res = op!([ctx] user_follow_list {
		kind: user_follow::list::request::Kind::Mutual as i32,
		user_ids: vec![user_a.into(), user_b.into(), user_c.into()],
		limit: 10,
		anchor: None,
	})
	.await
	.unwrap();

	// Create follower map for easy lookup
	let following_map = rows.iter().fold(
		HashMap::<Uuid, Vec<Uuid>>::new(),
		|mut acc, (user_id_a, user_id_b)| {
			acc.entry(*user_id_a).or_default().push(*user_id_b);
			acc
		},
	);

	// Validate the follows match
	assert_eq!(following_map.len(), res.follows.len());
	for follows in &res.follows {
		let follower_user_id = follows.user_id.unwrap().as_uuid();
		let following_list = following_map
			.get(&follower_user_id)
			.expect("invalid follower id");

		for follow in &follows.follows {
			let following_user_id = follow.user_id.unwrap().as_uuid();

			assert!(follow.is_mutual);
			assert!(
				following_list.contains(&following_user_id),
				"missing mutual"
			);
		}
	}
}

// TODO: Pagination tests
