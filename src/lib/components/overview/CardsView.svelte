<!--
@component
CardsView — responsive grid of resource cards. Drop-in alternative to
`TableView`: same prop shape, same callbacks, same action semantics. Two
columns at md, three at xl.

Each card surfaces name + status, project/env breadcrumb, FQDN or image
ref, an image-freshness badge with last-deploy time, and the per-row
action buttons. Selection is shown with a primary-coloured ring. Action
buttons stop propagation so they don't double-fire the card click.

Props: identical to `TableView` (see that component for full prop docs).
-->
<script lang="ts">
	import type { Resource, ResourceKind } from "$lib/api/types";
	import StatusBadge from "$lib/components/badges/StatusBadge.svelte";
	import ImageBadge from "$lib/components/badges/ImageBadge.svelte";
	import { imageCache } from "$lib/stores/image-cache.svelte";
	import { Button } from "$lib/components/ui/button";
	import {
		Card,
		CardContent,
		CardFooter,
		CardHeader,
		CardTitle,
	} from "$lib/components/ui/card";
	import { relativeTime } from "$lib/util/relative-time";
	import { Play, RotateCw, Square } from "@lucide/svelte";

	let {
		resources,
		selectedUuid,
		onSelect,
		onRestart,
		onStop,
		onDeploy,
	}: {
		resources: Resource[];
		selectedUuid: string | null;
		onSelect: (uuid: string) => void;
		onRestart: (uuid: string, kind: ResourceKind) => void;
		onStop: (uuid: string, kind: ResourceKind) => void;
		onDeploy: (uuid: string, kind: ResourceKind) => void;
		onCheckImages: () => void;
	} = $props();

	function imageBadgeFor(
		refs: string[],
		lastDeployedAt?: string | null,
	): {
		stale: number;
		unknown: number;
		total: number;
		checkedAt: number | null;
	} {
		if (!refs || refs.length === 0) {
			return { stale: 0, unknown: 0, total: 0, checkedAt: null };
		}
		let stale = 0;
		let unknown = 0;
		let earliest: number | null = null;
		for (const ref of refs) {
			const state = imageCache.isStale(ref, lastDeployedAt);
			if (state === "newer-available") stale += 1;
			else if (state === "unknown") unknown += 1;
			const entry = imageCache.entries[ref];
			if (entry) {
				earliest =
					earliest === null
						? entry.checked_at
						: Math.min(earliest, entry.checked_at);
			}
		}
		return { stale, unknown, total: refs.length, checkedAt: earliest };
	}
</script>

<div class="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
	{#each resources as r (r.uuid)}
		{@const isSelected = r.uuid === selectedUuid}
		{@const isExited = r.status.state === "exited"}
		<button
			type="button"
			class="text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/50 rounded-xl {isSelected
				? 'ring-2 ring-primary'
				: ''}"
			onclick={() => onSelect(r.uuid)}
		>
			<Card class="h-full">
				<CardHeader class="flex flex-row items-start justify-between gap-2">
					<CardTitle class="truncate">{r.name}</CardTitle>
					<StatusBadge status={r.status} />
				</CardHeader>
				<CardContent class="flex flex-col gap-1 text-sm text-muted-foreground">
					<div class="text-xs uppercase tracking-wide">
						{r.kind}
						{#if r.project_name}· {r.project_name}{/if}
						{#if r.environment_name}· {r.environment_name}{/if}
					</div>
					<div class="truncate text-foreground">
						{r.fqdn ?? r.image_ref ?? "—"}
					</div>
					{@const badge = imageBadgeFor(r.image_refs ?? [], r.last_deployed_at)}
					<div class="flex items-center gap-2 text-xs">
						<ImageBadge
							stale={badge.stale}
							unknown={badge.unknown}
							total={badge.total}
							checkedAt={badge.checkedAt}
						/>
						<span title="Apps: real last-deploy from /deployments history. Non-running services: last-seen heartbeat.">
							{#if r.last_deployed_at}
								{relativeTime(r.last_deployed_at)}
							{:else if r.status.state !== "running" && r.last_online_at}
								{relativeTime(r.last_online_at)}
							{:else}
								—
							{/if}
						</span>
					</div>
				</CardContent>
				<CardFooter class="flex justify-end gap-1 pb-4">
					<Button
						variant="ghost"
						size="icon-sm"
						aria-label="Restart"
						title="Restart"
						disabled={isExited}
						onclick={(e) => {
							e.stopPropagation();
							onRestart(r.uuid, r.kind);
						}}
					>
						<RotateCw />
					</Button>
					<Button
						variant="ghost"
						size="icon-sm"
						aria-label="Stop"
						title="Stop"
						disabled={isExited}
						onclick={(e) => {
							e.stopPropagation();
							onStop(r.uuid, r.kind);
						}}
					>
						<Square />
					</Button>
					<Button
						variant="ghost"
						size="icon-sm"
						aria-label="Deploy"
						title="Deploy"
						onclick={(e) => {
							e.stopPropagation();
							onDeploy(r.uuid, r.kind);
						}}
					>
						<Play />
					</Button>
				</CardFooter>
			</Card>
		</button>
	{/each}
	{#if resources.length === 0}
		<div class="col-span-full rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
			No resources to display.
		</div>
	{/if}
</div>
