use proto::backend::pkg::*;
use rivet_operation::prelude::*;

#[operation(name = "module-game-version-publish")]
async fn handle(
	ctx: OperationContext<module::game_version_publish::Request>,
) -> GlobalResult<module::game_version_publish::Response> {
	let _crdb = ctx.crdb().await?;

	let version_id = unwrap_ref!(ctx.version_id).as_uuid();
	let config = unwrap_ref!(ctx.config);

	let mut config_buf = Vec::with_capacity(config.encoded_len());
	config.encode(&mut config_buf)?;

	sql_execute!(
		[ctx]
		"INSERT INTO db_module.game_versions (version_id, config) VALUES ($1, $2)",
		version_id,
		config_buf,
	)
	.await?;

	Ok(module::game_version_publish::Response {})
}
