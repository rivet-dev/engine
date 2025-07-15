import { type Context, Hono } from "hono";
import type { UpgradeWebSocket } from "hono/ws";

type GetUpgradeWebSocketFn = (app: Hono) => UpgradeWebSocket;

export function createAndStartServer(
	getUpgradeWebSocket: GetUpgradeWebSocketFn,
): { app: Hono; port: number } {
	// Setup auto-exit timer
	// setTimeout(() => {
	// 	console.error(
	// 		"Actor should've been destroyed by now. Automatically exiting.",
	// 	);
	//
	// 	if (typeof Deno !== "undefined") Deno.exit(1);
	// 	else process.exit(1);
	// }, 60 * 1000);

	let tickIndex = 0;
	setInterval(() => {
		tickIndex++;
		console.log("Tick", tickIndex);
	}, 1000);

	// Get port from environment
	const portEnv =
		typeof Deno !== "undefined"
			? Deno.env.get("PORT_HTTP")
			: process.env.PORT_HTTP;
	if (!portEnv) {
		throw new Error("missing PORT_HTTP");
	}
	const port = Number.parseInt(portEnv);

	// Create app with health endpoint
	const app = new Hono();

	app.get("/health", (c) => c.text("ok"));

	app.get("/exit", (c) => {
		const query = c.req.query("code");
		const exitCode = query ? Number(query) : 0;

		if (typeof Deno != "undefined") Deno.exit(exitCode);
		else process.exit(exitCode);

		return c.text("unreachable");
	});

	// Add WebSocket endpoint with handler
	const upgradeWebSocket = getUpgradeWebSocket(app);
	app.get(
		"/ws",
		upgradeWebSocket((c: Context) => {
			return {
				onOpen(_, ws) {
					ws.send(
						JSON.stringify([
							"init",
							{
								forwardedFor: c.req?.header("x-forwarded-for"),
							},
						]),
					);
				},
				onMessage(message, ws) {
					const data =
						typeof message.data === "string"
							? message.data
							: (message as unknown as string);

					if (typeof data === "string") {
						const [eventType, eventData] = JSON.parse(
							data.slice(0, 2 ** 13),
						);
						switch (eventType) {
							case "ping":
								ws.send(JSON.stringify(["pong", eventData]));
								break;
							default:
								console.warn("unknown event", eventType);
								break;
						}
					}
				},
				onClose(event) {
					console.log(
						`WebSocket closed: ${event.code} ${event.reason}`,
					);
				},
				onError(event) {
					console.error("WebSocket error:", event);
				},
			};
		}),
	);

	console.log(`Listening on port ${port}`);

	return { app, port };
}
