use anyhow::*;
use rivet_api::{apis, models};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
	build, paths,
	project::environment::TEMPEnvironment,
	ToolchainCtx,
	{
		config::{self, build::Runtime},
		util::task,
	},
};

pub mod docker;
pub mod js;

#[derive(Deserialize)]
pub struct Input {
	pub environment_id: Uuid,
	pub build_tags: Option<HashMap<String, String>>,
	pub version_name: String,
	pub build_name: String,
	pub runtime: config::build::Runtime,
	#[serde(default)]
	pub skip_upgrade: bool,
}

#[derive(Serialize)]
pub struct Output {
	pub build_id: Uuid,
}

pub struct Task;

impl task::Task for Task {
	type Input = Input;
	type Output = Output;

	fn name() -> &'static str {
		"build_publish"
	}

	async fn run(task: task::TaskCtx, input: Self::Input) -> Result<Self::Output> {
		let ctx = crate::toolchain_ctx::load().await?;

		// Check for deno.json or deno.jsonc
		let project_root = paths::project_root()?;
		if project_root.join("deno.json").exists() || project_root.join("deno.jsonc").exists() {
			task.log("[WARNING] deno.json and deno.jsonc are not supported at the moment. Please use package.json with NPM instead.");
		}

		let env = crate::project::environment::get_env(&ctx, input.environment_id).await?;

		// Build
		let build_id = build_and_upload(
			&ctx,
			task.clone(),
			&env,
			input.version_name.clone(),
			input.build_name.clone(),
			input.build_tags.clone(),
			&input.runtime,
			input.skip_upgrade,
		)
		.await?;

		Ok(Output { build_id })
	}
}

/// Builds the required resources and uploads it to Rivet.
///
/// Returns the resulting build ID.
async fn build_and_upload(
	ctx: &ToolchainCtx,
	task: task::TaskCtx,
	env: &TEMPEnvironment,
	version_name: String,
	build_name: String,
	extra_build_tags: Option<HashMap<String, String>>,
	runtime: &Runtime,
	skip_upgrade: bool,
) -> Result<Uuid> {
	task.log("");

	// Build tags
	let mut build_tags = HashMap::from([
		(build::tags::NAME.to_string(), build_name.to_string()),
		(build::tags::VERSION.to_string(), version_name.to_string()),
		(build::tags::CURRENT.to_string(), "true".to_string()),
	]);
	if let Some(extra_build_tags) = extra_build_tags {
		build_tags.extend(extra_build_tags.clone());
	}

	// Build & upload
	let build_id = match &runtime {
		config::build::Runtime::Docker(docker) => {
			docker::build_and_upload(
				&ctx,
				task.clone(),
				docker::BuildAndUploadOpts {
					env: env.clone(),
					tags: build_tags.clone(),
					build_config: docker.clone(),
				},
			)
			.await?
		}
		config::build::Runtime::JavaScript(js) => {
			js::build_and_upload(
				&ctx,
				task.clone(),
				js::BuildAndUploadOpts {
					env: env.clone(),
					tags: build_tags.clone(),
					build_config: js.clone(),
				},
			)
			.await?
		}
	};

	// Find existing builds with current tag
	let list_res = apis::builds_api::builds_list(
		&ctx.openapi_config_cloud,
		Some(&ctx.project.name_id),
		Some(&env.slug),
		Some(&serde_json::to_string(&json!({
			build::tags::NAME: build_name,
			build::tags::CURRENT: "true",
		}))?),
	)
	.await?;

	// Remove current tag if needed
	for build in list_res.builds {
		apis::builds_api::builds_patch_tags(
			&ctx.openapi_config_cloud,
			&build.id.to_string(),
			models::BuildsPatchBuildTagsRequest {
				tags: Some(serde_json::to_value(&json!({
					build::tags::CURRENT: null
				}))?),
				exclusive_tags: None,
			},
			Some(&ctx.project.name_id),
			Some(&env.slug),
		)
		.await?;
	}

	// Tag build
	let complete_res = apis::builds_api::builds_patch_tags(
		&ctx.openapi_config_cloud,
		&build_id.to_string(),
		models::BuildsPatchBuildTagsRequest {
			tags: Some(serde_json::to_value(&build_tags)?),
			exclusive_tags: None,
		},
		Some(&ctx.project.name_id),
		Some(&env.slug),
	)
	.await;
	if let Err(err) = complete_res.as_ref() {
		task.log(format!("{err:?}"));
	}
	complete_res.context("complete_res")?;

	// Upgrade actors
	if !skip_upgrade {
		task.log(format!("[Upgrading Actors]"));
		let res = apis::actors_api::actors_upgrade_all(
			&ctx.openapi_config_cloud,
			models::ActorsUpgradeAllActorsRequest {
				tags: Some(json!({
					build::tags::NAME: build_name,
				})),
				build: Some(build_id),
				build_tags: None,
			},
			Some(&ctx.project.name_id),
			Some(&env.slug),
		)
		.await?;

		task.log(format!(
			"[Upgraded {} Actor{}]",
			res.count,
			if res.count == 1 { "" } else { "s" }
		));
	} else {
		task.log(format!("[Skipping Actor Upgrade]"));
	}

	let hub_origin = &ctx.bootstrap.origins.hub;
	let project_slug = &ctx.project.name_id;
	let env_slug = &env.slug;
	task.log(format!(
		"[Build Published] {hub_origin}/projects/{project_slug}/environments/{env_slug}/builds",
	));

	Ok(build_id)
}
