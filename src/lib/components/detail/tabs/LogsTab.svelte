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

	let { uuid, kind }: { uuid: string; kind: string } = $props();

	let text = $state("");
	let loading = $state(false);
	let lastRefreshAt = $state<number | null>(null);
	let now = $state(Date.now());
	let preEl: HTMLPreElement | null = $state(null);

	const lineCount = $derived(text.length === 0 ? 0 : text.split("\n").length);

	// Re-render the relative timestamp once per second. The interval is owned
	// by the component so it cleans up automatically on unmount.
	$effect(() => {
		const id = setInterval(() => {
			now = Date.now();
		}, 1000);
		return () => clearInterval(id);
	});

	// Initial fetch + refetch when the resource changes.
	$effect(() => {
		// Touch uuid so the effect re-runs on switch.
		void uuid;
		void refresh();
	});

	async function refresh() {
		loading = true;
		try {
			text = await api.tailLogs(uuid, kind, 500);
			lastRefreshAt = Date.now();
			// Scroll on next microtask once the new text is rendered.
			queueMicrotask(() => {
				if (preEl) preEl.scrollTop = preEl.scrollHeight;
			});
		} catch {
			toast.error("Failed to load logs");
		} finally {
			loading = false;
		}
	}

	function relative(t: number | null): string {
		if (t == null) return "never";
		const sec = Math.max(0, Math.round((now - t) / 1000));
		if (sec < 60) return `${sec}s ago`;
		const min = Math.round(sec / 60);
		return `${min}m ago`;
	}
</script>

<div class="flex flex-col gap-2">
	<div class="flex items-center justify-between">
		<div class="text-xs text-muted-foreground">
			Tailing last 500 lines · manual refresh
		</div>
		<Button
			variant="outline"
			size="sm"
			onclick={refresh}
			disabled={loading}
		>
			{loading ? "Loading…" : "Refresh"}
		</Button>
	</div>

	<pre
		bind:this={preEl}
		class="font-mono text-xs whitespace-pre overflow-auto rounded-md border border-border bg-muted/20 p-3 max-h-[60vh] min-h-[200px]"
	>{text || (loading ? "Loading logs…" : "No log output.")}</pre>

	<div class="text-xs text-muted-foreground">
		Loaded {lineCount} line{lineCount === 1 ? "" : "s"} · refreshed {relative(
			lastRefreshAt,
		)}
	</div>
</div>
