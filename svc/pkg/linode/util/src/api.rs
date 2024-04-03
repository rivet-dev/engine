use std::{net::Ipv4Addr, str, time::Duration};

use chrono::{DateTime, Utc};
use proto::backend::pkg::*;
use rivet_operation::prelude::*;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use ssh_key::PrivateKey;

use crate::{generate_password, ApiErrorResponse, Client};

pub struct ProvisionCtx {
	pub datacenter: String,
	pub name: String,
	pub hardware: String,
	pub vlan_ip: Option<String>,
	pub tags: Vec<String>,
	pub firewall_inbound: Vec<util::net::FirewallRule>,
}

#[derive(Deserialize)]
struct CreateSshKeyResponse {
	id: u64,
}

pub struct SshKeyResponse {
	pub id: u64,
	pub public_key: String,
}

pub async fn create_ssh_key(
	client: &Client,
	label: &str,
) -> GlobalResult<SshKeyResponse> {
	tracing::info!("creating linode ssh key");

	let private_key_openssh =
		util::env::read_secret(&["ssh", "server", "private_key_openssh"]).await?;
	let private_key = PrivateKey::from_openssh(private_key_openssh.as_bytes())?;

	// Extract the public key
	let public_key = private_key.public_key().to_string();

	let res = client
		.post::<CreateSshKeyResponse>(
			"/profile/sshkeys",
			json!({
				// Label must be < 64 characters for some stupid reason
				"label": label,
				"ssh_key": public_key,
			}),
		)
		.await?;

	Ok(SshKeyResponse {
		id: res.id,
		public_key,
	})
}

#[derive(Deserialize)]
pub struct CreateInstanceResponse {
	pub id: u64,
	pub specs: InstanceSpec,
}

#[derive(Deserialize)]
pub struct InstanceSpec {
	pub disk: u64,
}

pub async fn create_instance(
	client: &Client,
	server: &ProvisionCtx,
	ssh_key: &str,
) -> GlobalResult<CreateInstanceResponse> {
	let ns = util::env::namespace();

	tracing::info!("creating linode instance");

	client
		.post(
			"/linode/instances",
			json!({
				"label": server.name,
				"group": ns,
				"region": server.datacenter,
				"type": server.hardware,
				"authorized_keys": vec![ssh_key],
				"tags": server.tags,
				"private_ip": true,
				"backups_enabled": false,
			}),
		)
		.await
}

#[derive(Deserialize)]
pub struct CreateDiskResponse {
	pub id: u64,
}

pub struct CreateDisksResponse {
	pub boot_id: u64,
	pub swap_id: u64,
}

pub async fn create_disks(
	client: &Client,
	ssh_key: &str,
	linode_id: u64,
	image: &str,
	server_disk_size: u64,
) -> GlobalResult<CreateDisksResponse> {
	tracing::info!("creating boot disk");

	let boot_disk_res = client
		.post::<CreateDiskResponse>(
			&format!("/linode/instances/{linode_id}/disks"),
			json!({
				"label": "boot",
				"size": server_disk_size - 512,
				"authorized_keys": vec![ssh_key],
				"root_pass": generate_password(16),
				"image": image,
			}),
		)
		.await?;

	wait_disk_ready(client, linode_id, boot_disk_res.id).await?;

	tracing::info!("creating swap disk");

	let swap_disk_res = client
		.post::<CreateDiskResponse>(
			&format!("/linode/instances/{linode_id}/disks"),
			json!({
				"label": "swap",
				"size": 512,
				"filesystem": "swap",
			}),
		)
		.await?;

	Ok(CreateDisksResponse {
		boot_id: boot_disk_res.id,
		swap_id: swap_disk_res.id,
	})
}

