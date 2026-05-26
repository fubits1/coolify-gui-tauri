<script lang="ts">
	import { onMount } from "svelte";
	import { invoke } from "@tauri-apps/api/core";
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
	} from "$lib/components/ui/card";
	import { Button } from "$lib/components/ui/button";
	import { Input } from "$lib/components/ui/input";
	import { Label } from "$lib/components/ui/label";
	import { instances } from "$lib/stores/instances.svelte";
	import { resourcesRegistry } from "$lib/stores/resources.svelte";
	import { api } from "$lib/api/client";
	import { toast } from "svelte-sonner";

	let editingId = $state<string | null>(null);
	let url = $state("");
	let token = $state("");
	let alias = $state("");
	let pollingPaused = $state(false);
	let dockerHubPat = $state("");
	let ghcrPat = $state("");
	let testing = $state(false);
	let testResult = $state<{ ok: boolean; team?: string; error?: string } | null>(
		null,
	);

	onMount(async () => {
		await instances.load();
		const current = instances.active ?? instances.list[0] ?? null;
		if (current) {
			editingId = current.id;
			url = current.url;
			alias = current.alias;
		}
	});

	$effect(() => {
		if (!editingId) return;
		const inst = instances.list.find((i) => i.id === editingId);
		if (inst) {
			url = inst.url;
			alias = inst.alias;
		}
	});

	async function testConnection() {
		if (!token) {
			toast.error("Enter a token before testing");
			return;
		}
		testing = true;
		testResult = null;
		try {
			const res = await api.testConnection(url, token);
			if (!res.ok) {
				testResult = { ok: false, error: res.error ?? "Unknown error" };
				toast.error(`Connection failed: ${res.error ?? "Unknown error"}`);
				return;
			}
			testResult = { ok: true, team: res.team_name };
			toast.success(
				res.team_name
					? `Connected to team "${res.team_name}"`
					: "Connection OK",
			);
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			testResult = { ok: false, error: message };
			toast.error(`Connection failed: ${message}`);
		} finally {
			testing = false;
		}
	}

	async function saveInstance() {
		if (!editingId) return;
		try {
			if (token) {
				await api.setCredentials(editingId, url, token);
				token = "";
			}
			// Update the persisted url + alias on this instance entry.
			const idx = instances.list.findIndex(
				(instance) => instance.id === editingId,
			);
			if (idx !== -1) {
				const next = [...instances.list];
				const ready = next[idx].ready;
				next[idx] = { id: editingId, url, alias, ready };
				instances.list = next;
				await instances.setActive(instances.activeId ?? editingId);
			}
			toast.success("Instance settings saved");
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			toast.error(`Save failed: ${message}`);
		}
	}

	async function signOut() {
		if (!editingId) return;
		try {
			resourcesRegistry.drop(editingId);
			await instances.remove(editingId);
			window.location.assign("/");
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			toast.error(`Sign out failed: ${message}`);
		}
	}

	async function togglePolling() {
		const active = instances.active;
		if (!active) return;
		const store = resourcesRegistry.ensure(active.id);
		pollingPaused = !pollingPaused;
		try {
			if (pollingPaused) {
				store.stop();
				toast.info("Polling paused");
			} else {
				await store.start();
				toast.info("Polling resumed");
			}
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			toast.error(`Polling toggle failed: ${message}`);
		}
	}

	async function saveRegistryToken(registry: string, value: string) {
		if (!value) {
			toast.error("Enter a token before saving");
			return;
		}
		try {
			await invoke("set_registry_token", { registry, token: value });
			toast.success(`${registry} token saved`);
			if (registry === "docker_hub") dockerHubPat = "";
			if (registry === "ghcr") ghcrPat = "";
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			if (message.toLowerCase().includes("not registered")) {
				toast.message(
					"Registry tokens coming soon — pending Rust handler",
				);
			} else {
				toast.error(`Failed to save ${registry} token: ${message}`);
			}
		}
	}
</script>

