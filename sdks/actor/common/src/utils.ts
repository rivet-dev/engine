import { z } from "zod";

export const ActorTagsSchema = z
	.object({
		name: z.string(),
	})
	.catchall(z.string());

export const BuildTagsSchema = z
	.object({
		name: z.string(),
	})
	.catchall(z.string());

export type ActorTags = z.infer<typeof ActorTagsSchema>;
export type BuildTags = z.infer<typeof BuildTagsSchema>;

export interface RivetEnvironment {
	project?: string;
	environment?: string;
}

export function assertUnreachable(x: never): never {
	throw new Error(`Unreachable case: ${x}`);
}

/**
 * Safely stringifies an object, ensuring that the stringified object is under a certain size.
 * @param obj any object to stringify
 * @param maxSize maximum size of the stringified object in bytes
 * @returns stringified object
 */
export function safeStringify(obj: unknown, maxSize: number) {
	let size = 0;

	function replacer(key: string, value: unknown) {
		if (value === null || value === undefined) return value;
		const valueSize =
			typeof value === "string"
				? value.length
				: JSON.stringify(value).length;
		size += key.length + valueSize;

		if (size > maxSize) {
			throw new Error(
				`JSON object exceeds size limit of ${maxSize} bytes.`,
			);
		}

		return value;
	}

	return JSON.stringify(obj, replacer);
}
