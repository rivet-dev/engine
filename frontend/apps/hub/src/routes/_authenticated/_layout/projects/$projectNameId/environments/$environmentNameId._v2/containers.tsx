import { ErrorComponent } from "@/components/error-component";
import { ActorsActorDetailsWrapper } from "@/domains/project/components/actors/actors-actor-details-wrapper";
import { ActorsProvider } from "@/domains/project/components/actors/actors-provider";
import { useEnvironment } from "@/domains/project/data/environment-context";
import { useProject } from "@/domains/project/data/project-context";
import * as Layout from "@/domains/project/layouts/servers-layout";
import { actorBuildsCountQueryOptions } from "@/domains/project/queries";
import { useDialog } from "@/hooks/use-dialog";
import type { Rivet } from "@rivet-gg/api-full";
import { toRecord } from "@rivet-gg/components";
import {
	ActorFeature,
	ActorNotFound,
	ActorsActorEmptyDetails,
	ActorsListFiltersSchema,
	ActorsListPreview,
	ActorsViewContext,
	type Actor as StateActor,
	currentActorAtom,
	pickActorListFilters,
} from "@rivet-gg/components/actors";
import { GettingStarted } from "@rivet-gg/components/actors";
import { useSuspenseQuery } from "@tanstack/react-query";
import {
	type ErrorComponentProps,
	createFileRoute,
	useRouter,
} from "@tanstack/react-router";
import { zodValidator } from "@tanstack/zod-adapter";
import { useAtomValue } from "jotai";
import { z } from "zod";

function Actor() {
	const navigate = Route.useNavigate();
	const { tab } = Route.useSearch();

	const actor = useAtomValue(currentActorAtom);

	if (!actor) {
		return (
			<ActorNotFound
				features={[
					ActorFeature.Config,
					ActorFeature.Logs,
					ActorFeature.Metrics,
				]}
			/>
		);
	}

	return (
		<ActorsActorDetailsWrapper
			actor={actor}
			tab={tab}
			onTabChange={(tab) => {
				navigate({
					to: ".",
					search: (old) => ({ ...old, tab }),
				});
			}}
		/>
	);
}

const FIXED_TAGS = {};

const ACTORS_FILTER = (actor: Rivet.actors.Actor) =>
	toRecord(actor.tags).framework !== "rivetkit";

const ACTORS_VIEW_CONTEXT = {
	copy: {
		goToActor: "Go to Container",
		selectActor: (
			<>
				No Container selected.
				<br />
				<span className="text-sm text-muted-foreground">
					Select a Container from the list to view its details.
				</span>
			</>
		),
		showActorList: "Show Container list",
		noActorsFound: "No Containers found",
		createActor: "Create Container",
		createActorUsingForm: "Create Container using simple form",
		actorId: "Container ID",
		noMoreActors: "No more Containers to load.",
		noActorsMatchFilter: "No Containers match the filters.",
		showHiddenActors: "Show hidden Containers",

		createActorModal: {
			title: "Create Container",
			description:
				"Choose a build to create a Container from. Container will be created using default settings.",
		},

		actorNotFound: "Container not found",
		actorNotFoundDescription:
			"Change your filters to find the Container you are looking for.",

		gettingStarted: {
			title: "Getting Started with Containers",
			description:
				"Use a quick start guide to start deploying Containers to your environment.",
		},
	},
	canCreate: false,
};

const IS_ACTOR_INTERNAL = (actor: StateActor) =>
	toRecord(actor?.tags)?.type === "function";

function Content() {
	const { nameId: projectNameId } = useProject();
	const { nameId: environmentNameId } = useEnvironment();
	const { actorId, modal, ...restSearch } = Route.useSearch();

	const CreateActorDialog = useDialog.CreateActor.Dialog;
	const GoToActorDialog = useDialog.GoToActor.Dialog;
	const router = useRouter();
	const navigate = Route.useNavigate();

	function handleOpenChange(open: boolean) {
		router.navigate({
			to: ".",
			search: (old) => ({
				...old,
				modal: !open ? undefined : modal,
			}),
		});
	}

	const filters = pickActorListFilters(restSearch);

	return (
		<ActorsViewContext.Provider value={ACTORS_VIEW_CONTEXT}>
			<ActorsProvider
				projectNameId={projectNameId}
				environmentNameId={environmentNameId}
				actorId={actorId}
				fixedTags={FIXED_TAGS}
				filter={ACTORS_FILTER}
				isActorInternal={IS_ACTOR_INTERNAL}
				{...filters}
			>
				<ActorsListPreview>
					{actorId ? (
						<Actor />
					) : (
						<ActorsActorEmptyDetails
							features={[ActorFeature.Config, ActorFeature.Logs]}
						/>
					)}
				</ActorsListPreview>

				<CreateActorDialog
					dialogProps={{
						open: modal === "create-actor",
						onOpenChange: handleOpenChange,
					}}
				/>
				<GoToActorDialog
					onSubmit={(actorId) => {
						navigate({
							to: ".",
							search: (old) => ({
								...old,
								actorId,
								modal: undefined,
							}),
						});
					}}
					dialogProps={{
						open: modal === "go-to-actor",
						onOpenChange: handleOpenChange,
					}}
				/>
			</ActorsProvider>
		</ActorsViewContext.Provider>
	);
}

function ProjectActorsRoute() {
	const { nameId: projectNameId } = useProject();
	const { nameId: environmentNameId } = useEnvironment();
	const { tags, createdAt, destroyedAt } = Route.useSearch();

	const { data } = useSuspenseQuery({
		...actorBuildsCountQueryOptions({
			projectNameId,
			environmentNameId,
		}),
		refetchInterval: (query) =>
			(query.state.data?.builds.length || 0) > 0 ? false : 2000,
	});

	if (data === 0 && !tags && !createdAt && !destroyedAt) {
		return <GettingStarted />;
	}

	return (
		<div className="flex flex-col max-w-full w-full h-full bg-card">
			<Content />
		</div>
	);
}

const searchSchema = z
	.object({
		actorId: z.string().optional(),
		tab: z.string().optional(),
	})
	.merge(ActorsListFiltersSchema);

export const Route = createFileRoute(
	"/_authenticated/_layout/projects/$projectNameId/environments/$environmentNameId/_v2/containers",
)({
	validateSearch: zodValidator(searchSchema),
	staticData: {
		layout: "v2",
	},
	component: ProjectActorsRoute,
	pendingComponent: () => (
		<div className="p-4">
			<Layout.Content.Skeleton />
		</div>
	),
	errorComponent(props: ErrorComponentProps) {
		return (
			<div className="p-4">
				<div className="max-w-5xl mx-auto">
					<ErrorComponent {...props} />
				</div>
			</div>
		);
	},
});
