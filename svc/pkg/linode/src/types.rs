use chirp_workflow::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceType {
	pub hardware_id: String,
	pub memory: u32,
	pub disk: u32,
	pub vcpus: u32,
	pub transfer: u32,
	pub network_out: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum FirewallPreset {
	// TODO: Rename to game
	Job,
	Gg,
	Ats,
}

impl FirewallPreset {
	pub fn rules(&self) -> Vec<util::net::FirewallRule> {
		match self {
			FirewallPreset::Job => util::net::job::firewall(),
			FirewallPreset::Gg => util::net::gg::firewall(),
			FirewallPreset::Ats => util::net::ats::firewall(),
		}
	}
}

impl std::fmt::Display for FirewallPreset {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FirewallPreset::Job => write!(f, "job"),
			FirewallPreset::Gg => write!(f, "gg"),
			FirewallPreset::Ats => write!(f, "ats"),
		}
	}
}
