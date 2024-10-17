use std::{path::Path, process::Command, sync::Arc};

use anyhow::Result;
use lazy_static::lazy_static;
use tokio::sync::Semaphore;

use crate::{config, context::ProjectContext, utils::command_helper::CommandHelper};

lazy_static! {
	static ref TF_INIT_SEMAPHORE: Arc<Semaphore> = Arc::new(Semaphore::new(1));
}

/// Builds the workspace name that's used for the specific plan. This lets us
/// store multiple workspaces on the same backend.
pub fn build_localized_workspace_name(ns: &str, plan: &str) -> String {
	format!("{}-{}", ns.replace('_', "-"), plan.replace('_', "-"))
}

pub async fn build_command(ctx: &ProjectContext, plan_id: &str) -> Command {
	let mut cmd = Command::new("terraform");
	cmd.current_dir(ctx.tf_path().join(plan_id));
	cmd
}

pub async fn init_if_needed(ctx: &ProjectContext, plan_id: &str) {
	init_if_needed_quiet(ctx, plan_id, false).await
}

pub async fn init_if_needed_quiet(ctx: &ProjectContext, plan_id: &str, quiet: bool) {
	if std::env::var("BOLT_IGNORE_TERRAFORM")
		.ok()
		.map_or(false, |x| x == "1")
	{
		rivet_term::status::info("Skipping Terrafrom Init", "");
		return;
	}

	let localized_workspace_name = build_localized_workspace_name(ctx.ns_id(), plan_id);

	// Get current workspace
	let mut show_cmd = build_command(ctx, plan_id).await;
	show_cmd.arg("workspace").arg("show");
	let current_workspace = show_cmd
		.exec_string_with_err("Command failed", true)
		.await
		.unwrap();

	// Switch workspace if needed
	if current_workspace.trim() != localized_workspace_name {
		let _permit = TF_INIT_SEMAPHORE.clone().acquire_owned().await.unwrap();

		println!();
		rivet_term::status::info("Switching Workspace", &localized_workspace_name);

		// Configure backend
		let backend_args = match ctx.ns().terraform.backend {
			config::ns::TerraformBackend::Local {} => Vec::new(),
			config::ns::TerraformBackend::Postgres {} => {
				let tf_conn_str = ctx
					.read_secret(&["terraform", "pg_backend", "conn_str"])
					.await
					.unwrap();
				vec![
					format!("-backend-config=conn_str={tf_conn_str}"),
					"-reconfigure".to_string(),
				]
			}
		};

		// Setup Terraform repo in case hasn't been initiated before. This
		// has to be done before switching workspaces.
		let mut init_cmd = build_command(ctx, plan_id).await;
		init_cmd.arg("init");
		init_cmd.args(backend_args);
		init_cmd.exec_quiet(quiet, quiet).await.unwrap();

		// Attempt to select workspace
		let mut select_cmd = build_command(ctx, plan_id).await;
		select_cmd
			.arg("workspace")
			.arg("select")
			.arg(&localized_workspace_name);
		let workspace_exists = select_cmd.exec_quiet(quiet, quiet).await.is_ok();

		// Create workspace if it doesn't exist
		if !workspace_exists {
			println!();
			rivet_term::status::progress("Creating Workspace", &localized_workspace_name);
			let mut new_cmd = build_command(ctx, plan_id).await;
			new_cmd
				.arg("workspace")
				.arg("new")
				.arg(&localized_workspace_name);
			new_cmd.exec_quiet(quiet, quiet).await.unwrap();
		}
	}
}

pub async fn apply(
	ctx: &ProjectContext,
	plan_id: &str,
	yes: bool,
	varfile_path: &Path,
) -> Result<()> {
	let mut cmd = build_command(ctx, plan_id).await;
	cmd.arg("apply")
		.arg(format!("-var-file={}", varfile_path.display()))
		.arg("-parallelism=16");
	if yes {
		cmd.arg("-auto-approve");
	}
	cmd.exec().await?;

	Ok(())
}

pub async fn destroy(ctx: &ProjectContext, plan_id: &str, varfile_path: &Path) -> Result<()> {
	let mut cmd = build_command(ctx, plan_id).await;
	cmd.arg("destroy")
		.arg(format!("-var-file={}", varfile_path.display()));
	cmd.exec().await?;

	Ok(())
}

pub async fn output(ctx: &ProjectContext, plan_id: &str, quiet: bool) -> serde_json::Value {
	init_if_needed_quiet(ctx, plan_id, quiet).await;

	let mut cmd = build_command(ctx, plan_id).await;
	cmd.arg("output");
	cmd.arg("-json");
	cmd.exec_value_with_err("Command failed", quiet)
		.await
		.unwrap()
}

pub async fn state_list(ctx: &ProjectContext, plan_id: &str) -> Option<Vec<String>> {
	// Don't return state if not enabled
	if !crate::tasks::infra::all_terraform_plans(ctx)
		.ok()
		.map_or(false, |x| x.iter().any(|x| x.as_str() == plan_id))
	{
		return None;
	}

	// Return all resources
	init_if_needed(ctx, plan_id).await;
	let mut cmd = build_command(ctx, plan_id).await;
	cmd.arg("state").arg("list");
	cmd.exec_string().await.ok().map(|x| {
		x.split('\n')
			.filter(|x| !x.is_empty())
			.map(|x| x.to_string())
			.collect()
	})
}

pub async fn has_applied(ctx: &ProjectContext, plan_id: &str) -> bool {
	// Check if Terraform plan is required for both:
	// - Return false if the plan was disabled for any reason
	// - Faster performance
	let terraform_plans = crate::tasks::infra::all_terraform_plans(ctx).unwrap();
	if terraform_plans.iter().all(|x| x != plan_id) {
		return false;
	}

	// Check if there is any output
	//
	// HACK: This will have a false negative for plans that have no output variables
	output(ctx, plan_id, true).await != serde_json::json!({})
}
