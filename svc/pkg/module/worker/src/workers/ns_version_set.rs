use chirp_worker::prelude::*;
use proto::backend::pkg::*;
use std::collections::HashSet;

// A note on gradual deploys:
//
// We reuse the same Fly app for each unique namespace ID & key combination
// in order ot ensure version changes are made safely.
//
// We don't use namespace ID & module IDs because there might be multiple of the same module per namespace.
//
// We don't use namespace ID & version IDs because we want a gradual deploy when changing the version.

#[derive(Debug, sqlx::FromRow)]
struct NamespaceInstances {
	key: String,
	instance_id: Uuid,
}

#[worker(name = "module-ns-version-set")]
async fn worker(
	ctx: &OperationContext<game::msg::ns_version_set_complete::Message>,
) -> Result<(), GlobalError> {
	let _crdb = ctx.crdb().await?;

	let namespace_id = unwrap_ref!(ctx.namespace_id).as_uuid();
	let version_id = unwrap_ref!(ctx.version_id).as_uuid();

	// TODO: Transaction

	// Get version config
	let game_versions = op!([ctx] module_game_version_get {
		version_ids: vec![version_id.into()],
	})
	.await?;
	let Some(game_version) = game_versions
		.versions
		.first()
		.and_then(|x| x.config.as_ref())
	else {
		tracing::info!("no game version found");
		return Ok(());
	};

	let new_version_keys = game_version
		.dependencies
		.iter()
		.map(|x| x.key.as_str())
		.collect::<HashSet<&str>>();

	// Find all existing instances for ns
	let existing_instances = sql_fetch_all!(
		[ctx, NamespaceInstances]
		"
		SELECT key, instance_id
		FROM db_module.namespace_instances
		WHERE namespace_id = $1
		",
		namespace_id,
	)
	.await?;
	let current_version_keys = existing_instances
		.iter()
		.map(|x| x.key.as_str())
		.collect::<HashSet<&str>>();

	// New instances
	let new_dep_keys = new_version_keys
		.difference(&current_version_keys)
		.collect::<Vec<_>>();
	for dep_key in &new_dep_keys {
		let version_id = game_version
			.dependencies
			.iter()
			.find(|x| x.key == **dep_key)
			.and_then(|x| x.module_version_id)
			.map(|x| x.as_uuid());
		create_instances(ctx, namespace_id, dep_key, unwrap!(version_id)).await?;
	}

	// Update instances
	let update_dep_keys = new_version_keys
		.intersection(&current_version_keys)
		.collect::<Vec<_>>();
	for dep_key in &update_dep_keys {
		let instance_id = existing_instances
			.iter()
			.find(|x| x.key == **dep_key)
			.map(|x| x.instance_id);
		let version_id = game_version
			.dependencies
			.iter()
			.find(|x| x.key == **dep_key)
			.and_then(|x| x.module_version_id)
			.map(|x| x.as_uuid());

		update_instance(ctx.chirp(), unwrap!(instance_id), unwrap!(version_id)).await?;
	}

	// Delete instances
	let delete_dep_keys = current_version_keys
		.difference(&new_version_keys)
		.collect::<Vec<_>>();
	for dep_key in &delete_dep_keys {
		let instance_id = existing_instances
			.iter()
			.find(|x| x.key == **dep_key)
			.map(|x| x.instance_id);

		delete_instance(ctx, namespace_id, dep_key, unwrap!(instance_id)).await?;
	}

	msg!([ctx] module::msg::ns_version_set_complete(namespace_id) {
		namespace_id: Some(namespace_id.into()),
		version_id: Some(version_id.into()),
	})
	.await?;

	Ok(())
}

async fn create_instances(
	ctx: &OperationContext<game::msg::ns_version_set_complete::Message>,
	namespace_id: Uuid,
	dep_key: &str,
	version_id: Uuid,
) -> Result<(), GlobalError> {
	// Create instance
	let instance_id = Uuid::new_v4();
	msg!([ctx] module::msg::instance_create(instance_id) -> module::msg::instance_create_complete {
		instance_id: Some(instance_id.into()),
		module_version_id: Some(version_id.into()),
		driver: Some(module::msg::instance_create::message::Driver::Fly(module::msg::instance_create::message::Fly {})),
	})
	.await
	.unwrap();

	// Insert instance
	sql_execute!(
		[ctx]
		"
		INSERT INTO db_module.namespace_instances (namespace_id, key, instance_id)
		VALUES ($1, $2, $3)
		",
		namespace_id,
		dep_key,
		instance_id,
	)
	.await?;

	Ok(())
}

async fn update_instance(
	client: &chirp_client::Client,
	instance_id: Uuid,
	version_id: Uuid,
) -> Result<(), GlobalError> {
	// Update instance
	msg!([client] module::msg::instance_version_set(instance_id) -> module::msg::instance_version_set_complete {
		instance_id: Some(instance_id.into()),
		version_id: Some(version_id.into()),
	})
	.await
	.unwrap();

	Ok(())
}

async fn delete_instance(
	ctx: &OperationContext<game::msg::ns_version_set_complete::Message>,
	namespace_id: Uuid,
	dep_key: &str,
	instance_id: Uuid,
) -> Result<(), GlobalError> {
	// Delete instance
	msg!([ctx] module::msg::instance_destroy(instance_id) -> module::msg::instance_destroy_complete {
		instance_id: Some(instance_id.into()),
	})
	.await
	.unwrap();

	// Remove instance
	sql_execute!(
		[ctx]
		"
		DELETE FROM namespace_instances
		WHERE namespace_id = $1 AND key = $2 AND instance_id = $3
		",
		namespace_id,
		dep_key,
		instance_id,
	)
	.await?;

	Ok(())
}
