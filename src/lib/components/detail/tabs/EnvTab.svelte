<!--
@component
EnvTab — masked key/value view of a resource's environment variables.

Values render as dots until the user reveals them; clicking either the key
or the value copies the value to the clipboard. Reveal state lives locally
to the component (a $state Set) so navigating away resets it — secrets
should not stay revealed across selections.

Props:
- `env: EnvVar[]` — vars to render; empty array renders the empty state.
-->
<script lang="ts">
	import type { EnvVar } from "$lib/api/types";
	import { Button } from "$lib/components/ui/button";
	import { toast } from "$lib/util/toast";
	import { SvelteSet } from "svelte/reactivity";

	let { env }: { env: EnvVar[] } = $props();

	// SvelteSet is reactively-tracked, so we can mutate in place rather than
	// reassigning a new Set on every toggle.
	let revealed = new SvelteSet<string>();

	function toggleReveal(key: string) {
		if (revealed.has(key)) revealed.delete(key);
		else revealed.add(key);
	}

	async function copyValue(value: string) {
		try {
			await navigator.clipboard.writeText(value);
			toast.success("Copied");
		} catch {
			toast.error("Copy failed");
		}
	}

	function mask(value: string): string {
		// Cap the dot count so very-long secrets don't visually dominate.
		const len = Math.min(Math.max(value.length, 4), 24);
		return "•".repeat(len);
	}
</script>

{#if env.length === 0}
	<div
		class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground"
	>
		No environment variables.
	</div>
{:else}
	<div class="overflow-hidden rounded-md border border-border">
		<table class="w-full text-sm">
			<thead class="bg-muted/30">
				<tr class="text-left text-xs text-muted-foreground">
					<th class="px-3 py-2 font-medium">Key</th>
					<th class="px-3 py-2 font-medium">Value</th>
					<th class="px-3 py-2 font-medium w-20"></th>
				</tr>
			</thead>
			<tbody>
				{#each env as v (v.key)}
					<tr class="border-t border-border">
						<td class="px-3 py-1.5 align-top">
							<button
								type="button"
								class="font-mono text-xs hover:text-primary"
								title="Copy value"
								onclick={() => copyValue(v.value)}
							>
								{v.key}
							</button>
						</td>
						<td class="px-3 py-1.5 align-top">
							<button
								type="button"
								class="text-left font-mono text-xs break-all hover:text-primary"
								title="Copy value"
								onclick={() => copyValue(v.value)}
							>
								{revealed.has(v.key) ? v.value : mask(v.value)}
							</button>
						</td>
						<td class="px-3 py-1 align-top text-right">
							<Button
								variant="ghost"
								size="xs"
								onclick={() => toggleReveal(v.key)}
							>
								{revealed.has(v.key) ? "Hide" : "Reveal"}
							</Button>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
{/if}
