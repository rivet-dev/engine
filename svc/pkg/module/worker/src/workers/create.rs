use chirp_worker::prelude::*;
use proto::backend::pkg::*;
use serde_json::json;

#[worker(name = "module-create")]
async fn worker(ctx: &OperationContext<module::msg::create::Message>) -> Result<(), GlobalError> {
	let _crdb = ctx.crdb().await?;

	let module_id = unwrap_ref!(ctx.module_id).as_uuid();
	let team_id = unwrap_ref!(ctx.team_id).as_uuid();
	let creator_user_id = ctx.creator_user_id.map(|x| x.as_uuid());

	sql_execute!(
		[ctx]
		"
		INSERT INTO db_module.modules (module_id, name_id, team_id, create_ts, creator_user_id)
		VALUES ($1, $2, $3, $4, $5)
		",
		module_id,
		&ctx.name_id,
		team_id,
		ctx.ts(),
		creator_user_id,
	)
	.await?;

	msg!([ctx] module::msg::create_complete(module_id) {
		module_id: ctx.module_id,
	})
	.await?;

	msg!([ctx] analytics::msg::event_create() {
		events: vec![
			analytics::msg::event_create::Event {
				event_id: Some(Uuid::new_v4().into()),
				name: "module.create".into(),
				properties_json: Some(serde_json::to_string(&json!({
					"user_id": ctx.creator_user_id.map(|x| x.as_uuid()),
					"module_id": module_id,
					"name_id": ctx.name_id,
				}))?),
				..Default::default()
			},
		],
	})
	.await?;

	Ok(())
}
