<!--
@component
DetailPane — right-side drill-down for the currently-selected resource.

Renders an empty state when `resource` is null, otherwise a top bar
(name + status + action buttons + breadcrumb + FQDN), a five-tab control
(Overview / Env / Compose / Logs / Images), and a keyboard-hint strip.

Tab visibility depends on resource kind:
- `Database` → Compose tab hidden.
- A resource without `image_ref` AND without `docker_compose_raw` → Images
  tab hidden (no image to track).

`Images` content is owned by a sibling agent and intentionally left as a
placeholder slot; this component only renders the trigger so it appears in
the right position.

Props:
- `resource: Resource | null` — currently-selected row, or null.
-->
<script lang="ts">
	import type { Resource, ResourceDetail } from "$lib/api/types";
	import { api } from "$lib/api/client";
	import { Button } from "$lib/components/ui/button";
	import {
		Tabs,
		TabsList,
		TabsTrigger,
		TabsContent,
	} from "$lib/components/ui/tabs";
	import StatusBadge from "$lib/components/badges/StatusBadge.svelte";
	import { instance } from "$lib/stores/instance.svelte";
	import { toast } from "$lib/util/toast";
	import DeployDialog from "./DeployDialog.svelte";
	import OverviewTab from "./tabs/OverviewTab.svelte";
	import EnvTab from "./tabs/EnvTab.svelte";
	import BuildTab from "./tabs/BuildTab.svelte";
	import LogsTab from "./tabs/LogsTab.svelte";
	import ImagesTab from "./tabs/ImagesTab.svelte";

	let { resource }: { resource: Resource | null } = $props();

	type TabKey = "overview" | "env" | "compose" | "logs" | "images";
	let activeTab = $state<TabKey>("overview");
	let detail = $state<ResourceDetail | null>(null);
	let envs = $state<import("$lib/api/types").EnvVar[]>([]);
	let detailLoading = $state(false);
	let detailError = $state<string | null>(null);
	let deployOpen = $state(false);

	// Stable identity for the *selected* resource. Polling reassigns the
	// `resource` prop every 5s with refreshed status, but the underlying
	// row identity (uuid + kind) doesn't change. Effects below depend on
	// this derived value so they fire ONCE per selection switch, not on
	// every poll refresh.
	const selectionKey = $derived(
		resource ? `${resource.kind}:${resource.uuid}` : null,
	);

	// Fetch detail whenever the SELECTION switches. Polling status updates
	// no longer triggers a re-fetch (was hammering the API every 5s + reset
	// the user's open tab back to Overview).
	//
	// Envs come from a SEPARATE Coolify endpoint that's noticeably slower —
	// fetched independently so the detail pane renders immediately and the
	// Env tab populates in a second pass.
	$effect(() => {
		const key = selectionKey;
		detail = null;
		envs = [];
		detailError = null;
		if (key == null || resource == null) return;

		let cancelled = false;
		detailLoading = true;
		const uuid = resource.uuid;
		const kind = resource.kind;
		api
			.getResourceDetail(uuid, kind)
			.then((d) => {
				if (cancelled) return;
				detail = d;
			})
			.catch((err: unknown) => {
				if (cancelled) return;
				detailError = err instanceof Error ? err.message : String(err);
			})
			.finally(() => {
				if (!cancelled) detailLoading = false;
			});

		// Envs land in their own $state slot so resolution order doesn't matter
		// (previously: if envs resolved before detail, the merge was skipped
		// because detail was still null → tab label said "(N)" but content
		// rendered the empty-state).
		api
			.getResourceEnvs(uuid, kind)
			.then((next) => {
				if (cancelled) return;
				envs = next;
			})
			.catch((err) => {
				console.warn("envs fetch failed", err);
			});

		return () => {
			cancelled = true;
		};
	});

	// Reset the active tab to Overview when SWITCHING resources. Same trick:
	// drive off `selectionKey` so polling refreshes don't reset the tab.
	$effect(() => {
		void selectionKey;
		activeTab = "overview";
	});

	const showBuildTab = $derived(
		resource != null && resource.kind !== "Database",
	);
	// Tab label reflects the actual build pack so users don't see "Compose"
	// on a nixpacks app or vice-versa.
	const buildTabLabel = $derived.by(() => {
		const bp = (detail?.build_pack ?? resource?.build_pack ?? "").toLowerCase();
		if (bp === "dockercompose" || resource?.kind === "Service") return "Compose";
		if (bp === "dockerfile") return "Dockerfile";
		if (bp === "nixpacks" || bp === "railpack" || bp === "static") return "Build";
		return "Build";
	});
	// Images tab needs *something* to inspect — either a direct image_ref or
	// a compose file we can parse refs out of. If neither, hide the tab.
	const showImagesTab = $derived(
		resource != null &&
			(resource.image_ref != null ||
				(detail?.docker_compose_raw ?? null) != null),
	);

	const envCount = $derived(envs.length);
	// Use `!important` variants + dark-mode equivalents to override the
	// base TabsTrigger classes (which set dark:text-muted-foreground at
	// higher specificity in the cascade).
	const activeTabClass =
		"bg-background shadow-sm font-semibold !text-amber-400 dark:!text-amber-400";
	const breadcrumb = $derived.by(() => {
		if (!resource) return "";
		const parts: string[] = [];
		if (resource.project_name) parts.push(resource.project_name);
		if (resource.environment_name) parts.push(resource.environment_name);
		return parts.join(" / ");
	});

	async function handleRestart() {
		if (!resource) return;
		try {
			await api.restart(resource.uuid, resource.kind);
			toast.success("Restart triggered");
		} catch (err) {
			toast.error(
				"Restart failed",
				err instanceof Error ? err.message : String(err),
			);
		}
	}

	async function handleStop() {
		if (!resource) return;
		try {
			await api.stop(resource.uuid, resource.kind);
			toast.success("Stop triggered");
		} catch (err) {
			toast.error(
				"Stop failed",
				err instanceof Error ? err.message : String(err),
			);
		}
	}

	async function handleDeployConfirm(force: boolean) {
		deployOpen = false;
		if (!resource) return;
		try {
			await api.deploy(resource.uuid, force);
			toast.success(force ? "Deploy (force) triggered" : "Deploy triggered");
		} catch (err) {
			toast.error(
				"Deploy failed",
				err instanceof Error ? err.message : String(err),
			);
		}
	}
