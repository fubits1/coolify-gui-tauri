<!--
@component
EnvTab — env vars grouped by environment (Production first, then Development).

Each var is rendered as a 3-line card: key on top, masked value middle,
scope badges + Reveal button at the bottom. Vertical layout fits the
narrow detail aside without overlap. Click key or value copies the value.

Coolify env vars carry an `is_preview` flag — true = development (preview
deploys), false = production. Same key can exist in both.

Props:
- `env: EnvVar[]` — vars to render; empty array renders the empty state.
-->
<script lang="ts">
	import type { EnvVar } from "$lib/api/types";
	import { Button } from "$lib/components/ui/button";
	import { toast } from "$lib/util/toast.svelte";
	import { SvelteSet } from "svelte/reactivity";

	let { env }: { env: EnvVar[] } = $props();

	let revealed = new SvelteSet<string>();

	function toggleReveal(rowId: string) {
		if (revealed.has(rowId)) revealed.delete(rowId);
		else revealed.add(rowId);
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
		// Fixed-width mask so the dots don't push other UI off-screen on
		// long secrets. Reveal toggle exposes the real value.
		return "•".repeat(value.length > 0 ? 12 : 0);
	}

	function extraScopes(v: EnvVar): string[] {
		const out: string[] = [];
		if (v.is_buildtime) out.push("build");
		if (v.is_runtime === false) out.push("no-runtime");
		if (v.is_shared) out.push("shared");
		return out;
	}

	// Production first, then Development. Within each group sort
	// alphabetically by key (case-insensitive, locale-aware) so users can
	// scan/search predictably; Coolify's server order isn't stable.
	const grouped = $derived.by<{ env: "Production" | "Development"; items: Array<{ v: EnvVar; rowId: string }> }[]>(() => {
		const prod: { v: EnvVar; rowId: string }[] = [];
		const dev: { v: EnvVar; rowId: string }[] = [];
		env.forEach((v, i) => {
			const entry = { v, rowId: `${v.key}#${i}` };
			if (v.is_preview) dev.push(entry);
			else prod.push(entry);
		});
		const cmp = (a: { v: EnvVar }, b: { v: EnvVar }) =>
			a.v.key.localeCompare(b.v.key, undefined, { sensitivity: "base" });
		prod.sort(cmp);
		dev.sort(cmp);
		const out: { env: "Production" | "Development"; items: typeof prod }[] = [];
		if (prod.length > 0) out.push({ env: "Production", items: prod });
		if (dev.length > 0) out.push({ env: "Development", items: dev });
		return out;
	});
</script>

{#if env.length === 0}
	<div
		class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground"
	>
		No environment variables.
	</div>
{:else}
	<div class="flex flex-col gap-4">
		{#each grouped as group (group.env)}
			<section class="flex flex-col gap-2">
				<header class="flex items-center gap-2">
					<h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
						{group.env}
					</h3>
				</header>
				<div class="flex flex-col gap-2">
					{#each group.items as item (item.rowId)}
						{@const v = item.v}
						{@const rowId = item.rowId}
						{@const extras = extraScopes(v)}
						<div
							class="flex flex-col gap-1.5 rounded-md border border-border bg-muted/10 px-3 py-2"
						>
							<button
								type="button"
								class="text-left font-mono text-xs font-medium hover:text-primary"
								title="Copy value"
								onclick={() => copyValue(v.value)}
							>
								{v.key}
							</button>
							<button
								type="button"
								class="text-left font-mono text-xs break-all text-muted-foreground hover:text-primary"
								title="Copy value"
								onclick={() => copyValue(v.value)}
							>
								{revealed.has(rowId) ? v.value : mask(v.value)}
							</button>
							<div class="flex items-center justify-between gap-2">
								<div class="flex flex-wrap gap-1">
									{#each extras as s (s)}
										<span
											class="rounded border border-border bg-muted/40 px-1.5 py-0.5 text-[0.65rem] uppercase tracking-wide text-muted-foreground"
										>
											{s}
										</span>
									{/each}
								</div>
								<Button
									variant="ghost"
									size="xs"
									onclick={() => toggleReveal(rowId)}
								>
									{revealed.has(rowId) ? "Hide" : "Reveal"}
								</Button>
							</div>
						</div>
					{/each}
				</div>
			</section>
		{/each}
	</div>
{/if}
