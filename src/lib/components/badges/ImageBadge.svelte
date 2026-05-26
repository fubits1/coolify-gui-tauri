<!--
@component
ImageBadge — compact shadcn `<Badge>` summarizing image-staleness for a service.

Display rules:
- `checkedAt == null` → muted `?` ("unchecked")
- `stale > 0` → amber `{stale} stale`
- `unknown > 0` (and no staleness signal) → muted `?` — refuses to claim green
   when any image's drift is undeterminable
- otherwise → green `✓`

The `title` attribute carries `"Last checked: <localized date>"` when `checkedAt` is set.

Props:
- `stale: number` — number of stale image digests
- `unknown: number` — number of images whose drift couldn't be determined
- `total: number` — total images considered (currently informational)
- `checkedAt: number | null` — epoch ms of the last check, or `null` if never checked
-->
<script lang="ts">
	import { Badge, type BadgeVariant } from "$lib/components/ui/badge";

	let {
		stale,
		unknown = 0,
		total,
		checkedAt,
	}: {
		stale: number;
		unknown?: number;
		total: number;
		checkedAt: number | null;
	} = $props();

	type View = { variant: BadgeVariant; class: string; label: string };

	const view: View = $derived.by(() => {
		if (checkedAt == null) {
			return { variant: "secondary", class: "", label: "?" };
		}
		if (total === 0) {
			return { variant: "secondary", class: "", label: "?" };
		}
		if (stale > 0) {
			return {
				variant: "default",
				class: "bg-amber-600/20 text-amber-400 border-amber-600/30",
				label: `${stale} stale`,
			};
		}
		// No stale-positive signal — but green only when EVERY image was
		// classifiable. Any "unknown" image means the green ✓ would lie.
		if (unknown > 0) {
			return { variant: "secondary", class: "", label: `${unknown}?` };
		}
		return {
			variant: "default",
			class: "bg-green-600/20 text-green-400 border-green-600/30",
			label: "✓",
		};
	});

	const title = $derived(
		checkedAt == null
			? `Unchecked (${total} images)`
			: `Last checked: ${new Date(checkedAt).toLocaleString()} (${total} images)`,
	);
</script>

<Badge variant={view.variant} class={view.class} {title}>
	{view.label}
</Badge>
