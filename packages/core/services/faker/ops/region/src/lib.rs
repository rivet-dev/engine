use proto::backend::pkg::*;
use rivet_operation::prelude::*;

#[operation(name = "faker-region")]
async fn handle(
	ctx: OperationContext<faker::region::Request>,
) -> GlobalResult<faker::region::Response> {
	let region_list = op!([ctx] region_list { }).await?;

	// Get the region data
	let region_get = op!([ctx] region_get {
		region_ids: region_list.region_ids.clone(),
	})
	.await?;

	// For consistency
	let mut regions = region_get.regions.clone();
	regions.sort_by_key(|region| region.region_id.map(|id| id.as_uuid()));

	let region = unwrap!(region_get.regions.first());

	Ok(faker::region::Response {
		region_id: region.region_id,
		region: Some(region.clone()),
	})
}
