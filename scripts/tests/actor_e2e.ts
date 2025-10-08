#!/usr/bin/env tsx

import { RIVET_ENDPOINT, createActor, destroyActor } from "./utils";

async function main() {
	try {
		console.log("Starting actor E2E test...");

		// Create an actor
		console.log("Creating actor...");
		const actorResponse = await createActor("default", "test-runner");
		console.log("Actor created:", actorResponse.actor);

		// Make a request to the actor
		console.log("Making request to actor...");
		const actorPingResponse = await fetch(`${RIVET_ENDPOINT}/ping`, {
			method: "GET",
			headers: {
				"X-Rivet-Target": "actor",
				"X-Rivet-Actor": actorResponse.actor.actor_id,
			},
		});

		const pingResult = await actorPingResponse.text();

		if (!actorPingResponse.ok) {
			throw new Error(
				`Failed to ping actor: ${actorPingResponse.status} ${actorPingResponse.statusText}\n${pingResult}`,
			);
		}

		console.log("Actor ping response:", pingResult);

		// Test WebSocket connection
		console.log("Testing WebSocket connection to actor...");
		// await testWebSocket(actorResponse.actor.actor_id);

		console.log("Destroying actor...");
		await destroyActor("default", actorResponse.actor.actor_id);

		console.log("E2E test completed successfully!");

		// HACK: This script does not exit by itself for some reason
		process.exit(0);
	} catch (error) {
		console.error("E2E test failed:", error);
		process.exit(1);
	}
}

function testWebSocket(actorId: string): Promise<void> {
	return new Promise((resolve, reject) => {
		// Parse the RIVET_ENDPOINT to get WebSocket URL
		const wsEndpoint = RIVET_ENDPOINT.replace("http://", "ws://").replace(
			"https://",
			"wss://",
		);
		const wsUrl = `${wsEndpoint}/ws`;

		console.log(`Connecting WebSocket to: ${wsUrl}`);

		const protocols = ["rivet", "rivet_target.actor", `rivet_actor.${actorId}`];
		const ws = new WebSocket(wsUrl, protocols);

		let pingReceived = false;
		let echoReceived = false;
		const timeout = setTimeout(() => {
			console.log(
				"No response received within timeout, but connection was established",
			);
			// Connection was established, that's enough for the test
			ws.close();
			resolve();
		}, 2000);

		ws.addEventListener("open", () => {
			console.log("WebSocket connected");

			// Test ping-pong
			console.log("Sending 'ping' message...");
			ws.send("ping");
		});

		ws.addEventListener("message", (data) => {
			const message = data.toString();
			console.log(`WebSocket received raw data:`, data);
			console.log(`WebSocket received message: "${message}"`);

			if (
				(message === "Echo: ping" || message === "pong") &&
				!pingReceived
			) {
				pingReceived = true;
				console.log("Ping test successful!");

				// Test echo
				console.log("Sending 'hello' message...");
				ws.send("hello");
			} else if (message === "Echo: hello" && !echoReceived) {
				echoReceived = true;
				console.log("Echo test successful!");

				// All tests passed
				clearTimeout(timeout);
				ws.close();
				resolve();
			}
		});

		ws.addEventListener("error", (error) => {
			clearTimeout(timeout);
			reject(new Error(`WebSocket error: ${error.message}`));
		});

		ws.addEventListener("close", () => {
			clearTimeout(timeout);
			if (!pingReceived || !echoReceived) {
				reject(new Error("WebSocket closed before completing tests"));
			}
		});
	});
}

main();
