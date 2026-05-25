<!--
@component
DeployDialog — confirmation dialog for the Deploy action.

Shows explanatory text and a `force_rebuild` checkbox (skip Docker cache).
The parent controls visibility via `open`; `onConfirm(force)` is invoked
when the user clicks Deploy. `onClose` fires for Cancel / overlay close.

Props:
- `open: boolean` — bound externally; dialog visibility.
- `onClose: () => void` — fired when user cancels.
- `onConfirm: (force: boolean) => void` — fired when user confirms.
-->
<script lang="ts">
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogDescription,
		DialogFooter,
	} from "$lib/components/ui/dialog";
	import { Button } from "$lib/components/ui/button";

	let {
		open,
		onClose,
		onConfirm,
	}: {
		open: boolean;
		onClose: () => void;
		onConfirm: (force: boolean) => void;
	} = $props();

	let force = $state(false);

	// Reset the checkbox each time the dialog opens, so a previous "force"
	// choice doesn't silently carry over into the next deploy.
	$effect(() => {
		if (open) force = false;
	});

	function handleOpenChange(next: boolean) {
		if (!next) onClose();
	}

	function handleConfirm() {
		onConfirm(force);
	}
</script>

<Dialog {open} onOpenChange={handleOpenChange}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>Deploy resource?</DialogTitle>
			<DialogDescription>
				Triggers a new deployment on Coolify. The resource may briefly go
				offline while the new container starts.
			</DialogDescription>
		</DialogHeader>

		<label class="flex items-start gap-2 text-sm">
			<input
				type="checkbox"
				class="mt-0.5 size-4 rounded border-input bg-background accent-primary"
				bind:checked={force}
			/>
			<span>
				<span class="font-medium">Force rebuild</span>
				<span class="block text-xs text-muted-foreground">
					Skip the Docker build cache. Slower, but ensures a fresh build.
				</span>
			</span>
		</label>

		<DialogFooter>
			<Button variant="outline" onclick={onClose}>Cancel</Button>
			<Button onclick={handleConfirm}>Deploy</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
