use chirp_workflow::prelude::*;
use cluster::types::GuardPublicHostname;

use crate::types::{EndpointType, GameGuardProtocol};

pub mod consts;
pub mod nomad_job;
mod oci_config;
mod seccomp;
pub mod test;

pub const NOMAD_REGION: &str = "global";
pub const RUNC_SETUP_CPU: i32 = 50;
pub const RUNC_SETUP_MEMORY: i32 = 32;
pub const RUNC_CLEANUP_CPU: i32 = 50;
pub const RUNC_CLEANUP_MEMORY: i32 = 32;

/// Determines if a Nomad job is dispatched from our dynamic server.
///
/// We use this when monitoring Nomad in order to determine which events to
/// pay attention to.
pub fn is_nomad_ds(job_id: &str) -> bool {
	job_id.starts_with("ds-") && job_id.contains("/dispatch-")
}

pub fn build_ds_hostname_and_path(
	server_id: Uuid,
	port_name: &str,
	datacenter_id: Uuid,
	protocol: GameGuardProtocol,
	endpoint_type: EndpointType,
	guard_public_hostname: &GuardPublicHostname,
) -> GlobalResult<(String, Option<String>)> {
	let is_http = matches!(protocol, GameGuardProtocol::Http | GameGuardProtocol::Https);
	match (is_http, endpoint_type, guard_public_hostname) {
		// Non-HTTP protocols can use any hostname (since they route by port), but including the
		// server in the subdomain is a convenience
		(true, EndpointType::Hostname, GuardPublicHostname::DnsParent(dns_parent))
		| (false, _, GuardPublicHostname::DnsParent(dns_parent)) => {
			Ok((format!("{server_id}-{port_name}.actor.{dns_parent}"), None))
		}

		(true, EndpointType::Hostname, GuardPublicHostname::Static(_)) => {
			bail!("cannot use hostname endpoint type with static hostname")
		}

		(true, EndpointType::Path, GuardPublicHostname::DnsParent(dns_parent)) => Ok((
			// This will not collide with host-based routing since server IDs are always UUIDs.
			//
			// This is stored on a subdomain of `actor` instead of `actor.{dns_parent}` since
			// hosting actors on a parent domain of the `{actor_id}.actor.{dns_parent}` could lead
			// to a security vulnerability if cookies on the parent domain have a misconfigured
			// domain scope that grants access to the children. This is a very niche security
			// vulnerability, but worth avoiding regardless.
			format!("route.actor.{dns_parent}"),
			Some(format!("/{server_id}-{port_name}")),
		)),

		(true, EndpointType::Path, GuardPublicHostname::Static(static_)) => {
			Ok((static_.clone(), Some(format!("/{server_id}-{port_name}"))))
		}

		// Non-HTTP protocols will be routed via the port, so we can use the static protocol
		(false, _, GuardPublicHostname::Static(static_)) => Ok((static_.clone(), None)),
	}
}

/// Formats the port label to be used in Nomad and Pegboard.
///
/// Prefixing this port ensure that the user defined port names don't interfere
/// with other ports.
///
/// See also SQL `concat` in `svc/api/traefik-provider/src/route/game_guard/dynamic_servers.rs`.
pub fn nomad_prefix_port_label(port_label: &str) -> String {
	let snake_port_label = heck::SnakeCase::to_snake_case(port_label);
	format!("ds_{snake_port_label}")
}

/// Standardize the port label format.
pub fn pegboard_normalize_port_label(port_label: &str) -> String {
	heck::SnakeCase::to_snake_case(port_label)
}

// Have to patch `nomad_client::apis::allocations_api::signal_allocation` because it uses `/allocation`
// instead of `/client/allocation`
pub async fn signal_allocation(
	configuration: &nomad_client::apis::configuration::Configuration,
	alloc_id: &str,
	namespace: Option<&str>,
	region: Option<&str>,
	index: Option<i64>,
	wait: Option<&str>,
	alloc_signal_request: Option<nomad_client_old::models::AllocSignalRequest>,
) -> Result<
	(),
	nomad_client::apis::Error<nomad_client_old::apis::allocations_api::SignalAllocationError>,
> {
	let local_var_client = &configuration.client;

	let local_var_uri_str = format!(
		"{}/client/allocation/{alloc_id}/signal",
		configuration.base_path,
		alloc_id = nomad_client::apis::urlencode(alloc_id),
	);
	let mut local_var_req_builder = local_var_client.post(local_var_uri_str.as_str());

	if let Some(ref local_var_str) = namespace {
		local_var_req_builder =
			local_var_req_builder.query(&[("namespace", &local_var_str.to_string())]);
	}
	if let Some(ref local_var_str) = region {
		local_var_req_builder =
			local_var_req_builder.query(&[("region", &local_var_str.to_string())]);
	}
	if let Some(ref local_var_str) = index {
		local_var_req_builder =
			local_var_req_builder.query(&[("index", &local_var_str.to_string())]);
	}
	if let Some(ref local_var_str) = wait {
		local_var_req_builder =
			local_var_req_builder.query(&[("wait", &local_var_str.to_string())]);
	}
	if let Some(ref local_var_user_agent) = configuration.user_agent {
		local_var_req_builder =
			local_var_req_builder.header(http::header::USER_AGENT, local_var_user_agent.clone());
	}
	local_var_req_builder = local_var_req_builder.json(&alloc_signal_request);

	let local_var_req = local_var_req_builder.build()?;
	let local_var_resp = local_var_client.execute(local_var_req).await?;

	let local_var_status = local_var_resp.status();
	let local_var_content = local_var_resp.text().await?;

	if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
		Ok(())
	} else {
		let local_var_entity: Option<
			nomad_client_old::apis::allocations_api::SignalAllocationError,
		> = serde_json::from_str(&local_var_content).ok();
		let local_var_error = nomad_client::apis::ResponseContent {
			status: local_var_status,
			content: local_var_content,
			entity: local_var_entity,
		};
		Err(nomad_client::apis::Error::ResponseError(local_var_error))
	}
}
