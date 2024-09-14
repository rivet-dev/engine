use chirp_workflow::prelude::*;

use crate::types::PoolType;

pub mod cf;
pub mod metrics;
pub mod test;

// Use the hash of the server install script in the image variant so that if the install scripts are updated
// we won't be using the old image anymore
pub const INSTALL_SCRIPT_HASH: &str = include_str!(concat!(env!("OUT_DIR"), "/hash.txt"));

// TTL of the token written to prebake images. Prebake images are renewed before the token would expire
pub const SERVER_TOKEN_TTL: i64 = util::duration::days(30 * 6);

// NOTE: We don't reserve CPU because Nomad is running as a higher priority process than the rest and
// shouldn't be doing much heavy lifting.
const RESERVE_SYSTEM_MEMORY: u64 = 512;
// See module.traefik_job resources
const RESERVE_LB_MEMORY: u64 = 512;
const RESERVE_MEMORY: u64 = RESERVE_SYSTEM_MEMORY + RESERVE_LB_MEMORY;

const CPU_PER_CORE: u64 = 1999;

/// Provider agnostic hardware specs.
#[derive(Debug)]
pub struct JobNodeConfig {
	pub cpu_cores: u64,
	/// Mhz
	pub cpu: u64,
	/// MB
	pub memory: u64,
	/// MB
	pub disk: u64,
	/// Kbps
	pub bandwidth: u64,
}

impl JobNodeConfig {
	pub fn from_linode(instance_type: &linode::types::InstanceType) -> JobNodeConfig {
		// Account for kernel memory overhead
		// https://www.linode.com/community/questions/17791/why-doesnt-free-m-match-the-full-amount-of-ram-of-my-nanode-plan
		let memory = instance_type.memory * 96 / 100;
		// Remove reserved resources
		let memory = memory - RESERVE_MEMORY;

		JobNodeConfig {
			cpu_cores: instance_type.vcpus,
			cpu: instance_type.vcpus * CPU_PER_CORE,
			memory,
			disk: instance_type.disk,
			bandwidth: instance_type.network_out * 1000,
		}
	}

	pub fn from_vultr(instance_type: &vultr::types::InstanceType) -> JobNodeConfig {
		unimplemented!();
		// // Account for kernel memory overhead
		// // https://www.linode.com/community/questions/17791/why-doesnt-free-m-match-the-full-amount-of-ram-of-my-nanode-plan
		// let memory = instance_type.memory * 96 / 100;
		// // Remove reserved resources
		// let memory = memory - RESERVE_MEMORY;

		// JobNodeConfig {
		// 	cpu_cores: instance_type.vcpus,
		// 	cpu: instance_type.vcpus * CPU_PER_CORE,
		// 	memory,
		// 	disk: instance_type.disk,
		// 	bandwidth: instance_type.network_out * 1000,
		// }
	}

	pub fn cpu_per_core(&self) -> u64 {
		CPU_PER_CORE
	}

	pub fn memory_per_core(&self) -> u64 {
		self.memory / self.cpu_cores
	}

	pub fn disk_per_core(&self) -> u64 {
		self.disk / self.cpu_cores
	}

	pub fn bandwidth_per_core(&self) -> u64 {
		self.bandwidth / self.cpu_cores
	}
}

// Cluster id for provisioning servers
pub fn default_cluster_id() -> Uuid {
	Uuid::nil()
}

pub fn server_name(provider_datacenter_id: &str, pool_type: PoolType, server_id: Uuid) -> String {
	let ns = util::env::namespace();
	let pool_type_str = match pool_type {
		PoolType::Job => "job",
		PoolType::Gg => "gg",
		PoolType::Ats => "ats",
	};

	format!("{ns}-{provider_datacenter_id}-{pool_type_str}-{server_id}",)
}
