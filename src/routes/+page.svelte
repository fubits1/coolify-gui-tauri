<script lang="ts">
	import ConnectScreen from "$lib/components/onboarding/ConnectScreen.svelte";
	import TableView from "$lib/components/overview/TableView.svelte";
	import CardsView from "$lib/components/overview/CardsView.svelte";
	import ViewToggle from "$lib/components/overview/ViewToggle.svelte";
	import DetailPane from "$lib/components/detail/DetailPane.svelte";
	import DeployDialog from "$lib/components/detail/DeployDialog.svelte";
	import ConnectionStrip from "$lib/components/shell/ConnectionStrip.svelte";
	import { Button } from "$lib/components/ui/button";
	import { RefreshCw } from "@lucide/svelte";
	import { api } from "$lib/api/client";
	import type { ResourceKind } from "$lib/api/types";
	import { instance } from "$lib/stores/instance.svelte";
	import { resources } from "$lib/stores/resources.svelte";
	import { connection } from "$lib/stores/connection.svelte";
	import { imageCache } from "$lib/stores/image-cache.svelte";
	import { runStartupCheck } from "$lib/util/image-scheduler";
	import { installShortcuts } from "$lib/util/shortcuts";
	import { toast } from "$lib/util/toast";

	// Kick off persisted-state loads on mount. Both stores are reactive once
	// these resolve, so the UI rerenders automatically.
	instance.load();
	imageCache.load();

	const onboarded = $derived(instance.url != null);

	let viewMode = $state<"table" | "cards">("table");
	let deployTarget = $state<{ uuid: string; kind: ResourceKind } | null>(null);

	// Single boot guard: covers both the cold-start case (already onboarded
	// on first paint) and the fresh-onboard case (user just saved creds via
	// ConnectScreen). Either path flips `instance.url` non-null and we run
	// the resource poll + image scheduler exactly once.
	let bootStarted = false;
	$effect(() => {
		if (instance.url == null || bootStarted) return;
		bootStarted = true;
		void (async () => {
			await resources.start();
			void runStartupCheck(resources.list);
		})();
	});

	function handleConnected(_url: string, _alias: string) {
		// ConnectScreen has already called instance.save(); reload to be
		// safe (also flips `onboarded` via the derived above).
		instance.load();
	}

	async function handleRestart(uuid: string, kind: ResourceKind) {
		await toast.promise(api.restart(uuid, kind), {
			loading: "Restarting…",
			success: "Restart triggered",
			error: (e) => `Restart failed: ${e instanceof Error ? e.message : String(e)}`,
		});
		await resources.refresh();
	}

	async function handleStop(uuid: string, kind: ResourceKind) {
		await toast.promise(api.stop(uuid, kind), {
			loading: "Stopping…",
			success: "Stop triggered",
			error: (e) => `Stop failed: ${e instanceof Error ? e.message : String(e)}`,
		});
		await resources.refresh();
	}

	function handleDeploy(uuid: string, kind: ResourceKind) {
		deployTarget = { uuid, kind };
	}

	async function handleDeployConfirm(force: boolean) {
		const target = deployTarget;
		deployTarget = null;
		if (!target) return;
		await toast.promise(api.deploy(target.uuid, force), {
			loading: "Deploying…",
			success: force ? "Deploy (force) triggered" : "Deploy triggered",
			error: (e) => `Deploy failed: ${e instanceof Error ? e.message : String(e)}`,
		});
		await resources.refresh();
	}

	// Overview-header "Check all images": collect refs across the entire list
	// (compose YAML refs ∪ direct image_ref) and fan out a batched check.
	// `runStartupCheck` already encapsulates the detail-fetch-for-compose
	// dance, so we reuse it rather than re-implementing here.
	function handleCheckAllImages() {
		void runStartupCheck(resources.list);
	}

	// Global keyboard shortcuts. Re-installs whenever the selected resource
	// changes so handlers close over the current selection without polling.
	$effect(() => {
		const selected = resources.selectedResource;
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
			onLogs: undefined, // future: switch DetailPane to Logs tab
		});
		return cleanup;
	});
</script>

{#if !onboarded}
	<ConnectScreen onConnected={handleConnected} />
{:else}
	<div class="flex h-screen flex-col">
		<ConnectionStrip
			state={connection.state}
			alias={instance.alias}
			retryInSec={connection.reconnectInSec ?? 0}
		/>

		<div class="flex items-center justify-between gap-2 border-b border-border px-4 py-2">
			<h1 class="text-sm font-semibold">Resources</h1>
			<div class="flex items-center gap-2">
				<ViewToggle mode={viewMode} onChange={(m) => (viewMode = m)} />
				<Button
					variant="outline"
					size="sm"
					onclick={() => void resources.refresh()}
					disabled={resources.loading}
				>
					<RefreshCw />
					Refresh
				</Button>
			</div>
		</div>

		<div class="flex min-h-0 flex-1">
			<div class="flex-1 overflow-auto p-4">
				{#if viewMode === "table"}
					<TableView
						resources={resources.list}
						selectedUuid={resources.selectedUuid}
						onSelect={(uuid) => resources.select(uuid)}
						onRestart={handleRestart}
						onStop={handleStop}
						onDeploy={handleDeploy}
						onCheckImages={handleCheckAllImages}
					/>
				{:else}
					<CardsView
						resources={resources.list}
						selectedUuid={resources.selectedUuid}
						onSelect={(uuid) => resources.select(uuid)}
						onRestart={handleRestart}
						onStop={handleStop}
						onDeploy={handleDeploy}
						onCheckImages={handleCheckAllImages}
					/>
				{/if}
			</div>

			<aside class="w-[28rem] shrink-0 border-l border-border">
				<DetailPane resource={resources.selectedResource} />
			</aside>
		</div>
	</div>

	<DeployDialog
		open={deployTarget != null}
		onClose={() => (deployTarget = null)}
		onConfirm={handleDeployConfirm}
	/>
{/if}