<div class="mx-auto flex max-w-2xl flex-col gap-6 p-6">
	<div class="flex items-center gap-3">
		<Button variant="outline" size="sm" href="/">
			← Back
		</Button>
		<h1 class="text-2xl font-semibold">Settings</h1>
	</div>

	<Card>
		<CardHeader>
			<CardTitle>Instances</CardTitle>
			<CardDescription>
				Edit the URL, rotate the bearer token, or remove a Coolify instance.
				Switch the dropdown to edit a different instance.
			</CardDescription>
		</CardHeader>
		<CardContent class="flex flex-col gap-4">
			<div class="flex flex-col gap-2">
				<Label for="instance-select">Editing</Label>
				<select
					id="instance-select"
					class="h-9 rounded-md border border-input bg-background px-2 text-sm"
					bind:value={editingId}
				>
					{#each instances.list as inst (inst.id)}
						<option value={inst.id}>{inst.alias} — {inst.url}</option>
					{/each}
				</select>
			</div>

			<div class="flex flex-col gap-2">
				<Label for="instance-url">URL</Label>
				<Input
					id="instance-url"
					type="url"
					placeholder="https://coolify.example.com"
					bind:value={url}
				/>
			</div>
			<div class="flex flex-col gap-2">
				<Label for="instance-token">Token</Label>
				<Input
					id="instance-token"
					type="password"
					placeholder="leave blank to keep current"
					bind:value={token}
				/>
			</div>
			<div class="flex flex-col gap-2">
				<Label for="instance-alias">Alias</Label>
				<Input
					id="instance-alias"
					type="text"
					placeholder="acme-prod"
					bind:value={alias}
				/>
			</div>

			{#if testResult}
				<p
					class="text-xs {testResult.ok
						? 'text-green-500'
						: 'text-destructive'}"
				>
					{testResult.ok
						? `OK${testResult.team ? ` — team: ${testResult.team}` : ""}`
						: `Error: ${testResult.error}`}
				</p>
			{/if}

			<div class="flex gap-2">
				<Button
					variant="outline"
					onclick={testConnection}
					disabled={testing || !token}
				>
					{testing ? "Testing…" : "Test"}
				</Button>
				<Button onclick={saveInstance}>Save</Button>
				<Button variant="destructive" class="ml-auto" onclick={signOut}>
					Remove instance
				</Button>
			</div>
		</CardContent>
	</Card>

	<Card>
		<CardHeader>
			<CardTitle>Polling</CardTitle>
			<CardDescription>
				Live resource refresh on the ACTIVE instance while the window is
				focused.
			</CardDescription>
		</CardHeader>
		<CardContent class="flex flex-col gap-4">
			<div class="flex items-center justify-between">
				<div class="flex flex-col gap-1">
					<Label for="polling-pause">Pause polling</Label>
					<p class="text-xs text-muted-foreground">
						When paused, the resource list stops auto-refreshing.
					</p>
				</div>
				<Button
					id="polling-pause"
					variant={pollingPaused ? "default" : "outline"}
					onclick={togglePolling}
				>
					{pollingPaused ? "Resume" : "Pause"}
				</Button>
			</div>
			<div class="flex items-center justify-between text-sm">
				<span class="text-muted-foreground">Cadence</span>
				<span>5s</span>
			</div>
		</CardContent>
	</Card>

	<Card>
		<CardHeader>
			<CardTitle>Registries</CardTitle>
			<CardDescription>
				Personal access tokens for private image registries. Stored in the
				OS keyring.
			</CardDescription>
		</CardHeader>
		<CardContent class="flex flex-col gap-4">
			<div class="flex flex-col gap-2">
				<Label for="registry-dockerhub">Docker Hub PAT</Label>
				<div class="flex gap-2">
					<Input
						id="registry-dockerhub"
						type="password"
						placeholder="dckr_pat_…"
						bind:value={dockerHubPat}
					/>
					<Button
						variant="outline"
						onclick={() => saveRegistryToken("docker_hub", dockerHubPat)}
					>
						Save
					</Button>
				</div>
			</div>
			<div class="flex flex-col gap-2">
				<Label for="registry-ghcr">GHCR PAT</Label>
				<div class="flex gap-2">
					<Input
						id="registry-ghcr"
						type="password"
						placeholder="ghp_…"
						bind:value={ghcrPat}
					/>
					<Button
						variant="outline"
						onclick={() => saveRegistryToken("ghcr", ghcrPat)}
					>
						Save
					</Button>
				</div>
			</div>
		</CardContent>
	</Card>

	<Card>
		<CardHeader>
			<CardTitle>About</CardTitle>
		</CardHeader>
		<CardContent class="flex flex-col gap-2 text-sm">
			<div class="flex items-center justify-between">
				<span class="text-muted-foreground">Version</span>
				<span>0.1.0</span>
			</div>
			<div class="flex items-center justify-between">
				<span class="text-muted-foreground">Coolify docs</span>
				<a
					class="text-primary underline-offset-4 hover:underline"
					href="https://coolify.io/docs"
					target="_blank"
					rel="noreferrer"
				>
					coolify.io/docs
				</a>
			</div>
		</CardContent>
	</Card>
</div>
