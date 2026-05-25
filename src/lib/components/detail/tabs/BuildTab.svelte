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
		push("Pre-deployment", detail.pre_deployment_command);
		push("Pre-deployment container", detail.pre_deployment_command_container);
		push("Post-deployment", detail.post_deployment_command);
		push("Post-deployment container", detail.post_deployment_command_container);
		push("Custom docker run options", detail.custom_docker_run_options);
		push("Static image", detail.static_image);
		return out;
	});

	const hasCustomCommands = $derived(
		nonEmpty(detail.install_command) != null ||
			nonEmpty(detail.build_command) != null ||
			nonEmpty(detail.start_command) != null,
	);

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
				<CardContent class="grid grid-cols-[14rem_1fr] gap-y-2 gap-x-3 p-4 text-xs">
					{#each commands as item (item.label)}
						<span class="text-muted-foreground">{item.label}</span>
						<span class="font-mono break-all whitespace-pre-wrap">{item.value}</span>
					{/each}
				</CardContent>
			</Card>
		{/if}
		{#if !hasCustomCommands}
			<div class="rounded-md border border-dashed border-border px-4 py-3 text-xs text-muted-foreground">
				No install / build / start commands set — {buildPack} uses its
				detected defaults. The actual <code class="font-mono">nixpacks.toml</code>
				(if any) lives in your git repository; Coolify's API doesn't surface
				it. Override by setting commands in the Coolify dashboard.
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
