<script lang="ts">
	import ConnectScreen from "$lib/components/onboarding/ConnectScreen.svelte";
	import TableView from "$lib/components/overview/TableView.svelte";
	import CardsView from "$lib/components/overview/CardsView.svelte";
	import ViewToggle from "$lib/components/overview/ViewToggle.svelte";
	import DetailPane from "$lib/components/detail/DetailPane.svelte";
	import DeployDialog from "$lib/components/detail/DeployDialog.svelte";
	import ConnectionStrip from "$lib/components/shell/ConnectionStrip.svelte";
	import { Button } from "$lib/components/ui/button";
	import { RefreshCw, Settings, X } from "@lucide/svelte";
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

	// Tri-state hydration: null while we wait for instance.load + the keyring
	// rehydration to settle; false → no stored token, show ConnectScreen;
	// true → token rehydrated, client is live in Rust state.
	let credentialsReady = $state<boolean | null>(null);
	let credentialsProbedUrl = $state<string | null>(null);

	$effect(() => {
		const url = instance.url;
		if (url == null) {
			// Either still loading, or user signed out — reset probe state so a
			// future url change re-triggers loadCredentials.
			if (credentialsProbedUrl != null) {
				credentialsReady = null;
				credentialsProbedUrl = null;
			}
			return;
		}
		if (credentialsProbedUrl === url) return;
		credentialsProbedUrl = url;
		void (async () => {
			try {
				const ok = await api.loadCredentials(url, instance.alias ?? undefined);
				credentialsReady = ok;
			} catch (err) {
				const msg = err instanceof Error ? err.message : String(err);
				toast.error("Failed to load credentials", msg);
				credentialsReady = false;
			}
		})();
	});

	const onboarded = $derived(instance.url != null && credentialsReady === true);

	let viewMode = $state<"table" | "cards">("table");
	let deployTarget = $state<{ uuid: string; kind: ResourceKind } | null>(null);

	// Single boot guard: must wait for credentialsReady=true (load_credentials
	// resolved and the Rust client is live). Firing resources.start() on
	// instance.url alone races the keyring probe and the first poll hits
	// "no Coolify credentials set" before the client is built.
	let bootStarted = false;
	$effect(() => {
		if (
			instance.url == null ||
			credentialsReady !== true ||
			bootStarted
		)
			return;
		bootStarted = true;
		void (async () => {
			await resources.start();
			void runStartupCheck(resources.list);
		})();
	});

	function handleConnected(url: string, _alias: string) {
		// ConnectScreen has already called instance.save() and set_credentials
		// (which persists the token to the keyring + builds the live client).
		// Mark the keyring rehydration as already-satisfied for this url so the
		// boot effect doesn't redundantly probe.
		credentialsReady = true;
		credentialsProbedUrl = url;
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
		// Manual "Check all images" must BYPASS the scheduler's 24h cache gate.
		// Collect every image ref across the resource list and force a fresh
		// check via imageCache.checkMany. The scheduler-style flow is only used
		// for the daily-startup heartbeat in the boot effect above.
		const refs = new Set<string>();
		for (const r of resources.list) {
			for (const ref of r.image_refs ?? []) {
				if (ref && ref.trim().length > 0) refs.add(ref);
			}
		}
		if (refs.size === 0) {
			toast.info("No image refs to check");
			return;
		}
		toast.info(`Checking ${refs.size} images for updates…`);
		void imageCache.checkMany([...refs]);
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

{#if instance.url != null && credentialsReady === null}
	<!-- Probing the keyring after a cold-start with a persisted URL.
	     Render an empty shell so we don't flash ConnectScreen. -->
	<div class="flex min-h-screen items-center justify-center bg-background"></div>
{:else if !onboarded}
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

			{#if resources.selectedResource}
				<aside class="relative w-96 shrink-0 overflow-auto border-l border-border">
					<button
						type="button"
						class="absolute right-2 top-2 z-10 inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
						aria-label="Close detail pane"
						title="Close"
						onclick={() => resources.select(null)}
					>
						<X class="size-4" />
					</button>
					<DetailPane resource={resources.selectedResource} />
				</aside>
			{/if}
		</div>
	</div>

	<DeployDialog
		open={deployTarget != null}
		onClose={() => (deployTarget = null)}
		onConfirm={handleDeployConfirm}
	/>
{/if}
