use proto::backend::{self, pkg::*};
use rivet_operation::prelude::*;

#[derive(sqlx::FromRow)]
struct Game {
	game_id: Uuid,
	create_ts: i64,
	name_id: String,
	display_name: String,
	url: String,
	developer_team_id: Uuid,
	description: String,
	tags: Vec<String>,
	logo_upload_id: Option<Uuid>,
	banner_upload_id: Option<Uuid>,
}

impl From<Game> for game::get::CacheGame {
	fn from(val: Game) -> Self {
		game::get::CacheGame {
			game_id: Some(val.game_id.into()),
			create_ts: val.create_ts,
			name_id: val.name_id,
			display_name: val.display_name,
			url: val.url,
			developer_team_id: Some(val.developer_team_id.into()),
			description: val.description,
			tags: val.tags,
			logo_upload_id: val.logo_upload_id.map(Into::into),
			banner_upload_id: val.banner_upload_id.map(Into::into),
		}
	}
}

#[operation(name = "game-get")]
async fn handle(ctx: OperationContext<game::get::Request>) -> GlobalResult<game::get::Response> {
	let game_ids = ctx
		.game_ids
		.iter()
		.map(|id| id.as_uuid())
		.collect::<Vec<_>>();

	let games = ctx
		.cache()
		.fetch_all_proto("game", game_ids, {
			let ctx = ctx.clone();
			move |mut cache, game_ids| {
				let ctx = ctx.clone();
				async move {
					let games = sql_fetch_all!(
						[ctx, Game]
						"
						SELECT
							game_id,
							create_ts,
							name_id,
							display_name,
							url,
							developer_team_id,
							description,
							array(
								SELECT tag
								FROM db_game.game_tags
								WHERE game_tags.game_id = games.game_id
							) AS tags,
							logo_upload_id,
							banner_upload_id
						FROM db_game.games
						WHERE game_id = ANY($1)
						",
						game_ids,
					)
					.await?;

					for row in games {
						cache.resolve(&row.game_id.clone(), game::get::CacheGame::from(row));
					}

					Ok(cache)
				}
			}
		})
		.await?;

	let upload_ids = games
		.iter()
		.flat_map(|game| [game.logo_upload_id, game.banner_upload_id])
		.collect::<Vec<_>>()
		.into_iter()
		.flatten()
		.map(Into::into)
		.collect::<Vec<_>>();

	let upload_res = op!([ctx] upload_get {
		upload_ids: upload_ids.clone(),
	})
	.await?;

	let files_res = op!([ctx] upload_file_list {
		upload_ids: upload_ids.clone(),
	})
	.await?;

	Ok(game::get::Response {
		games: games
			.into_iter()
			.map(|game| {
				let logo_upload_id = game.logo_upload_id.map(Into::<common::Uuid>::into);
				let banner_upload_id = game.banner_upload_id.map(Into::<common::Uuid>::into);

				// Fetch all information relating to the logo image
				let (logo_upload_complete_ts, logo_file_name, logo_provider) = {
					let upload = upload_res
						.uploads
						.iter()
						.find(|upload| upload.upload_id == logo_upload_id);
					let file = files_res
						.files
						.iter()
						.find(|file| file.upload_id == logo_upload_id);

					if let (Some(upload), Some(file)) = (upload, file) {
						let logo_file_name = file
							.path
							.rsplit_once('/')
							.map(|(_, file_name)| file_name.to_owned())
							.or(Some(file.path.clone()));
						(upload.complete_ts, logo_file_name, Some(upload.provider))
					} else {
						Default::default()
					}
				};

				// Fetch all information relating to the banner image
				let (banner_upload_complete_ts, banner_file_name, banner_provider) = {
					let upload = upload_res
						.uploads
						.iter()
						.find(|upload| upload.upload_id == banner_upload_id);
					let file = files_res
						.files
						.iter()
						.find(|file| file.upload_id == banner_upload_id);

					if let (Some(upload), Some(file)) = (upload, file) {
						let banner_file_name = file
							.path
							.rsplit_once('/')
							.map(|(_, file_name)| file_name.to_owned())
							.or(Some(file.path.clone()));
						(upload.complete_ts, banner_file_name, Some(upload.provider))
					} else {
						Default::default()
					}
				};

				backend::game::Game {
					game_id: game.game_id,
					create_ts: game.create_ts,
					name_id: game.name_id,
					display_name: game.display_name,
					url: game.url,
					developer_team_id: game.developer_team_id,
					description: game.description,
					tags: game.tags,

					logo_upload_id: if logo_upload_complete_ts.is_some() {
						logo_upload_id
					} else {
						None
					},
					logo_file_name,
					logo_provider,
					banner_upload_id: if banner_upload_complete_ts.is_some() {
						banner_upload_id
					} else {
						None
					},
					banner_file_name,
					banner_provider,
				}
			})
			.collect::<Vec<_>>(),
	})
}
