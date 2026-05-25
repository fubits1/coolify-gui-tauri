<!--
@component
OverviewTab — read-only key/value summary of a `ResourceDetail`.

Two-column grid (label · value). Skips null/empty fields so we don't render
empty rows. Git fields collapse into a single clickable github link when
both `git_repository` and a sha are present.

Props:
- `detail: ResourceDetail` — fully-fetched resource detail.
-->
<script lang="ts">
	import type { ResourceDetail } from "$lib/api/types";

	let { detail }: { detail: ResourceDetail } = $props();

	function relativeTime(iso: string | undefined): string | null {
		if (!iso) return null;
		const then = Date.parse(iso);
		if (Number.isNaN(then)) return iso;
		const diffMs = Date.now() - then;
		const sec = Math.round(diffMs / 1000);
		if (sec < 60) return `${sec}s ago`;
		const min = Math.round(sec / 60);
		if (min < 60) return `${min}m ago`;
		const hr = Math.round(min / 60);
		if (hr < 24) return `${hr}h ago`;
		const day = Math.round(hr / 24);
		return `${day}d ago`;
	}

	// Best-effort: convert a `git_repository` like git@github.com:owner/repo.git
	// or https://github.com/owner/repo into a browsable web URL. Returns null
	// if we don't recognise the host (we don't want to ship broken links).
	function gitWebUrl(
		repo: string | undefined,
		sha: string | undefined,
	): string | null {
		if (!repo) return null;
		let owner = "";
		let name = "";
		const ssh = repo.match(/git@([^:]+):([^/]+)\/(.+?)(?:\.git)?$/);
		const https = repo.match(/https?:\/\/([^/]+)\/([^/]+)\/(.+?)(?:\.git)?$/);
		let host = "";
		if (ssh) {
			host = ssh[1];
			owner = ssh[2];
			name = ssh[3];
		} else if (https) {
			host = https[1];
			owner = https[2];
			name = https[3];
		} else {
			return null;
		}
		if (!host.endsWith("github.com")) return null;
		const base = `https://${host}/${owner}/${name}`;
		return sha ? `${base}/commit/${sha}` : base;
	}

	const gitHref = $derived(
		gitWebUrl(detail.git_repository, detail.git_commit_sha),
	);
	const lastDeploy = $derived(relativeTime(detail.last_deployed_at));
	const healthcheck = $derived(detail.healthcheck);
	const healthcheckStr = $derived.by(() => {
		const hc = detail.healthcheck;
		if (!hc) return null;
		const parts: string[] = [];
		if (hc.path) parts.push(hc.path);
		if (hc.interval != null) parts.push(`${hc.interval}s`);
		if (hc.retries != null) parts.push(`${hc.retries} retries`);
		return parts.length > 0 ? parts.join(" · ") : null;
	});
</script>

<dl class="grid grid-cols-[140px_1fr] gap-y-1 text-sm">
	<dt class="text-muted-foreground">UUID</dt>
	<dd class="font-mono text-xs">{detail.uuid}</dd>

	{#if detail.build_pack}
		<dt class="text-muted-foreground">Build pack</dt>
		<dd>{detail.build_pack}</dd>
	{/if}

	{#if detail.git_repository}
		<dt class="text-muted-foreground">Git</dt>
		<dd class="flex flex-wrap items-baseline gap-x-2">
			{#if gitHref}
				<a
					href={gitHref}
					target="_blank"
					rel="noopener noreferrer"
					class="text-primary underline-offset-4 hover:underline"
				>
					{detail.git_repository}
				</a>
			{:else}
				<span>{detail.git_repository}</span>
			{/if}
			{#if detail.git_branch}
				<span class="text-muted-foreground">@ {detail.git_branch}</span>
			{/if}
			{#if detail.git_commit_sha}
				<span class="font-mono text-xs text-muted-foreground">
					{detail.git_commit_sha.slice(0, 7)}
				</span>
			{/if}
		</dd>
	{/if}

	{#if detail.fqdn}
		<dt class="text-muted-foreground">FQDN</dt>
		<dd>
			<a
				href={detail.fqdn}
				target="_blank"
				rel="noopener noreferrer"
				class="text-primary underline-offset-4 hover:underline"
			>
				{detail.fqdn}
			</a>
		</dd>
	{/if}

	{#if detail.ports_exposes}
		<dt class="text-muted-foreground">Ports</dt>
		<dd class="font-mono text-xs">{detail.ports_exposes}</dd>
	{/if}

	{#if lastDeploy}
		<dt class="text-muted-foreground">Last deploy</dt>
		<dd>{lastDeploy}</dd>
	{/if}

	{#if healthcheck && healthcheckStr}
		<dt class="text-muted-foreground">Healthcheck</dt>
		<dd>{healthcheckStr}</dd>
	{/if}

	{#if detail.server_name}
		<dt class="text-muted-foreground">Server</dt>
		<dd>{detail.server_name}</dd>
	{/if}
</dl>
