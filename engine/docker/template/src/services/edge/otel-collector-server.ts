import * as yaml from "js-yaml";
import type { TemplateContext } from "../../context";

export function generateDatacenterOtelCollectorServer(
	context: TemplateContext,
	dcId: string,
) {
	const clickhouseHost =
		context.config.networkMode === "host" ? "127.0.0.1" : "clickhouse";
	const otelConfig = {
		receivers: {
			otlp: {
				protocols: {
					grpc: {
						endpoint: "0.0.0.0:4317",
					},
				},
			},
		},
		processors: {
			batch: {
				timeout: "5s",
				send_batch_size: 10000,
			},
		},
		exporters: {
			clickhouse: {
				endpoint: `http://${clickhouseHost}:9300`,
				database: "otel",
				username: "default",
				password: "${env:CLICKHOUSE_PASSWORD}",
				async_insert: true,
				ttl: "72h",
				compress: "lz4",
				create_schema: true,
				logs_table_name: "otel_logs",
				traces_table_name: "otel_traces",
				timeout: "5s",
				metrics_tables: {
					gauge: {
						name: "otel_metrics_gauge",
					},
					sum: {
						name: "otel_metrics_sum",
					},
					summary: {
						name: "otel_metrics_summary",
					},
					histogram: {
						name: "otel_metrics_histogram",
					},
					exponential_histogram: {
						name: "otel_metrics_exp_histogram",
					},
				},
				retry_on_failure: {
					enabled: true,
					initial_interval: "5s",
					max_interval: "30s",
					max_elapsed_time: "300s",
				},
			},
		},
		service: {
			pipelines: {
				logs: {
					receivers: ["otlp"],
					processors: ["batch"],
					exporters: ["clickhouse"],
				},
				traces: {
					receivers: ["otlp"],
					processors: ["batch"],
					exporters: ["clickhouse"],
				},
				metrics: {
					receivers: ["otlp"],
					processors: ["batch"],
					exporters: ["clickhouse"],
				},
			},
		},
	};

	const yamlContent = yaml.dump(otelConfig);

	context.writeDatacenterServiceFile(
		"otel-collector-server",
		dcId,
		"config.yaml",
		yamlContent,
	);
}
