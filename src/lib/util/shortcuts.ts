/**
 * Global keyboard shortcut handler.
 *
 * All bindings use the **Cmd+Shift+** (macOS) / **Ctrl+Shift+** (Win/Linux)
 * prefix to dodge browser + OS conflicts. Earlier `⌘R` / `⌘D` etc. clashed
 * with reload, bookmark, page-info, and address-bar — `⌘R` accidentally
 * restarting a production container is unacceptable.
 *
 * Bindings:
 *   ⌘⇧R / Ctrl+Shift+R  → onRestart  (the unshifted ⌘R stays reload)
 *   ⌘⇧D / Ctrl+Shift+D  → onDeploy
 *   ⌘⇧I / Ctrl+Shift+I  → onCheckImages
 *   ⌘⇧L / Ctrl+Shift+L  → onLogs
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
    if (!(e.metaKey || e.ctrlKey) || !e.shiftKey) return;
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
