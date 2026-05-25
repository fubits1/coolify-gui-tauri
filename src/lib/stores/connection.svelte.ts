/**
 * Connection store — drives the top-of-window status strip.
 *
 * One singleton (`connection`) shared across the app. Transitions:
 *
 * ```
 *  connected ── markFailure ──▶ reconnecting (1s) ──▶ (2,4,8,16,30,30…s)
 *      ▲              │
 *      └── markOk ────┘ (resets countdown to baseline)
 *
 *  any state ── markOffline ──▶ offline (countdown cleared)
 * ```
 *
 * The store only models presentation state — it does not schedule retries.
 * The poll loop in `resources.svelte.ts` drives the cadence; this store
 * just answers "what string + colour does the strip render?" via `state`
 * and "what countdown number do we show?" via `reconnectInSec`.
 *
 * Idiom note: class with `$state` fields + a singleton export. This avoids
 * the "can't export reassigned `$state`" caveat (see Svelte 5 docs on
 * passing state across modules) and lets methods read/write peer fields
 * without prop drilling.
 */

type ConnectionState = "connected" | "reconnecting" | "offline";

/** Exponential countdown schedule used while reconnecting (seconds). */
const RECONNECT_SCHEDULE = [1, 2, 4, 8, 16, 30] as const;

class ConnectionStore {
  state: ConnectionState = $state("connected");
  lastPingAt: number | null = $state(null);
  reconnectInSec: number | null = $state(null);

  /** Index into RECONNECT_SCHEDULE for the next failure. Hidden from UI. */
  #stepIdx = 0;

  /** Record a successful round-trip. Resets failure counter + countdown. */
  markOk() {
    this.state = "connected";
    this.lastPingAt = Date.now();
    this.reconnectInSec = null;
    this.#stepIdx = 0;
  }

  /**
   * Record a transient failure. Bumps the strip to `reconnecting` and
   * advances the countdown. Repeated calls climb the schedule until the
   * cap (30s), then plateau there.
   */
  markFailure() {
    this.state = "reconnecting";
    const idx = Math.min(this.#stepIdx, RECONNECT_SCHEDULE.length - 1);
    this.reconnectInSec = RECONNECT_SCHEDULE[idx];
    this.#stepIdx = idx + 1;
  }

  /** Hard offline (e.g. user toggled the wifi off). Stops the countdown. */
  markOffline() {
    this.state = "offline";
    this.reconnectInSec = null;
  }
}

export const connection = new ConnectionStore();
