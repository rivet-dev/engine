import { TemplateContext } from "../../context";
import { Datacenter } from "../../config";

const API_PEER_PORT = 6421;
const GUARD_PORT = 6420;

export function generateDatacenterRivetEngine(
	context: TemplateContext,
	datacenter: Datacenter,
) {
	const clickhouseHost =
		context.config.networkMode === "host" ? "127.0.0.1" : "clickhouse";
	const datacenters = [];

	for (let dc of context.config.datacenters) {
		let serviceHost = context.getServiceHost("rivet-engine", dc.name, 0);
		datacenters.push({
			name: dc.name,
			datacenter_label: dc.id,
			is_leader: dc.id == 1,
			peer_url: `http://${serviceHost}:${API_PEER_PORT}`,
			public_url: `http://${serviceHost}:${GUARD_PORT}`,
		});
	}

	// Generate a separate config file for each engine node
	for (let i = 0; i < datacenter.engines; i++) {
		let serviceHost = context.getServiceHost(
			"rivet-engine",
			datacenter.name,
			0,
		);
		const topology = {
			datacenter_label: datacenter.id,
			datacenters,
		};

		// Config structure matching Rust schema in packages/common/config/src/config/mod.rs
		const config = {
			auth: {
				admin_token: "dev",
			},
			guard: {
				port: GUARD_PORT,
				// https is optional and not configured for local development
			},
			api_peer: {
				host: "0.0.0.0",
				port: API_PEER_PORT,
			},
			topology,
			postgres: {
				url: `postgresql://postgres:postgres@${context.getServiceHost("postgres", datacenter.name)}:5432/rivet_engine`,
			},
			cache: {
				driver: "in_memory",
			},
			clickhouse: {
				http_url: `http://${clickhouseHost}:9300`, // TODO:
				native_url: `http://${clickhouseHost}:9301`, // TODO:
				username: "system",
				password: "default",
				// TODO: Move this to init migrations
				provision_users: {
					vector: {
						username: "vector",
						password: "vector",
						role: "write",
					},
				},
				secure: false,
			},
			vector_http: {
				host: context.getServiceHost("vector-client", datacenter.name),
				port: 5022,
			},
		};

		context.writeDatacenterServiceFile(
			"rivet-engine",
			datacenter.name,
			"config.jsonc",
			JSON.stringify(config, null, "\t"),
			i,
		);
	}
}
