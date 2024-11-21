use rivet_api::models;
use rivet_operation::prelude::*;

use crate::{ApiFrom, ApiInto};

impl ApiFrom<models::ProvisionPoolType> for cluster::types::PoolType {
	fn api_from(value: models::ProvisionPoolType) -> cluster::types::PoolType {
		match value {
			models::ProvisionPoolType::Job => cluster::types::PoolType::Job,
			models::ProvisionPoolType::Gg => cluster::types::PoolType::Gg,
			models::ProvisionPoolType::Ats => cluster::types::PoolType::Ats,
			models::ProvisionPoolType::Pegboard => cluster::types::PoolType::Pegboard,
			models::ProvisionPoolType::PegboardIsolate => cluster::types::PoolType::PegboardIsolate,
			models::ProvisionPoolType::Fdb => cluster::types::PoolType::Fdb,
		}
	}
}

impl ApiFrom<cluster::types::PoolType> for models::ProvisionPoolType {
	fn api_from(value: cluster::types::PoolType) -> models::ProvisionPoolType {
		match value {
			cluster::types::PoolType::Job => models::ProvisionPoolType::Job,
			cluster::types::PoolType::Gg => models::ProvisionPoolType::Gg,
			cluster::types::PoolType::Ats => models::ProvisionPoolType::Ats,
			cluster::types::PoolType::Pegboard => models::ProvisionPoolType::Pegboard,
			cluster::types::PoolType::PegboardIsolate => models::ProvisionPoolType::PegboardIsolate,
			cluster::types::PoolType::Fdb => models::ProvisionPoolType::Fdb,
		}
	}
}

impl ApiFrom<cluster::types::Server> for models::ProvisionServer {
	fn api_from(value: cluster::types::Server) -> models::ProvisionServer {
		models::ProvisionServer {
			server_id: value.server_id,
			datacenter_id: value.datacenter_id,
			pool_type: value.pool_type.api_into(),
			vlan_ip: value.vlan_ip.map(|ip| ip.to_string()),
			public_ip: value.public_ip.map(|ip| ip.to_string()),
		}
	}
}
