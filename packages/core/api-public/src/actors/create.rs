use anyhow::Result;
use axum::{
	extract::{Extension, Query},
	http::HeaderMap,
	response::{IntoResponse, Json, Response},
};
use rivet_api_builder::{ApiCtx, ApiError};
use rivet_api_client::request_remote_datacenter;
use rivet_api_types::actors::create::{CreateRequest, CreateResponse};
use rivet_types::actors::CrashPolicy;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct CreateQuery {
	pub namespace: String,
	pub datacenter: Option<String>,
}

/// ## Datacenter Round Trips
///
/// **If actor is created in the current datacenter:**
///
/// 2 round trips:
/// - namespace::ops::resolve_for_name_global
/// - [pegboard::workflows::actor] Create actor workflow (includes Epoxy key allocation)
///
/// **If actor is created in a different datacenter:**
///
/// 3 round trips:
/// - namespace::ops::resolve_for_name_global
/// - POST /actors to remote datacenter
/// - [pegboard::workflows::actor] Create actor workflow (includes Epoxy key allocation)
///
/// actor::get will always be in the same datacenter.
#[utoipa::path(
    post,
	operation_id = "actors_create",
    path = "/actors",
    params(CreateQuery),
    request_body(content = CreateRequest, content_type = "application/json"),
    responses(
        (status = 200, body = CreateResponse),
    ),
)]
pub async fn create(
	Extension(ctx): Extension<ApiCtx>,
	headers: HeaderMap,
	Query(query): Query<CreateQuery>,
	Json(body): Json<CreateRequest>,
) -> Response {
	match create_inner(ctx, headers, query, body).await {
		Ok(response) => Json(response).into_response(),
		Err(err) => ApiError::from(err).into_response(),
	}
}

async fn create_inner(
	ctx: ApiCtx,
	headers: HeaderMap,
	query: CreateQuery,
	body: CreateRequest,
) -> Result<CreateResponse> {
	// Determine which datacenter to create the actor in
	let target_dc_label = if let Some(dc_name) = &query.datacenter {
		ctx.config()
			.dc_for_name(dc_name)
			.ok_or_else(|| crate::errors::Datacenter::NotFound.build())?
			.datacenter_label
	} else {
		ctx.config().dc_label()
	};

	let query = rivet_api_types::actors::create::CreateQuery {
		namespace: query.namespace,
	};

	if target_dc_label == ctx.config().dc_label() {
		rivet_api_peer::actors::create::create(ctx, (), query, body).await
	} else {
		request_remote_datacenter::<CreateResponse>(
			ctx.config(),
			target_dc_label,
			"/actors",
			axum::http::Method::POST,
			headers,
			Some(&query),
			Some(&body),
		)
		.await
	}
}
