use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::*;
use futures_util::stream::StreamExt;
use tokio::{
	fs,
	process::Command,
	sync::{Mutex, Semaphore},
	task::JoinSet,
};

use crate::{
	config::{
		self,
		service::{ComponentClass, RuntimeKind},
	},
	context::{
		BuildContext, BuildOptimization, ProjectContext, RunContext, ServiceBuildPlan,
		ServiceContext,
	},
	dep::{
		self, cargo,
		k8s::gen::{ExecServiceContext, ExecServiceDriver},
	},
	tasks,
	utils::{self},
};

pub async fn up_all(
	ctx: &ProjectContext,
	load_tests: bool,
	build_only: bool,
	skip_deploy: bool,
	skip_config_sync_check: bool,
) -> Result<()> {
	let all_svc_names = ctx
		.all_services()
		.await
		.iter()
		.map(|svc| svc.name())
		.collect::<Vec<_>>();
	up_services(
		ctx,
		&all_svc_names,
		load_tests,
		build_only,
		skip_deploy,
		skip_config_sync_check,
	)
	.await?;

	Ok(())
}

pub async fn up_services<T: AsRef<str>>(
	ctx: &ProjectContext,
	svc_names: &[T],
	load_tests: bool,
	build_only: bool,
	skip_deploy: bool,
	skip_config_sync_check: bool,
) -> Result<Vec<ServiceContext>> {
	let event = utils::telemetry::build_event(ctx, "bolt_up").await?;
	utils::telemetry::capture_event(ctx, event).await?;

	let build_context = BuildContext::Bin {
		optimization: ctx.build_optimization(),
	};

	// Find all matching services
	let all_svcs = ctx.services_with_patterns(svc_names).await;
	ensure!(!all_svcs.is_empty(), "input matched no services");

	// Find all services that are executables
	let all_exec_svcs = all_svcs
		.iter()
		.filter(|svc| svc.config().kind.component_class() == ComponentClass::Executable)
		.filter(|svc| load_tests || !svc.config().service.load_test)
		.cloned()
		.collect::<Vec<_>>();

	ensure!(
		!all_exec_svcs.is_empty(),
		"no services to bring up (to bring up load tests, use the `--load-tests` flag)"
	);

	// Telemetry
	let mut event = utils::telemetry::build_event(ctx, "bolt_up").await?;
	event.insert_prop(
		"svc_names",
		all_exec_svcs.iter().map(|x| x.name()).collect::<Vec<_>>(),
	)?;
	utils::telemetry::capture_event(ctx, event).await?;

	// Generate configs
	tasks::gen::generate_project(ctx, skip_config_sync_check).await;

	eprintln!();
	rivet_term::status::progress("Preparing", format!("{} services", all_exec_svcs.len()));

	// Generate service config
	{
		eprintln!();
		rivet_term::status::progress("Generating", "");
		{
			let mut join_handles = Vec::new();
			let pb = utils::MultiProgress::new(all_exec_svcs.len());
			let all_exec_svcs = Arc::new(Mutex::new(all_exec_svcs.clone()));
			for _ in 0..32 {
				let pb = pb.clone();
				let all_svc = all_exec_svcs.clone();

				let handle = tokio::spawn(async move {
					while let Some(svc_ctx) = {
						let mut lock = all_svc.lock().await;
						let val = (*lock).pop();
						drop(lock);
						val
					} {
						let svc_name = svc_ctx.name();

						pb.insert(&svc_name).await;
						// Generate artifacts
						tasks::artifact::generate_service(&svc_ctx).await;
						// Generate service
						tasks::gen::generate_service(&svc_ctx).await;
						pb.remove(&svc_name).await;
					}
				});

				join_handles.push(handle);
			}
			futures_util::future::try_join_all(join_handles).await?;
			pb.finish();
		}
	}

	// Fetch build plans
	eprintln!();
	rivet_term::status::progress("Planning builds", "");
	let pb = utils::progress_bar(all_exec_svcs.len());
	let all_exec_svcs_with_build_plan = futures_util::stream::iter(all_exec_svcs.clone())
		.map(|svc| {
			let build_context = build_context.clone();
			let pb = pb.clone();

			async move {
				let build_plan = svc.build_plan(&build_context).await.unwrap();
				pb.inc(1);
				(svc, build_plan)
			}
		})
		.buffer_unordered(16)
		.collect::<Vec<_>>()
		.await;
	pb.finish();

	let existing_count = all_exec_svcs_with_build_plan
		.iter()
		.filter(|(_, bp)| matches!(bp, ServiceBuildPlan::ExistingUploadedBuild { .. }))
		.count();

	eprintln!();
	rivet_term::status::progress(
		"Plan",
		format!(
			"{} services to build, {} existing binaries",
			all_exec_svcs_with_build_plan.len() - existing_count,
			existing_count
		),
	);

	// Run batch commands for all given services
	eprintln!();
	rivet_term::status::progress("Building", "(batch)");
	{
		// Build all the Rust modules in parallel
		let rust_svcs = all_exec_svcs_with_build_plan
			.iter()
			.filter(|(svc, build_plan)| match svc.config().runtime {
				RuntimeKind::Rust {} => {
					!matches!(build_plan, ServiceBuildPlan::ExistingUploadedBuild { .. })
				}
				_ => false,
			})
			.map(|(svc, _)| svc);

		// Collect rust services by their workspace root
		let mut svcs_by_workspace = HashMap::new();
		for svc in rust_svcs {
			let workspace = svcs_by_workspace
				.entry(svc.workspace_path())
				.or_insert_with(Vec::new);
			workspace.push(svc.cargo_name().expect("no cargo name"));
		}

		if !svcs_by_workspace.is_empty() {
			// Run build
			cargo::cli::build(
				ctx,
				cargo::cli::BuildOpts {
					build_calls: svcs_by_workspace
						.iter()
						.map(|(workspace_path, svc_names)| cargo::cli::BuildCall {
							path: workspace_path.strip_prefix(ctx.path()).unwrap(),
							bins: svc_names,
						})
						.collect::<Vec<_>>(),
					release: ctx.build_optimization() == BuildOptimization::Release,
					jobs: ctx.config_local().rust.num_jobs,
				},
			)
			.await
			.unwrap();
		}
	}

	if build_only {
		return Ok(all_svcs.iter().cloned().collect());
	}

	let mut upload_join_set = JoinSet::<Result<()>>::new();
	let upload_semaphore = Arc::new(Semaphore::new(4));

	// Login to docker repo for uploading
	match &ctx.ns().cluster.kind {
		config::ns::ClusterKind::SingleNode { .. } => {}
		config::ns::ClusterKind::Distributed { .. } => {
			if let Some((repo, _)) = ctx.ns().docker.repository.split_once('/') {
				let username = ctx
					.read_secret(&["docker", "registry", repo, "write", "username"])
					.await?;
				let password = ctx
					.read_secret(&["docker", "registry", repo, "write", "password"])
					.await?;

				let mut cmd = Command::new("sh");
				cmd.arg("-c").arg(format!(
					"echo {password} | docker login {repo} -u {username} --password-stdin"
				));

				let status = cmd.status().await?;
				ensure!(status.success());
			} else {
				bail!("docker repo must end with a slash");
			};
		}
	}

	// Build exec ctx contexts
	eprintln!();
	rivet_term::status::progress("Building", "(individual)");
	let mut exec_ctxs = Vec::new();
	{
		let pb = utils::progress_bar(all_exec_svcs_with_build_plan.len());
		for (svc_ctx, build_plan) in &all_exec_svcs_with_build_plan {
			pb.set_message(svc_ctx.name());

			// Build the service if needed
			if let ServiceBuildPlan::BuildAndUpload { .. } = &build_plan {
				// Read modified ts
				let svc_path = svc_ctx.path().to_owned();
				let _svc_modified_ts =
					tokio::task::spawn_blocking(move || utils::deep_modified_ts(&svc_path))
						.await
						.unwrap()
						.unwrap();

				// Build service
				build_svc(svc_ctx, &build_context, ctx.build_optimization()).await;

				// Upload build
				upload_join_set.spawn(upload_svc_build(svc_ctx.clone(), upload_semaphore.clone()));
			}

			// Save exec ctx
			exec_ctxs.push(ExecServiceContext {
				svc_ctx: svc_ctx.clone().clone(),
				run_context: RunContext::Service {},
				driver: match &build_plan {
					ServiceBuildPlan::BuildLocally { exec_path } => {
						derive_local_build_driver(svc_ctx, exec_path.clone()).await
					}
					ServiceBuildPlan::ExistingUploadedBuild { image_tag }
					| ServiceBuildPlan::BuildAndUpload { image_tag } => {
						derive_uploaded_svc_driver(svc_ctx, image_tag.clone(), false).await
					}
				},
			});

			pb.inc(1);
		}
		pb.finish_with_message("");
	}

	// Wait for builds to finish uploading
	if !upload_join_set.is_empty() {
		eprintln!();
		rivet_term::status::progress("Waiting for uploads to complete", "");
		utils::join_set_progress(upload_join_set).await?;
	}

	if skip_deploy {
		return Ok(all_svcs.to_vec());
	}

	// Generate Kubernetes deployments
	//
	// We resolve the upstream services after applying Terraform since the services we need to
	// resolve won't exist yet.
	let mut specs = Vec::new();
	{
		eprintln!();
		rivet_term::status::progress("Generating specs", "");

		// Create directory for specs
		fs::create_dir_all(ctx.gen_path().join("kubernetes")).await?;

		let pb = utils::progress_bar(all_exec_svcs.len());
		for exec_ctx in &exec_ctxs {
			pb.set_message(exec_ctx.svc_ctx.name());

			// Save specs
			specs.extend(dep::k8s::gen::gen_svc(exec_ctx).await);

			pb.inc(1);
		}
		pb.finish();
	}

	// Apply specs
	eprintln!();
	rivet_term::status::progress("Applying", "");
	dep::k8s::cli::apply_specs(ctx, specs).await?;

	eprintln!();
	rivet_term::status::success("Finished", "");

	// Return all deployed services.
	Ok(all_svcs.to_vec())
}

