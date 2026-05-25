<!--
@component
ViewToggle — pair of segmented buttons for switching between the dense
table view and the grid-of-cards view on the overview screen.

Visually a button group: the active mode renders as the `default` (filled)
variant, the inactive one as `ghost` so they read as a single control.

Props:
- `mode: 'table' | 'cards'` — current view mode (controlled by parent)
- `onChange: (m: 'table' | 'cards') => void` — fired when the user picks the other mode
-->
<script lang="ts">
	import { Button } from "$lib/components/ui/button";
	import { LayoutGrid, Rows3 } from "@lucide/svelte";

	type Mode = "table" | "cards";

	let {
		mode,
		onChange,
	}: {
		mode: Mode;
		onChange: (m: Mode) => void;
	} = $props();
</script>

<div class="inline-flex items-center gap-1 rounded-lg border border-border bg-background p-0.5">
	<Button
		variant={mode === "table" ? "default" : "ghost"}
		size="icon-sm"
		aria-label="Table view"
		aria-pressed={mode === "table"}
		title="Table view"
		onclick={() => onChange("table")}
	>
		<Rows3 />
	</Button>
	<Button
		variant={mode === "cards" ? "default" : "ghost"}
		size="icon-sm"
		aria-label="Cards view"
		aria-pressed={mode === "cards"}
		title="Cards view"
		onclick={() => onChange("cards")}
	>
		<LayoutGrid />
	</Button>
</div>
