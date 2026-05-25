<script lang="ts">
	import ConnectScreen from "$lib/components/onboarding/ConnectScreen.svelte";
	import { instance } from "$lib/stores/instance.svelte";

	// Kick off load; instance state is reactive once it lands.
	instance.load();

	const needsOnboarding = $derived(instance.url == null);

	function handleConnected(_url: string, _alias: string) {
		// Other agents own the overview UI / resource refresh.
		// Triggering instance.load() ensures derived `needsOnboarding` flips.
		instance.load();
	}
</script>

{#if needsOnboarding}
	<ConnectScreen onConnected={handleConnected} />
{:else}
	<div class="p-6">Overview list — coming soon</div>
{/if}
