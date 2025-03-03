use api_helper::{anchor::WatchIndexQuery, ctx::Ctx};
use rivet_api::models;
use rivet_convert::ApiTryInto;
use rivet_operation::prelude::*;

use crate::auth::Auth;

// MARK: GET /region-tiers
pub async fn list_tiers(
	ctx: Ctx<Auth>,
	_watch_index: WatchIndexQuery,
) -> GlobalResult<models::CloudGetRegionTiersResponse> {
	let default_cluster_id = ctx.config().server()?.rivet.default_cluster_id()?;

	let datacenters_res = ctx
		.op(cluster::ops::datacenter::list::Input {
			cluster_ids: vec![default_cluster_id],
		})
		.await?;
	let cluster = unwrap!(datacenters_res.clusters.first());

	let res = ctx
		.op(tier::ops::list::Input {
			datacenter_ids: cluster.datacenter_ids.clone(),
			pegboard: false,
		})
		.await?;

	let dc = unwrap!(res.datacenters.first());

	Ok(models::CloudGetRegionTiersResponse {
		tiers: dc
			.tiers
			.clone()
			.into_iter()
			.map(ApiTryInto::api_try_into)
			.collect::<GlobalResult<Vec<_>>>()?,
	})
}
