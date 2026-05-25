<!--
@component
StatusBadge — colored shadcn `<Badge>` representing a Docker container/service status.

Palette by `status.state`:
- `running` → green (custom on `default` variant)
- `exited` → red (`destructive` variant)
- `degraded` / `starting` → amber (custom)
- `excluded` → muted/gray (`secondary` variant)

Displays `status.raw` (e.g. `"running:healthy"`) as the badge label.

Props:
- `status: { state: string; health?: string; raw: string }`
-->
<script lang="ts">
	import { Badge, type BadgeVariant } from "$lib/components/ui/badge";

	let {
		status,
	}: {
		status: { state: string; health?: string; raw: string };
	} = $props();

	type Style = { variant: BadgeVariant; class: string };

	const style: Style = $derived.by(() => {
		switch (status.state) {
			case "running":
				return {
					variant: "default",
					class: "bg-green-600/20 text-green-400 border-green-600/30",
				};
			case "exited":
				return { variant: "destructive", class: "" };
			case "degraded":
			case "starting":
				return {
					variant: "default",
					class: "bg-amber-600/20 text-amber-400 border-amber-600/30",
				};
			case "excluded":
				return { variant: "secondary", class: "" };
			default:
				return { variant: "outline", class: "" };
		}
	});
</script>

<Badge variant={style.variant} class={style.class} title={status.raw}>
	{status.raw}
</Badge>
