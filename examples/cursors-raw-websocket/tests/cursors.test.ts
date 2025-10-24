import { setupTest } from "rivetkit/test";
import { expect, test } from "vitest";
import { registry } from "../src/backend/registry";

test("Cursor room can be created and initialized", async (ctx: any) => {
	const { client } = await setupTest(ctx, registry);
	const room = client.cursorRoom.getOrCreate(["test-room"]);

	// Test that the getOrCreate action works
	const result = await room.getOrCreate();
	expect(result).toEqual({ status: "ok" });
});

test("Cursor room can get initial room state", async (ctx: any) => {
	const { client } = await setupTest(ctx, registry);
	const room = client.cursorRoom.getOrCreate(["test-state"]);

	// Test initial state
	const state = await room.getRoomState();
	expect(state).toEqual({
		cursors: {},
		textLabels: [],
	});
});
