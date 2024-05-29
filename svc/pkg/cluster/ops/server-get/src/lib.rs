use std::{
	convert::{TryFrom, TryInto},
	net::IpAddr,
};

use proto::backend::{self, pkg::*};
use rivet_operation::prelude::*;

#[derive(sqlx::FromRow)]
struct Server {
	server_id: Uuid,
	cluster_id: Uuid,
	datacenter_id: Uuid,
	pool_type: i64,
	vlan_ip: Option<IpAddr>,
	public_ip: Option<IpAddr>,
	cloud_destroy_ts: Option<i64>,
}

impl TryFrom<Server> for backend::cluster::Server {
	type Error = GlobalError;

	fn try_from(value: Server) -> GlobalResult<Self> {
		Ok(backend::cluster::Server {
			server_id: Some(value.server_id.into()),
			cluster_id: Some(value.cluster_id.into()),
			datacenter_id: Some(value.datacenter_id.into()),
			pool_type: value.pool_type.try_into()?,
			vlan_ip: value.vlan_ip.map(|ip| ip.to_string()),
			public_ip: value.public_ip.map(|ip| ip.to_string()),
			cloud_destroy_ts: value.cloud_destroy_ts,
		})
	}
}

#[operation(name = "cluster-server-get")]
pub async fn handle(
	ctx: OperationContext<cluster::server_get::Request>,
) -> GlobalResult<cluster::server_get::Response> {
	let server_ids = ctx
		.server_ids
		.iter()
		.map(common::Uuid::as_uuid)
		.collect::<Vec<_>>();

	let servers = sql_fetch_all!(
		[ctx, Server]
		"
		SELECT
			server_id,
			d.cluster_id,
			s.datacenter_id,
			pool_type,
			vlan_ip,
			public_ip,
			cloud_destroy_ts
		FROM db_cluster.servers AS s
		LEFT JOIN db_cluster.datacenters AS d ON s.datacenter_id = d.datacenter_id
		WHERE server_id = ANY($1)
		",
		server_ids
	)
	.await?;

	Ok(cluster::server_get::Response {
		servers: servers
			.into_iter()
			.map(TryInto::try_into)
			.collect::<GlobalResult<Vec<_>>>()?,
	})
}
