import { useFormContext } from "react-hook-form";
import z from "zod";
import * as ConnectManualServerlessForm from "@/app/forms/connect-manual-serverless-form";
import {
	Code,
	CodeFrame,
	CodePreview,
	FormControl,
	FormDescription,
	FormField,
	FormItem,
	FormLabel,
	FormMessage,
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components";
import { defineStepper } from "@/components/ui/stepper";

const endpointSchema = z
	.string()
	.nonempty("Endpoint is required")
	.url("Please enter a valid URL")
	.endsWith("/api/rivet", "Endpoint must end with /api/rivet");

export const stepper = defineStepper(
	{
		id: "initial-info",
		title: "Configure",
		assist: false,
		next: "Next",
		schema: z.object({
			plan: z.string().min(1, "Please select a Vercel plan"),
			runnerName: z.string().min(1, "Runner name is required"),
			datacenters: z
				.record(z.boolean())
				.refine(
					(data) => Object.values(data).some(Boolean),
					"At least one datacenter must be selected",
				),
			headers: z.array(z.tuple([z.string(), z.string()])).default([]),
			slotsPerRunner: z.coerce.number().min(1, "Must be at least 1"),
			maxRunners: z.coerce.number().min(1, "Must be at least 1"),
			minRunners: z.coerce.number().min(0, "Must be 0 or greater"),
			runnerMargin: z.coerce.number().min(0, "Must be 0 or greater"),
		}),
	},
	{
		id: "api-route",
		title: "Configure maxDuration in API route handler",
		assist: false,
		schema: z.object({}),
		next: "Next",
	},
	{
		id: "vercel-settings",
		title: "Configure Vercel settings",
		assist: false,
		next: "Next",
		schema: z.object({}),
	},
	{
		id: "deploy",
		title: "Deploy to Vercel",
		assist: true,
		next: "Done",
		schema: z.object({
			success: z.boolean().refine((val) => val, "Connection failed"),
			endpoint: endpointSchema,
		}),
	},
);

export const Plan = ({ className }: { className?: string }) => {
	const { control } = useFormContext();
	return (
		<FormField
			control={control}
			name="plan"
			render={({ field }) => (
				<FormItem className={className}>
					<FormLabel className="col-span-1">Vercel Plan</FormLabel>
					<FormControl className="row-start-2">
						<Select
							onValueChange={field.onChange}
							value={field.value}
						>
							<SelectTrigger>
								<SelectValue placeholder="Select your Vercel plan..." />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="hobby">Hobby</SelectItem>
								<SelectItem value="pro">Pro</SelectItem>
								<SelectItem value="enterprise">
									Enterprise
								</SelectItem>
							</SelectContent>
						</Select>
					</FormControl>
					<FormDescription className="col-span-1">
						Your Vercel plan determines the configuration required
						to properly connect Rivet to Vercel Functions.
					</FormDescription>
					<FormMessage className="col-span-1" />
				</FormItem>
			)}
		/>
	);
};

export const RunnerName = ConnectManualServerlessForm.RunnerName;

export const Datacenters = ConnectManualServerlessForm.Datacenters;

export const MinRunners = ConnectManualServerlessForm.MinRunners;

export const MaxRunners = ConnectManualServerlessForm.MaxRunners;

export const SlotsPerRunner = ConnectManualServerlessForm.SlotsPerRunner;

export const RunnerMargin = ConnectManualServerlessForm.RunnerMargin;

export const Headers = ConnectManualServerlessForm.Headers;

export const PLAN_TO_MAX_DURATION: Record<string, number> = {
	hobby: 60,
	pro: 300,
	enterprise: 900,
};

const code = ({ plan }: { plan: string }) =>
	`{
	"$schema": "https://openapi.vercel.sh/vercel.json",
	"fluid": false,	// [!code highlight]
}`;

export const Json = ({ plan }: { plan: string }) => {
	return (
		<div className="space-y-2 mt-2">
			<CodeFrame
				language="json"
				title="vercel.json"
				code={() =>
					code({ plan }).replaceAll("	// [!code highlight]", "")
				}
			>
				<CodePreview
					className="w-full min-w-0"
					language="json"
					code={code({ plan })}
				/>
			</CodeFrame>
			<p>Rivet provides its own intelligent load balancing mechanism.</p>
		</div>
	);
};

const integrationCode = ({ plan }: { plan: string }) =>
	`import { toNextHandler } from "@rivetkit/next-js";
import { registry } from "@/rivet/registry";

export const maxDuration = ${PLAN_TO_MAX_DURATION[plan] || 60};	// [!code highlight]

export const { GET, POST, PUT, PATCH, HEAD, OPTIONS } = toNextHandler(registry);`;

export const IntegrationCode = ({ plan }: { plan: string }) => {
	return (
		<div className="space-y-2 mt-2">
			<p>
				Update your Rivet API route handler to export the{" "}
				<Code>maxDuration</Code> configuration.
			</p>
			<CodeFrame
				language="typescript"
				title="src/app/api/rivet/[...all]/route.ts"
				code={() =>
					integrationCode({ plan }).replaceAll(
						"	// [!code highlight]",
						"",
					)
				}
			>
				<CodePreview
					className="w-full min-w-0"
					language="typescript"
					code={integrationCode({ plan })}
				/>
			</CodeFrame>
		</div>
	);
};

export const Endpoint = ConnectManualServerlessForm.Endpoint;

export const ConnectionCheck = ConnectManualServerlessForm.ConnectionCheck;
