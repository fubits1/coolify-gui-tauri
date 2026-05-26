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
		projectUuid = null,
		environmentUuid = null,
		environmentName = null,
	}: {
		uuid: string;
		kind: string;
		active?: boolean;
		/** Service-only nested container handles. Currently unused — the
		 *  Coolify v1 API has no per-container logs endpoint, so we just
		 *  surface a clear empty-state for services. */
		containers?: ServiceContainer[];
		/** Used to build the dashboard deep-link below. */
		instanceUrl?: string | null;
		projectUuid?: string | null;
		environmentUuid?: string | null;
		environmentName?: string | null;
	} = $props();

	// Logs feature is intentionally disabled in v1. Even for Applications,
	// the per-resource fetch (5s poll × N resources) hammers Coolify
	// instances behind Cloudflare and triggers 429 rate limits that cascade
	// across the whole UI. The Coolify dashboard's own terminal view is
	// the right home for logs — we just deep-link there.
	const isLogsSupported = false;

	/**
	 * Coolify dashboard route — verified against cf.fubits.dev:
	 *   {instance}/project/{project_uuid}/environment/{env_uuid_or_name}/{kind_singular}/{uuid}/logs
	 * The env segment accepts either the UUID or the name (the API mirrors
	 * this via /projects/{uuid}/{environment_name_or_uuid}). Prefer UUID
	 * when present so URLs survive env renames.
	 */
	const dashboardUrl = $derived.by(() => {
		if (!instanceUrl) return null;
		const base = instanceUrl.replace(/\/$/, "");
		if (!projectUuid) return base;
		const envSeg = environmentUuid ?? environmentName;
		if (!envSeg) return base;
		const kindSeg = kind.toLowerCase();
		return `${base}/project/${projectUuid}/environment/${envSeg}/${kindSeg}/${uuid}/logs`;
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

	/**
	 * Coolify forwards `?timestamps=true` to `docker logs`, producing lines
	 * prefixed with `2026-05-25T21:30:45.123456789Z `. Normalize to
	 * `2026-05-25 21:30:45 ` (human-readable, second precision, space
	 * separator) — matches how the user sees Coolify-side timestamps.
	 * Lines that already have a non-Docker prefix (e.g. an app's own
	 * structured logger) are passed through untouched.
	 */
	function humanizeTimestamps(raw: string): string {
		const rfc3339 = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?Z?\s?/;
		return raw
			.split("\n")
			.map((line) => line.replace(rfc3339, "$1 $2 "))
			.join("\n");
	}

	async function refresh(interactive: boolean) {
		// Only flip the spinner/Loading label for interactive or first-load
		// refreshes. Auto-poll every 5s shouldn't make the Refresh button
		// flicker between "Loading…" and "Refresh" forever.
		const showLoading = interactive || lastRefreshAt == null;
		if (showLoading) loading = true;
		try {
			const raw = await api.tailLogs(
				uuid,
				kind,
				500,
				selectedContainer || undefined,
			);
			const next = humanizeTimestamps(raw);
			// Successful fetch — clear stale error + bump refresh timestamp
			// BEFORE any early-return optimizations so the UI doesn't sit
			// at "refreshed never" forever when the body is empty.
			lastErrorMsg = null;
			lastRefreshAt = Date.now();
			// Empty response from an auto-poll = preserve whatever we have
			// (transient — container restarting, log rotation). Manual
			// refresh allows the clear.
			if (next.length === 0 && text.length > 0 && !interactive) return;
			// No content diff → skip the reassignment to avoid Svelte
			// re-rendering the whole <pre> + scroll jump.
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
			if (showLoading) loading = false;
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
		<p class="font-medium">Logs live in the Coolify dashboard.</p>
		<p class="text-xs">
			Per-resource log polling against a Cloudflare-fronted Coolify
			triggers 429 rate limits that cascade across the whole UI. Open
			the resource in the Coolify dashboard to use its built-in
			terminal view.
		</p>
		{#if dashboardUrl}
			<div class="flex flex-col items-center gap-1">
				<a
					class="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-accent"
					href={dashboardUrl}
					target="_blank"
					rel="noopener noreferrer"
				>
					Open Coolify dashboard ↗
				</a>
				<span class="text-[0.65rem] text-muted-foreground">
					Opens the resource's Logs view in your Coolify dashboard.
				</span>
			</div>
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
