import { SvelteMap } from "svelte/reactivity";

/**
 * Per-instance connection store. Each Coolify instance has its own
 * state machine: `connected` ↔ `reconnecting` ↔ `offline`. The
 * `connectionRegistry` hands out a singleton per `instanceId`; the
 * resources poller calls `markOk()` / `markFailure()` on the entry
 * matching the instance it's polling.
 *
 * Top-strip UI reads the ACTIVE instance's store (`instances.active.id`)
 * for its label; the InstanceTabStrip reads each tab's store for the
 * status dot color.
 */

type ConnectionState = "connected" | "reconnecting" | "offline";

const RECONNECT_SCHEDULE = [1, 2, 4, 8, 16, 30] as const;

export class ConnectionStore {
  state: ConnectionState = $state("connected");
  lastPingAt: number | null = $state(null);
  reconnectInSec: number | null = $state(null);

  #stepIdx = 0;

  markOk() {
    this.state = "connected";
    this.lastPingAt = Date.now();
    this.reconnectInSec = null;
    this.#stepIdx = 0;
  }

  markFailure() {
    this.state = "reconnecting";
    const idx = Math.min(this.#stepIdx, RECONNECT_SCHEDULE.length - 1);
    this.reconnectInSec = RECONNECT_SCHEDULE[idx];
    this.#stepIdx = idx + 1;
  }

  markOffline() {
    this.state = "offline";
    this.reconnectInSec = null;
  }
}

class ConnectionRegistry {
  #stores: SvelteMap<string, ConnectionStore> = new SvelteMap();

  /** Pure read. Returns the store if it exists; null otherwise. Safe to
   *  call from `$derived` / template expressions because it never
   *  mutates the underlying map. */
  get(instanceId: string): ConnectionStore | null {
    return this.#stores.get(instanceId) ?? null;
  }

  /** Imperative: create + cache the store if missing. Call from instance
   *  lifecycle (add, load) — NEVER from a $derived or template. */
  ensure(instanceId: string): ConnectionStore {
    let store = this.#stores.get(instanceId);
    if (!store) {
      store = new ConnectionStore();
      this.#stores.set(instanceId, store);
    }
    return store;
  }

  drop(instanceId: string): void {
    this.#stores.delete(instanceId);
  }
}

export const connectionRegistry = new ConnectionRegistry();
