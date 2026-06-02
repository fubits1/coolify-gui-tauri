<script lang="ts">
	import ConnectScreen from "$lib/components/onboarding/ConnectScreen.svelte";
	import TableView from "$lib/components/overview/TableView.svelte";
	import CardsView from "$lib/components/overview/CardsView.svelte";
	import ViewToggle from "$lib/components/overview/ViewToggle.svelte";
	import DetailPane from "$lib/components/detail/DetailPane.svelte";
	import DeployDialog from "$lib/components/detail/DeployDialog.svelte";
	import ConnectionStrip from "$lib/components/shell/ConnectionStrip.svelte";
	import InstanceTabStrip from "$lib/components/shell/InstanceTabStrip.svelte";
	import { Button } from "$lib/components/ui/button";
	import { RefreshCw, Settings } from "@lucide/svelte";
	import { api } from "$lib/api/client";
	import type { ResourceKind } from "$lib/api/types";
	import { instances } from "$lib/stores/instances.svelte";
	import {
		resourcesRegistry,
		pollingController,
		type ResourcesStore,
	} from "$lib/stores/resources.svelte";
	import {
		connectionRegistry,
		type ConnectionStore,
	} from "$lib/stores/connection.svelte";
	import { imageCache, isNewerState } from "$lib/stores/image-cache.svelte";
	import { runStartupCheck } from "$lib/util/image-scheduler";
	import { installShortcuts } from "$lib/util/shortcuts";
	import { toast } from "$lib/util/toast.svelte";

	// Kick off persisted-state loads on mount. Both stores are reactive once
	// these resolve, so the UI rerenders automatically. `instances.load()`
	// also hydrates each instance's keyring token + sets its `ready` flag
	// before assigning `this.list`, so we never have to set $state from
	// inside a $effect (banned by `code-style-svelte`).
	void instances.load();
	void imageCache.load();

	const activeInstance = $derived(instances.active);
	const activeResources: ResourcesStore | null = $derived(
		activeInstance ? resourcesRegistry.get(activeInstance.id) : null,
	);
	const activeConnection: ConnectionStore | null = $derived(
		activeInstance ? connectionRegistry.get(activeInstance.id) : null,
	);
	const activeReady = $derived(activeInstance?.ready === true);

	let viewMode = $state<"table" | "cards">("table");
	let deployTarget = $state<{ uuid: string; kind: ResourceKind } | null>(null);
	let addingInstance = $state(false);

	// Polling lifecycle: only the active instance polls. `pollingController`
	// holds the "currently running id" in a NON-reactive private field, so
	// we can call switchTo() from a $effect without violating "no $effect
	// writes to $state". The store's own list/loading/etc are $state, but
	// that's a side effect of the polling running — not a re-render of
	// reactive data assigned by this effect.
	$effect(() => {
		const next = activeInstance;
		void pollingController.switchTo(next?.ready ? next.id : null);
	});

	// Image-freshness scheduler runs ONCE per (instance, app boot). The
	// scheduler itself already has a 24h cache gate, but firing it every
	// time `activeResources.list` mutates (every 5s poll) is wasteful: it
	// reads the on-disk cache, filters, and toasts a summary even when no
	// new check is due. Track which instances we've already kicked off
	// during this app session and skip subsequent calls.
	const startupCheckedInstances = new Set<string>();
	$effect(() => {
		const inst = activeInstance;
		const store = activeResources;
		if (!inst || !store || store.list.length === 0) return;
		if (startupCheckedInstances.has(inst.id)) return;
		startupCheckedInstances.add(inst.id);
		void runStartupCheck(store.list);
	});

	function handleConnected(id: string) {
		addingInstance = false;
		void instances.setActive(id);
	}

	async function handleRestart(uuid: string, kind: ResourceKind) {
		if (!activeInstance || !activeResources) return;
		const instId = activeInstance.id;
		await toast.promise(api.restart(instId, uuid, kind), {
			loading: "Restarting…",
			success: "Restart triggered",
			error: (e) =>
				`Restart failed: ${e instanceof Error ? e.message : String(e)}`,
		});
		await activeResources.refresh();
	}

	async function handleStop(uuid: string, kind: ResourceKind) {
		if (!activeInstance || !activeResources) return;
		const instId = activeInstance.id;
		await toast.promise(api.stop(instId, uuid, kind), {
			loading: "Stopping…",
			success: "Stop triggered",
			error: (e) =>
				`Stop failed: ${e instanceof Error ? e.message : String(e)}`,
		});
		await activeResources.refresh();
	}

	function handleDeploy(uuid: string, kind: ResourceKind) {
		deployTarget = { uuid, kind };
	}

	async function handleDeployConfirm(force: boolean) {
		const target = deployTarget;
		deployTarget = null;
		if (!target || !activeInstance || !activeResources) return;
		const instId = activeInstance.id;
		await toast.promise(api.deploy(instId, target.uuid, force), {
			loading: "Deploying…",
			success: force ? "Deploy (force) triggered" : "Deploy triggered",
			error: (e) =>
				`Deploy failed: ${e instanceof Error ? e.message : String(e)}`,
		});
		await activeResources.refresh();
	}

	async function handleCheckAllImages() {
		if (!activeResources) return;
		const refs = new Set<string>();
		const refDeployTimes = new Map<string, string | null>();
		for (const r of activeResources.list) {
			for (const ref of r.image_refs ?? []) {
				if (!ref || ref.trim().length === 0) continue;
				refs.add(ref);
				if (!refDeployTimes.has(ref)) {
					refDeployTimes.set(ref, r.last_deployed_at ?? null);
				}
			}
		}
		if (refs.size === 0) {
			toast.info("No image refs to check");
			return;
		}
		const all = [...refs];
		toast.info(`Checking ${all.length} images for updates…`);
		await imageCache.checkMany(all);
		const newer = all.filter((ref) =>
			isNewerState(imageCache.isStale(ref, refDeployTimes.get(ref) ?? null)),
		).length;
		if (newer > 0) {
			toast.warning(
				`${newer} of ${all.length} image${newer === 1 ? " has" : "s have"} a newer version available`,
			);
		} else {
			toast.success(
				`All ${all.length} image${all.length === 1 ? " is" : "s are"} up to date`,
			);
		}
	}

	$effect(() => {
		const store = activeResources;
		const selected = store?.selectedResource ?? null;
		const cleanup = installShortcuts({
			onRestart: selected
				? () => void handleRestart(selected.uuid, selected.kind)
				: undefined,
			onDeploy: selected
				? () => {
						deployTarget = { uuid: selected.uuid, kind: selected.kind };
					}
				: undefined,
			onCheckImages: selected?.image_ref
				? () => void imageCache.checkMany([selected.image_ref as string])
				: undefined,
			onLogs: undefined,
			onEscape: store && selected ? () => store.select(null) : undefined,
		});
		return cleanup;
	});
