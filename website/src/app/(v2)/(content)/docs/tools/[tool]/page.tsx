import { redirect } from "next/navigation";

interface PageProps {
	params: {
		tool: string;
	};
}

export default function Page({ params }: PageProps) {
	// HACK: This page allows us to put tools in the sidebar but redirect to a different page. We can't use `href` since that will change which sidebar/tab is active when on the tool's page.
	redirect(`/docs/${params.tool}`);
}

export function generateStaticParams() {
	return [{ tool: "actors" }];
}
