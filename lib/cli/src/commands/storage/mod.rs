use anyhow::*;
use clap::Parser;

use crate::run_config::RunConfig;

mod provision;

#[derive(Parser)]
pub enum SubCommand {
	Provision(provision::Opts),
}

impl SubCommand {
	pub async fn execute(self, config: rivet_config::Config, run_config: &RunConfig) -> Result<()> {
		match self {
			Self::Provision(opts) => opts.execute(config, run_config).await,
		}
	}
}
