import { CreateOrganization } from "@clerk/clerk-react";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { match } from "ts-pattern";
import CreateNamespacesFrameContent from "@/app/dialogs/create-namespace-frame";
import { InspectorRoot } from "@/app/inspector-root";
import { Logo } from "@/app/logo";
import { RouteLayout } from "@/app/route-layout";
import { Card } from "@/components";

export const Route = createFileRoute("/_context/")({
	component: () =>
		match(__APP_TYPE__)
			.with("cloud", () => <CloudRoute />)
			.with("engine", () => <EngineRoute />)
			.with("inspector", () => <InspectorRoute />)
			.exhaustive(),
	beforeLoad: async ({ context, search }) => {
		return match(context)
			.with({ __type: "cloud" }, () => {
				const { organization } = context.clerk ?? {};
				if (!organization) {
					return;
				}
				throw redirect({
					to: "/orgs/$organization",
					params: { organization: organization?.id },
				});
			})
			.with({ __type: "engine" }, async (ctx) => {
				const result = await ctx.queryClient.fetchInfiniteQuery(
					ctx.dataProvider.namespacesQueryOptions(),
				);
				const firstNamespace = result.pages[0]?.namespaces[0];
				if (!firstNamespace) {
					return;
				}
				throw redirect({
					to: "/ns/$namespace",
					params: { namespace: firstNamespace.name },
				});
			})
			.with({ __type: "inspector" }, async (ctx) => {
				if (!search.t || !search.u) {
					return { connectedInPreflight: false };
				}

				try {
					const result = await ctx.queryClient.fetchQuery(
						ctx.dataProvider.statusQueryOptions(),
					);

					return { connectedInPreflight: result === true };
				} catch {
					return { connectedInPreflight: false };
				}
			})
			.exhaustive();
	},
});

function CloudRoute() {
	return (
		<RouteLayout>
			<div className="bg-card h-full border my-2 mr-2 rounded-lg">
				<div className="mt-2 flex flex-col items-center justify-center h-full">
					<div className="w-full sm:w-96">
						<CreateOrganization
							hideSlug
							appearance={{
								variables: {
									colorBackground: "hsl(var(--card))",
								},
							}}
						/>
					</div>
				</div>
			</div>
		</RouteLayout>
	);
}

function EngineRoute() {
	return (
		<div className="flex min-h-screen flex-col items-center justify-center bg-background py-4">
			<div className="flex flex-col items-center gap-6 w-full">
				<Logo className="h-10 mb-4" />
				<Card className="w-full sm:w-96">
					<CreateNamespacesFrameContent />
				</Card>
			</div>
		</div>
	);
}

function InspectorRoute() {
	return <InspectorRoot />;
}
