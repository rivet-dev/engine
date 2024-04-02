use chirp_worker::prelude::*;
use proto::backend::{self, cluster::PoolType, pkg::*};
use util_linode::api;

#[worker(name = "linode-prebake-provision")]
async fn worker(
	ctx: &OperationContext<linode::msg::prebake_provision::Message>,
) -> GlobalResult<()> {
	let crdb = ctx.crdb().await?;
	let datacenter_id = unwrap_ref!(ctx.datacenter_id).as_uuid();
	let pool_type = unwrap!(PoolType::from_i32(ctx.pool_type));

	let datacenter_res = op!([ctx] cluster_datacenter_get {
		datacenter_ids: vec![datacenter_id.into()],
	})
	.await?;
	let datacenter = unwrap!(datacenter_res.datacenters.first());

	let ns = util::env::namespace();
	let pool_type_str = match pool_type {
		PoolType::Job => "job",
		PoolType::Gg => "gg",
		PoolType::Ats => "ats",
	};
	let provider_datacenter_id = &ctx.provider_datacenter_id;
	// Prebake server labels just have to be unique, they are ephemeral
	let name = format!("{ns}-{}", Uuid::new_v4());

	let tags = ctx
		.tags
		.iter()
		.cloned()
		.chain([
			"prebake".to_string(),
			format!("rivet-{ns}"),
			format!("{ns}-{provider_datacenter_id}"),
			format!("{ns}-{pool_type_str}"),
			format!("{ns}-{provider_datacenter_id}-{pool_type_str}"),
		])
		.collect::<Vec<_>>();

	// Build context
	let prebake_server = api::ProvisionCtx {
		datacenter: provider_datacenter_id.clone(),
		name,
		hardware: util_linode::consts::PREBAKE_HARDWARE.to_string(),
		vlan_ip: None,
		tags,
		firewall_inbound: vec![util::net::default_firewall()],
	};

	// Build HTTP client
	let api_token = if let Some(api_token) = datacenter.provider_api_token.clone() {
		api_token
	} else {
		util::env::read_secret(&["linode", "token"]).await?
	};
	let client = util_linode::Client::new(&api_token).await?;

	match provision(ctx, &crdb, &client, datacenter_id, &prebake_server).await {
		Ok(public_ip) => {
			// Continue to install
			msg!([ctx] cluster::msg::server_install(&public_ip) {
				public_ip: public_ip,
				pool_type: ctx.pool_type,
				server_id: None,
				datacenter_id: ctx.datacenter_id,
				provider: backend::cluster::Provider::Linode as i32,
				initialize_immediately: false,
			})
			.await?;
		}
		// Handle provisioning errors gracefully
		Err(err) => {
			tracing::error!(?err, "failed to provision server, destroying");
			destroy(ctx, &crdb, &client, datacenter_id).await?;

			// NOTE: This will retry indefinitely to provision a prebake server
			retry_bail!("failed to provision server");
		}
	}

	Ok(())
}

async fn provision(
	ctx: &OperationContext<linode::msg::prebake_provision::Message>,
	crdb: &CrdbPool,
	client: &util_linode::Client,
	datacenter_id: Uuid,
	server: &api::ProvisionCtx,
) -> GlobalResult<String> {
	// Create SSH key
	let ssh_key_res = api::create_ssh_key(client, &Uuid::new_v4().to_string()).await?;

	// Write SSH key id
	sql_execute!(
		[ctx]
		"
		INSERT INTO db_cluster.server_images_linode_misc (
			install_hash,
			datacenter_id,
			pool_type,
			ssh_key_id
		)
		VALUES ($1, $2, $3, $4)
		",
		util_cluster::INSTALL_SCRIPT_HASH,
		datacenter_id,
		ctx.pool_type as i64,
		ssh_key_res.id as i64,
	)
	.await?;

	let create_instance_res = api::create_instance(client, server, &ssh_key_res.public_key).await?;
	let linode_id = create_instance_res.id;

	// Write linode id
	sql_execute!(
		[ctx]
		"
		UPDATE db_cluster.server_images_linode_misc
		SET linode_id = $4
		WHERE
			install_hash = $1 AND
			datacenter_id = $2 AND
			pool_type = $3
		",
		util_cluster::INSTALL_SCRIPT_HASH,
		datacenter_id,
		ctx.pool_type as i64,
		linode_id as i64,
	)
	.await?;

	api::wait_instance_ready(client, linode_id).await?;

	let create_disks_res = api::create_disks(
		client,
		&ssh_key_res.public_key,
		linode_id,
		"linode/debian11",
		create_instance_res.specs.disk,
	)
	.await?;

	api::create_instance_config(client, server, linode_id, &create_disks_res).await?;

	let firewall_res = api::create_firewall(client, server, linode_id).await?;

	// Write firewall id
	sql_execute!(
		[ctx, &crdb]
		"
		UPDATE db_cluster.server_images_linode_misc
		SET firewall_id = $4
		WHERE
			install_hash = $1 AND
			datacenter_id = $2 AND
			pool_type = $3
		",
		util_cluster::INSTALL_SCRIPT_HASH,
		datacenter_id,
		ctx.pool_type as i64,
		firewall_res.id as i64,
	)
	.await?;

	api::boot_instance(client, linode_id).await?;

	let public_ip = api::get_public_ip(client, linode_id).await?.to_string();

	// Write SSH key id
	sql_execute!(
		[ctx, &crdb]
		"
		UPDATE db_cluster.server_images_linode_misc
		SET
			disk_id = $4,
			public_ip = $5
		WHERE
			install_hash = $1 AND
			datacenter_id = $2 AND
			pool_type = $3
		",
		util_cluster::INSTALL_SCRIPT_HASH,
		datacenter_id,
		ctx.pool_type as i64,
		create_disks_res.boot_id as i64,
		&public_ip,
	)
	.await?;

	Ok(public_ip)
}

#[derive(sqlx::FromRow)]
struct LinodeData {
	ssh_key_id: i64,
	linode_id: Option<i64>,
	firewall_id: Option<i64>,
}

async fn destroy(
	ctx: &OperationContext<linode::msg::prebake_provision::Message>,
	crdb: &CrdbPool,
	client: &util_linode::Client,
	datacenter_id: Uuid,
) -> GlobalResult<()> {
	let data = sql_fetch_optional!(
		[ctx, LinodeData, &crdb]
		"
		SELECT ssh_key_id, linode_id, firewall_id
		FROM db_cluster.server_images_linode_misc
		WHERE
			install_hash = $1 AND
			datacenter_id = $2 AND
			pool_type = $3
		",
		util_cluster::INSTALL_SCRIPT_HASH,
		datacenter_id,
		ctx.pool_type as i64,
	)
	.await?;

	let Some(data) = data else {
		tracing::warn!("deleting server that doesn't exist");
		return Ok(());
	};

	if let Some(linode_id) = data.linode_id {
		api::delete_instance(client, linode_id).await?;
	}

	api::delete_ssh_key(client, data.ssh_key_id).await?;

	if let Some(firewall_id) = data.firewall_id {
		api::delete_firewall(client, firewall_id).await?;
	}

	// Remove record
	sql_execute!(
		[ctx]
		"
		DELETE FROM db_cluster.server_images_linode_misc
		WHERE
			install_hash = $1 AND
			datacenter_id = $2 AND
			pool_type = $3
		",
		util_cluster::INSTALL_SCRIPT_HASH,
		datacenter_id,
		ctx.pool_type as i64,
	)
	.await?;

	Ok(())
}
