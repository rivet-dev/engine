use std::collections::HashMap;

use chirp_workflow::prelude::*;
use serde_json::json;

use crate::types::PoolType;

pub const TUNNEL_VECTOR_PORT: u16 = 5020;
pub const TUNNEL_VECTOR_TCP_JSON_PORT: u16 = 5021;

pub fn install() -> String {
	include_str!("../files/vector_install.sh").to_string()
}

pub struct Config {
	pub prometheus_targets: HashMap<String, PrometheusTarget>,
}

pub struct PrometheusTarget {
	pub endpoint: String,
	pub scrape_interval: usize,
}

pub fn configure(config: &Config, pool_type: PoolType) -> GlobalResult<String> {
	let sources = config
		.prometheus_targets
		.keys()
		.map(|x| format!("prometheus_{x}"))
		.collect::<Vec<_>>();

	let mut config_json = json!({
		"api": {
			"enabled": true
		},
		"transforms": {
			"filter_metrics": {
				"type": "filter",
				"inputs": sources,
				"condition": "!starts_with!(.name, \"go_\") && !starts_with!(.name, \"promhttp_\")"
			},
			"metrics_add_meta": {
				"type": "remap",
				"inputs": ["filter_metrics"],
				"source": formatdoc!(
					r#"
					.tags.server_id = "___SERVER_ID___"
					.tags.datacenter_id = "___DATACENTER_ID___"
					.tags.cluster_id = "___CLUSTER_ID___"
					.tags.pool_type = "{pool_type}"
					.tags.public_ip = "${{PUBLIC_IP}}"
					"#
				),
			}
		},
		"sinks": {
			"vector_sink": {
				"type": "vector",
				"inputs": ["metrics_add_meta"],
				"address": format!("127.0.0.1:{}", TUNNEL_VECTOR_PORT),
				"healthcheck": {
					"enabled": false
				},
				"compression": true,

				//Buffer to disk for durability & reduce memory usage
				"buffer": {
					"type": "disk",
					"max_size": 268435488,  // 256 MB
					"when_full": "block"
				}
			}
		}
	});

	// Add pegboard manager and runner logs
	match pool_type {
		PoolType::Pegboard | PoolType::PegboardIsolate => {
			config_json["sources"]["pegboard_manager"] = json!({
				"type": "file",
				"include": ["/etc/pegboard/log"]
			});

			config_json["transforms"]["pegboard_manager_add_meta"] = json!({
				"type": "remap",
				"inputs": ["pegboard_manager"],
				"source": formatdoc!(
					r#"
					.source = "pegboard_manager"

					.client_id = "___SERVER_ID___"
					.server_id = "___SERVER_ID___"
					.datacenter_id = "___DATACENTER_ID___"
					.cluster_id = "___CLUSTER_ID___"
					.pool_type = "{pool_type}"
					.public_ip = "${{PUBLIC_IP}}"
					"#
				),
			});

			config_json["sources"]["pegboard_v8_isolate_runner"] = json!({
				"type": "file",
				"include": ["/etc/pegboard/runner/log"]
			});

			config_json["transforms"]["pegboard_v8_isolate_runner_add_meta"] = json!({
				"type": "remap",
				"inputs": ["pegboard_v8_isolate_runner"],
				"source": formatdoc!(
					r#"
					.source = "pegboard_v8_isolate_runner"

					.client_id = "___SERVER_ID___"
					.server_id = "___SERVER_ID___"
					.datacenter_id = "___DATACENTER_ID___"
					.cluster_id = "___CLUSTER_ID___"
					.pool_type = "{pool_type}"
					.public_ip = "${{PUBLIC_IP}}"
					"#
				),
			});

			config_json["sources"]["pegboard_container_runners"] = json!({
				"type": "file",
				"include": ["/etc/pegboard/actors/*/log"]
			});

			config_json["transforms"]["pegboard_container_runner_add_meta"] = json!({
				"type": "remap",
				"inputs": ["pegboard_container_runners"],
				"source": formatdoc!(
					r#"
					.source = "pegboard_container_runner"
					.actor_id, err = parse_regex(.file, r'/etc/pegboard/actors/(?P<actor_id>[0-9a-fA-F-]+)/log').actor_id

					.client_id = "___SERVER_ID___"
					.server_id = "___SERVER_ID___"
					.datacenter_id = "___DATACENTER_ID___"
					.cluster_id = "___CLUSTER_ID___"
					.pool_type = "{pool_type}"
					.public_ip = "${{PUBLIC_IP}}"
					"#
				),
			});

			let inputs = unwrap!(config_json["sinks"]["vector_sink"]["inputs"].as_array_mut());
			inputs.push(json!("pegboard_manager_add_meta"));
			inputs.push(json!("pegboard_v8_isolate_runner_add_meta"));
			inputs.push(json!("pegboard_container_runner_add_meta"));
		}
		_ => {}
	}

	for (
		key,
		PrometheusTarget {
			endpoint,
			scrape_interval,
		},
	) in &config.prometheus_targets
	{
		config_json["sources"][&format!("prometheus_{key}")] = json!({
			"type": "prometheus_scrape",
			"endpoints": [endpoint],
			"scrape_interval_secs": scrape_interval,
		});
	}

	let config_str = serde_json::to_string(&config_json)?;

	Ok(include_str!("../files/vector_configure.sh").replace("__VECTOR_CONFIG__", &config_str))
}
