use std::{fmt::Display, marker::PhantomData, time::Instant};

use global_error::{GlobalError, GlobalResult};
use serde::Serialize;
use tracing::Instrument;
use uuid::Uuid;

use crate::{
	builder::{BuilderError, WorkflowRepr},
	ctx::common,
	db::{DatabaseHandle, WorkflowData},
	error::WorkflowError,
	metrics,
	workflow::{Workflow, WorkflowInput},
};

pub struct WorkflowBuilder<T, I: WorkflowInput> {
	db: DatabaseHandle,
	ray_id: Uuid,
	repr: T,
	tags: serde_json::Map<String, serde_json::Value>,
	unique: bool,
	error: Option<BuilderError>,
	_marker: PhantomData<I>,
}

impl<T, I> WorkflowBuilder<T, I>
where
	T: WorkflowRepr<I>,
	I: WorkflowInput,
	<I as WorkflowInput>::Workflow: Workflow<Input = I>,
{
	pub(crate) fn new(db: DatabaseHandle, ray_id: Uuid, repr: T, from_workflow: bool) -> Self {
		WorkflowBuilder {
			db,
			ray_id,
			repr,
			tags: serde_json::Map::new(),
			unique: false,
			error: from_workflow.then_some(BuilderError::CannotDispatchFromOpInWorkflow),
			_marker: PhantomData,
		}
	}

	pub fn tags(mut self, tags: serde_json::Value) -> Self {
		if self.error.is_some() {
			return self;
		}

		match tags {
			serde_json::Value::Object(map) => {
				self.tags.extend(map);
			}
			_ => self.error = Some(BuilderError::TagsNotMap),
		}

		self
	}

	pub fn tag(mut self, k: impl Display, v: impl Serialize) -> Self {
		if self.error.is_some() {
			return self;
		}

		match serde_json::to_value(&v) {
			Ok(v) => {
				self.tags.insert(k.to_string(), v);
			}
			Err(err) => self.error = Some(err.into()),
		}

		self
	}

	/// Does not dispatch a workflow if one already exists with the given name and tags. Has no effect if no
	/// tags are provided (will always spawn a new workflow).
	pub fn unique(mut self) -> Self {
		if self.error.is_some() {
			return self;
		}

		self.unique = true;

		self
	}

	#[tracing::instrument(skip_all, fields(workflow_name=I::Workflow::NAME, workflow_id, unique=self.unique))]
	pub async fn dispatch(self) -> GlobalResult<Uuid> {
		if let Some(err) = self.error {
			return Err(err.into());
		}

		let workflow_name = I::Workflow::NAME;
		let workflow_id = Uuid::new_v4();
		let start_instant = Instant::now();
		let input = self.repr.as_input()?;

		let no_tags = self.tags.is_empty();
		let tags = serde_json::Value::Object(self.tags);
		let tags = if no_tags { None } else { Some(&tags) };

		if self.unique {
			tracing::debug!(?tags, ?input, "dispatching unique workflow");
		} else {
			tracing::debug!(?tags, ?input, "dispatching workflow");
		}

		// Serialize input
		let input_val = serde_json::value::to_raw_value(&input)
			.map_err(WorkflowError::SerializeWorkflowInput)
			.map_err(GlobalError::raw)?;

		let actual_workflow_id = self
			.db
			.dispatch_workflow(
				self.ray_id,
				workflow_id,
				workflow_name,
				tags,
				&input_val,
				self.unique,
			)
			.await
			.map_err(GlobalError::raw)?;

		tracing::Span::current().record("workflow_id", actual_workflow_id.to_string());

		if self.unique {
			if workflow_id == actual_workflow_id {
				tracing::debug!(?tags, "dispatched unique workflow");
			} else {
				tracing::debug!(?tags, "unique workflow already exists");
			}
		}

		if workflow_id == actual_workflow_id {
			let dt = start_instant.elapsed().as_secs_f64();
			metrics::WORKFLOW_DISPATCH_DURATION
				.with_label_values(&["", workflow_name])
				.observe(dt);
			metrics::WORKFLOW_DISPATCHED
				.with_label_values(&["", workflow_name])
				.inc();
		}

		Ok(actual_workflow_id)
	}

	#[tracing::instrument(name="workflow", skip_all, fields(workflow_name=I::Workflow::NAME))]
	pub async fn output(
		self,
	) -> GlobalResult<<<I as WorkflowInput>::Workflow as Workflow>::Output> {
		if !self.tags.is_empty() {
			return Err(
				BuilderError::TagsOnSubWorkflowOutputNotSupported(I::Workflow::NAME).into(),
			);
		}

		let db = self.db.clone();

		let workflow_id = if let Ok(workflow_id) = self.repr.as_workflow_id() {
			workflow_id
		} else {
			self.dispatch().await?
		};

		common::wait_for_workflow_output::<I::Workflow>(&db, workflow_id)
			.in_current_span()
			.await
	}

	#[tracing::instrument(skip_all, fields(workflow_name=I::Workflow::NAME))]
	pub async fn get(self) -> GlobalResult<Option<WorkflowData>> {
		let db = self.db.clone();
		let workflow_id = self.repr.as_workflow_id()?;

		db.get_workflow(workflow_id).await.map_err(GlobalError::raw)
	}
}
