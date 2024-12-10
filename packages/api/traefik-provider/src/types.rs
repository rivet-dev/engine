use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// MARK: GET /traefik/config
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikConfigResponse {
	pub http: TraefikHttp,
	pub tcp: TraefikHttp,
	pub udp: TraefikHttp,
}

/// Traefik will throw an error if we don't list any services, so this lets us exclude empty maps.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikConfigResponseNullified {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub http: Option<TraefikHttpNullified>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tcp: Option<TraefikHttpNullified>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub udp: Option<TraefikHttpNullified>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikHttp {
	pub services: HashMap<String, TraefikService>,
	pub routers: HashMap<String, TraefikRouter>,
	pub middlewares: HashMap<String, TraefikMiddlewareHttp>,
}

/// See above.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikHttpNullified {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub services: Option<HashMap<String, TraefikService>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub routers: Option<HashMap<String, TraefikRouter>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub middlewares: Option<HashMap<String, TraefikMiddlewareHttp>>,
}

impl TraefikHttp {
	pub fn nullified(self) -> Option<TraefikHttpNullified> {
		if self.services.is_empty() && self.routers.is_empty() && self.middlewares.is_empty() {
			None
		} else {
			Some(TraefikHttpNullified {
				services: if self.services.is_empty() {
					None
				} else {
					Some(self.services)
				},
				routers: if self.routers.is_empty() {
					None
				} else {
					Some(self.routers)
				},
				middlewares: if self.middlewares.is_empty() {
					None
				} else {
					Some(self.middlewares)
				},
			})
		}
	}
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikService {
	pub load_balancer: TraefikLoadBalancer,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikLoadBalancer {
	#[serde(default)]
	pub servers: Vec<TraefikServer>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sticky: Option<TraefikLoadBalancerSticky>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TraefikLoadBalancerSticky {
	#[serde(rename = "cookie", rename_all = "camelCase")]
	Cookie {},
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikServer {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub address: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikRouter {
	pub entry_points: Vec<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rule: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub priority: Option<usize>,
	pub service: String,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub middlewares: Vec<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tls: Option<TraefikTls>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikTls {
	#[serde(skip_serializing_if = "Option::is_none")]
	cert_resolver: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	domains: Option<Vec<TraefikTlsDomain>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	options: Option<String>,
}

impl TraefikTls {
	/// Builds a `TraefikTls` object relevant to the environment.
	///
	/// We don't associate a cert resolver if in local development because we generate certificates
	/// with mkcert.
	pub fn build(domains: Vec<TraefikTlsDomain>) -> TraefikTls {
		TraefikTls {
			cert_resolver: None,
			domains: Some(domains),
			options: None,
		}
	}

	pub fn build_cloudflare() -> TraefikTls {
		TraefikTls {
			cert_resolver: None,
			domains: None,
			options: Some("traefik-ingress-cloudflare@kubernetescrd".into()),
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikTlsDomain {
	pub main: String,
	pub sans: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TraefikMiddlewareHttp {
	#[serde(rename = "chain", rename_all = "camelCase")]
	Chain { middlewares: Vec<String> },
	#[serde(rename = "ipAllowList", rename_all = "camelCase")]
	IpAllowList {
		source_range: Vec<String>,
		#[serde(skip_serializing_if = "Option::is_none")]
		ip_strategy: Option<IpStrategy>,
	},
	#[serde(rename = "replacePathRegex", rename_all = "camelCase")]
	ReplacePathRegex { regex: String, replacement: String },
	#[serde(rename = "stripPrefix", rename_all = "camelCase")]
	StripPrefix { prefixes: Vec<String> },
	#[serde(rename = "addPrefix", rename_all = "camelCase")]
	AddPrefix { prefix: String },
	#[serde(rename = "rateLimit", rename_all = "camelCase")]
	RateLimit {
		average: usize,
		period: String,
		burst: usize,
		source_criterion: InFlightReqSourceCriterion,
	},
	#[serde(rename = "inFlightReq", rename_all = "camelCase")]
	InFlightReq {
		amount: usize,
		source_criterion: InFlightReqSourceCriterion,
	},
	#[serde(rename = "retry", rename_all = "camelCase")]
	Retry {
		attempts: usize,
		initial_interval: String,
	},
	#[serde(rename = "compress", rename_all = "camelCase")]
	Compress {},
	#[serde(rename = "headers", rename_all = "camelCase")]
	Headers(TraefikMiddlewareHeaders),
	#[serde(rename = "redirectRegex", rename_all = "camelCase")]
	RedirectRegex {
		permanent: bool,
		regex: String,
		replacement: String,
	},
	#[serde(rename = "basicAuth", rename_all = "camelCase")]
	BasicAuth {
		users: Vec<String>,
		#[serde(skip_serializing_if = "Option::is_none")]
		realm: Option<String>,
		#[serde(default)]
		remove_header: bool,
	},
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TraefikMiddlewareHeaders {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub access_control_allow_methods: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub access_control_allow_origin_list: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub access_control_max_age: Option<usize>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub custom_request_headers: Option<HashMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub custom_response_headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct IpStrategy {
	pub depth: usize,

	#[serde(rename = "excludedIPs", skip_serializing_if = "Option::is_none")]
	pub exclude_ips: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum InFlightReqSourceCriterion {
	#[serde(rename = "ipStrategy")]
	IpStrategy(IpStrategy),
	#[serde(rename = "requestHeaderName", rename_all = "camelCase")]
	RequestHeaderName(String),
	#[serde(rename = "requestHost", rename_all = "camelCase")]
	RequestHost {},
}
