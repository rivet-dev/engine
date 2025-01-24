use std::time::{SystemTime, UNIX_EPOCH};

use global_error::GlobalError;
use tokio::time::Instant;
use uuid::Uuid;

use crate::ctx::common::RETRY_TIMEOUT_MS;

pub type WorkflowResult<T> = Result<T, WorkflowError>;

/// Throwing this will eject from the workflow scope back in to the engine.
///
/// This error should not be touched by the user and is only intended to be handled by the workflow
/// engine.
#[derive(thiserror::Error, Debug)]
pub enum WorkflowError {
	#[error("workflow failure: {0:?}")]
	WorkflowFailure(GlobalError),

	// Includes error count
	#[error("activity failure: {0:?}")]
	ActivityFailure(GlobalError, usize),

	#[error("activity failure, max retries reached: {0:?}")]
	ActivityMaxFailuresReached(GlobalError),

	#[error("operation failure: {0:?}")]
	OperationFailure(GlobalError),

	#[error("workflow missing from registry: {0}")]
	WorkflowMissingFromRegistry(String),

	#[error("workflow not found")]
	WorkflowNotFound,

	#[error("history diverged: {0}")]
	HistoryDiverged(String),

	#[error("latent history found: {0}")]
	LatentHistoryFound(String),

	#[error("serialize workflow input: {0}")]
	SerializeWorkflowInput(serde_json::Error),

	#[error("deserialize workflow input: {0}")]
	DeserializeWorkflowInput(serde_json::Error),

	#[error("serialize workflow output: {0}")]
	SerializeWorkflowOutput(serde_json::Error),

	#[error("deserialize workflow output: {0}")]
	DeserializeWorkflowOutput(serde_json::Error),

	#[error("serialize activity input: {0}")]
	SerializeActivityInput(serde_json::Error),

	#[error("serialize activity output: {0}")]
	SerializeActivityOutput(serde_json::Error),

	#[error("deserialize activity output: {0}")]
	DeserializeActivityOutput(serde_json::Error),

	#[error("serialize signal body: {0}")]
	SerializeSignalBody(serde_json::Error),

	#[error("deserialize signal body: {0}")]
	DeserializeSignalBody(serde_json::Error),

	#[error("serialize message body: {0}")]
	SerializeMessageBody(serde_json::Error),

	#[error("deserialize message body: {0}")]
	DeserializeMessageBody(serde_json::Error),

	#[error("serialize message: {0}")]
	SerializeMessage(serde_json::Error),

	#[error("deserialize message: {0}")]
	DeserializeMessage(serde_json::Error),

	#[error("failed to serialize cjson tags: {0:?}")]
	CjsonSerializeTags(cjson::Error),

	#[error("failed to serialize tags: {0:?}")]
	SerializeTags(serde_json::Error),

	#[error("failed to deserialize tags: {0}")]
	DeserializeTags(serde_json::Error),

	#[error("tags must be a json object")]
	InvalidTags,

	#[error("failed to serialize loop state: {0}")]
	SerializeLoopState(serde_json::Error),

	#[error("failed to deserialize loop state: {0}")]
	DeserializeLoopState(serde_json::Error),

	#[error("failed to serialize loop output: {0}")]
	SerializeLoopOutput(serde_json::Error),

	#[error("failed to deserialize loop output: {0}")]
	DeserializeLoopOutput(serde_json::Error),

	#[error("invalid sleep state: {0}")]
	InvalidSleepState(i64),

	#[error("invalid event type: {0}")]
	InvalidEventType(i64),

	#[error("failed to create subscription: {0}")]
	CreateSubscription(rivet_pools::prelude::nats::Error),

	#[error("failed to flush nats: {0}")]
	FlushNats(rivet_pools::prelude::nats::Error),

	#[error("subscription unsubscribed")]
	SubscriptionUnsubscribed,

	#[error("missing message data")]
	MissingMessageData,

	#[error("redis error: {source}")]
	Redis {
		#[from]
		source: rivet_pools::prelude::redis::RedisError,
	},

