use proto::backend::pkg::*;
use rivet_operation::prelude::*;
use util_linode::api;

#[derive(sqlx::FromRow)]
struct LinodeData {
	ssh_key_id: i64,
	linode_id: Option<i64>,
	firewall_id: Option<i64>,
}

#[operation(name = "linode-server-destroy")]
pub async fn handle(
	ctx: OperationContext<linode::server_destroy::Request>,
) -> GlobalResult<linode::server_destroy::Response> {
	let crdb = ctx.crdb().await?;
	let server_id = unwrap_ref!(ctx.server_id).as_uuid();
	let datacenter_id = unwrap!(ctx.datacenter_id);

	let datacenter_res = op!([ctx] cluster_datacenter_get {
		datacenter_ids: vec![datacenter_id],
	})
	.await?;
	let datacenter = unwrap!(datacenter_res.datacenters.first());

	let data = sql_fetch_optional!(
		[ctx, LinodeData, &crdb]
		"
		SELECT ssh_key_id, linode_id, firewall_id
		FROM db_cluster.linode_misc
		WHERE server_id = $1
		",
		server_id,
	)
	.await?;

	let Some(data) = data else {
		tracing::warn!("deleting server that doesn't exist");
		return Ok(linode::server_destroy::Response {});
	};

	// Build HTTP client
	let api_token = if let Some(api_token) = datacenter.provider_api_token.clone() {
		api_token
	} else {
		util::env::read_secret(&["linode", "token"]).await?
	};
	let client = util_linode::Client::new(&api_token).await?;

	if let Some(linode_id) = data.linode_id {
		api::delete_instance(&client, linode_id).await?;
	}

	api::delete_ssh_key(&client, data.ssh_key_id).await?;

	if let Some(firewall_id) = data.firewall_id {
		api::delete_firewall(&client, firewall_id).await?;
	}

	// Remove record
	sql_execute!(
		[ctx, &crdb]
		"
		DELETE FROM db_cluster.linode_misc
		WHERE server_id = $1
		",
		server_id,
	)
	.await?;

	Ok(linode::server_destroy::Response {})
}
