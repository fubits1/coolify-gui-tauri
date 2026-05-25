<!--
@component
BuildTab — adapts content to the Resource's build pack.

- `dockercompose` → raw docker-compose YAML viewer.
- `dockerfile` → dockerfile path/target + dockerfile contents.
- `nixpacks` / `railpack` / `static` → install / build / start commands +
  base / publish directories + watch paths.
- Anything else → key/value summary fallback.

Props:
- `detail: ResourceDetail` — the resource being viewed.
-->
<script lang="ts">
	import type { ResourceDetail } from "$lib/api/types";
	import { Card, CardContent, CardHeader, CardTitle } from "$lib/components/ui/card";

	let { detail }: { detail: ResourceDetail } = $props();

	const buildPack = $derived((detail.build_pack ?? "").toLowerCase());
	const isCompose = $derived(buildPack === "dockercompose");
	const isDockerfile = $derived(buildPack === "dockerfile");
	const isCommandBased = $derived(
		buildPack === "nixpacks" || buildPack === "railpack" || buildPack === "static",
	);

	function nonEmpty(s: string | null | undefined): string | null {
		if (s == null) return null;
		const t = s.trim();
		return t.length === 0 ? null : t;
	}

	const composeYaml = $derived(nonEmpty(detail.docker_compose_raw));
	const dockerfile = $derived(nonEmpty(detail.dockerfile));

	const commands: Array<{ label: string; value: string }> = $derived.by(() => {
		const out: Array<{ label: string; value: string }> = [];
		const push = (label: string, value: string | null | undefined) => {
			const v = nonEmpty(value);
			if (v) out.push({ label, value: v });
		};
		push("Install", detail.install_command);
		push("Build", detail.build_command);
		push("Start", detail.start_command);
		push("Base directory", detail.base_directory);
		push("Publish directory", detail.publish_directory);
		push("Watch paths", detail.watch_paths);
		return out;
	});

	const dockerfileMeta: Array<{ label: string; value: string }> = $derived.by(() => {
		const out: Array<{ label: string; value: string }> = [];
		const push = (label: string, value: string | null | undefined) => {
			const v = nonEmpty(value);
			if (v) out.push({ label, value: v });
		};
		push("Location", detail.dockerfile_location);
		push("Target build", detail.dockerfile_target_build);
		return out;
	});
</script>

<div class="flex flex-col gap-3">
	<div class="text-xs text-muted-foreground">
		Build pack: <span class="font-mono text-foreground">{detail.build_pack ?? "—"}</span>
	</div>

	{#if isCompose}
		{#if composeYaml}
			<Card>
				<CardHeader>
					<CardTitle class="text-sm">docker-compose.yml</CardTitle>
				</CardHeader>
				<CardContent class="p-3">
					<pre class="font-mono text-xs whitespace-pre overflow-x-auto max-h-[60vh]"><code>{composeYaml}</code></pre>
				</CardContent>
			</Card>
		{:else}
			<div class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
				No compose configuration on file.
			</div>
		{/if}
	{:else if isDockerfile}
		{#if dockerfileMeta.length > 0}
			<Card>
				<CardContent class="grid grid-cols-[8rem_1fr] gap-y-1 gap-x-3 p-4 text-xs">
					{#each dockerfileMeta as item (item.label)}
						<span class="text-muted-foreground">{item.label}</span>
						<span class="font-mono break-all">{item.value}</span>
					{/each}
				</CardContent>
			</Card>
		{/if}
		{#if dockerfile}
			<Card>
				<CardHeader>
					<CardTitle class="text-sm">Dockerfile</CardTitle>
				</CardHeader>
				<CardContent class="p-3">
					<pre class="font-mono text-xs whitespace-pre overflow-x-auto max-h-[60vh]"><code>{dockerfile}</code></pre>
				</CardContent>
			</Card>
		{:else if dockerfileMeta.length === 0}
			<div class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
				No Dockerfile contents stored.
			</div>
		{/if}
	{:else if isCommandBased}
		{#if commands.length > 0}
			<Card>
				<CardContent class="grid grid-cols-[10rem_1fr] gap-y-2 gap-x-3 p-4 text-xs">
					{#each commands as item (item.label)}
						<span class="text-muted-foreground">{item.label}</span>
						<span class="font-mono break-all">{item.value}</span>
					{/each}
				</CardContent>
			</Card>
		{:else}
			<div class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
				No build commands configured. Coolify will use the build pack defaults.
			</div>
		{/if}
	{:else}
		<!-- Unknown/missing build_pack — show whatever is present. -->
		{@const fallback = [
			...commands,
			...(composeYaml ? [{ label: "Compose", value: composeYaml }] : []),
			...(dockerfile ? [{ label: "Dockerfile", value: dockerfile }] : []),
			...dockerfileMeta,
		]}
		{#if fallback.length === 0}
			<div class="rounded-md border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
				No build configuration available.
			</div>
		{:else}
			<Card>
				<CardContent class="flex flex-col gap-2 p-4 text-xs">
					{#each fallback as item, i (`${item.label}-${i}`)}
						<div class="flex flex-col gap-1">
							<span class="text-muted-foreground">{item.label}</span>
							<pre class="font-mono whitespace-pre-wrap break-all">{item.value}</pre>
						</div>
					{/each}
				</CardContent>
			</Card>
		{/if}
	{/if}
</div>
