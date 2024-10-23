use chirp_workflow::prelude::*;
use cloudflare::{endpoints as cf, framework as cf_framework};

use crate::types::PoolType;

pub mod metrics;
pub mod test;

// Use the hash of the server install script in the image variant so that if the install scripts are updated
// we won't be using the old image anymore
pub const INSTALL_SCRIPT_HASH: &str = include_str!(concat!(env!("OUT_DIR"), "/hash.txt"));

// TTL of the token written to prebake images. Prebake images are renewed before the token would expire
pub const SERVER_TOKEN_TTL: i64 = util::duration::days(30 * 6);

// Cluster id for provisioning servers
pub fn default_cluster_id() -> Uuid {
	Uuid::nil()
}

pub fn server_name(
	config: &rivet_config::Config,
	provider_datacenter_id: &str,
	pool_type: PoolType,
	server_id: Uuid,
) -> GlobalResult<String> {
	let ns = &config.server()?.rivet.namespace;

	Ok(format!(
		"{ns}-{provider_datacenter_id}-{pool_type}-{server_id}"
	))
}

pub(crate) async fn cf_client(
	config: &rivet_config::Config,
	cf_token: Option<&str>,
) -> GlobalResult<cf_framework::async_api::Client> {
	// Create CF client
	let cf_token = if let Some(cf_token) = cf_token {
		cf_token.to_string()
	} else {
		config.server()?.cloudflare()?.auth_token.read().clone()
	};
	let client = cf_framework::async_api::Client::new(
		cf_framework::auth::Credentials::UserAuthToken { token: cf_token },
		Default::default(),
		cf_framework::Environment::Production,
	)?;

	Ok(client)
}

/// Tries to create a DNS record. If a 400 error is received, it deletes the existing record and tries again.
pub(crate) async fn create_dns_record(
	client: &cf_framework::async_api::Client,
	cf_token: &str,
	zone_id: &str,
	record_name: &str,
	content: cf::dns::DnsContent,
) -> GlobalResult<String> {
	tracing::info!(%record_name, "creating dns record");

	let create_record_res = client
		.request(&cf::dns::CreateDnsRecord {
			zone_identifier: zone_id,
			params: cf::dns::CreateDnsRecordParams {
				name: record_name,
				content: content.clone(),
				proxied: Some(false),
				ttl: Some(60),
				priority: None,
			},
		})
		.await;

	match create_record_res {
		Ok(create_record_res) => Ok(create_record_res.result.id),
		// Try to delete record on error
		Err(err) => {
			if let cf_framework::response::ApiFailure::Error(
				http::status::StatusCode::BAD_REQUEST,
				_,
			) = err
			{
				tracing::warn!(%record_name, "failed to create dns record, trying to delete");

				let dns_type = match content {
					cf::dns::DnsContent::A { .. } => "A",
					cf::dns::DnsContent::AAAA { .. } => "AAAA",
					cf::dns::DnsContent::CNAME { .. } => "CNAME",
					cf::dns::DnsContent::NS { .. } => "NS",
					cf::dns::DnsContent::MX { .. } => "MX",
					cf::dns::DnsContent::TXT { .. } => "TXT",
					cf::dns::DnsContent::SRV { .. } => "SRV",
				};

				// Find record to delete
				let list_records_res = match content {
					cf::dns::DnsContent::A { .. } => {
						get_dns_record(cf_token, zone_id, record_name, dns_type).await?
					}
					cf::dns::DnsContent::TXT { .. } => {
						// Get DNS record with content comparison
						client
							.request(&cf::dns::ListDnsRecords {
								zone_identifier: zone_id,
								params: cf::dns::ListDnsRecordsParams {
									record_type: Some(content.clone()),
									name: Some(record_name.to_string()),
									..Default::default()
								},
							})
							.await?
							.result
							.into_iter()
							.next()
					}
					_ => {
						unimplemented!("must configure whether to search for records via content vs no content for this DNS record type");
					}
				};

				if let Some(record) = list_records_res {
					delete_dns_record(client, zone_id, &record.id).await?;
					tracing::info!(%record_name, "deleted dns record, trying again");

					// Second try
					let create_record_res2 = client
						.request(&cf::dns::CreateDnsRecord {
							zone_identifier: zone_id,
							params: cf::dns::CreateDnsRecordParams {
								name: record_name,
								content,
								proxied: Some(false),
								ttl: Some(60),
								priority: None,
							},
						})
						.await?;

					return Ok(create_record_res2.result.id);
				} else {
					tracing::warn!(%record_name, "failed to get matching dns record");
				}
			}

			// Throw original error
			Err(err.into())
		}
	}
}

pub(crate) async fn delete_dns_record(
	client: &cf_framework::async_api::Client,
	zone_id: &str,
	record_id: &str,
) -> GlobalResult<()> {
	tracing::info!(%record_id, "deleting dns record");

	client
		.request(&cf::dns::DeleteDnsRecord {
			zone_identifier: zone_id,
			identifier: record_id,
		})
		.await?;

	Ok(())
}

/// Fetches a dns record by name and type, not content.
async fn get_dns_record(
	cf_token: &str,
	zone_id: &str,
	record_name: &str,
	dns_type: &str,
) -> GlobalResult<Option<cf::dns::DnsRecord>> {
	let list_records_res = reqwest::Client::new()
		.get(format!(
			"https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records"
		))
		.bearer_auth(cf_token)
		.query(&[("name", record_name), ("type", dns_type)])
		.send()
		.await?;

	let status = list_records_res.status();
	if status.is_success() {
		match list_records_res
			.json::<cf_framework::response::ApiSuccess<Vec<cf::dns::DnsRecord>>>()
			.await
		{
			Ok(api_resp) => Ok(api_resp.result.into_iter().next()),
			Err(e) => Err(cf_framework::response::ApiFailure::Invalid(e).into()),
		}
	} else {
		let parsed: Result<cf_framework::response::ApiErrors, reqwest::Error> =
			list_records_res.json().await;
		let errors = parsed.unwrap_or_default();
		Err(cf_framework::response::ApiFailure::Error(status, errors).into())
	}
}
