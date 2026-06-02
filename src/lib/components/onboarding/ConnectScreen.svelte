<!--
@component
ConnectScreen — first-run onboarding for the Coolify instance.

Collects URL + API token + optional alias, calls Rust `test_connection`,
and on success persists the token to the OS keyring and the URL/alias to
the instance store, then notifies the parent via `onConnected`.

Required token scope: `read:sensitive` (env values) + `deploy` (Restart/Stop/Deploy).

Props:
- `onConnected: (url: string, alias: string) => void` — fired after a successful Save.
-->
<script lang="ts">
	import { Button } from "$lib/components/ui/button";
	import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "$lib/components/ui/card";
	import { Input } from "$lib/components/ui/input";
	import { Label } from "$lib/components/ui/label";
	import { api } from "$lib/api/client";
	import { instances } from "$lib/stores/instances.svelte";
	import { toast } from "$lib/util/toast.svelte";

	let {
		onConnected,
		embed = false,
	}: {
		/** Called with the newly-added instance id once it's persisted. */
		onConnected: (id: string) => void;
		/** When true, renders inline (no full-screen wrapper) for the
		 *  "+ add instance" flow inside the multi-instance shell. */
		embed?: boolean;
	} = $props();

	type TeamOption = { id: number; name: string };

	let url = $state("");
	let token = $state("");
	let alias = $state("");
	let testing = $state(false);
	let saving = $state(false);
	let tested = $state<{
		ok: boolean;
		teams?: TeamOption[];
		/** The single team this token actually queries against. Coolify's
		 *  `/applications` etc. ignore client team selection — they filter
		 *  by the team this PAT was bound to at creation. Other entries
		 *  in `teams` are visible but not queryable with this token. */
		boundTeamId?: number | null;
		error?: string;
	} | null>(null);
	/** User's pick in the team dropdown. Pre-filled to `/teams/current`
	 *  on a successful test; can be re-selected before save. */
	let selectedTeamId = $state<number | null>(null);

	const canTest = $derived(url.trim().length > 0 && token.trim().length > 0 && !testing);
	const canSave = $derived(
		tested?.ok === true && selectedTeamId !== null && !saving,
	);

	const selectedTeam = $derived(
		tested?.teams?.find((t) => t.id === selectedTeamId) ?? null,
	);

	async function handleTest() {
		testing = true;
		tested = null;
		try {
			const result = await api.testConnection(url.trim(), token.trim());
			if (result.ok) {
				if (!result.teams || result.teams.length === 0) {
					const msg =
						"Coolify returned no teams for this token. Check the token's scope (`read:sensitive` + `deploy`).";
					tested = { ok: false, error: msg };
					toast.error("Connection failed", msg);
					return;
				}
				tested = {
					ok: true,
					teams: result.teams,
					boundTeamId: result.current_team_id ?? null,
				};
				// Coolify's API hard-filters /applications by the token's
				// bound team — selecting another team in the dropdown
				// would just mislabel a tab while showing the wrong
				// resources. Force the bound team as the only valid pick.
				const bound =
					result.teams.find((t) => t.id === result.current_team_id) ??
					result.teams[0];
				selectedTeamId = bound.id;
			} else {
				const err = result.error ?? "Unknown error";
				tested = { ok: false, error: err };
				toast.error("Connection failed", err);
			}
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			tested = { ok: false, error: msg };
			toast.error("Connection failed", msg);
		} finally {
			testing = false;
		}
	}

	async function handleSave() {
		if (!tested?.ok || !selectedTeam) return;
		saving = true;
		try {
			const aliasValue = alias.trim() || new URL(url.trim()).host;
			const added = await instances.add(
				url.trim(),
				token.trim(),
				aliasValue,
				selectedTeam.id,
				selectedTeam.name,
			);
			toast.success(`Connected to ${aliasValue} · ${selectedTeam.name}`);
			onConnected(added.id);
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			toast.error("Failed to save credentials", msg);
		} finally {
			saving = false;
		}
	}

	// Editing the form invalidates a previous successful test.
	function invalidate() {
		if (tested) {
			tested = null;
			selectedTeamId = null;
		}
	}