async fn upload_svc_build(svc_ctx: ServiceContext, upload_semaphore: Arc<Semaphore>) -> Result<()> {
	let _permit = upload_semaphore.acquire().await?;
	svc_ctx.upload_build().await?;
	Result::Ok(())
}

async fn build_svc(
	svc_ctx: &ServiceContext,
	_build_context: &BuildContext,
	_optimization: BuildOptimization,
) {
	match &svc_ctx.config().runtime {
		RuntimeKind::Rust {} => {
			// Do nothing
		}
		RuntimeKind::CRDB { .. }
		| RuntimeKind::ClickHouse { .. }
		| RuntimeKind::Redis { .. }
		| RuntimeKind::S3 { .. }
		| RuntimeKind::Nats { .. } => {
			unreachable!()
		}
	}
}

async fn derive_local_build_driver(
	svc_ctx: &ServiceContext,
	exec_path: PathBuf,
) -> ExecServiceDriver {
	match &svc_ctx.config().runtime {
		RuntimeKind::Rust {} => ExecServiceDriver::LocalBinaryArtifact {
			// Convert path to be relative to the project root
			exec_path: exec_path
				.strip_prefix(svc_ctx.project().await.cargo_target_dir())
				.expect("rust binary path not inside of project dir")
				.to_owned(),
			args: Vec::new(),
		},
		RuntimeKind::CRDB { .. }
		| RuntimeKind::ClickHouse { .. }
		| RuntimeKind::Redis { .. }
		| RuntimeKind::S3 { .. }
		| RuntimeKind::Nats { .. } => {
			unreachable!()
		}
	}
}

async fn derive_uploaded_svc_driver(
	svc_ctx: &ServiceContext,
	image_tag: String,
	force_pull: bool,
) -> ExecServiceDriver {
	match &svc_ctx.config().runtime {
		RuntimeKind::Rust {} => ExecServiceDriver::Docker {
			image_tag,
			force_pull,
		},
		RuntimeKind::CRDB { .. }
		| RuntimeKind::ClickHouse { .. }
		| RuntimeKind::Redis { .. }
		| RuntimeKind::S3 { .. }
		| RuntimeKind::Nats { .. } => {
			unreachable!()
		}
	}
}
