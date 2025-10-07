use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

use crate::pagination::Pagination;

#[derive(Debug, Serialize, Deserialize, Clone, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
	pub namespace: String,
	pub limit: Option<usize>,
	pub cursor: Option<String>,
	pub variant: Option<rivet_types::keys::namespace::runner_config::RunnerConfigVariant>,
	#[serde(default)]
	pub runner_names: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ListPath {}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
	pub runner_configs: HashMap<String, rivet_types::runner_configs::RunnerConfig>,
	pub pagination: Pagination,
}
