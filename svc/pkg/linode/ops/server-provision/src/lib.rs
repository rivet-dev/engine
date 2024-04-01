use proto::backend::{self, cluster::PoolType, pkg::*};
use rivet_operation::prelude::*;
use util_linode::api;

#[operation(name = "linode-server-provision", timeout = 150)]
pub async fn handle(
	ctx: OperationContext<linode::server_provision::Request>,
) -> GlobalResult<linode::server_provision::Response> {
	let crdb = ctx.crdb().await?;
	let server_id = unwrap_ref!(ctx.server_id).as_uuid();
	let provider_datacenter_id = ctx.provider_datacenter_id.clone();
	let pool_type = unwrap!(PoolType::from_i32(ctx.pool_type));
	let provider_hardware = unwrap_ref!(ctx.hardware).provider_hardware.clone();

	let ns = util::env::namespace();
	let pool_type_str = match pool_type {
		PoolType::Job => "job",
		PoolType::Gg => "gg",
		PoolType::Ats => "ats",
	};
	// Linode label must be 3-64 characters, UUID's are 36
	let name = format!("{ns}-{server_id}");

	let tags = ctx
		.tags
		.iter()
		.cloned()
		.chain([
			// HACK: Linode requires tags to be > 3 characters. We extend the namespace to make sure it
			// meets the minimum length requirement.
			format!("rivet-{ns}"),
			format!("{ns}-{provider_datacenter_id}"),
			format!("{ns}-{pool_type_str}"),
			format!("{ns}-{provider_datacenter_id}-{pool_type_str}"),
		])
		.collect::<Vec<_>>();

	let firewall_inbound = match pool_type {
		PoolType::Job => util::net::job::firewall(),
		PoolType::Gg => util::net::gg::firewall(),
		PoolType::Ats => util::net::ats::firewall(),
	};

	// Build context
	let server = api::ProvisionCtx {
		datacenter: provider_datacenter_id,
		name,
		hardware: provider_hardware,
		vlan_ip: Some(ctx.vlan_ip.clone()),
		tags,
		firewall_inbound,
	};

	// Build HTTP client
	let client = util_linode::Client::new().await?;

	// Create SSH key
	let ssh_key_res = api::create_ssh_key(&client, &server_id.to_string()).await?;

	// Write SSH key id
	sql_execute!(
		[ctx, &crdb]
		"
		INSERT INTO db_cluster.linode_misc (
			server_id,
			ssh_key_id
		)
		VALUES ($1, $2)
		",
		server_id,
		ssh_key_res.id as i64,
	)
	.await?;

	let create_instance_res =
		api::create_instance(&client, &server, &ssh_key_res.public_key).await?;
	let linode_id = create_instance_res.id;

	// Write linode id
	sql_execute!(
		[ctx, &crdb]
		"
		UPDATE db_cluster.linode_misc
		SET linode_id = $2
		WHERE server_id = $1
		",
		server_id,
		linode_id as i64,
	)
	.await?;

	api::wait_instance_ready(&client, linode_id).await?;

	let (create_disks_res, used_custom_image) = create_disks(
		&ctx,
		&crdb,
		&client,
		&server,
		pool_type,
		&ssh_key_res.public_key,
		linode_id,
		create_instance_res.specs.disk,
	)
	.await?;

	api::create_instance_config(&client, &server, linode_id, &create_disks_res).await?;

	let firewall_res = api::create_firewall(&client, &server, linode_id).await?;

	// Write firewall id
	sql_execute!(
		[ctx, &crdb]
		"
		UPDATE db_cluster.linode_misc
		SET firewall_id = $2
		WHERE server_id = $1
		",
		server_id,
		firewall_res.id as i64,
	)
	.await?;

	api::boot_instance(&client, linode_id).await?;

	let public_ip = api::get_public_ip(&client, linode_id).await?;

	Ok(linode::server_provision::Response {
		provider_server_id: linode_id.to_string(),
		public_ip: public_ip.to_string(),
		already_installed: used_custom_image,
	})
}

async fn create_disks(
	ctx: &OperationContext<linode::server_provision::Request>,
	crdb: &CrdbPool,
	client: &util_linode::Client,
	server: &api::ProvisionCtx,
	pool_type: PoolType,
	ssh_key: &str,
	linode_id: u64,
	server_disk_size: u64,
) -> GlobalResult<(api::CreateDisksResponse, bool)> {
	// Try to get custom image (if exists)
	let image_variant = util_cluster::image_variant(
		backend::cluster::Provider::Linode,
		&server.datacenter,
		pool_type,
	);
	let (custom_image, updated) = get_custom_image(ctx, crdb, &image_variant).await?;

	// Default image
	let used_custom_image = custom_image.is_some();
	let image = if let Some(custom_image) = custom_image {
		tracing::info!("using custom image {}", custom_image);

		custom_image
	} else {
		tracing::info!("custom image not ready yet, continuing normally");

		"linode/debian11".to_string()
	};

	// Start custom image creation process
	if updated {
		msg!([ctx] linode::msg::prebake_provision(&image_variant) {
			variant: image_variant,
			provider_datacenter_id: server.datacenter.clone(),
			pool_type: pool_type as i32,
			tags: Vec::new(),
		})
		.await?;
	}

	let create_disks_res =
		api::create_disks(client, ssh_key, linode_id, &image, server_disk_size).await?;

	Ok((create_disks_res, used_custom_image))
}

async fn get_custom_image(
	ctx: &OperationContext<linode::server_provision::Request>,
	crdb: &CrdbPool,
	variant: &str,
) -> GlobalResult<(Option<String>, bool)> {
	// Get the custom image id for this server, or insert a record and start creating one
	let (image_id, updated) = sql_fetch_one!(
		[ctx, (Option<String>, bool), &crdb]
		"
		WITH
			updated AS (
				INSERT INTO db_cluster.server_images AS s (
					variant, create_ts
				)
				VALUES ($1, $2)
				ON CONFLICT (variant) DO UPDATE
					SET
						image_id = NULL,
						create_ts = $2
					WHERE s.create_ts < $3
				RETURNING variant
			),
			selected AS (
				SELECT variant, image_id
				FROM db_cluster.server_images
				WHERE variant = $1
			)
		SELECT
			selected.image_id AS image_id,
			(updated.variant IS NOT NULL) AS updated
		FROM selected
		FULL OUTER JOIN updated
		ON selected.variant = updated.variant;
		",
		variant,
		util::timestamp::now(),
		// 5 month expiration
		util::timestamp::now() - util::duration::days(5 * 30),
	)
	.await?;

	// Updated is true if this specific sql call either reset (if expired) or inserted the row
	Ok((image_id, updated))
}
