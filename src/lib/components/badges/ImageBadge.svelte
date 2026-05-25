<!--
@component
ImageBadge — compact shadcn `<Badge>` summarizing image-staleness for a service.

Display rules:
- `checkedAt == null` → muted `?` ("unchecked")
- `stale === 0` → green `✓`
- `stale > 0` → amber `{stale} stale`

The `title` attribute carries `"Last checked: <localized date>"` when `checkedAt` is set.

Props:
- `stale: number` — number of stale image digests
- `total: number` — total images considered (currently informational)
- `checkedAt: number | null` — epoch ms of the last check, or `null` if never checked
-->
<script lang="ts">
	import { Badge, type BadgeVariant } from "$lib/components/ui/badge";

	let {
		stale,
		total,
		checkedAt,
	}: {
		stale: number;
		total: number;
		checkedAt: number | null;
	} = $props();

	type View = { variant: BadgeVariant; class: string; label: string };

	const view: View = $derived.by(() => {
		if (checkedAt == null) {
			return { variant: "secondary", class: "", label: "?" };
		}
		if (stale === 0) {
			return {
				variant: "default",
				class: "bg-green-600/20 text-green-400 border-green-600/30",
				label: "✓",
			};
		}
		return {
			variant: "default",
			class: "bg-amber-600/20 text-amber-400 border-amber-600/30",
			label: `${stale} stale`,
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
