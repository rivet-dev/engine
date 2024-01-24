use std::convert::TryInto;

use chirp_worker::prelude::*;
use proto::backend::pkg::*;
use serde_json::json;

#[worker(name = "module-version-create")]
async fn worker(
	ctx: &OperationContext<module::msg::version_create::Message>,
) -> Result<(), GlobalError> {
	let crdb = ctx.crdb().await?;

	let version_id = unwrap_ref!(ctx.version_id).as_uuid();

	rivet_pools::utils::crdb::tx(&crdb, |tx| Box::pin(update_db(ctx.clone(), tx, ctx.ts())))
		.await?;

	msg!([ctx] module::msg::version_create_complete(version_id) {
		version_id: ctx.version_id,
	})
	.await?;

	msg!([ctx] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "module.create".into(),
				properties_json: Some(serde_json::to_string(&json!({
					"user_id": ctx.creator_user_id.map(|x| x.as_uuid()),
					"module_id": unwrap_ref!(ctx.module_id).as_uuid(),
					"module_version_id": unwrap_ref!(ctx.version_id).as_uuid(),
				}))?),
				..Default::default()
			},
		],
	})
	.await?;

	Ok(())
}

#[tracing::instrument(skip_all)]
async fn update_db(
	ctx: OperationContext<module::msg::version_create::Message>,
	tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
	now: i64,
) -> GlobalResult<()> {
	let version_id = unwrap_ref!(ctx.version_id).as_uuid();
	let module_id = unwrap_ref!(ctx.module_id).as_uuid();

	sql_execute!(
		[ctx, @tx tx]
		"
		INSERT INTO db_module.versions (version_id, module_id, create_ts, creator_user_id, major, minor, patch)
		VALUES ($1, $2, $3, $4, $5, $6, $7)
		",
		version_id,
		module_id,
		now,
		ctx.creator_user_id.map(|x| x.as_uuid()),
		TryInto::<i64>::try_into(ctx.major)?,
		TryInto::<i64>::try_into(ctx.minor)?,
		TryInto::<i64>::try_into(ctx.patch)?,
	)
	.await?;

	match unwrap_ref!(ctx.image) {
		module::msg::version_create::message::Image::Docker(docker) => {
			sql_execute!(
				[ctx, @tx tx]
				"
                INSERT INTO db_module.versions_image_docker (version_id, image_tag)
                VALUES ($1, $2)
                ",
				version_id,
				&docker.image_tag,
			)
			.await?;
		}
	}

	for script in &ctx.scripts {
		sql_execute!(
			[ctx, @tx tx]
			"
            INSERT INTO db_module.scripts (version_id, name, request_schema, response_schema)
            VALUES ($1, $2, $3, $4)
            ",
			version_id,
			&script.name,
			&script.request_schema,
			&script.response_schema,
		)
		.await?;

		if script.callable.is_some() {
			sql_execute!(
				[ctx, @tx tx]
				"
                INSERT INTO db_module.scripts_callable (version_id, name)
                VALUES ($1, $2)
            ",
				version_id,
				&script.name,
			)
			.await?;
		}
	}

	Ok(())
}
