import {
	CatchBoundary,
	createFileRoute,
	notFound,
	redirect,
} from "@tanstack/react-router";
import { Actors } from "@/app/actors";
import { BuildPrefiller } from "@/app/build-prefiller";

export const Route = createFileRoute(
	"/_context/_cloud/orgs/$organization/projects/$project/ns/$namespace/",
)({
	component: RouteComponent,
	beforeLoad: async ({ context }) => {
		if (context.__type !== "cloud") {
			throw notFound();
		}

		const result = await context.queryClient.fetchInfiniteQuery(
			context.dataProvider.buildsQueryOptions(),
		);

		const build = result.pages[0].builds[0];

		if (!build) {
			throw redirect({ from: Route.to, replace: true, to: "./connect" });
		}
	},
});

export function RouteComponent() {
	const { actorId, n } = Route.useSearch();

	return (
		<>
			<CatchBoundary getResetKey={() => actorId ?? "no-actor-id"}>
				<Actors actorId={actorId} />
				{!n ? <BuildPrefiller /> : null}
			</CatchBoundary>
		</>
	);
}