</script>

{#if instances.list.length === 0}
	<ConnectScreen onConnected={handleConnected} />
{:else}
	<div class="flex h-screen flex-col">
		<InstanceTabStrip onAddRequested={() => (addingInstance = true)} />

		{#if addingInstance}
			<div class="border-b border-border bg-muted/10">
				<ConnectScreen embed onConnected={handleConnected} />
				<div class="flex justify-end px-4 pb-2">
					<Button
						variant="ghost"
						size="sm"
						onclick={() => (addingInstance = false)}
					>
						Cancel
					</Button>
				</div>
			</div>
		{/if}

		{#if activeInstance && activeReady && activeResources && activeConnection}
			<ConnectionStrip
				state={activeConnection.state}
				alias={activeInstance.teamName
					? `${activeInstance.alias} · ${activeInstance.teamName}`
					: activeInstance.alias}
				retryInSec={activeConnection.reconnectInSec ?? 0}
			/>

			<div class="flex items-center justify-between gap-2 border-b border-border px-4 py-2">
				<h1 class="text-sm font-semibold">Resources</h1>
				<div class="flex items-center gap-2">
					<ViewToggle mode={viewMode} onChange={(m) => (viewMode = m)} />
					<Button
						variant="outline"
						size="sm"
						onclick={() => void activeResources.refresh()}
						disabled={activeResources.loading}
					>
						<RefreshCw />
						Refresh
					</Button>
					<Button
						variant="outline"
						size="icon-sm"
						aria-label="Settings"
						title="Settings"
						href="/settings"
					>
						<Settings />
					</Button>
				</div>
			</div>

			<div class="flex min-h-0 flex-1">
				<div class="flex min-h-0 min-w-0 flex-1 flex-col p-4">
					{#if viewMode === "table"}
						<TableView
							resources={activeResources.list}
							selectedUuid={activeResources.selectedUuid}
							onSelect={(uuid) => activeResources.select(uuid)}
							onRestart={handleRestart}
							onStop={handleStop}
							onDeploy={handleDeploy}
							onCheckImages={handleCheckAllImages}
						/>
					{:else}
						<CardsView
							resources={activeResources.list}
							selectedUuid={activeResources.selectedUuid}
							onSelect={(uuid) => activeResources.select(uuid)}
							onRestart={handleRestart}
							onStop={handleStop}
							onDeploy={handleDeploy}
							onCheckImages={handleCheckAllImages}
						/>
					{/if}
				</div>

				{#if activeResources.selectedResource}
					<aside
						class="relative w-1/2 min-w-[24rem] shrink-0 overflow-auto border-l border-border"
					>
						<DetailPane
							instanceId={activeInstance.id}
							instanceUrl={activeInstance.url}
							resource={activeResources.selectedResource}
							onClose={() => activeResources.select(null)}
						/>
					</aside>
				{/if}
			</div>
		{:else if activeInstance && !activeReady}
			<div class="flex flex-1 items-center justify-center text-sm text-muted-foreground">
				Hydrating {activeInstance.alias}…
			</div>
		{/if}
	</div>

	<DeployDialog
		open={deployTarget != null}
		onClose={() => (deployTarget = null)}
		onConfirm={handleDeployConfirm}
	/>
{/if}
