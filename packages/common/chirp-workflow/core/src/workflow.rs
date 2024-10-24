use async_trait::async_trait;
use global_error::GlobalResult;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

use crate::ctx::WorkflowCtx;

#[async_trait]
pub trait Workflow {
	type Input: WorkflowInput;
	type Output: Serialize + DeserializeOwned + Debug + Send;

	const NAME: &'static str;

	async fn run(ctx: &mut WorkflowCtx, input: &Self::Input) -> GlobalResult<Self::Output>;
}

pub trait WorkflowInput: Serialize + DeserializeOwned + Debug + Send {
	type Workflow: Workflow;
}
