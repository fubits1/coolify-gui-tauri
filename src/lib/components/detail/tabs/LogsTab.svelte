<!--
@component
LogsTab — last-N-lines log viewer with manual refresh.

Polls `api.tailLogs(uuid, kind, 500)` on demand. A `$effect` keyed on
`uuid` performs an initial fetch on mount and on resource-switch. After
each successful refresh, the pre block is auto-scrolled to the bottom so
newest entries are visible — the surrounding ResizeObserver-y dance is
unnecessary because the pre's contents fully replace each refresh.

Props:
- `uuid: string` — resource UUID.
- `kind: string` — resource kind (Application / Service / Database).
-->
<script lang="ts">
	import { api } from "$lib/api/client";
	import { Button } from "$lib/components/ui/button";
	import { toast } from "$lib/util/toast";

	import type { ServiceContainer } from "$lib/api/types";

	let {
		uuid,
		kind,
		active = true,
		containers = [],
		instanceUrl = null,
	}: {
		uuid: string;
		kind: string;
		active?: boolean;
		/** Service-only nested container handles. Currently unused — the
		 *  Coolify v1 API has no per-container logs endpoint, so we just
		 *  surface a clear empty-state for services. */
		containers?: ServiceContainer[];
		/** Used to build a deep-link to the Coolify dashboard's resource
		 *  page when our own logs endpoint isn't available. */
		instanceUrl?: string | null;
	} = $props();

	const isLogsSupported = $derived(kind === "Application" || kind === "application");

	/**
	 * Build the Coolify dashboard URL for this resource. Coolify routes
	 * resource pages at predictable plural-kind paths:
	 *   `{base}/applications/{uuid}`
	 *   `{base}/services/{uuid}`
	 *   `{base}/databases/{uuid}`
	 *
	 * The longer `project/{project_uuid}/{env}/...` form is what the
	 * dashboard ITSELF uses internally, but the short form 302-redirects
	 * to it, so we use the short form to avoid depending on data the
	 * list endpoints don't ship (environment_name, project_uuid — see
	 * docs/coolify-api.md).
	 */
	const dashboardUrl = $derived.by(() => {
		if (!instanceUrl) return null;
		const base = instanceUrl.replace(/\/$/, "");
		const kindLower = kind.toLowerCase();
		const segment =
			kindLower === "service"
				? "services"
				: kindLower === "database"
					? "databases"
					: "applications";
		return `${base}/${segment}/${uuid}`;
	});


	let text = $state("");
	let loading = $state(false);
	let lastRefreshAt = $state<number | null>(null);
	let now = $state(Date.now());
	let preEl: HTMLPreElement | null = $state(null);
	let autoPaused = $state(false);
	let selectedContainer = $state<string>("");

	// Reset selection when the resource changes; default to the FIRST
	// container uuid (Coolify has no aggregated /services/{uuid}/logs
	// endpoint — per-container logs go through /applications/{c_uuid}/logs).
	$effect(() => {
		void uuid;
		const first = containers[0];
		selectedContainer = first?.uuid ?? "";
	});

	const lineCount = $derived(text.length === 0 ? 0 : text.split("\n").length);

	const POLL_MS = 5000;

	// Re-render the relative timestamp once per second. The interval is owned
	// by the component so it cleans up automatically on unmount.
	$effect(() => {
		const id = setInterval(() => {
			now = Date.now();
		}, 1000);
		return () => clearInterval(id);
	});

	// Initial / resource-switch fetch. Only when the Logs tab is actually
	// active AND the kind has a logs endpoint (currently Application only).
	$effect(() => {
		void uuid;
		lastErrorMsg = null;
		text = "";
		lastRefreshAt = null;
		if (!active || !isLogsSupported) return;
		void refresh(false);
	});

	// Auto-poll while the tab is active, window focused, and not paused.
	// Tab switch / window blur / pause toggle all stop the interval cleanly.
	$effect(() => {
		if (!active || autoPaused || !isLogsSupported) return;
		let focused =
			typeof document === "undefined" ? true : document.hasFocus();
		const onFocus = () => {
			focused = true;
		};
		const onBlur = () => {
			focused = false;
		};
		window.addEventListener("focus", onFocus);
		window.addEventListener("blur", onBlur);
		const id = setInterval(() => {
			if (focused && !loading) void refresh(false);
		}, POLL_MS);
		return () => {
			clearInterval(id);
			window.removeEventListener("focus", onFocus);
			window.removeEventListener("blur", onBlur);
		};
	});

	let lastErrorMsg = $state<string | null>(null);

	/**
	 * @param interactive true when triggered by the Refresh button — toasts
	 *   on failure. Auto-poll passes false → silent failure (inline notice
	 *   stays in the pre block; no sticky toast spam on every poll).
	 */
	const SCROLL_STICKY_THRESHOLD_PX = 24;

	async function refresh(interactive: boolean) {
		loading = true;
		try {
			const next = await api.tailLogs(
				uuid,
				kind,
				500,
				selectedContainer || undefined,
			);
			lastErrorMsg = null;
			lastRefreshAt = Date.now();
			// Skip the reassignment entirely when content hasn't changed —
			// otherwise every 5s poll forces Svelte to re-render the entire
			// <pre> + scroll jumps to bottom even when there's no diff.
			if (next === text) return;
			// Preserve the user's scroll position unless they're already
			// pinned to the bottom (typical "follow log" intent).
			const wasAtBottom = isPinnedToBottom();
			text = next;
			if (wasAtBottom || interactive) {
				queueMicrotask(() => {
					if (preEl) preEl.scrollTop = preEl.scrollHeight;
				});
			}
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			lastErrorMsg = msg;
			if (interactive) {
				toast.error("Failed to load logs", msg);
			}
		} finally {
			loading = false;
		}
	}

	function isPinnedToBottom(): boolean {
		if (!preEl) return true;
		const delta = preEl.scrollHeight - preEl.scrollTop - preEl.clientHeight;
		return delta <= SCROLL_STICKY_THRESHOLD_PX;
	}

	// Re-fetch when the container selection changes.
	$effect(() => {
		void selectedContainer;
		if (!active) return;
		void refresh(false);
	});

	function relative(t: number | null): string {
		if (t == null) return "never";
		const sec = Math.max(0, Math.round((now - t) / 1000));
		if (sec < 60) return `${sec}s ago`;
		const min = Math.round(sec / 60);
		return `${min}m ago`;
	}
