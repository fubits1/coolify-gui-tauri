/**
 * Global keyboard shortcut handler.
 *
 * Install once from the root layout. The caller wires handlers to the
 * currently-selected resource (see `stores/resources.svelte.ts`). When no
 * resource is selected the caller should leave the relevant handler
 * undefined — the dispatcher then no-ops for that key.
 *
 * Bindings (Cmd on macOS, Ctrl on Windows/Linux):
 *   ⌘R / Ctrl+R  → onRestart
 *   ⌘D / Ctrl+D  → onDeploy
 *   ⌘I / Ctrl+I  → onCheckImages
 *   ⌘L / Ctrl+L  → onLogs
 *
 * Returns a cleanup function (idiomatic for `$effect` in Svelte 5).
 */
export interface ShortcutHandlers {
  onRestart?: () => void;
  onDeploy?: () => void;
  onCheckImages?: () => void;
  onLogs?: () => void;
}

export function installShortcuts(handlers: ShortcutHandlers): () => void {
  function onKey(e: KeyboardEvent) {
    if (!(e.metaKey || e.ctrlKey)) return;
    const key = e.key.toLowerCase();
    if (key === "r") {
      e.preventDefault();
      handlers.onRestart?.();
    } else if (key === "d") {
      e.preventDefault();
      handlers.onDeploy?.();
    } else if (key === "i") {
      e.preventDefault();
      handlers.onCheckImages?.();
    } else if (key === "l") {
      e.preventDefault();
      handlers.onLogs?.();
    }
  }
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