</script>

<div
	class={embed
		? "p-4"
		: "flex min-h-screen items-center justify-center bg-background p-6"}
>
	<Card class={embed ? "w-full" : "w-full max-w-md"}>
		<CardHeader>
			<CardTitle>Connect to Coolify</CardTitle>
			<CardDescription>
				Paste your Coolify URL and API token to get started.
			</CardDescription>
		</CardHeader>
		<CardContent>
			<form
				class="flex flex-col gap-4"
				onsubmit={(e) => {
					e.preventDefault();
					if (canSave) {
						handleSave();
					} else if (canTest) {
						handleTest();
					}
				}}
			>
				<div class="flex flex-col gap-1.5">
					<Label for="coolify-url">Coolify URL</Label>
					<Input
						id="coolify-url"
						type="url"
						placeholder="https://coolify.example.com"
						autocomplete="off"
						bind:value={url}
						oninput={invalidate}
					/>
				</div>

				<div class="flex flex-col gap-1.5">
					<Label for="coolify-token">API token</Label>
					<Input
						id="coolify-token"
						type="password"
						autocomplete="off"
						bind:value={token}
						oninput={invalidate}
					/>
					<p class="text-xs text-muted-foreground">
						Token scope required:
						<code class="rounded bg-muted px-1 py-0.5 font-mono text-[0.7rem]">read:sensitive</code>
						+
						<code class="rounded bg-muted px-1 py-0.5 font-mono text-[0.7rem]">deploy</code>.
						<span
							class="ml-0.5 inline-flex size-3.5 cursor-help items-center justify-center rounded-full border border-muted-foreground/40 text-[0.6rem] text-muted-foreground"
							title="read:sensitive lets us show env values; deploy enables Restart/Stop/Deploy buttons."
							aria-label="Token scope details"
						>
							?
						</span>
					</p>
				</div>

				<div class="flex flex-col gap-1.5">
					<Label for="coolify-alias">Alias <span class="text-muted-foreground">(optional)</span></Label>
					<Input
						id="coolify-alias"
						type="text"
						placeholder="production"
						autocomplete="off"
						bind:value={alias}
					/>
				</div>

				{#if tested?.ok && tested.teams}
					<div
						class="flex items-center gap-2 rounded-md border border-green-500/40 bg-green-500/10 px-3 py-2 text-sm text-green-400"
						role="status"
					>
						<span aria-hidden="true">✓</span>
						<span>
							Connected — {tested.teams.length} team{tested.teams.length === 1 ? "" : "s"} visible.
						</span>
					</div>
					<div class="flex flex-col gap-1.5">
						<Label for="coolify-team">Team</Label>
						<select
							id="coolify-team"
							class="h-9 rounded-md border border-input bg-background px-2 text-sm"
							bind:value={selectedTeamId}
						>
							{#each tested.teams as team (team.id)}
								{@const bound = team.id === tested.boundTeamId}
								<option value={team.id} disabled={!bound}>
									{team.name} (id: {team.id}){bound ? "" : " — needs separate PAT"}
								</option>
							{/each}
						</select>
						<p class="text-xs text-muted-foreground">
							Coolify's API hard-filters resources by the team this PAT was created in.
							Other teams here are visible but not queryable with this token —
							switch teams in Coolify's dashboard first, then create a new PAT for that team.
						</p>
					</div>
				{:else if tested && !tested.ok}
					<div
						class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive"
						role="alert"
					>
						{tested.error ?? "Connection failed."}
					</div>
				{/if}

				{#if canSave}
					<Button type="submit" disabled={!canSave}>
						{saving ? "Saving…" : "Save & open"}
					</Button>
				{:else}
					<Button type="submit" disabled={!canTest}>
						{testing ? "Testing…" : "Test connection"}
					</Button>
				{/if}
			</form>
		</CardContent>
	</Card>
</div>
