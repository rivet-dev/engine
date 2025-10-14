import { faCheck, faSpinnerThird, Icon } from "@rivet-gg/icons";
import {
	useInfiniteQuery,
	usePrefetchInfiniteQuery,
} from "@tanstack/react-query";
import { useEffect } from "react";
import { useController, useFormContext } from "react-hook-form";
import z from "zod";
import * as ConnectVercelForm from "@/app/forms/connect-vercel-form";
import {
	cn,
	FormControl,
	FormDescription,
	FormField,
	FormItem,
	FormLabel,
	FormMessage,
} from "@/components";
import { useEngineCompatDataProvider } from "@/components/actors";
import { RegionSelect } from "@/components/actors/region-select";

export const RunnerName = ConnectVercelForm.RunnerName;
export const Datacenter = function Datacenter() {
	const { control } = useFormContext();
	return (
		<FormField
			control={control}
			name="datacenter"
			render={({ field }) => (
				<FormItem>
					<FormLabel>Datacenter</FormLabel>
					<FormControl>
						<RegionSelect
							showAuto={false}
							onValueChange={field.onChange}
							value={field.value}
						/>
					</FormControl>
					<FormDescription>
						You can find the region your Railway runners are running
						in under <i>Settings &gt; Deploy</i>
					</FormDescription>
					<FormMessage />
				</FormItem>
			)}
		/>
	);
};

export const ConnectionCheck = function ConnectionCheck({
	provider,
}: {
	provider?: string;
}) {
	usePrefetchInfiniteQuery({
		...useEngineCompatDataProvider().runnersQueryOptions(),
		pages: Infinity,
	});

	const { data: queryData } = useInfiniteQuery({
		...useEngineCompatDataProvider().runnersQueryOptions(),
		refetchInterval: 1000,
		maxPages: Infinity,
	});

	const { watch } = useFormContext();

	const datacenter: string = watch("datacenter");
	const runnerName: string = watch("runnerName");

	const success = !!queryData?.find(
		(runner) =>
			runner.datacenter === datacenter && runner.name === runnerName,
	);

	const {
		field: { onChange },
	} = useController({ name: "success" });

	useEffect(() => {
		onChange(success);
	}, [success]);

	return (
		<div
			className={cn(
				"text-center h-24 text-muted-foreground text-sm overflow-hidden flex items-center justify-center",
				success && "text-primary-foreground",
			)}
		>
			{success ? (
				<>
					<Icon icon={faCheck} className="mr-1.5 text-primary" />{" "}
					Runner successfully connected
				</>
			) : (
				<div className="flex flex-col items-center gap-2">
					<div className="flex items-center">
						<Icon
							icon={faSpinnerThird}
							className="mr-1.5 animate-spin"
						/>{" "}
						Waiting for Runner to connect...
					</div>
				</div>
			)}
		</div>
	);
};
