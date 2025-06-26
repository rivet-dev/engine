import { serve } from "@hono/node-server";
import { createNodeWebSocket } from "@hono/node-ws";
import { createAndStartServer } from "../shared/server.js";
import dgram from 'dgram';
import fs from 'fs';

// Print hosts file contents before starting
try {
	const hostsContent = fs.readFileSync('/etc/hosts', 'utf8');
	console.log('=== /etc/hosts contents ===');
	console.log(hostsContent);
	console.log('=== End of /etc/hosts ===');
} catch (err) {
	console.error('Failed to read /etc/hosts:', err);
}

let injectWebSocket: any;
const { app, port } = createAndStartServer((app) => {
	// Get Node.js WebSocket handler
	const result = createNodeWebSocket({ app });
	injectWebSocket = result.injectWebSocket;
	return result.upgradeWebSocket;
});

const server = serve({ fetch: app.fetch, port });
injectWebSocket(server);


// async function contactApi() {
// 	console.log('Contacting', process.env.RIVET_API_ENDPOINT);
// 	const res = await fetch(process.env.RIVET_API_ENDPOINT!);
// 	console.log('API response', res.ok, res.status);
// }
//
// contactApi();

// Get port from environment
const portEnv =
	typeof Deno !== "undefined"
		? Deno.env.get("PORT_UDP")
		: process.env.PORT_UDP;

if (portEnv) {
	// Create a UDP socket
	const udpServer = dgram.createSocket('udp4');

	// Listen for incoming messages
	udpServer.on('message', (msg, rinfo) => {
		console.log(`UDP server received: ${msg} from ${rinfo.address}:${rinfo.port}`);

		// Echo the message back to the sender
		udpServer.send(msg, rinfo.port, rinfo.address, (err) => {
			if (err) console.error('Failed to send UDP response:', err);
		});
	});

	// Handle errors
	udpServer.on('error', (err) => {
		console.error('UDP server error:', err);
		udpServer.close();
	});


	const port2 = Number.parseInt(portEnv);

	udpServer.bind(port2, () => {
		console.log(`UDP echo server running on port ${port2}`);
	});
} else {
	console.warn("missing PORT_UDP");
}
