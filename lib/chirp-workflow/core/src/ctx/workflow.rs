use std::{collections::HashMap, sync::Arc, time::Instant};

use global_error::{GlobalError, GlobalResult};
use serde::{de::DeserializeOwned, Serialize};
use tokio::time::Duration;
use uuid::Uuid;

use crate::{
	activity::ActivityId,
	activity::{Activity, ActivityInput},
	ctx::{ActivityCtx, ListenCtx, MessageCtx},
	db::{DatabaseHandle, PulledWorkflow},
	error::{WorkflowError, WorkflowResult},
	event::Event,
	executable::{closure, AsyncResult, Executable},
	listen::{CustomListener, Listen},
	message::Message,
	metrics,
	registry::RegistryHandle,
	signal::Signal,
	util::{
		self,
		time::{DurationToMillis, TsToMillis},
		GlobalErrorExt, Location,
	},
	worker,
	workflow::{Workflow, WorkflowInput},
};

// Time to delay a workflow from retrying after an error
pub const RETRY_TIMEOUT_MS: usize = 2000;
// Poll interval when polling for signals in-process
const SIGNAL_RETRY: Duration = Duration::from_millis(100);
// Most in-process signal poll tries
const MAX_SIGNAL_RETRIES: usize = 16;
// Poll interval when polling for a sub workflow in-process
pub const SUB_WORKFLOW_RETRY: Duration = Duration::from_millis(150);
// Most in-process sub workflow poll tries
const MAX_SUB_WORKFLOW_RETRIES: usize = 4;
// Retry interval for failed db actions
const DB_ACTION_RETRY: Duration = Duration::from_millis(150);
// Most db action retries
const MAX_DB_ACTION_RETRIES: usize = 5;

// TODO: Use generics to store input instead of a json value
// NOTE: Clonable because of inner arcs
#[derive(Clone)]
pub struct WorkflowCtx {
	workflow_id: Uuid,
	/// Name of the workflow to run in the registry.
	name: String,
	create_ts: i64,
	ts: i64,
	ray_id: Uuid,

	registry: RegistryHandle,
	pub(crate) db: DatabaseHandle,

	conn: rivet_connection::Connection,

	/// All events that have ever been recorded on this workflow.
	/// The reason this type is a hashmap is to allow querying by location.
	event_history: Arc<HashMap<Location, Vec<Event>>>,
	/// Input data passed to this workflow.
	pub(crate) input: Arc<serde_json::Value>,

	root_location: Location,
	location_idx: usize,

	/// If this context is currently in a loop, this is the location of the where the loop started.
	loop_location: Option<Box<[usize]>>,

	msg_ctx: MessageCtx,
}

impl WorkflowCtx {
	pub async fn new(
		registry: RegistryHandle,
		db: DatabaseHandle,
		conn: rivet_connection::Connection,
		workflow: PulledWorkflow,
	) -> GlobalResult<Self> {
		let msg_ctx = MessageCtx::new(&conn, workflow.workflow_id, workflow.ray_id).await?;

		Ok(WorkflowCtx {
			workflow_id: workflow.workflow_id,
			name: workflow.workflow_name,
			create_ts: workflow.create_ts,
			ts: rivet_util::timestamp::now(),

			ray_id: workflow.ray_id,

			registry,
			db,

			conn,

			event_history: Arc::new(workflow.events),
			input: Arc::new(workflow.input),

			root_location: Box::new([]),
			location_idx: 0,
			loop_location: None,

			msg_ctx,
		})
	}

	/// Creates a new workflow run with one more depth in the location. Meant to be implemented and not used
	/// directly in workflows.
	pub fn branch(&mut self) -> Self {
		let branch = WorkflowCtx {
			workflow_id: self.workflow_id,
			name: self.name.clone(),
			create_ts: self.create_ts,
			ts: self.ts,
			ray_id: self.ray_id,

			registry: self.registry.clone(),
			db: self.db.clone(),

			conn: self.conn.clone(),

			event_history: self.event_history.clone(),
			input: self.input.clone(),

			root_location: self
				.root_location
				.iter()
				.cloned()
				.chain(std::iter::once(self.location_idx))
				.collect(),
			location_idx: 0,
			loop_location: self.loop_location.clone(),

			msg_ctx: self.msg_ctx.clone(),
		};

		self.location_idx += 1;

		branch
	}

	/// Like `branch` but it does not add another layer of depth. Meant to be implemented and not used
	/// directly in workflows.
	pub fn step(&mut self) -> Self {
		let branch = self.clone();

		self.location_idx += 1;

		branch
	}

	/// Returns only the history relevant to this workflow run (based on location).
	fn relevant_history(&self) -> impl Iterator<Item = &Event> {
		self.event_history
			.get(&self.root_location)
			// `into_iter` and `flatten` are for the `Option`
			.into_iter()
			.flatten()
	}

