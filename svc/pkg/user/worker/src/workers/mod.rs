mod admin_set;
mod create;
mod delete;
mod event_team_member_remove;
mod event_user_mm_lobby_join;
mod event_user_update;
mod profile_set;
mod search_update;
mod search_update_user_follow_create;
mod search_update_user_update;
mod updated_user_follow_create;
mod updated_user_follow_delete;
mod updated_user_update;

chirp_worker::workers![
	admin_set,
	create,
	delete,
	event_team_member_remove,
	event_user_mm_lobby_join,
	event_user_update,
	profile_set,
	search_update,
	search_update_user_follow_create,
	search_update_user_update,
	updated_user_follow_create,
	updated_user_follow_delete,
	updated_user_update,
];
