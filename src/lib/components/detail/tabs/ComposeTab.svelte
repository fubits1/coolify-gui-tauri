<!--
@component
ComposeTab — read-only docker-compose YAML viewer.

No syntax highlighting (deferred). The raw YAML is rendered inside a
scrollable monospace pre block. `null` renders the empty state.

Props:
- `yaml: string | null` — `docker_compose_raw` or null.
-->
<script lang="ts">
	import { Card, CardContent } from "$lib/components/ui/card";

	let { yaml }: { yaml: string | null } = $props();
</script>

{#if yaml == null}
	<div
		class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground"
	>
		No compose configuration.
	</div>
{:else}
	<Card>
		<CardContent class="p-3">
			<pre
				class="font-mono text-xs whitespace-pre overflow-x-auto max-h-[60vh]"><code
					>{yaml}</code
				></pre>
		</CardContent>
	</Card>
{/if}