	pub(crate) fn full_location(&self) -> Location {
		self.root_location
			.iter()
			.cloned()
			.chain(std::iter::once(self.location_idx))
			.collect()
	}

	/// For debugging, pretty prints the current location
	fn loc(&self) -> String {
		util::format_location(&self.full_location())
	}

	pub(crate) fn loop_location(&self) -> Option<&[usize]> {
		self.loop_location.as_deref()
	}

	// Purposefully infallible
	pub(crate) async fn run(mut self) {
		if let Err(err) = Self::run_inner(&mut self).await {
			tracing::error!(?err, "unhandled error");
		}
	}

	async fn run_inner(&mut self) -> WorkflowResult<()> {
		tracing::info!(name=%self.name, id=%self.workflow_id, "running workflow");

		// Lookup workflow
		let workflow = self.registry.get_workflow(&self.name)?;

		// Run workflow
		match (workflow.run)(self).await {
			Ok(output) => {
				tracing::info!(name=%self.name, id=%self.workflow_id, "workflow success");

				let mut retries = 0;
				let mut interval = tokio::time::interval(DB_ACTION_RETRY);
				interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

				// Retry loop
				loop {
					interval.tick().await;

					// Write output
					if let Err(err) = self.db.commit_workflow(self.workflow_id, &output).await {
						if retries > MAX_DB_ACTION_RETRIES {
							return Err(err.into());
						}
						retries += 1;
					} else {
						break;
					}
				}
			}
			Err(err) => {
				// Retry the workflow if its recoverable
				let deadline_ts = if let Some(deadline_ts) = err.deadline_ts() {
					Some(deadline_ts)
				} else if err.is_retryable() {
					Some(rivet_util::timestamp::now() + RETRY_TIMEOUT_MS as i64)
				} else {
					None
				};

				// These signals come from a `listen` call that did not receive any signals. The workflow will
				// be retried when a signal is published
				let wake_signals = err.signals();

				// This sub workflow comes from a `wait_for_workflow` call on a workflow that did not
				// finish. This workflow will be retried when the sub workflow completes
				let wake_sub_workflow = err.sub_workflow();

				if err.is_recoverable() && !err.is_retryable() {
					tracing::info!(name=%self.name, id=%self.workflow_id, ?err, "workflow sleeping");
				} else {
					tracing::error!(name=%self.name, id=%self.workflow_id, ?err, "workflow error");

					metrics::WORKFLOW_ERRORS
						.with_label_values(&[&self.name, err.to_string().as_str()])
						.inc();
				}

				let err_str = err.to_string();

				let mut retries = 0;
				let mut interval = tokio::time::interval(DB_ACTION_RETRY);
				interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

				// Retry loop
				loop {
					interval.tick().await;

					// Write output
					let res = self
						.db
						.fail_workflow(
							self.workflow_id,
							false,
							deadline_ts,
							wake_signals,
							wake_sub_workflow,
							&err_str,
						)
						.await;

					if let Err(err) = res {
						if retries > MAX_DB_ACTION_RETRIES {
							return Err(err.into());
						}
						retries += 1;
					} else {
						break;
					}
				}
			}
		}

		Ok(())
	}

	/// Run then handle the result of an activity.
	async fn run_activity<A: Activity>(
		&mut self,
		input: &A::Input,
		activity_id: &ActivityId,
		create_ts: i64,
	) -> WorkflowResult<A::Output> {
		tracing::debug!(name=%self.name, id=%self.workflow_id, activity_name=%A::NAME, "running activity");

		let ctx = ActivityCtx::new(
			self.workflow_id,
			self.db.clone(),
			&self.conn,
			self.create_ts,
			self.ray_id,
			A::NAME,
		);

		let start_instant = Instant::now();

		let res = tokio::time::timeout(A::TIMEOUT, A::run(&ctx, input))
			.await
			.map_err(|_| WorkflowError::ActivityTimeout);

		let dt = start_instant.elapsed().as_secs_f64();

		match res {
			Ok(Ok(output)) => {
				tracing::debug!("activity success");

				// Write output
				let input_val =
					serde_json::to_value(input).map_err(WorkflowError::SerializeActivityInput)?;
				let output_val = serde_json::to_value(&output)
					.map_err(WorkflowError::SerializeActivityOutput)?;
				self.db
					.commit_workflow_activity_event(
						self.workflow_id,
						self.full_location().as_ref(),
						activity_id,
						create_ts,
						input_val,
						Ok(output_val),
						self.loop_location(),
					)
					.await?;

				metrics::ACTIVITY_DURATION
					.with_label_values(&[&self.name, A::NAME, ""])
					.observe(dt);

				Ok(output)
			}
			Ok(Err(err)) => {
				tracing::debug!(?err, "activity error");

				let err_str = err.to_string();
				let input_val =
					serde_json::to_value(input).map_err(WorkflowError::SerializeActivityInput)?;

				// Write error (failed state)
				self.db
					.commit_workflow_activity_event(
						self.workflow_id,
						self.full_location().as_ref(),
						activity_id,
						create_ts,
						input_val,
						Err(&err_str),
						self.loop_location(),
					)
					.await?;

				if !err.is_workflow_recoverable() {
					metrics::ACTIVITY_ERRORS
						.with_label_values(&[&self.name, A::NAME, &err_str])
						.inc();
				}
				metrics::ACTIVITY_DURATION
					.with_label_values(&[&self.name, A::NAME, &err_str])
					.observe(dt);

				Err(WorkflowError::ActivityFailure(err, 0))
			}
			Err(err) => {
				tracing::debug!("activity timeout");

				let err_str = err.to_string();
				let input_val =
					serde_json::to_value(input).map_err(WorkflowError::SerializeActivityInput)?;

				self.db
					.commit_workflow_activity_event(
						self.workflow_id,
						self.full_location().as_ref(),
						activity_id,
						create_ts,
						input_val,
						Err(&err_str),
						self.loop_location(),
					)
					.await?;

				metrics::ACTIVITY_ERRORS
					.with_label_values(&[&self.name, A::NAME, &err_str])
					.inc();
				metrics::ACTIVITY_DURATION
					.with_label_values(&[&self.name, A::NAME, &err_str])
					.observe(dt);

				Err(err)
			}
		}
	}
}

