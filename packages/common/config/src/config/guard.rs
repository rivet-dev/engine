use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Guard {
	/// Host for HTTP traffic
	pub host: Option<IpAddr>,
	/// Port for HTTP traffic
	pub port: Option<u16>,
	/// Enable & configure HTTPS
	pub https: Option<Https>,
}

impl Guard {
	pub fn host(&self) -> IpAddr {
		self.host.unwrap_or(crate::defaults::hosts::GUARD)
	}

	pub fn port(&self) -> u16 {
		self.port.unwrap_or(crate::defaults::ports::GUARD)
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
pub struct Https {
	pub port: u16, // Port for HTTPS traffic
	pub tls: Tls,  // TLS configuration
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
pub struct Tls {
	pub actor_cert_path: PathBuf,
	pub actor_key_path: PathBuf,
	pub api_cert_path: PathBuf,
	pub api_key_path: PathBuf,
}
