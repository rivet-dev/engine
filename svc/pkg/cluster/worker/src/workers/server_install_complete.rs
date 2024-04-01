use chirp_worker::prelude::*;
use proto::backend::{self, pkg::*};

#[worker(name = "cluster-server-install-complete")]
async fn worker(
	ctx: &OperationContext<cluster::msg::server_install_complete::Message>,
) -> GlobalResult<()> {
	let provider = unwrap!(backend::cluster::Provider::from_i32(ctx.provider));

	match provider {
		backend::cluster::Provider::Linode => {
			if ctx.server_id.is_none() {
				msg!([ctx] linode::msg::prebake_install_complete(&ctx.ip) {
					ip: ctx.ip.clone(),
				})
				.await?;
			}
		}
	}

	Ok(())
}
