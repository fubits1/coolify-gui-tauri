<!--
@component
ConnectionStrip — fixed 24px top strip showing connection state to the Coolify backend.

States:
- `connected` → green dot + "Connected to {alias}"
- `reconnecting` → amber dot + "Reconnecting in {retryInSec}s"
- `offline` → red dot + "Offline"

Purely presentational — parent passes the resolved `state` (and supporting fields).
Store wiring happens at the layout level; this component takes props in only.

Props:
- `state: "connected" | "reconnecting" | "offline"`
- `alias: string | null` — instance alias, used when `state === "connected"`
- `retryInSec?: number` — countdown until next reconnect attempt
-->
<script lang="ts">
	type State = "connected" | "reconnecting" | "offline";

	let {
		state,
		alias,
		retryInSec = 0,
	}: {
		state: State;
		alias: string | null;
		retryInSec?: number;
	} = $props();

	type View = { dot: string; text: string };

	const view: View = $derived.by(() => {
		switch (state) {
			case "connected":
				return {
					dot: "bg-green-500",
					text: `Connected to ${alias ?? "unknown"}`,
				};
			case "reconnecting":
				return {
					dot: "bg-amber-500",
					text: `Reconnecting in ${retryInSec}s`,
				};
			case "offline":
				return { dot: "bg-red-500", text: "Offline" };
		}
	});
</script>

<div
	class="flex h-6 w-full items-center gap-2 border-b border-border bg-background px-3 text-xs text-muted-foreground"
	role="status"
	aria-live="polite"
>
	<span class="inline-block size-2 rounded-full {view.dot}"></span>
	<span>{view.text}</span>
</div>
