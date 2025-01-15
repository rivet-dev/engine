import { converEmojiToUriFriendlyString } from "@/lib/emoji";
import { AssetImage, Flex, WithTooltip } from "@rivet-gg/components";
import { Icon, type IconProp, faComputer } from "@rivet-gg/icons";
import { useSuspenseQuery } from "@tanstack/react-query";
import { projectRegionQueryOptions } from "../../queries";

export const REGION_ICON: Record<string, string | IconProp> = {
	local: faComputer,
	unknown: "❓",
	atlanta: "🇺🇸", // Atlanta
	san_francisco: "🇺🇸", // San Francisco
	frankfurt: "🇩🇪", // Frankfurt
	sydney: "🇦🇺", // Sydney
	tokyo: "🇯🇵", // Tokyo
	mumbai: "🇮🇳", // Mumbai
	toronto: "🇨🇦", // Toronto
	washington_dc: "🇺🇸", // Washington DC
	dallas: "🇺🇸", // Dallas
	new_york_city: "🇺🇸", // Newark
	london: "🇬🇧", // London
	singapore: "🇸🇬", // Singapore
	amsterdam: "🇳🇱", // Amsterdam
	chicago: "🇺🇸", // Chicago
	bangalore: "🇮🇳", // Bangalore
	paris: "🇫🇷", // Paris
	seattle: "🇺🇸", // Seattle
	stockholm: "🇸🇪", // Stockholm
	newark: "🇺🇸", // Newark
	sao_paulo: "🇧🇷", // Sao Paulo
	chennai: "🇮🇳", // Chennai
	osaka: "🇯🇵", // Osaka
	milan: "🇮🇹", // Milan
	miami: "🇺🇸", // Miami
	jakarta: "🇮🇩", // Jakarta
	los_angeles: "🇺🇸", // Los Angeles
	atl: "🇺🇸", // Atlanta
	sfo: "🇺🇸", // San Francisco
	fra: "🇩🇪", // Frankfurt
	syd: "🇦🇺", // Sydney
	tok: "🇯🇵", // Tokyo
	mba: "🇮🇳", // Mumbai
	tor: "🇨🇦", // Toronto
	dca: "🇺🇸", // Washington DC
	dfw: "🇺🇸", // Dallas
	ewr: "🇺🇸", // Newark
	lon: "🇬🇧", // London
	sgp: "🇸🇬", // Singapore
	lax: "🇺🇸", // Los Angeles
	osa: "🇯🇵", // Osaka
	gru: "🇧🇷", // Sao Paulo
	bom: "🇮🇳", // Mumbai
	sin: "🇸🇬", // Singapore
};

export const REGION_LABEL: Record<string, string> = {
	local: "Local",
	unknown: "Unknown",
	atlanta: "Atlanta, Georgia, USA",
	san_francisco: "San Francisco",
	frankfurt: "Frankfurt",
	sydney: "Sydney",
	tokyo: "Tokyo",
	mumbai: "Mumbai",
	toronto: "Toronto",
	washington_dc: "Washington DC",
	dallas: "Dallas",
	new_york_city: "New York City",
	london: "London",
	singapore: "Singapore",
	amsterdam: "Amsterdam",
	chicago: "Chicago",
	bangalore: "Bangalore",
	paris: "Paris",
	seattle: "Seattle",
	stockholm: "Stockholm",
	newark: "Newark",
	sao_paulo: "Sao Paulo",
	chennai: "Chennai",
	osaka: "Osaka",
	milan: "Milan",
	miami: "Miami",
	jakarta: "Jakarta",
	los_angeles: "Los Angeles",
	atl: "Atlanta, Georgia, USA",
	sfo: "San Francisco, California, USA",
	fra: "Frankfurt, Germany",
	syd: "Sydney, Australia",
	tok: "Tokyo, Japan",
	mba: "Mumbai, India",
	tor: "Toronto, Canada",
	dca: "Washington DC, USA",
	dfw: "Dallas, Texas, USA",
	ewr: "Newark, New Jersey, USA",
	lon: "London, UK",
	sgp: "Singapore",
	lax: "Los Angeles, California, USA",
	osa: "Osaka, Japan",
	gru: "Sao Paulo",
	bom: "Mumbai, India",
	sin: "Singapore",
};

interface LobbyRegionProps {
	regionId: string;
	projectId: string;
	showLabel?: boolean | "abbreviated";
}

export function getRegionKey(regionNameId: string | undefined) {
	// HACK: Remove prefix for old regions with format `lnd-atl`
	const regionIdSplit = (regionNameId || "").split("-");
	return regionIdSplit[regionIdSplit.length - 1];
}

export function RegionIcon({
	region = "",
	...props
}: { region: string | undefined; className?: string }) {
	const regionIcon = REGION_ICON[region] ?? REGION_ICON.unknown;

	if (typeof regionIcon === "string") {
		return (
			<AssetImage
				{...props}
				src={`/icons/emoji/${converEmojiToUriFriendlyString(regionIcon)}.svg`}
			/>
		);
	}

	return <Icon {...props} icon={regionIcon} />;
}

export function LobbyRegion({
	projectId,
	regionId,
	showLabel,
}: LobbyRegionProps) {
	const { data: region } = useSuspenseQuery(
		projectRegionQueryOptions({ projectId, regionId }),
	);

	const regionKey = getRegionKey(region?.regionNameId);

	if (showLabel) {
		return (
			<Flex gap="2" items="center" justify="center">
				<RegionIcon region={regionKey} className="w-5 min-w-5" />
				<span>
					{showLabel === "abbreviated"
						? regionKey
						: (REGION_LABEL[regionKey] ?? REGION_LABEL.unknown)}
				</span>
			</Flex>
		);
	}

	return (
		<WithTooltip
			content={REGION_LABEL[regionKey] ?? REGION_LABEL.unknown}
			trigger={
				<Flex gap="2" items="center" justify="center">
					<RegionIcon region={regionKey} className="w-5 min-w-5" />
				</Flex>
			}
		/>
	);
}

export function LobbyRegionIcon({
	regionNameId,
	className,
}: { regionNameId: string; className?: string }) {
	return <RegionIcon region={regionNameId} className={className} />;
}

export function LobbyRegionName({ regionNameId }: { regionNameId: string }) {
	const regionKey = getRegionKey(regionNameId);
	return REGION_LABEL[regionKey] ?? REGION_LABEL.unknown;
}
