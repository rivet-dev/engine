use proto::backend::pkg::*;
use rivet_operation::prelude::*;

const MAX_UPLOAD_SIZE: u64 = util::file_size::gigabytes(1);

#[operation(name = "cdn-site-create")]
async fn handle(
	ctx: OperationContext<cdn::site_create::Request>,
) -> GlobalResult<cdn::site_create::Response> {
	let game_id = unwrap_ref!(ctx.game_id).as_uuid();
	ensure!(
		util::check::display_name_long(&ctx.display_name),
		"invalid display name"
	);
	ensure_with!(
		ctx.files
			.iter()
			.fold(0, |acc, file| acc + file.content_length)
			< MAX_UPLOAD_SIZE,
		UPLOAD_TOO_LARGE
	);

	// Validate game exists
	let game_res = op!([ctx] game_get {
		game_ids: vec![game_id.into()],
	})
	.await?;
	let game = game_res.games.first();
	let _game = unwrap_ref!(game, "game not found");

	// Create the upload. Don't log since there might be a lot of files in this
	// upload.
	let upload_prepare_res = op!([ctx] @dont_log_body upload_prepare {
		bucket: "bucket-cdn".into(),
		files: ctx.files.clone(),
	})
	.await?;
	let upload_id = unwrap_ref!(upload_prepare_res.upload_id).as_uuid();

	// Create site
	let site_id = Uuid::new_v4();
	sql_execute!(
		[ctx]
		"
		INSERT INTO db_cdn.sites (site_id, game_id, upload_id, display_name, create_ts)
		VALUES ($1, $2, $3, $4, $5)
		",
		site_id,
		game_id,
		upload_id,
		&ctx.display_name,
		ctx.ts(),
	)
	.await?;

	Ok(cdn::site_create::Response {
		site_id: Some(site_id.into()),
		upload_id: Some(upload_id.into()),
		presigned_requests: upload_prepare_res.presigned_requests.clone(),
	})
}