pub async fn create_instance_config(
	client: &Client,
	server: &ProvisionCtx,
	linode_id: u64,
	disks: &CreateDisksResponse,
) -> GlobalResult<()> {
	tracing::info!("creating instance config");

	let ns = util::env::namespace();

	let interfaces = if let Some(vlan_ip) = &server.vlan_ip {
		let region_vlan = util::net::region::vlan_ip_net();
		let ipam_address = format!("{}/{}", vlan_ip, region_vlan.prefix_len());

		json!([
			{
				"purpose": "public",
			},
			{
				"purpose": "vlan",
				"label": format!("{ns}-vlan"),
				"ipam_address": ipam_address,
			},
		])
	} else {
		json!([{
			"purpose": "public",
		}])
	};

	client
		.post_no_res(
			&format!("/linode/instances/{linode_id}/configs"),
			json!({
				"label": "boot_config",
				"booted": true,
				"kernel": "linode/latest-64bit",
				"root_device": "/dev/sda",
				"devices": {
					"sda": {
						"disk_id": disks.boot_id,
					},
					"sdb": {
						"disk_id": disks.swap_id,
					},
				},
				"interfaces": interfaces,
			}),
		)
		.await
}

#[derive(Deserialize)]
pub struct CreateFirewallResponse {
	pub id: u64,
}

pub async fn create_firewall(
	client: &Client,
	server: &ProvisionCtx,
	linode_id: u64,
) -> GlobalResult<CreateFirewallResponse> {
	tracing::info!("creating firewall");

	let ns = util::env::namespace();

	let firewall_inbound = server
		.firewall_inbound
		.iter()
		.map(|rule| {
			json!({
				"label": rule.label,
				"action": "ACCEPT",
				"protocol": rule.protocol.to_uppercase(),
				"ports": rule.ports,
				"addresses": {
					"ipv4": rule.inbound_ipv4_cidr,
					"ipv6": rule.inbound_ipv6_cidr,
				},

			})
		})
		.collect::<Vec<_>>();

	client
		.post(
			"/networking/firewalls",
			json!({
				// Label doesn't matter
				"label": format!("{ns}-{}", generate_password(16)),
				"rules": {
					"inbound": firewall_inbound,
					"inbound_policy": "DROP",
					"outbound_policy": "ACCEPT",
				},
				"devices": {
					"linodes": [linode_id],
				},
				"tags": server.tags,
			}),
		)
		.await
}

pub async fn boot_instance(client: &Client, linode_id: u64) -> GlobalResult<()> {
	tracing::info!("booting instance");

	client
		.post_no_res(&format!("/linode/instances/{linode_id}/boot"), json!({}))
		.await
}

#[derive(Deserialize)]
pub struct LinodeInstanceResponse {
	status: String,
}

// Helpful: https://www.linode.com/community/questions/11588/linodeerrorsapierror-400-linode-busy
/// Polls linode API until an instance is available.
pub async fn wait_instance_ready(client: &Client, linode_id: u64) -> GlobalResult<()> {
	tracing::info!("waiting for instance to be ready");

	loop {
		let res = client
			.get::<LinodeInstanceResponse>(&format!("/linode/instances/{linode_id}"))
			.await?;

		// Check if ready
		match res.status.as_str() {
			"booting" | "rebooting" | "shutting_down" | "provisioning" | "deleting"
			| "migrating" | "rebuilding" | "cloning" | "restoring" => {}
			_ => break,
		}

		tokio::time::sleep(Duration::from_secs(1)).await;
	}

	Ok(())
}

#[derive(Deserialize)]
pub struct LinodeDiskResponse {
	status: String,
}

/// Polls linode API until a linode disk is available.
pub async fn wait_disk_ready(client: &Client, linode_id: u64, disk_id: u64) -> GlobalResult<()> {
	tracing::info!("waiting for linode disk to be ready");

	loop {
		let res = client
			.inner()
			.get(&format!(
				"https://api.linode.com/v4/linode/instances/{linode_id}/disks/{disk_id}"
			))
			.send()
			.await?;

		// Manually handle the disk showing up as not found yet
		if res.status() == reqwest::StatusCode::NOT_FOUND {
			tracing::info!("disk not found yet");
		} else {
			if !res.status().is_success() {
				tracing::info!(status=?res.status(), "api request failed");
				bail_with!(ERROR, error = res.json::<ApiErrorResponse>().await?);
			}

			let res = res.json::<LinodeDiskResponse>().await?;

			// Check if ready
			match res.status.as_str() {
				"not ready" => {}
				_ => break,
			}
		}

		tokio::time::sleep(Duration::from_secs(3)).await;
	}

	Ok(())
}

#[derive(Deserialize)]
pub struct GetPublicIpResponse {
	ipv4: LinodeIpv4,
}