</script>

{#if resource == null}
	<div
		class="flex h-full items-center justify-center p-8 text-sm text-muted-foreground"
	>
		Select a resource to see details.
	</div>
{:else}
	<div class="flex h-full flex-col gap-3 p-4">
		<!-- Top bar: name + status + actions -->
		<div class="flex flex-col gap-2">
			<div class="flex items-center justify-between gap-3">
				<div class="flex items-center gap-2 min-w-0">
					<h2 class="truncate text-lg font-semibold">{resource.name}</h2>
					<StatusBadge status={resource.status} />
				</div>
				<div class="flex shrink-0 items-center gap-1">
					<Button variant="outline" size="sm" onclick={handleRestart}>
						Restart
					</Button>
					<Button variant="outline" size="sm" onclick={handleStop}>
						Stop
					</Button>
					<Button size="sm" onclick={() => (deployOpen = true)}>
						Deploy
					</Button>
				</div>
			</div>

			<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1 text-xs">
				{#if breadcrumb}
					<span class="text-muted-foreground">{breadcrumb}</span>
				{/if}
				{#if resource.fqdn}
					<a
						href={resource.fqdn}
						target="_blank"
						rel="noopener noreferrer"
						class="text-primary underline-offset-4 hover:underline"
					>
						{resource.fqdn}
					</a>
				{/if}
			</div>
		</div>

		<!-- Tabs -->
		<Tabs bind:value={activeTab} class="flex-1 min-h-0">
			<TabsList>
				<TabsTrigger
					value="overview"
					class={activeTab === "overview" ? activeTabClass : ""}
				>
					Overview
				</TabsTrigger>
				<TabsTrigger
					value="env"
					class={activeTab === "env" ? activeTabClass : ""}
				>
					Env{envCount > 0 ? ` (${envCount})` : ""}
				</TabsTrigger>
				{#if showBuildTab}
					<TabsTrigger
						value="compose"
						class={activeTab === "compose" ? activeTabClass : ""}
					>
						{buildTabLabel}
					</TabsTrigger>
				{/if}
				<TabsTrigger
					value="logs"
					class={activeTab === "logs" ? activeTabClass : ""}
				>
					Logs
				</TabsTrigger>
				{#if showImagesTab}
					<TabsTrigger
						value="images"
						class={activeTab === "images" ? activeTabClass : ""}
					>
						Images
					</TabsTrigger>
				{/if}
			</TabsList>

			<TabsContent value="overview" class="overflow-auto">
				{#if detailLoading && detail == null}
					<div class="text-sm text-muted-foreground">Loading…</div>
				{:else if detailError}
					<div class="text-sm text-destructive">{detailError}</div>
				{:else if detail}
					<OverviewTab {detail} />
				{/if}
			</TabsContent>

			<TabsContent value="env" class="overflow-auto">
				<EnvTab env={envs} />
			</TabsContent>

			{#if showBuildTab}
				<TabsContent value="compose" class="overflow-auto">
					{#if detail}
						<BuildTab {detail} />
					{:else if detailLoading}
						<div class="text-sm text-muted-foreground">Loading…</div>
					{/if}
				</TabsContent>
			{/if}

			<TabsContent value="logs" class="overflow-auto">
				<LogsTab
					uuid={resource.uuid}
					kind={resource.kind}
					active={activeTab === "logs"}
					containers={detail?.service_containers ?? []}
					instanceUrl={instance.url}
					projectUuid={detail?.project_uuid ?? resource.project_uuid ?? null}
					environmentUuid={detail?.environment_uuid ?? resource.environment_uuid ?? null}
					environmentName={detail?.environment_name ?? resource.environment_name ?? null}
				/>
			</TabsContent>

			{#if showImagesTab}
				<TabsContent value="images" class="overflow-auto">
					<ImagesTab
						dockerComposeRaw={detail?.docker_compose_raw ?? undefined}
						imageRef={resource.image_ref ?? undefined}
						lastDeployedAt={resource.last_deployed_at ?? null}
					/>
				</TabsContent>
			{/if}
		</Tabs>

		<!-- Keyboard hints -->
		<div
			class="flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-border pt-2 text-[0.7rem] text-muted-foreground"
		>
			<span><kbd class="font-mono">⌘R</kbd> restart</span>
			<span><kbd class="font-mono">⌘D</kbd> deploy</span>
			<span><kbd class="font-mono">⌘I</kbd> check images</span>
			<span><kbd class="font-mono">⌘L</kbd> logs</span>
		</div>
	</div>

	<DeployDialog
		open={deployOpen}
		onClose={() => (deployOpen = false)}
		onConfirm={handleDeployConfirm}
	/>
{/if}
