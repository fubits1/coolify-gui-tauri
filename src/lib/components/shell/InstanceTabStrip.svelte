<!--
@component
InstanceTabStrip — horizontal tab list of all configured Coolify instances.

Each tab shows the alias + a colored status dot (driven by the
per-instance `connectionRegistry` entry). Clicking a tab makes that
instance active; clicking `×` removes it (with a confirm). The
trailing `+` button opens an inline ConnectScreen.

Renders ABOVE the main toolbar. Stays mounted at all times once at
least one instance exists — the only screen without it is the truly
fresh first-run state.
-->
<script lang="ts">
	import Plus from "@lucide/svelte/icons/plus";
	import XIcon from "@lucide/svelte/icons/x";
	import { getVersion } from "@tauri-apps/api/app";
	import { instances } from "$lib/stores/instances.svelte";
	import { connectionRegistry } from "$lib/stores/connection.svelte";
	import { toast } from "$lib/util/toast.svelte";

	let {
		onAddRequested,
	}: {
		onAddRequested: () => void;
	} = $props();

	// Read the version from Tauri's bundle metadata (tauri.conf.json's
	// `version` field — single source of truth). Hardcoding here would
	// drift; this stays in sync automatically on every build.
	let version = $state<string>("");
	getVersion()
		.then((v) => (version = v))
		.catch(() => {});

	function dotClass(state: "connected" | "reconnecting" | "offline"): string {
		switch (state) {
			case "connected":
				return "bg-green-400";
			case "reconnecting":
				return "bg-amber-400";
			case "offline":
				return "bg-red-400";
		}
	}

	async function handleRemove(id: string, alias: string) {
		try {
			await instances.remove(id);
			toast.success(`Removed ${alias}`);
		} catch (error) {
			const message =
				error instanceof Error ? error.message : String(error);
			toast.error(`Failed to remove ${alias}`, message);
		}
	}
</script>

<div
	class="flex items-center gap-1 border-b border-border bg-muted/20 px-2 py-1"
>
	{#each instances.list as inst (inst.id)}
		{@const conn = connectionRegistry.get(inst.id)}
		{@const state = conn?.state ?? "connected"}
		{@const active = inst.id === instances.activeId}
		<div
			class="group flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs {active
				? 'border-border bg-background text-foreground'
				: 'border-transparent text-muted-foreground hover:bg-accent hover:text-foreground'}"
		>
			<button
				type="button"
				class="flex items-center gap-1.5"
				onclick={() => void instances.setActive(inst.id)}
			>
				<span
					class="size-2 rounded-full {dotClass(state)}"
					title={state}
					aria-hidden="true"
				></span>
				<span class="truncate max-w-[10rem]">{inst.alias}</span>
			</button>
			<button
				type="button"
				class="rounded p-0.5 text-muted-foreground opacity-0 group-hover:opacity-100 hover:bg-muted hover:text-foreground"
				aria-label="Remove instance"
				title="Remove instance"
				onclick={() => void handleRemove(inst.id, inst.alias)}
			>
				<XIcon class="size-3" />
			</button>
		</div>
	{/each}
	<button
		type="button"
		class="inline-flex items-center gap-1 rounded-md border border-dashed border-border px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
		title="Add Coolify instance"
		onclick={onAddRequested}
	>
		<Plus class="size-3.5" />
		Add
	</button>
	{#if version}
		<span
			class="ml-auto select-none font-mono text-[0.65rem] text-muted-foreground"
			title="Coolify GUI version"
		>
			v{version}
		</span>
	{/if}
</div>
