import { Button, DocsSheet, ScrollArea } from "@rivet-gg/components";
import { Icon, faBooks } from "@rivet-gg/icons";
import { useRef } from "react";
import { useActorRpcs, useActorWorker } from "../worker/actor-worker-context";
import { ActorConsoleMessage } from "./actor-console-message";
import { ReplInput, type ReplInputRef, replaceCode } from "./repl-input";

export function ActorConsoleInput() {
	const worker = useActorWorker();
	const rpcs = useActorRpcs();
	const ref = useRef<ReplInputRef>(null);

	return (
		<div className="border-t w-full max-h-20 flex flex-col">
			<ScrollArea className="w-full flex-1">
				<ActorConsoleMessage variant="input" className="border-b-0">
					<ReplInput
						ref={ref}
						className="w-full"
						rpcs={rpcs}
						onRun={(code) => {
							worker.run(code);
						}}
					/>
				</ActorConsoleMessage>
			</ScrollArea>
			<div className="flex items-center  mt-1 pb-1">
				<DocsSheet title="Console" path="docs/ui" showTooltip>
					<Button
						size="icon-xs"
						variant="outline"
						className="place-self-center ml-4 mr-3 "
					>
						<Icon icon={faBooks} />
					</Button>
				</DocsSheet>
				<div className="flex flex-wrap gap-1">
					{rpcs.map((rpc) => (
						<Button
							variant="outline"
							size="xs"
							key={rpc}
							onClick={() => {
								if (!ref.current?.view) {
									return;
								}
								replaceCode(ref.current.view, `actor.${rpc}(`);
							}}
							className="rounded-full"
							startIcon={
								<span className="bg-secondary px-1 rounded-full">
									RPC
								</span>
							}
						>
							<span className="font-mono-console">
								actor.{rpc}(...)
							</span>
						</Button>
					))}
				</div>
			</div>
		</div>
	);
}
