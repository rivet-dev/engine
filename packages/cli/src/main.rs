use anyhow::*;
use clap::Parser;
use rivet_cli::SubCommand;
use std::{path::PathBuf, sync::Arc};

#[derive(Parser)]
#[command(name = "Rivet", version, about)]
struct Cli {
	#[command(subcommand)]
	command: SubCommand,

	/// Path to the config file or directory of config files
	#[clap(long, global = true)]
	config: Vec<PathBuf>,
}

fn main() -> Result<()> {
	rivet_runtime::run(async { main_inner().await })??;
	Ok(())
}

async fn main_inner() -> Result<()> {
	let cli = Cli::parse();

	// Load config
	let config = rivet_config::Config::load(&cli.config)
		.await
		.map_err(|err| anyhow!("{err}"))?;

	// Build run config
	let run_config = Arc::new(rivet_cli::run_config::config(config.clone())?);

	// Execute command
	cli.command.execute(config, run_config).await
}
