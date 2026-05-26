import { SvelteMap } from "svelte/reactivity";
import { api } from "$lib/api/client";
import type { Resource } from "$lib/api/types";
import { connectionRegistry } from "./connection.svelte";
import { toast } from "$lib/util/toast.svelte";

/**
 * Per-instance resources store. Each Coolify instance owns its own
 * polling loop + list state. The registry hands out a singleton per
 * `instanceId`. Only the ACTIVE instance has its `start()` called; the
 * rest stay paused with their last-known list (and selection) intact,
 * so switching tabs is instant without API calls.
 */

const POLL_MS = 5000;

export class ResourcesStore {
  readonly instanceId: string;

  list: Resource[] = $state([]);
  selectedUuid: string | null = $state(null);
  loading: boolean = $state(false);
  lastRefreshAt: number | null = $state(null);
  lastError: string | null = $state(null);
  endpointErrors: Record<string, string> = $state({});

  selectedResource: Resource | null = $derived(
    this.selectedUuid === null
      ? null
      : (this.list.find((r) => r.uuid === this.selectedUuid) ?? null),
  );

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
  #started = false;
  #errorToasted = false;
  #partialToasted = new Set<string>();

  constructor(instanceId: string) {
    this.instanceId = instanceId;
  }

  async start(): Promise<void> {
    if (this.#started) return;
    this.#started = true;
    if (typeof window !== "undefined") {
      window.addEventListener("blur", this.#onBlur);
      window.addEventListener("focus", this.#onFocus);
    }
    await this.refresh();
    this.#resume();
  }

  stop(): void {
    this.#pause();
    if (typeof window !== "undefined") {
      window.removeEventListener("blur", this.#onBlur);
      window.removeEventListener("focus", this.#onFocus);
    }
    this.#started = false;
  }

  async refresh(): Promise<void> {
    this.loading = true;
    const conn = connectionRegistry.ensure(this.instanceId);
    try {
      const result = await api.listResources(this.instanceId);
      this.list = result.resources;
      this.endpointErrors = result.errors;
      this.lastRefreshAt = Date.now();
      this.lastError = null;
      this.#errorToasted = false;
      for (const [endpoint, msg] of Object.entries(result.errors)) {
        if (!this.#partialToasted.has(endpoint)) {
          this.#partialToasted.add(endpoint);
          console.warn(
            `[resources ${this.instanceId}] /${endpoint} failed:`,
            msg,
          );
          toast.error(`Failed to load /${endpoint}`, msg);
        }
      }
      conn.markOk();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.lastError = msg;
      console.error(
        `[resources ${this.instanceId}] listResources failed:`,
        msg,
      );
      if (!this.#errorToasted) {
        toast.error("Failed to load resources", msg);
        this.#errorToasted = true;
      }
      conn.markFailure();
    } finally {
      this.loading = false;
    }
  }

  select(uuid: string | null): void {
    this.selectedUuid = uuid;
  }

  #pause(): void {
    if (this.#timer !== null) {
      clearInterval(this.#timer);
      this.#timer = null;
    }
  }

  #resume(): void {
    if (this.#timer !== null) return;
    if (typeof document !== "undefined" && !document.hasFocus()) return;
    this.#timer = setInterval(() => {
      void this.refresh();
    }, POLL_MS);
  }
}

class ResourcesRegistry {
  #stores: SvelteMap<string, ResourcesStore> = new SvelteMap();

  /** Pure read; null if absent. Safe in $derived / template expressions. */
  get(instanceId: string): ResourcesStore | null {
    return this.#stores.get(instanceId) ?? null;
  }

  /** Imperative create-or-get. Call from instance lifecycle only. */
  ensure(instanceId: string): ResourcesStore {
    let store = this.#stores.get(instanceId);
    if (!store) {
      store = new ResourcesStore(instanceId);
      this.#stores.set(instanceId, store);
    }
    return store;
  }

  drop(instanceId: string): void {
    const store = this.#stores.get(instanceId);
    if (store) {
      store.stop();
      this.#stores.delete(instanceId);
    }
  }
}

export const resourcesRegistry = new ResourcesRegistry();

/**
 * Owns "which instance is currently polling". Holds non-reactive state
 * so callers can drive switches from `$effect` without violating the
 * "no $effect writes to $state" rule from `code-style-svelte`.
 */
class PollingController {
  #runningId: string | null = null;

  /**
   * Pause the currently-polling instance (if any) and start polling the
   * new one. Pass `null` to stop without starting anything new. Safe to
   * call repeatedly with the same id — no-ops on match.
   */
  async switchTo(instanceId: string | null): Promise<void> {
    if (this.#runningId === instanceId) return;
    if (this.#runningId !== null) {
      resourcesRegistry.get(this.#runningId)?.stop();
    }
    this.#runningId = instanceId;
    if (instanceId !== null) {
      const store = resourcesRegistry.ensure(instanceId);
      await store.start();
    }
  }
}

export const pollingController = new PollingController();
