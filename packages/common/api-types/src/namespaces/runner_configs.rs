use std::collections::HashMap;

use gas::prelude::*;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunnerConfig {
	Serverless {
		url: String,
		headers: Option<HashMap<String, String>>,
		/// Seconds.
		request_lifespan: u32,
		slots_per_runner: u32,
		min_runners: Option<u32>,
		max_runners: u32,
		runners_margin: Option<u32>,
	},
}
