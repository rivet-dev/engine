use proto::backend::{self, pkg::*};
use rivet_operation::prelude::*;

const MAX_UPLOAD_SIZE: u64 = util::file_size::gigabytes(8);

#[operation(name = "build-create")]
async fn handle(
	ctx: OperationContext<build::create::Request>,
) -> GlobalResult<build::create::Response> {
	let kind = unwrap!(backend::build::BuildKind::from_i32(ctx.kind));
	let compression = unwrap!(backend::build::BuildCompression::from_i32(ctx.compression));

	let game_id = ctx.game_id.map(|x| x.as_uuid());
	let env_id = ctx.env_id.map(|x| x.as_uuid());
	ensure!(
		util::check::display_name_long(&ctx.display_name),
		"invalid display name"
	);

	// Validate game exists
	if let Some(game_id) = game_id {
		let game_res = op!([ctx] game_get {
			game_ids: vec![game_id.into()],
		})
		.await?;
		let game = game_res.games.first();
		ensure!(game.is_some(), "game not found");
	}
	if let Some(env_id) = env_id {
		let env_res = op!([ctx] game_namespace_get {
			namespace_ids: vec![env_id.into()],
		})
		.await?;
		let env = env_res.namespaces.first();
		ensure!(env.is_some(), "game not found");
	}

	let (image_tag, upload_id, image_presigned_requests) =
		if let Some(build_kind) = &ctx.default_build_kind {
			let default_build_row = sql_fetch_optional!(
				[ctx, (String, Uuid)]
				"SELECT image_tag, upload_id FROM db_build.default_builds WHERE kind = $1",
				build_kind,
			)
			.await?;

			let (image_tag, upload_id) =
				unwrap!(default_build_row, "default build missing: {build_kind}",);

			(image_tag, upload_id, Vec::new())
		} else {
			let image_file = unwrap_ref!(ctx.image_file);
			let image_tag = unwrap_ref!(ctx.image_tag);

			let tag_split = image_tag.split_once(':');
			let (tag_base, tag_tag) = unwrap_ref!(tag_split, "missing separator in image tag");
			ensure!(
				util::check::docker_ident(tag_base),
				"invalid image tag base"
			);
			ensure!(util::check::docker_ident(tag_tag), "invalid image tag tag");

			ensure_with!(
				image_file.content_length < MAX_UPLOAD_SIZE,
				UPLOAD_TOO_LARGE
			);

			// Check if build is unique
			let (build_exists,) = sql_fetch_one!(
				[ctx, (bool,)]
				"SELECT EXISTS (SELECT 1 FROM db_build.builds WHERE image_tag = $1)",
				image_tag,
			)
			.await?;
			if build_exists {
				tracing::info!(?image_tag, "build image is not unique");
				bail!("build image tag not unique");
			} else {
				tracing::info!(?image_tag, "build image is unique");
			}

			// Create the upload
			let file_name = util_build::file_name(kind, compression);
			let upload_prepare_res = op!([ctx] upload_prepare {
				bucket: "bucket-build".into(),
				files: vec![
					backend::upload::PrepareFile {
						path: file_name,
						content_length: image_file.content_length,
						..Default::default()
					},
				],
			})
			.await?;
			let upload_id = **unwrap_ref!(upload_prepare_res.upload_id);

			(
				image_tag.clone(),
				upload_id,
				upload_prepare_res.presigned_requests.clone(),
			)
		};

	// Create build
	let build_id = Uuid::new_v4();
	sql_execute!(
		[ctx]
		"
		INSERT INTO
			db_build.builds (
				build_id,
				game_id,
				env_id,
				upload_id,
				display_name,
				image_tag,
				create_ts,
				kind,
				compression
			)
		VALUES
			($1, $2, $3, $4, $5, $6, $7, $8, $9)
		",
		build_id,
		game_id,
		env_id,
		upload_id,
		&ctx.display_name,
		image_tag,
		ctx.ts(),
		kind as i32,
		compression as i32,
	)
	.await?;

	Ok(build::create::Response {
		build_id: Some(build_id.into()),
		upload_id: Some(upload_id.into()),
		image_presigned_requests,
	})
}
