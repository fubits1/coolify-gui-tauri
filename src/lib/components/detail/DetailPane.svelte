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
	import { toast } from "$lib/util/toast.svelte";
	import DeployDialog from "./DeployDialog.svelte";
	import OverviewTab from "./tabs/OverviewTab.svelte";
	import EnvTab from "./tabs/EnvTab.svelte";
	import XIcon from "@lucide/svelte/icons/x";
	import ExternalLink from "@lucide/svelte/icons/external-link";
	import BuildTab from "./tabs/BuildTab.svelte";
	import ImagesTab from "./tabs/ImagesTab.svelte";

	let {
		instanceId,
		instanceUrl,
		resource,
		onClose,
	}: {
		/** Active Coolify instance id — routes every backend call. */
		instanceId: string;
		/** Active instance base URL — used to build the dashboard logs link. */
		instanceUrl: string;
		resource: Resource | null;
		/** Caller-provided close handler. Renders an X button inline with
		 *  the action buttons so it doesn't overlap with Restart/Stop/Deploy. */
		onClose?: () => void;
	} = $props();

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
		// CRITICAL: track ONLY `selectionKey`, not `resource`. The polling
		// loop reassigns the `resource` prop every 5s with a fresh object —
		// if this effect read `resource.uuid` reactively, every poll would
		// wipe `detail` + `envs` (causing the Images tab + Env count badge
		// to flicker out between blank state and refetch). selectionKey is
		// a stable `"${kind}:${uuid}"` string; parse uuid + kind from it
		// so we don't re-subscribe to the resource object itself.
		const key = selectionKey;
		detail = null;
		envs = [];
		detailError = null;
		if (key == null) return;
		const colon = key.indexOf(":");
		if (colon === -1) return;
		const kind = key.slice(0, colon) as Resource["kind"];
		const uuid = key.slice(colon + 1);

		let cancelled = false;
		detailLoading = true;
		api
			.getResourceDetail(instanceId, uuid, kind)
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
			.getResourceEnvs(instanceId, uuid, kind)
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
	// Always render the Images tab trigger. Disable while detail hasn't
	// loaded; once loaded, the ImagesTab itself renders an empty state
	// when no image refs are present.
	const imagesTabReady = $derived(detail != null);
	const hasAnyImage = $derived(
		(resource?.image_ref ?? null) != null ||
			(detail?.docker_compose_raw ?? null) != null,
	);

	/**
	 * Coolify dashboard logs deep-link.
	 *   {instance}/project/{project_uuid}/environment/{env_uuid_or_name}/{kind}/{uuid}/logs
	 * `null` when project_uuid + env identifier aren't both resolved yet (still
	 *  loading detail / enrichment). The link button hides itself then.
	 */
	const dashboardLogsUrl = $derived.by(() => {
		if (!resource || !instanceUrl) return null;
		const projectUuid = detail?.project_uuid ?? resource.project_uuid ?? null;
		const envSeg =
			detail?.environment_uuid ??
			resource.environment_uuid ??
			detail?.environment_name ??
			resource.environment_name ??
			null;
		if (!projectUuid || !envSeg) return null;
		const base = instanceUrl.replace(/\/$/, "");
		return `${base}/project/${projectUuid}/environment/${envSeg}/${resource.kind.toLowerCase()}/${resource.uuid}/logs`;
	});

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
			await api.restart(instanceId, resource.uuid, resource.kind);
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
			await api.stop(instanceId, resource.uuid, resource.kind);
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
			await api.deploy(instanceId, resource.uuid, force);
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
					{#if onClose}
						<button
							type="button"
							class="ml-1 inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
							aria-label="Close detail pane"
							title="Close (Esc)"
							onclick={onClose}
						>
							<XIcon class="size-4" />
						</button>
					{/if}
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
			<div class="flex items-center justify-between gap-2">
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
						Env
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
						value="images"
						class={activeTab === "images" ? activeTabClass : ""}
						disabled={!imagesTabReady}
						title={imagesTabReady ? undefined : "Loading detail…"}
					>
						Images
					</TabsTrigger>
				</TabsList>
				{#if dashboardLogsUrl}
					<a
						href={dashboardLogsUrl}
						target="_blank"
						rel="noopener noreferrer"
						class="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
						title="Open this resource's Logs in the Coolify dashboard"
					>
						Logs
						<ExternalLink class="size-3.5" />
					</a>
				{/if}
			</div>

			<TabsContent value="overview" class="overflow-auto">
				{#if detailLoading && detail == null}
					<div class="text-sm text-muted-foreground">Loading…</div>
				{:else if detailError}
					<div class="text-sm text-destructive">{detailError}</div>
				{:else if detail}
					<OverviewTab {detail} {resource} />
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

			<TabsContent value="images" class="overflow-auto">
				{#if !imagesTabReady}
					<div class="text-sm text-muted-foreground p-4">Loading…</div>
				{:else if !hasAnyImage}
					<div
						class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground"
					>
						This resource has no images to track.
					</div>
				{:else}
					<ImagesTab
						dockerComposeRaw={detail?.docker_compose_raw ?? undefined}
						imageRef={resource.image_ref ?? undefined}
						lastDeployedAt={resource.last_deployed_at ?? null}
					/>
				{/if}
			</TabsContent>
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
