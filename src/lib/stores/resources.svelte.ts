import { api } from "$lib/api/client";
import type { Resource } from "$lib/api/types";
import { connection } from "./connection.svelte";
import { toast } from "$lib/util/toast";

/**
 * Resources store — owns the polling loop that drives the overview screen.
 *
 * Cadence 5s while `document.hasFocus()`; pauses on `window.blur`, resumes
 * on `window.focus`. Failures keep the last good list visible and bump the
 * `connection` store to `reconnecting` (the strip handles the countdown
 * presentation). Successes set it back to `connected`.
 *
 * Lifecycle ownership is explicit: routes call `start()` on mount and
 * `stop()` on destroy. We don't auto-bind to focus in the constructor
 * because that would fire during SSR/Vitest and during onboarding (before
 * credentials exist).
 *
 * Idiom note: same class-singleton pattern as the other stores. Derived
 * helpers live as `$derived` getter fields on the class so callers can do
 * `resources.selectedResource` like any other reactive read.
 */

const POLL_MS = 5000;

class ResourcesStore {
  list: Resource[] = $state([]);
  selectedUuid: string | null = $state(null);
  loading: boolean = $state(false);
  lastRefreshAt: number | null = $state(null);

  /** The currently selected row, or null if none selected / not in list. */
  selectedResource: Resource | null = $derived(
    this.selectedUuid === null
      ? null
      : (this.list.find((r) => r.uuid === this.selectedUuid) ?? null),
  );

  /**
   * List grouped by project name. Resources without a `project_name`
   * fall under the literal key `"(no project)"` so the UI can still show
   * them rather than silently dropping rows.
   */
  resourcesByProject: Record<string, Resource[]> = $derived.by(() => {
    const groups: Record<string, Resource[]> = {};
    for (const r of this.list) {
      const key = r.project_name ?? "(no project)";
      (groups[key] ??= []).push(r);
    }
    return groups;
  });

  #timer: ReturnType<typeof setInterval> | null = null;
  #onBlur = () => this.#pause();
  #onFocus = () => this.#resume();
  /** True once `start()` has been called. Guards double-start. */
  #started = false;

  /** Initial fetch + start the focus-aware polling loop. Idempotent. */
  async start(): Promise<void> {
    if (this.#started) return;
    this.#started = true;
    // Only attach focus listeners in browser-land; bail cleanly under SSR.
    if (typeof window !== "undefined") {
      window.addEventListener("blur", this.#onBlur);
      window.addEventListener("focus", this.#onFocus);
    }
    await this.refresh();
    this.#resume();
  }

  /** Tear down the loop + listeners. Safe to call when not started. */
  stop(): void {
    this.#pause();
    if (typeof window !== "undefined") {
      window.removeEventListener("blur", this.#onBlur);
      window.removeEventListener("focus", this.#onFocus);
    }
    this.#started = false;
  }

  /**
   * One-shot fetch. Always safe to call (e.g. from a "Refresh" button).
   * Does NOT clear `list` on failure — stale-but-visible beats blank.
   */
  /** Last error message from the most recent failed refresh, for debugging. */
  lastError: string | null = $state(null);
  /** True once an error toast has been shown — suppresses spam across retries. */
  #errorToasted = false;

  /** Per-endpoint partial-failure messages from the last refresh. */
  endpointErrors: Record<string, string> = $state({});
  /** Set of endpoint names we've already toasted, to avoid spam across polls. */
  #partialToasted = new Set<string>();

  async refresh(): Promise<void> {
    this.loading = true;
    try {
      const result = await api.listResources();
      this.list = result.resources;
      this.endpointErrors = result.errors;
      this.lastRefreshAt = Date.now();
      this.lastError = null;
      this.#errorToasted = false;
      // Surface partial failures (e.g. /applications 403 while /services ok)
      // ONCE per endpoint per session.
      for (const [endpoint, msg] of Object.entries(result.errors)) {
        if (!this.#partialToasted.has(endpoint)) {
          this.#partialToasted.add(endpoint);
          console.warn(`[resources] /${endpoint} failed:`, msg);
          toast.error(`Failed to load /${endpoint}`, msg);
        }
      }
      connection.markOk();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.lastError = msg;
      console.error("[resources] listResources failed:", msg);
      if (!this.#errorToasted) {
        toast.error("Failed to load resources", msg);
        this.#errorToasted = true;
      }
      // Keep last good list visible. Connection strip surfaces the failure.
      connection.markFailure();
    } finally {
      this.loading = false;
    }
  }

  /** Set selection. Pass `null` to clear. */
  select(uuid: string | null): void {
    this.selectedUuid = uuid;
  }

  /** Clear the interval (used by `stop()` and on blur). */
  #pause(): void {
    if (this.#timer !== null) {
      clearInterval(this.#timer);
      this.#timer = null;
    }
  }

  /**
   * Start (or restart) the 5s interval — but only if the window has focus.
   * If we're called while blurred (e.g. user `start()`s an unfocused window),
   * the blur listener will keep us paused until focus returns.
   */
  #resume(): void {
    if (this.#timer !== null) return;
    if (typeof document !== "undefined" && !document.hasFocus()) return;
    this.#timer = setInterval(() => {
      void this.refresh();
    }, POLL_MS);
  }
}

export const resources = new ResourcesStore();
