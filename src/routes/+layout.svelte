<script lang="ts">
  import "../app.css";
  import { Toaster } from "$lib/components/ui/sonner";
  import { toast } from "$lib/util/toast.svelte";

  let { children } = $props();

  // Truth source for the toast count is the LIVE DOM, not our wrapper
  // counter — sonner's onDismiss/onAutoClose callbacks were unreliable
  // (one-off drift after auto-close). A MutationObserver on document.body
  // recounts whenever a toast is added/removed; cheap because sonner's
  // toast list is tiny.
  let visibleCount = $state(0);

  $effect(() => {
    if (typeof document === "undefined") return;
    const recount = () => {
      visibleCount = document.querySelectorAll("[data-sonner-toast]").length;
    };
    recount();
    const observer = new MutationObserver(recount);
    observer.observe(document.body, { childList: true, subtree: true });
    return () => observer.disconnect();
  });
</script>

<Toaster richColors />
{#if visibleCount >= 2}
  <button
    type="button"
    class="fixed bottom-2 right-2 z-[2147483647] inline-flex items-center gap-1 rounded-md border border-border bg-background/95 px-2.5 py-1 text-xs font-medium text-foreground shadow-md backdrop-blur hover:bg-accent"
    onclick={() => toast.dismiss()}
    title="Dismiss all toasts"
  >
    Dismiss all ({visibleCount})
  </button>
{/if}
{@render children?.()}