impl WorkflowCtx {
	/// Dispatch another workflow.
	pub async fn dispatch_workflow<I>(&mut self, input: I) -> GlobalResult<Uuid>
	where
		I: WorkflowInput,
		<I as WorkflowInput>::Workflow: Workflow<Input = I>,
	{
		self.dispatch_workflow_inner(None, input).await
	}

	/// Dispatch another workflow with tags.
	pub async fn dispatch_tagged_workflow<I>(
		&mut self,
		tags: &serde_json::Value,
		input: I,
	) -> GlobalResult<Uuid>
	where
		I: WorkflowInput,
		<I as WorkflowInput>::Workflow: Workflow<Input = I>,
	{
		self.dispatch_workflow_inner(Some(tags), input).await
	}

	async fn dispatch_workflow_inner<I>(
		&mut self,
		tags: Option<&serde_json::Value>,
		input: I,
	) -> GlobalResult<Uuid>
	where
		I: WorkflowInput,
		<I as WorkflowInput>::Workflow: Workflow<Input = I>,
	{
		let event = self.relevant_history().nth(self.location_idx);

		// Signal received before
		let id = if let Some(event) = event {
			// Validate history is consistent
			let Event::SubWorkflow(sub_workflow) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found sub workflow {}",
					self.loc(),
					I::Workflow::NAME
				)))
				.map_err(GlobalError::raw);
			};

			if sub_workflow.name != I::Workflow::NAME {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found sub_workflow {}",
					self.loc(),
					I::Workflow::NAME
				)))
				.map_err(GlobalError::raw);
			}

			tracing::debug!(
				name=%self.name,
				id=%self.workflow_id,
				sub_workflow_name=%sub_workflow.name,
				sub_workflow_id=%sub_workflow.sub_workflow_id,
				"replaying workflow dispatch"
			);

			sub_workflow.sub_workflow_id
		}
		// Dispatch new workflow
		else {
			let sub_workflow_name = I::Workflow::NAME;
			let sub_workflow_id = Uuid::new_v4();

			tracing::info!(
				name=%self.name,
				id=%self.workflow_id,
				%sub_workflow_name,
				%sub_workflow_id,
				?tags,
				?input,
				"dispatching sub workflow"
			);

			// Serialize input
			let input_val = serde_json::to_value(input)
				.map_err(WorkflowError::SerializeWorkflowOutput)
				.map_err(GlobalError::raw)?;

			self.db
				.dispatch_sub_workflow(
					self.ray_id,
					self.workflow_id,
					self.full_location().as_ref(),
					sub_workflow_id,
					&sub_workflow_name,
					tags,
					input_val,
					self.loop_location(),
				)
				.await
				.map_err(GlobalError::raw)?;

			tracing::info!(
				name=%self.name,
				id=%self.workflow_id,
				%sub_workflow_name,
				?sub_workflow_id,
				"workflow dispatched"
			);

			sub_workflow_id
		};

		// Move to next event
		self.location_idx += 1;

		Ok(id)
	}

	/// Wait for another workflow's response. If no response was found after polling the database, this
	/// workflow will go to sleep until the sub workflow completes.
	pub async fn wait_for_workflow<W: Workflow>(
		&self,
		sub_workflow_id: Uuid,
	) -> GlobalResult<W::Output> {
		tracing::info!(name=%self.name, id=%self.workflow_id, sub_workflow_name=%W::NAME, ?sub_workflow_id, "waiting for workflow");

		let mut retries = 0;
		let mut interval = tokio::time::interval(SUB_WORKFLOW_RETRY);
		interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

		loop {
			interval.tick().await;

			// Check if state finished
			let workflow = self
				.db
				.get_workflow(sub_workflow_id)
				.await
				.map_err(GlobalError::raw)?
				.ok_or(WorkflowError::WorkflowNotFound)
				.map_err(GlobalError::raw)?;

			if let Some(output) = workflow.parse_output::<W>().map_err(GlobalError::raw)? {
				return Ok(output);
			} else {
				if retries > MAX_SUB_WORKFLOW_RETRIES {
					return Err(WorkflowError::SubWorkflowIncomplete(sub_workflow_id))
						.map_err(GlobalError::raw);
				}
				retries += 1;
			}
		}
	}

	/// Runs a sub workflow in the same process as the current workflow (if possible) and returns its
	/// response.
	pub async fn workflow<I>(
		&mut self,
		input: I,
	) -> GlobalResult<<<I as WorkflowInput>::Workflow as Workflow>::Output>
	where
		I: WorkflowInput,
		<I as WorkflowInput>::Workflow: Workflow<Input = I>,
	{
		self.workflow_inner(None, input).await
	}

	/// Runs a sub workflow with tags in the same process as the current workflow (if possible) and returns
	/// its response.
	pub async fn tagged_workflow<I>(
		&mut self,
		input: I,
	) -> GlobalResult<<<I as WorkflowInput>::Workflow as Workflow>::Output>
	where
		I: WorkflowInput,
		<I as WorkflowInput>::Workflow: Workflow<Input = I>,
	{
		self.workflow_inner(None, input).await
	}

	async fn workflow_inner<I>(
		&mut self,
		tags: Option<&serde_json::Value>,
		input: I,
	) -> GlobalResult<<<I as WorkflowInput>::Workflow as Workflow>::Output>
	where
		I: WorkflowInput,
		<I as WorkflowInput>::Workflow: Workflow<Input = I>,
	{
		// Lookup workflow
		let Ok(workflow) = self.registry.get_workflow(I::Workflow::NAME) else {
			tracing::warn!(
				name=%self.name,
				id=%self.workflow_id,
				sub_workflow_name=%I::Workflow::NAME,
				"sub workflow not found in current registry",
			);

			// TODO(RVT-3755): If a sub workflow is dispatched, then the worker is updated to include the sub
			// worker in the registry, this will diverge in history because it will try to run the sub worker
			// in-process during the replay
			// If the workflow isn't in the current registry, dispatch the workflow instead
			let sub_workflow_id = self.dispatch_workflow_inner(tags, input).await?;
			let output = self
				.wait_for_workflow::<I::Workflow>(sub_workflow_id)
				.await?;

			return Ok(output);
		};

		tracing::info!(name=%self.name, id=%self.workflow_id, sub_workflow_name=%I::Workflow::NAME, "running sub workflow");

		// Create a new branched workflow context for the sub workflow
		let mut ctx = WorkflowCtx {
			workflow_id: self.workflow_id,
			name: I::Workflow::NAME.to_string(),
			create_ts: rivet_util::timestamp::now(),
			ts: rivet_util::timestamp::now(),
			ray_id: self.ray_id,

			registry: self.registry.clone(),
			db: self.db.clone(),

			conn: self
				.conn
				.wrap(Uuid::new_v4(), self.ray_id, I::Workflow::NAME),

			event_history: self.event_history.clone(),

			// TODO(RVT-3756): This is redundant with the deserialization in `workflow.run` in the registry
			input: Arc::new(serde_json::to_value(input)?),

			root_location: self
				.root_location
				.iter()
				.cloned()
				.chain(std::iter::once(self.location_idx))
				.collect(),
			location_idx: 0,
			loop_location: self.loop_location.clone(),

			msg_ctx: self.msg_ctx.clone(),
		};

		self.location_idx += 1;

		// Run workflow
		let output = (workflow.run)(&mut ctx).await.map_err(GlobalError::raw)?;

		// TODO: RVT-3756
		// Deserialize output
		serde_json::from_value(output)
			.map_err(WorkflowError::DeserializeWorkflowOutput)
			.map_err(GlobalError::raw)
	}

	/// Run activity. Will replay on failure.
	pub async fn activity<I>(
		&mut self,
		input: I,
	) -> GlobalResult<<<I as ActivityInput>::Activity as Activity>::Output>
	where
		I: ActivityInput,
		<I as ActivityInput>::Activity: Activity<Input = I>,
	{
		let activity_id = ActivityId::new::<I::Activity>(&input);

		let event = self.relevant_history().nth(self.location_idx);

		// Activity was ran before
		let output = if let Some(event) = event {
			// Validate history is consistent
			let Event::Activity(activity) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found activity {}",
					self.loc(),
					activity_id.name
				)))
				.map_err(GlobalError::raw);
			};

			if activity.activity_id != activity_id {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected activity {}#{:x} at {}, found activity {}#{:x}",
					activity.activity_id.name,
					activity.activity_id.input_hash,
					self.loc(),
					activity_id.name,
					activity_id.input_hash,
				)))
				.map_err(GlobalError::raw);
			}

			// Activity succeeded
			if let Some(output) = activity.parse_output().map_err(GlobalError::raw)? {
				tracing::debug!(name=%self.name, id=%self.workflow_id, activity_name=%I::Activity::NAME, "replaying activity");

				output
			}
			// Activity failed, retry
			else {
				let error_count = activity.error_count;

				match self
					.run_activity::<I::Activity>(&input, &activity_id, activity.create_ts)
					.await
				{
					Err(err) => {
						// Convert error in the case of max retries exceeded. This will only act on retryable
						// errors
						let err = match err {
							WorkflowError::ActivityFailure(err, _) => {
								if error_count + 1 >= I::Activity::MAX_RETRIES {
									WorkflowError::ActivityMaxFailuresReached(err)
								} else {
									// Add error count to the error
									WorkflowError::ActivityFailure(err, error_count)
								}
							}
							WorkflowError::ActivityTimeout | WorkflowError::OperationTimeout => {
								if error_count + 1 >= I::Activity::MAX_RETRIES {
									WorkflowError::ActivityMaxFailuresReached(GlobalError::raw(err))
								} else {
									err
								}
							}
							_ => err,
						};

						return Err(GlobalError::raw(err));
					}
					x => x.map_err(GlobalError::raw)?,
				}
			}
		}
		// This is a new activity
		else {
			self.run_activity::<I::Activity>(&input, &activity_id, rivet_util::timestamp::now())
				.await
				.map_err(GlobalError::raw)?
		};

		// Move to next event
		self.location_idx += 1;

		Ok(output)
	}

	/// Joins multiple executable actions (activities, closures) and awaits them simultaneously.
	pub async fn join<T: Executable>(&mut self, exec: T) -> GlobalResult<T::Output> {
		exec.execute(self).await
	}

	/// Spawns a new thread to execute workflow steps in.
	pub fn spawn<F, T: Send + 'static>(&mut self, f: F) -> tokio::task::JoinHandle<GlobalResult<T>>
	where
		F: for<'a> FnOnce(&'a mut WorkflowCtx) -> AsyncResult<'a, T> + Send + 'static,
	{
		let mut ctx = self.clone();
		tokio::task::spawn(async move { closure(f).execute(&mut ctx).await })
	}

	/// Tests if the given error is unrecoverable. If it is, allows the user to run recovery code safely.
	/// Should always be used when trying to handle activity errors manually.
	pub fn catch_unrecoverable<T>(
		&mut self,
		res: GlobalResult<T>,
	) -> GlobalResult<GlobalResult<T>> {
		match res {
			Err(GlobalError::Raw(inner_err)) => {
				match inner_err.downcast::<WorkflowError>() {
					Ok(inner_err) => {
						// Despite "history diverged" errors being unrecoverable, they should not have be returned
						// by this function because the state of the history is already messed up and no new
						// workflow items can be run.
						if !inner_err.is_recoverable()
							&& !matches!(*inner_err, WorkflowError::HistoryDiverged(_))
						{
							self.location_idx += 1;

							return Ok(Err(GlobalError::Raw(inner_err)));
						} else {
							return Err(GlobalError::Raw(inner_err));
						}
					}
					Err(err) => {
						return Err(GlobalError::Raw(err));
					}
				}
			}
			Err(err) => Err(err),
			Ok(x) => Ok(Ok(x)),
		}
	}

	/// Sends a signal.
	pub async fn signal<T: Signal + Serialize>(
		&mut self,
		workflow_id: Uuid,
		body: T,
	) -> GlobalResult<Uuid> {
		let event = self.relevant_history().nth(self.location_idx);

		// Signal sent before
		let signal_id = if let Some(event) = event {
			// Validate history is consistent
			let Event::SignalSend(signal) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found signal send {}",
					self.loc(),
					T::NAME
				)))
				.map_err(GlobalError::raw);
			};

			if signal.name != T::NAME {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found signal send {}",
					self.loc(),
					T::NAME
				)))
				.map_err(GlobalError::raw);
			}

			tracing::debug!(name=%self.name, id=%self.workflow_id, signal_name=%signal.name, signal_id=%signal.signal_id, "replaying signal dispatch");

			signal.signal_id
		}
		// Send signal
		else {
			let signal_id = Uuid::new_v4();
			tracing::info!(name=%self.name, id=%self.workflow_id, signal_name=%T::NAME, to_workflow_id=%workflow_id, %signal_id, "dispatching signal");

			// Serialize input
			let input_val = serde_json::to_value(&body)
				.map_err(WorkflowError::SerializeSignalBody)
				.map_err(GlobalError::raw)?;

			self.db
				.publish_signal_from_workflow(
					self.workflow_id,
					self.full_location().as_ref(),
					self.ray_id,
					workflow_id,
					signal_id,
					T::NAME,
					input_val,
					self.loop_location(),
				)
				.await
				.map_err(GlobalError::raw)?;

			signal_id
		};

		// Move to next event
		self.location_idx += 1;

		Ok(signal_id)
	}

	/// Sends a tagged signal.
	pub async fn tagged_signal<T: Signal + Serialize>(
		&mut self,
		tags: &serde_json::Value,
		body: T,
	) -> GlobalResult<Uuid> {
		let event = self.relevant_history().nth(self.location_idx);

		// Signal sent before
		let signal_id = if let Some(event) = event {
			// Validate history is consistent
			let Event::SignalSend(signal) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found signal send {}",
					self.loc(),
					T::NAME
				)))
				.map_err(GlobalError::raw);
			};

			if signal.name != T::NAME {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found signal send {}",
					self.loc(),
					T::NAME
				)))
				.map_err(GlobalError::raw);
			}

			tracing::debug!(name=%self.name, id=%self.workflow_id, signal_name=%signal.name, signal_id=%signal.signal_id, "replaying tagged signal dispatch");

			signal.signal_id
		}
		// Send signal
		else {
			let signal_id = Uuid::new_v4();

			tracing::info!(name=%self.name, id=%self.workflow_id, signal_name=%T::NAME, ?tags, %signal_id, "dispatching tagged signal");

			// Serialize input
			let input_val = serde_json::to_value(&body)
				.map_err(WorkflowError::SerializeSignalBody)
				.map_err(GlobalError::raw)?;

			self.db
				.publish_tagged_signal_from_workflow(
					self.workflow_id,
					self.full_location().as_ref(),
					self.ray_id,
					tags,
					signal_id,
					T::NAME,
					input_val,
					self.loop_location(),
				)
				.await
				.map_err(GlobalError::raw)?;

			signal_id
		};

		// Move to next event
		self.location_idx += 1;

		Ok(signal_id)
	}

	/// Listens for a signal for a short time before setting the workflow to sleep. Once the signal is
	/// received, the workflow will be woken up and continue.
	pub async fn listen<T: Listen>(&mut self) -> GlobalResult<T> {
		let event = self.relevant_history().nth(self.location_idx);

		// Signal received before
		let signal = if let Some(event) = event {
			// Validate history is consistent
			let Event::Signal(signal) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found signal",
					self.loc(),
				)))
				.map_err(GlobalError::raw);
			};

			tracing::debug!(name=%self.name, id=%self.workflow_id, signal_name=%signal.name, "replaying signal");

			T::parse(&signal.name, signal.body.clone()).map_err(GlobalError::raw)?
		}
		// Listen for new messages
		else {
			tracing::info!(name=%self.name, id=%self.workflow_id, "listening for signal");

			let mut retries = 0;
			let mut interval = tokio::time::interval(SIGNAL_RETRY);
			interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			let ctx = ListenCtx::new(self);

			loop {
				interval.tick().await;

				match T::listen(&ctx).await {
					Ok(res) => break res,
					Err(err) if matches!(err, WorkflowError::NoSignalFound(_)) => {
						if retries > MAX_SIGNAL_RETRIES {
							return Err(err).map_err(GlobalError::raw);
						}
						retries += 1;
					}
					err => return err.map_err(GlobalError::raw),
				}
			}
		};

		// Move to next event
		self.location_idx += 1;

		Ok(signal)
	}

	/// Execute a custom listener.
	pub async fn custom_listener<T: CustomListener>(
		&mut self,
		listener: &T,
	) -> GlobalResult<<T as CustomListener>::Output> {
		let event = self.relevant_history().nth(self.location_idx);

		// Signal received before
		let signal = if let Some(event) = event {
			// Validate history is consistent
			let Event::Signal(signal) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found signal",
					self.loc(),
				)))
				.map_err(GlobalError::raw);
			};

			tracing::debug!(name=%self.name, id=%self.workflow_id, signal_name=%signal.name, "replaying signal");

			T::parse(&signal.name, signal.body.clone()).map_err(GlobalError::raw)?
		}
		// Listen for new messages
		else {
			tracing::info!(name=%self.name, id=%self.workflow_id, "listening for signal");

			let mut retries = 0;
			let mut interval = tokio::time::interval(SIGNAL_RETRY);
			interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			let ctx = ListenCtx::new(self);

			loop {
				interval.tick().await;

				match listener.listen(&ctx).await {
					Ok(res) => break res,
					Err(err) if matches!(err, WorkflowError::NoSignalFound(_)) => {
						if retries > MAX_SIGNAL_RETRIES {
							return Err(err).map_err(GlobalError::raw);
						}
						retries += 1;
					}
					err => return err.map_err(GlobalError::raw),
				}
			}
		};

		// Move to next event
		self.location_idx += 1;

		Ok(signal)
	}

	// TODO: Currently implemented wrong, if no signal is received it should still write a signal row to the
	// database so that upon replay it again receives no signal
	// /// Checks if the given signal exists in the database.
	// pub async fn query_signal<T: Listen>(&mut self) -> GlobalResult<Option<T>> {
	// 	let event = self.relevant_history().nth(self.location_idx);

	// 	// Signal received before
	// 	let signal = if let Some(event) = event {
	// 		tracing::debug!(name=%self.name, id=%self.workflow_id, "replaying signal");

	// 		// Validate history is consistent
	// 		let Event::Signal(signal) = event else {
	// 			return Err(WorkflowError::HistoryDiverged(format!(
	// 				"expected {event} at {}, found signal",
	// 				self.loc(),
	// 			)))
	// 			.map_err(GlobalError::raw);
	// 		};

	// 		Some(T::parse(&signal.name, signal.body.clone()).map_err(GlobalError::raw)?)
	// 	}
	// 	// Listen for new message
	// 	else {
	// 		let ctx = ListenCtx::new(self);

	// 		match T::listen(&ctx).await {
	// 			Ok(res) => Some(res),
	// 			Err(err) if matches!(err, WorkflowError::NoSignalFound(_)) => None,
	// 			Err(err) => return Err(err).map_err(GlobalError::raw),
	// 		}
	// 	};

	// 	// Move to next event
	// 	self.location_idx += 1;

	// 	Ok(signal)
	// }

	pub async fn msg<M>(&mut self, tags: serde_json::Value, body: M) -> GlobalResult<()>
	where
		M: Message,
	{
		let event = self.relevant_history().nth(self.location_idx);

		// Message sent before
		if let Some(event) = event {
			// Validate history is consistent
			let Event::MessageSend(msg) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found message send {}",
					self.loc(),
					M::NAME,
				)))
				.map_err(GlobalError::raw);
			};

			if msg.name != M::NAME {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found message send {}",
					self.loc(),
					M::NAME,
				)))
				.map_err(GlobalError::raw);
			}

			tracing::debug!(name=%self.name, id=%self.workflow_id, msg_name=%msg.name, "replaying message dispatch");
		}
		// Send message
		else {
			tracing::info!(name=%self.name, id=%self.workflow_id, msg_name=%M::NAME, ?tags, "dispatching message");

			// Serialize body
			let body_val = serde_json::to_value(&body)
				.map_err(WorkflowError::SerializeWorkflowOutput)
				.map_err(GlobalError::raw)?;
			let location = self.full_location();

			let (msg, write) = tokio::join!(
				self.db.commit_workflow_message_send_event(
					self.workflow_id,
					location.as_ref(),
					&tags,
					M::NAME,
					body_val,
					self.loop_location(),
				),
				self.msg_ctx.message(tags.clone(), body),
			);

			msg.map_err(GlobalError::raw)?;
			write.map_err(GlobalError::raw)?;
		}

		// Move to next event
		self.location_idx += 1;

		Ok(())
	}

	pub async fn msg_wait<M>(&mut self, tags: serde_json::Value, body: M) -> GlobalResult<()>
	where
		M: Message,
	{
		let event = self.relevant_history().nth(self.location_idx);

		// Message sent before
		if let Some(event) = event {
			// Validate history is consistent
			let Event::MessageSend(msg) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found message send {}",
					self.loc(),
					M::NAME,
				)))
				.map_err(GlobalError::raw);
			};

			if msg.name != M::NAME {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found message send {}",
					self.loc(),
					M::NAME,
				)))
				.map_err(GlobalError::raw);
			}

			tracing::debug!(name=%self.name, id=%self.workflow_id, msg_name=%msg.name, "replaying message dispatch");
		}
		// Send message
		else {
			tracing::info!(name=%self.name, id=%self.workflow_id, msg_name=%M::NAME, ?tags, "dispatching message");

			// Serialize body
			let body_val = serde_json::to_value(&body)
				.map_err(WorkflowError::SerializeWorkflowOutput)
				.map_err(GlobalError::raw)?;
			let location = self.full_location();

			let (msg, write) = tokio::join!(
				self.db.commit_workflow_message_send_event(
					self.workflow_id,
					location.as_ref(),
					&tags,
					M::NAME,
					body_val,
					self.loop_location(),
				),
				self.msg_ctx.message_wait(tags.clone(), body),
			);

			msg.map_err(GlobalError::raw)?;
			write.map_err(GlobalError::raw)?;
		}

		// Move to next event
		self.location_idx += 1;

		Ok(())
	}

	/// Runs workflow steps in a loop. **Ensure that there are no side effects caused by the code in this
	/// callback**. If you need side causes or side effects, use a native rust loop.
	pub async fn repeat<F, T>(&mut self, mut cb: F) -> GlobalResult<T>
	where
		F: for<'a> FnMut(&'a mut WorkflowCtx) -> AsyncResult<'a, Loop<T>>,
		T: Serialize + DeserializeOwned,
	{
		let event_location = self.location_idx;
		let loop_location = self.full_location();
		let mut loop_branch = self.branch();

		let event = self.relevant_history().nth(event_location);

		// Loop existed before
		let output = if let Some(event) = event {
			// Validate history is consistent
			let Event::Loop(loop_event) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found loop",
					self.loc(),
				)))
				.map_err(GlobalError::raw);
			};

			let output = loop_event.parse_output().map_err(GlobalError::raw)?;

			// Shift by iteration count
			loop_branch.location_idx = loop_event.iteration;

			output
		} else {
			None
		};

		// Loop complete
		let output = if let Some(output) = output {
			tracing::debug!(name=%self.name, id=%self.workflow_id, "replaying loop");

			output
		}
		// Run loop
		else {
			tracing::info!(name=%self.name, id=%self.workflow_id, "running loop");

			loop {
				let mut iteration_branch = loop_branch.branch();
				iteration_branch.loop_location = Some(loop_location.clone());

				match cb(&mut iteration_branch).await? {
					Loop::Continue => {
						self.db
							.update_loop(
								self.workflow_id,
								loop_location.as_ref(),
								loop_branch.location_idx,
								None,
								self.loop_location(),
							)
							.await?;
					}
					Loop::Break(res) => {
						let output_val = serde_json::to_value(&res)
							.map_err(WorkflowError::SerializeLoopOutput)
							.map_err(GlobalError::raw)?;

						self.db
							.update_loop(
								self.workflow_id,
								loop_location.as_ref(),
								loop_branch.location_idx,
								Some(output_val),
								self.loop_location(),
							)
							.await?;

						break res;
					}
				}
			}
		};

		Ok(output)
	}

	pub async fn sleep<T: DurationToMillis>(&mut self, duration: T) -> GlobalResult<()> {
		let ts = rivet_util::timestamp::now() as u64 + duration.to_millis()?;

		self.sleep_until(ts as i64)
			.await
	}

	pub async fn sleep_until<T: TsToMillis>(&mut self, time: T) -> GlobalResult<()> {
		let event = self.relevant_history().nth(self.location_idx);

		// Slept before
		let (deadline_ts, replay) = if let Some(event) = event {
			// Validate history is consistent
			let Event::Sleep(sleep) = event else {
				return Err(WorkflowError::HistoryDiverged(format!(
					"expected {event} at {}, found sleep",
					self.loc(),
				)))
				.map_err(GlobalError::raw);
			};

			tracing::debug!(name=%self.name, id=%self.workflow_id, "replaying sleep");

			(sleep.deadline_ts, true)
		}
		// Sleep
		else {
			let deadline_ts = time.to_millis()?;

			self.db
				.commit_workflow_sleep_event(
					self.workflow_id,
					self.full_location().as_ref(),
					deadline_ts,
					self.loop_location(),
				)
				.await?;

			(deadline_ts, false)
		};

		let duration = deadline_ts.saturating_sub(rivet_util::timestamp::now());

		// No-op
		if duration < 0 {
			if !replay {
				tracing::warn!("tried to sleep for a negative duration");
			}
		} else if duration < worker::TICK_INTERVAL.as_millis() as i64 + 1 {
			tracing::info!(name=%self.name, id=%self.workflow_id, %deadline_ts, "sleeping in memory");

			// Sleep in memory if duration is shorter than the worker tick
			tokio::time::sleep(std::time::Duration::from_millis(duration.try_into()?)).await;
		} else {
			tracing::info!(name=%self.name, id=%self.workflow_id, %deadline_ts, "sleeping");

			return Err(WorkflowError::Sleep(deadline_ts)).map_err(GlobalError::raw);
		}

		// Move to next event
		self.location_idx += 1;

		Ok(())
	}
}

impl WorkflowCtx {
	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn workflow_id(&self) -> Uuid {
		self.workflow_id
	}

	/// Timestamp at which this workflow run started.
	pub fn ts(&self) -> i64 {
		self.ts
	}

	/// Timestamp at which the workflow was created.
	pub fn create_ts(&self) -> i64 {
		self.create_ts
	}

	/// Time between when the timestamp was processed and when it was published.
	pub fn req_dt(&self) -> i64 {
		self.ts.saturating_sub(self.create_ts)
	}
}

pub enum Loop<T> {
	Continue,
	Break(T),
}