	#[error("no signal found: {0:?}")]
	NoSignalFound(Box<[&'static str]>),

	#[error("sub workflow incomplete: {0:?}")]
	SubWorkflowIncomplete(Uuid),

	#[error("integer conversion failed")]
	IntegerConversion,

	#[error("missing event data")]
	MissingEventData,

	#[error("failed to build sql pool: {0}")]
	BuildSqlx(sqlx::Error),

	#[error("sql error: {0}")]
	Sqlx(#[from] sqlx::Error),

	#[error("max sql retries, last error: {0}")]
	MaxSqlRetries(sqlx::Error),

	#[error("pools error: {0}")]
	Pools(#[from] rivet_pools::Error),

	#[error("activity timed out")]
	ActivityTimeout(usize),

	#[error("operation timed out")]
	OperationTimeout(usize),

	#[error("duplicate registered workflow: {0}")]
	DuplicateRegisteredWorkflow(String),

	#[error("sleeping until {0}")]
	Sleep(i64),

	#[error("no signal found: {0:?}. sleeping until {1}")]
	NoSignalFoundAndSleep(Box<[&'static str]>, i64),

	#[error("cannot have multiple concurrent calls to Database::wake")]
	WakeLock(tokio::sync::TryLockError),

	#[error("`ListenCtx` has already been used once (`listen_any` called)")]
	ListenCtxUsed,

	#[error("int conversion error: {0}")]
	TryFromIntError(#[from] std::num::TryFromIntError),

	#[error("failed to serialize location: {0}")]
	SerializeLocation(serde_json::Error),

	#[error("invalid version: {0}")]
	InvalidVersion(String),
}

impl WorkflowError {
	/// Returns the next deadline for a workflow to be woken up again based on the error.
	pub(crate) fn deadline_ts(&self) -> Option<i64> {
		match self {
			WorkflowError::ActivityFailure(_, error_count)
			| WorkflowError::ActivityTimeout(error_count)
			| WorkflowError::OperationTimeout(error_count) => {
				// NOTE: Max retry is handled in `WorkflowCtx::activity`
				let mut backoff =
					rivet_util::Backoff::new_at(8, None, RETRY_TIMEOUT_MS, 500, *error_count);
				let next = backoff.step().expect("should not have max retry");

				// Calculate timestamp based on the backoff
				let duration_until = next.duration_since(Instant::now());
				let deadline_ts = (SystemTime::now() + duration_until)
					.duration_since(UNIX_EPOCH)
					.unwrap_or_else(|err| unreachable!("time is broken: {}", err))
					.as_millis()
					.try_into()
					.expect("doesn't fit in i64");

				Some(deadline_ts)
			}
			WorkflowError::Sleep(ts) | WorkflowError::NoSignalFoundAndSleep(_, ts) => Some(*ts),
			_ => None,
		}
	}

	/// Any error that the workflow can continue on with its execution from.
	pub fn is_recoverable(&self) -> bool {
		match self {
			WorkflowError::ActivityFailure(_, _)
			| WorkflowError::ActivityTimeout(_)
			| WorkflowError::OperationTimeout(_)
			| WorkflowError::NoSignalFound(_)
			| WorkflowError::NoSignalFoundAndSleep(_, _)
			| WorkflowError::SubWorkflowIncomplete(_)
			| WorkflowError::Sleep(_) => true,
			_ => false,
		}
	}

	/// Any error that the workflow can try again on.
	pub(crate) fn is_retryable(&self) -> bool {
		match self {
			WorkflowError::ActivityFailure(_, _)
			| WorkflowError::ActivityTimeout(_)
			| WorkflowError::OperationTimeout(_) => true,
			_ => false,
		}
	}

	pub(crate) fn signals(&self) -> &[&'static str] {
		match self {
			WorkflowError::NoSignalFound(signals)
			| WorkflowError::NoSignalFoundAndSleep(signals, _) => signals,
			_ => &[],
		}
	}

	pub(crate) fn sub_workflow(&self) -> Option<Uuid> {
		if let WorkflowError::SubWorkflowIncomplete(sub_workflow_id) = self {
			Some(*sub_workflow_id)
		} else {
			None
		}
	}
}
