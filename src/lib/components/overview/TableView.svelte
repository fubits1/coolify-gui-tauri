<!--
@component
TableView — dense, sortable/groupable/searchable table of resources.

Header bar contains a search input, group selector, sort selector,
sort-direction toggle, and a "Check all images" action. Filter, sort,
and group are derived locally from the input list so the parent can stay
ignorant of the view-level UI state.

Action buttons (Restart / Stop / Deploy) are disabled when the underlying
state would make the operation a no-op (e.g. Stop on an `exited` resource).
Each action stops row-click propagation so it doesn't double-fire selection.

Props:
- `resources: Resource[]` — flat list of resources to render
- `selectedUuid: string | null` — currently selected resource (for highlight)
- `onSelect: (uuid: string) => void` — fired when a row is clicked
- `onRestart / onStop / onDeploy: (uuid, kind) => void` — per-row actions
- `onCheckImages: () => void` — fired by the header "Check all images" button
-->
<script lang="ts">
	import type { Resource, ResourceKind } from "$lib/api/types";
	import StatusBadge from "$lib/components/badges/StatusBadge.svelte";
	import ImageBadge from "$lib/components/badges/ImageBadge.svelte";
	import { Button } from "$lib/components/ui/button";
	import { Input } from "$lib/components/ui/input";
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
	} from "$lib/components/ui/table";
	import { relativeTime } from "$lib/util/relative-time";
	import {
		ArrowDown,
		ArrowUp,
		Play,
		RefreshCw,
		RotateCw,
		Square,
	} from "@lucide/svelte";

	type GroupBy = "none" | "project" | "environment" | "status";
	type SortBy = "name" | "last_deploy" | "status";
	type SortDir = "asc" | "desc";

	let {
		resources,
		selectedUuid,
		onSelect,
		onRestart,
		onStop,
		onDeploy,
		onCheckImages,
	}: {
		resources: Resource[];
		selectedUuid: string | null;
		onSelect: (uuid: string) => void;
		onRestart: (uuid: string, kind: ResourceKind) => void;
		onStop: (uuid: string, kind: ResourceKind) => void;
		onDeploy: (uuid: string, kind: ResourceKind) => void;
		onCheckImages: () => void;
	} = $props();

	let search = $state("");
	let groupBy = $state<GroupBy>("none");
	let sortBy = $state<SortBy>("name");
	let sortDir = $state<SortDir>("asc");

	const filtered = $derived.by(() => {
		const q = search.trim().toLowerCase();
		if (!q) return resources;
		return resources.filter((r) => {
			const hay = `${r.name} ${r.fqdn ?? ""} ${r.image_ref ?? ""}`.toLowerCase();
			return hay.includes(q);
		});
	});

	const sorted = $derived.by(() => {
		const list = filtered.slice();
		const dir = sortDir === "asc" ? 1 : -1;
		list.sort((a, b) => {
			switch (sortBy) {
				case "name":
					return a.name.localeCompare(b.name) * dir;
				case "last_deploy": {
					const av = a.last_deployed_at ?? "";
					const bv = b.last_deployed_at ?? "";
					if (av === bv) return 0;
					return (av < bv ? -1 : 1) * dir;
				}
				case "status":
					return a.status.state.localeCompare(b.status.state) * dir;
			}
		});
		return list;
	});

	const grouped = $derived.by<Record<string, Resource[]>>(() => {
		if (groupBy === "none") return { "": sorted };
		const groups: Record<string, Resource[]> = {};
		for (const r of sorted) {
			let key: string;
			switch (groupBy) {
				case "project":
					key = r.project_name ?? "(no project)";
					break;
				case "environment":
					key = r.environment_name ?? "(no environment)";
					break;
				case "status":
					key = r.status.state || "(unknown)";
					break;
			}
			(groups[key] ??= []).push(r);
		}
		return groups;
	});

	const groupKeys = $derived(Object.keys(grouped).sort());

	function toggleDir() {
		sortDir = sortDir === "asc" ? "desc" : "asc";
	}

	// Native-select base classes that mirror Input styling so the controls
	// align visually without pulling in a shadcn Select primitive.
	const selectClass =
		"h-8 rounded-lg border border-input bg-background px-2 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50";
</script>

<div class="flex flex-col gap-3">
	<div class="flex flex-wrap items-center gap-2">
		<Input
			type="search"
			placeholder="Search name, FQDN, image…"
			bind:value={search}
			class="h-8 max-w-xs"
		/>

		<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
			Group
			<select class={selectClass} bind:value={groupBy}>
				<option value="none">None</option>
				<option value="project">Project</option>
				<option value="environment">Environment</option>
				<option value="status">Status</option>
			</select>
		</label>

		<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
			Sort
			<select class={selectClass} bind:value={sortBy}>
				<option value="name">Name</option>
				<option value="last_deploy">Last deploy</option>
				<option value="status">Status</option>
			</select>
		</label>

		<Button
			variant="outline"
			size="icon-sm"
			aria-label="Toggle sort direction"
			title={sortDir === "asc" ? "Ascending" : "Descending"}
			onclick={toggleDir}
		>
			{#if sortDir === "asc"}
				<ArrowUp />
			{:else}
				<ArrowDown />
			{/if}
		</Button>

		<div class="ml-auto">
			<Button variant="outline" size="sm" onclick={onCheckImages}>
				<RefreshCw />
				Check all images
			</Button>
		</div>
	</div>

	<Table>
		<TableHeader>
			<TableRow>
				<TableHead>Name</TableHead>
				<TableHead>Type</TableHead>
				<TableHead>Status</TableHead>
				<TableHead>FQDN</TableHead>
				<TableHead>Last deploy</TableHead>
				<TableHead>Images</TableHead>
				<TableHead class="text-right">Actions</TableHead>
			</TableRow>
		</TableHeader>
		<TableBody>
			{#each groupKeys as key (key)}
				{#if groupBy !== "none"}
					<TableRow class="bg-muted/40 hover:bg-muted/40">
						<TableCell
							colspan={7}
							class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
						>
							{key} · {grouped[key].length}
						</TableCell>
					</TableRow>
				{/if}
				{#each grouped[key] as r (r.uuid)}
					{@const isExited = r.status.state === "exited"}
					<TableRow
						data-state={r.uuid === selectedUuid ? "selected" : undefined}
						class="cursor-pointer"
						onclick={() => onSelect(r.uuid)}
					>
						<TableCell class="font-medium">{r.name}</TableCell>
						<TableCell class="text-muted-foreground">{r.kind}</TableCell>
						<TableCell>
							<StatusBadge status={r.status} />
						</TableCell>
						<TableCell class="max-w-[16rem] truncate text-muted-foreground">
							{r.fqdn ?? "—"}
						</TableCell>
						<TableCell class="text-muted-foreground">
							{relativeTime(r.last_deployed_at)}
						</TableCell>
						<TableCell>
							<ImageBadge stale={0} total={0} checkedAt={null} />
						</TableCell>
						<TableCell class="text-right">
							<div class="inline-flex items-center gap-1">
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
							</div>
						</TableCell>
					</TableRow>
				{/each}
			{/each}
			{#if sorted.length === 0}
				<TableRow>
					<TableCell colspan={7} class="py-8 text-center text-sm text-muted-foreground">
						No resources match your filters.
					</TableCell>
				</TableRow>
			{/if}
		</TableBody>
	</Table>
</div>
