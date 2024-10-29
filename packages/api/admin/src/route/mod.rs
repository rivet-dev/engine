use api_helper::define_router;
use hyper::{Body, Request, Response};
use rivet_api::models;
use uuid::Uuid;

pub mod clusters;
pub mod login;

define_router! {
	routes: {
		"login": {
			POST: login::login(
				body: models::AdminLoginRequest,
			),
		},

		"clusters": {
			GET: clusters::list(),
			POST: clusters::create(
				body: models::AdminClustersCreateClusterRequest,
			),
		},

		"clusters" / Uuid / "servers": {
			GET: clusters::servers::list(
				query: clusters::servers::ServerFilterQuery,
			),
		},

		"clusters" / Uuid / "servers" / "taint": {
			POST: clusters::servers::taint(
				query: clusters::servers::ServerFilterQuery,
				body: serde_json::Value,
			),
		},

		"clusters" / Uuid / "servers" / "destroy": {
			POST: clusters::servers::destroy(
				query: clusters::servers::ServerFilterQuery,
				body: serde_json::Value,
			),
		},

		"clusters" / Uuid / "servers" / "lost": {
			GET: clusters::servers::list_lost(
				query: clusters::servers::ServerFilterQuery,
			),
		},

		"clusters" / Uuid / "servers" / "prune": {
			POST: clusters::servers::prune(
				query: clusters::servers::ServerFilterQuery,
				body: serde_json::Value,
			),
		},

		"clusters" / Uuid / "datacenters": {
			GET: clusters::datacenters::list(),
			POST: clusters::datacenters::create(
				body: models::AdminClustersCreateDatacenterRequest,
			),
		},

		"clusters" / Uuid / "datacenters" / Uuid : {
			PATCH: clusters::datacenters::update(
				body: models::AdminClustersUpdateDatacenterRequest,
			),
		},

	},
}
