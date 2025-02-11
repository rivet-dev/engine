use std::collections::HashMap;

use chirp_workflow::prelude::*;
use rivet_operation::prelude::proto;
use proto::backend::{pkg::*};

#[workflow_test]
async fn empty(ctx: TestCtx) {
	let user_a = Uuid::new_v4();
	let user_b = Uuid::new_v4();
	let user_c = Uuid::new_v4();

	let team_a = Uuid::new_v4();
	let team_b = Uuid::new_v4();
	let team_c = Uuid::new_v4();

	let members = vec![
		(user_a, team_a),
		(user_a, team_b),
		(user_b, team_a),
		(user_b, team_b),
		(user_b, team_c),
	];
	for (user_id, team_id) in &members {
		msg!([ctx] team::msg::member_create(team_id, user_id) -> team::msg::member_create_complete {
			team_id: Some((*team_id).into()),
			user_id: Some((*user_id).into()),
			invitation: None,
		})
		.await
		.unwrap();
	}

	let res = ctx.op(::user::ops::team_list::Input {
		user_ids: vec![user_a, user_b, user_c],
	})
	.await
	.unwrap();

	assert_eq!(3, res.users.len());
	let users_map = res
		.users
		.iter()
		.map(|u| (u.user_id, u.teams.len()))
		.collect::<HashMap<Uuid, usize>>();
	assert_eq!(2, *users_map.get(&user_a).unwrap());
	assert_eq!(3, *users_map.get(&user_b).unwrap());
	assert_eq!(0, *users_map.get(&user_c).unwrap());
}