</script>

{#if !isLogsSupported}
	<div
		class="flex flex-col items-center gap-3 rounded-md border border-dashed border-border bg-muted/10 px-4 py-8 text-center text-sm text-muted-foreground"
	>
		<p class="font-medium">Logs are not available for {kind}s via the Coolify API.</p>
		<p class="text-xs">
			Coolify only exposes
			<code class="font-mono">/applications/&#123;uuid&#125;/logs</code>. Use the
			Coolify dashboard's terminal view for container logs.
		</p>
		{#if dashboardUrl}
			<a
				class="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-accent"
				href={dashboardUrl}
				target="_blank"
				rel="noopener noreferrer"
			>
				Open in Coolify dashboard ↗
			</a>
		{/if}
	</div>
{:else}
<div class="flex flex-col gap-2">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<div class="text-xs text-muted-foreground">
			Tailing last 500 lines · auto-refresh every {POLL_MS / 1000}s
			{#if autoPaused}<span class="text-amber-400"> · paused</span>{/if}
		</div>
		<div class="flex items-center gap-2">
			{#if containers.length > 0}
				<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
					Container
					<select
						class="h-8 rounded-md border border-input bg-background px-2 text-xs text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
						bind:value={selectedContainer}
					>
						{#each containers as c (c.uuid)}
							<option value={c.uuid}>{c.name}</option>
						{/each}
					</select>
				</label>
			{/if}
			<Button
				variant="outline"
				size="sm"
				onclick={() => (autoPaused = !autoPaused)}
				title={autoPaused ? "Resume auto-refresh" : "Pause auto-refresh"}
			>
				{autoPaused ? "Resume" : "Pause"}
			</Button>
			<Button
				variant="outline"
				size="sm"
				onclick={() => refresh(true)}
				disabled={loading}
			>
				{loading ? "Loading…" : "Refresh"}
			</Button>
		</div>
	</div>

	<pre
		bind:this={preEl}
		class="font-mono text-xs whitespace-pre overflow-auto rounded-md border border-border bg-muted/20 p-3 max-h-[60vh] min-h-[200px]"
	>{text || (loading ? "Loading logs…" : "No log output.")}</pre>

	{#if lastErrorMsg}
		<div
			class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
		>
			Last fetch failed: <span class="font-mono">{lastErrorMsg}</span>
		</div>
	{/if}

	<div class="text-xs text-muted-foreground">
		Loaded {lineCount} line{lineCount === 1 ? "" : "s"} · refreshed {relative(
			lastRefreshAt,
		)}
	</div>
</div>
{/if}
