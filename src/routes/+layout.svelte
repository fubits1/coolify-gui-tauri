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

  function recount() {
    visibleCount = document.querySelectorAll("[data-sonner-toast]").length;
  }

  $effect(() => {
    if (typeof document === "undefined") return;
    recount();
    const observer = new MutationObserver(recount);
    observer.observe(document.body, { childList: true, subtree: true });
    return () => observer.disconnect();
  });

  /**
   * MutationObserver alone has been observed to leave the button +
   * count stale after dismiss-all (svelte-sonner sometimes re-renders
   * toast elements asynchronously, racing the observer's recount). Run
   * an explicit recount immediately AND on the next animation frame to
   * cover both the synchronous DOM-removal path and any deferred sonner
   * re-render.
   */
  function handleDismissAll() {
    toast.dismiss();
    recount();
    requestAnimationFrame(recount);
    setTimeout(recount, 200);
  }
</script>

<Toaster richColors />
{#if visibleCount >= 2}
  <button
    type="button"
    class="fixed bottom-2 right-2 z-[2147483647] inline-flex items-center gap-1 rounded-md border border-border bg-background/95 px-2.5 py-1 text-xs font-medium text-foreground shadow-md backdrop-blur hover:bg-accent"
    onclick={handleDismissAll}
    title="Dismiss all toasts"
  >
    Dismiss all ({visibleCount})
  </button>
{/if}
{@render children?.()}