#[derive(Deserialize)]
pub struct LinodeIpv4 {
	public: Vec<LinodeIpv4Config>,
}

#[derive(Deserialize)]
pub struct LinodeIpv4Config {
	address: Ipv4Addr,
}

pub async fn get_public_ip(client: &Client, linode_id: u64) -> GlobalResult<Ipv4Addr> {
	tracing::info!("getting ip");

	let res = client
		.get::<GetPublicIpResponse>(&format!("/linode/instances/{linode_id}/ips"))
		.await?;
	let public = unwrap!(res.ipv4.public.first());

	Ok(public.address)
}

pub async fn delete_ssh_key(client: &Client, ssh_key_id: i64) -> GlobalResult<()> {
	tracing::info!("deleting linode ssh key");

	client
		.delete(&format!("/profile/sshkeys/{ssh_key_id}"))
		.await
}

pub async fn delete_instance(client: &Client, linode_id: i64) -> GlobalResult<()> {
	tracing::info!(?linode_id, "deleting linode instance");

	client
		.delete(&format!("/linode/instances/{linode_id}"))
		.await
}

pub async fn delete_firewall(client: &Client, firewall_id: i64) -> GlobalResult<()> {
	tracing::info!("deleting firewall");

	client
		.delete(&format!("/networking/firewalls/{firewall_id}"))
		.await
}

pub async fn shut_down(client: &Client, linode_id: i64) -> GlobalResult<()> {
	tracing::info!("shutting down instance");

	client
		.post_no_res(
			&format!("/linode/instances/{linode_id}/shutdown"),
			json!({}),
		)
		.await
}

#[derive(Deserialize)]
pub struct CreateCustomImageResponse {
	pub id: String,
}

pub async fn create_custom_image(
	client: &Client,
	variant: &str,
	disk_id: i64,
) -> GlobalResult<CreateCustomImageResponse> {
	tracing::info!("creating custom image");

	client
		.post(
			"/images",
			json!({
			  "disk_id": disk_id,
			  "label": variant,
			}),
		)
		.await
}

pub async fn delete_custom_image(client: &Client, image_id: &str) -> GlobalResult<()> {
	tracing::info!(?image_id, "deleting custom image");

	client.delete(&format!("/images/{image_id}")).await
}

#[derive(Deserialize)]
pub struct ListCustomImagesResponse {
	pub data: Vec<CustomImage>,
}

#[derive(Deserialize)]
pub struct CustomImage {
	pub id: String,
	pub created_by: Option<String>,
	#[serde(deserialize_with = "deserialize_date")]
	pub created: DateTime<Utc>,
}

pub async fn list_custom_images(client: &Client) -> GlobalResult<Vec<CustomImage>> {
	tracing::info!("listing custom images");

	let res = client.get::<ListCustomImagesResponse>("/images").await?;

	Ok(res
		.data
		.into_iter()
		.filter(|img| {
			img.created_by
				.as_ref()
				.map(|created_by| created_by != "linode")
				.unwrap_or_default()
		})
		.collect::<Vec<_>>())
}

#[derive(Deserialize)]
pub struct ListInstanceTypesResponse {
	pub data: Vec<InstanceType>,
}

#[derive(Deserialize)]
pub struct InstanceType {
	pub id: String,
	pub memory: u64,
	pub disk: u64,
	pub vcpus: u64,
	pub transfer: u64,
	pub network_out: u64,
}

impl From<InstanceType> for linode::instance_type_get::response::InstanceType {
	fn from(value: InstanceType) -> Self {
		linode::instance_type_get::response::InstanceType {
			hardware_id: value.id,
			memory: value.memory,
			disk: value.disk,
			vcpus: value.vcpus,
			transfer: value.transfer,
			// network_out: value.network_out,
		}
	}
}

pub async fn list_instance_types(client: &Client) -> GlobalResult<Vec<InstanceType>> {
	tracing::info!("listing instance types");

	let res = client
		.get::<ListInstanceTypesResponse>("/linode/types")
		.await?;

	Ok(res.data)
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
	D: Deserializer<'de>,
{
	// Add Z timezone specifier
	let s = format!("{}Z", String::deserialize(deserializer)?);
	DateTime::parse_from_rfc3339(&s)
		.map_err(serde::de::Error::custom)
		.map(|dt| dt.with_timezone(&Utc))
}
