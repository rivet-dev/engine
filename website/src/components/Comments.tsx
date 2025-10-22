"use client";

import Giscus from "@giscus/react";
import { cn } from "@rivet-gg/components";

interface CommentsProps {
	className?: string;
}

export function Comments({ className }: CommentsProps) {
	return (
		<div className={cn(className, "mt-4 no-prose")}>
			<Giscus
				id="comments"
				repo="rivet-dev/engine"
				repoId="R_kgDOJwPLtw"
				category="Comments"
				categoryId="DIC_kwDOJwPLt84Co34O"
				mapping="pathname"
				strict="0"
				reactionsEnabled="1"
				emitMetadata="0"
				inputPosition="top"
				theme="https://rivet.dev/giscus.css"
				lang="en"
				loading="lazy"
			/>
		</div>
	);
}
