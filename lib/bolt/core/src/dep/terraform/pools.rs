// TODO: Move this file to a common place, since this isn't specific to Terraform

use std::collections::HashMap;

use anyhow::Result;
use derive_builder::Builder;
use ipnet::Ipv4AddrRange;
use serde::Serialize;

use super::net;
use crate::context::ProjectContext;

#[derive(Serialize, Clone, Builder)]
#[builder(setter(into))]
pub struct Pool {
	#[serde(skip)]
	pub vlan_addr_range: Ipv4AddrRange,

	/// Volumes attached to this node.
	#[builder(default)]
	volumes: HashMap<String, PoolVolume>,

	/// Cloud-based firewall rules to apply to this node.
	///
	/// Additional firewall rules are applied by Terraform depending on the use case.
	#[builder(default)]
	firewall_inbound: Vec<FirewallRule>,
}

#[derive(Serialize, Clone)]
pub struct PoolVolume {}

#[derive(Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FirewallRule {
	label: String,
	ports: String,
	protocol: String,
	inbound_ipv4_cidr: Vec<String>,
	inbound_ipv6_cidr: Vec<String>,
}

pub async fn build_pools(_ctx: &ProjectContext) -> Result<HashMap<String, Pool>> {
	let mut pools = HashMap::<String, Pool>::new();

	pools.insert(
		"gg".into(),
		PoolBuilder::default()
			.vlan_addr_range(net::gg::vlan_addr_range())
			.firewall_inbound(vec![
				// HTTP(S)
				FirewallRule {
					label: "http-tcp".into(),
					ports: "80".into(),
					protocol: "tcp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
				FirewallRule {
					label: "http-udp".into(),
					ports: "80".into(),
					protocol: "udp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
				FirewallRule {
					label: "https-tcp".into(),
					ports: "443".into(),
					protocol: "tcp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
				FirewallRule {
					label: "https-udp".into(),
					ports: "443".into(),
					protocol: "udp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
				// Dynamic TCP
				FirewallRule {
					label: "dynamic-tcp".into(),
					ports: "20000-31999".into(),
					protocol: "tcp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
				// Dynamic UDP
				FirewallRule {
					label: "dynamic-udp".into(),
					ports: "20000-31999".into(),
					protocol: "udp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
			])
			.build()?,
	);

	pools.insert(
		"job".into(),
		PoolBuilder::default()
			.vlan_addr_range(net::job::vlan_addr_range())
			.firewall_inbound(vec![
				// Ports available to Nomad jobs using the host network
				FirewallRule {
					label: "nomad-host-tcp".into(),
					ports: "26000-31999".into(),
					protocol: "tcp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
				FirewallRule {
					label: "nomad-host-udp".into(),
					ports: "26000-31999".into(),
					protocol: "udp".into(),
					inbound_ipv4_cidr: vec!["0.0.0.0/0".into()],
					inbound_ipv6_cidr: vec!["::/0".into()],
				},
			])
			.build()?,
	);

	pools.insert(
		"ats".into(),
		PoolBuilder::default()
			.vlan_addr_range(net::ats::vlan_addr_range())
			.build()?,
	);

	Ok(pools)
}
