import { load, type Store } from "@tauri-apps/plugin-store";
import { api } from "$lib/api/client";
import { toast } from "$lib/util/toast.svelte";
import { connectionRegistry } from "./connection.svelte";
import { resourcesRegistry } from "./resources.svelte";

/**
 * Multi-instance store — persisted list of Coolify instances + active selection.
 *
 * Schema (file `instances.json` in `app_data_dir`):
 * - `list`: ordered array of `{ id, url, alias }` records
 * - `activeId`: which one's currently in front of the user
 *
 * Bearer tokens are NOT held here — they live in the OS keyring on the
 * Rust side, keyed by `instance.id`. Adding/removing an instance is the
 * only point of write-through to the keyring (`api.setCredentials` /
 * `api.clearCredentials`).
 *
 * Migration: on first launch of the multi-instance build, if `list` is
 * empty AND a legacy `instance.json` exists with a `url + alias`, we
 * promote it to instance #0 by reading the legacy keyring entry (via
 * `api.migrateLegacyToken`) and re-saving under the new scheme. Legacy
 * file + keyring entry are wiped after.
 */

export type Instance = {
  id: string;
  url: string;
  alias: string;
  /** Coolify team this PAT is scoped to (`GET /teams/current`). Populated
   *  on add + backfilled on load for legacy entries that pre-date the
   *  per-team tab refactor. A null `teamId` means "not yet backfilled" —
   *  the tab still works, but dedupe won't catch a second token for the
   *  same team until backfill completes. */
  teamId: number | null;
  teamName: string | null;
  /** True once `api.loadCredentials` has resolved successfully for this
   *  instance (token rehydrated from keyring). Tracked here so consumers
   *  don't have to maintain a parallel `credentialsReady` map and we
   *  don't have to set $state from inside a $effect. */
  ready: boolean;
};

const STORE_FILE = "instances.json";
const LEGACY_FILE = "instance.json";
const KEY_LIST = "list";
const KEY_ACTIVE = "activeId";
const KEY_DEFAULT = "defaultId";

class InstancesStore {
  list: Instance[] = $state([]);
  activeId: string | null = $state(null);
  /** Persisted "open this tab on launch" pointer. Null = use first in
   *  list. Surfaced in Settings as a radio per row. */
  defaultId: string | null = $state(null);

  active: Instance | null = $derived(
    this.activeId == null
      ? null
      : (this.list.find((i) => i.id === this.activeId) ?? null),
  );

  #store: Store | null = null;
  #loadPromise: Promise<void> | null = null;

  async #getStore(): Promise<Store> {
    if (!this.#store) {
      this.#store = await load(STORE_FILE, { autoSave: true, defaults: {} });
    }
    return this.#store;
  }

  /**
   * Hydrate from disk + rehydrate every instance's keyring token. On
   * first call only, runs the legacy single-tenant migration if
   * applicable. Idempotent on subsequent calls.
   *
   * Credentials hydration happens here (not in a $effect) so we don't
   * have to set $state from inside a reactive effect — banned by
   * `code-style-svelte`. Each instance's `ready` field reflects the
   * keyring lookup outcome and is recomputed every load.
   */
  async load(): Promise<void> {
    // Memoise the in-flight load. Mutators (`add`, `remove`, `setActive`)
    // await this to avoid races where a user click lands between the
    // disk read and the list assignment — the cause of duplicated tabs
    // when the first connection was made.
    if (this.#loadPromise) return this.#loadPromise;
    this.#loadPromise = this.#doLoad();
    return this.#loadPromise;
  }

  async #doLoad(): Promise<void> {
    const store = await this.#getStore();
    // `teamId` / `teamName` are optional in the on-disk shape because old
    // installs predate the per-team refactor. Backfill runs below.
    type StoredInstance = Omit<Instance, "ready"> & {
      teamId?: number | null;
      teamName?: string | null;
    };
    const stored = (await store.get<StoredInstance[]>(KEY_LIST)) ?? [];
    const activeId = (await store.get<string>(KEY_ACTIVE)) ?? null;
    const defaultId = (await store.get<string>(KEY_DEFAULT)) ?? null;

    const hydrated: Instance[] = await Promise.all(
      stored.map(async (record) => {
        connectionRegistry.ensure(record.id);
        resourcesRegistry.ensure(record.id);
        let ready = false;
        try {
          ready = await api.loadCredentials(record.id, record.url);
        } catch (error) {
          const message =
            error instanceof Error ? error.message : String(error);
          console.warn(
            `[instances] loadCredentials failed for ${record.alias}: ${message}`,
          );
        }
        return {
          id: record.id,
          url: record.url,
          alias: record.alias,
          teamId: record.teamId ?? null,
          teamName: record.teamName ?? null,
          ready,
        };
      }),
    );

    this.list = hydrated;
    this.defaultId =
      defaultId && hydrated.find((i) => i.id === defaultId) ? defaultId : null;
    const preferred = activeId ?? this.defaultId;
    this.activeId =
      preferred && hydrated.find((instance) => instance.id === preferred)
        ? preferred
        : (hydrated[0]?.id ?? null);
    if (this.list.length === 0) {
      await this.#migrateLegacy();
    }
    // Fire-and-forget: any ready instance missing team metadata gets
    // backfilled from `/teams/current`. We don't block load — the tab is
    // usable with a placeholder label; the strip rerenders when each one
    // resolves.
    void this.#backfillTeams();
  }

  /**
   * For every `ready` instance lacking `teamId`, call `GET /teams/current`
   * to learn the team this PAT is bound to and persist. Idempotent —
   * skipped per-instance once both fields are set.
   */
  async #backfillTeams(): Promise<void> {
    const todo = this.list.filter(
      (i) => i.ready && (i.teamId == null || i.teamName == null),
    );
    if (todo.length === 0) return;
    let mutated = false;
    for (const inst of todo) {
      try {
        const team = await api.getCurrentTeam(inst.id);
        const idx = this.list.findIndex((i) => i.id === inst.id);
        if (idx === -1) continue;
        const next = [...this.list];
        next[idx] = {
          ...next[idx],
          teamId: team.team_id,
          teamName: team.team_name,
        };
        this.list = next;
        mutated = true;
      } catch (err) {
        console.warn(
          `[instances] team backfill failed for ${inst.alias}:`,
          err,
        );
      }
    }
    if (mutated) await this.#persist();
  }

  /**
   * Promote a legacy single-instance install to instance #0. Reads
   * legacy `instance.json` for `url + alias`, then calls Rust to migrate
   * the keyring token. No-op when no legacy file exists.
   */
  async #migrateLegacy(): Promise<void> {
    let legacy: Store | null = null;
    try {
      legacy = await load(LEGACY_FILE, { autoSave: false, defaults: {} });
    } catch (err) {
      console.warn("[migrate] legacy store load failed:", err);
      return;
    }
    const url = (await legacy.get<string>("url")) ?? null;
    const alias = (await legacy.get<string>("alias")) ?? null;
    console.info("[migrate] legacy file:", { url, alias });
    if (!url || !alias) {
      console.info("[migrate] no legacy url/alias — fresh install");
      return;
    }
    let token: string | null = null;
    try {
      token = await api.migrateLegacyToken(alias);
    } catch (err) {
      console.warn("[migrate] migrateLegacyToken threw:", err);
      toast.error(
        "Could not migrate previous instance",
        err instanceof Error ? err.message : String(err),
      );
      return;
    }
    if (!token) {
      console.warn(
        `[migrate] keyring had no entry for alias "${alias}" — user must re-enter token`,
      );
      toast.warning(
        "Could not find your previous token in the OS keyring",
        `Please re-enter the API token for ${url} (alias "${alias}").`,
      );
      return;
    }
    const id = crypto.randomUUID();
    await api.setCredentials(id, url, token);
    connectionRegistry.ensure(id);
    resourcesRegistry.ensure(id);
    // Team metadata stays null here — `#backfillTeams` (kicked off at the
    // end of `#doLoad`) fills it in on next load. We could fetch
    // synchronously, but the legacy migration runs once and a missing
    // team label for a single load cycle isn't worth blocking on.
    const instance: Instance = {
      id,
      url,
      alias,
      teamId: null,
      teamName: null,
      ready: true,
    };
    this.list = [instance];
    this.activeId = id;
    await this.#persist();
    console.info(`[migrate] success: alias=${alias} → instance ${id}`);
    toast.success(`Migrated "${alias}" to multi-instance store`);
    await legacy.delete("url");
    await legacy.delete("alias");
    await legacy.save();
  }

  /**
   * Add a NEW instance. Caller has already validated `url + token` via
   * `api.testConnection` and passes the resolved `team_id` + `team_name`
   * from that probe. We dedupe on `(url, teamId)` — two tabs for the
   * same team are an accident, not a feature.
   */
  async add(
    url: string,
    token: string,
    alias: string,
    teamId: number,
    teamName: string,
  ): Promise<Instance> {
    await this.load();
    const normalizedUrl = url.replace(/\/+$/, "");
    const duplicate = this.list.find(
      (i) => i.url.replace(/\/+$/, "") === normalizedUrl && i.teamId === teamId,
    );
    if (duplicate) {
      throw new Error(
        `Already connected to "${teamName}" on ${normalizedUrl} (tab "${duplicate.alias}"). Remove the existing tab first to replace its token.`,
      );
    }
    const id = crypto.randomUUID();
    await api.setCredentials(id, url, token);
    connectionRegistry.ensure(id);
    resourcesRegistry.ensure(id);
    const instance: Instance = {
      id,
      url,
      alias,
      teamId,
      teamName,
      ready: true,
    };
    this.list = [...this.list, instance];
    this.activeId = id;
    await this.#persist();
    return instance;
  }

  /**
   * Remove an instance: clear its keyring entry + per-instance Rust
   * caches via `api.clearCredentials`, drop from the list, and elect a
   * new active if needed.
   */
  async remove(id: string): Promise<void> {
    await this.load();
    await api.clearCredentials(id).catch(() => {});
    const next = this.list.filter((i) => i.id !== id);
    this.list = next;
    if (this.activeId === id) {
      this.activeId = next[0]?.id ?? null;
    }
    if (this.defaultId === id) {
      this.defaultId = null;
    }
    await this.#persist();
  }

  /**
   * Persist the "open this tab on launch" pointer. Pass `null` to clear
   * (boot will fall back to first-in-list).
   */
  async setDefault(id: string | null): Promise<void> {
    await this.load();
    if (id != null && !this.list.find((i) => i.id === id)) return;
    this.defaultId = id;
    await this.#persist();
  }

  /**
   * Switch active instance. Callers (resources/connection registries)
   * watch this via $derived `active` and start/stop their per-instance
   * loops accordingly.
   */
  async setActive(id: string): Promise<void> {
    await this.load();
    if (!this.list.find((instance) => instance.id === id)) return;
    this.activeId = id;
    await this.#persist();
  }

  async #persist(): Promise<void> {
    const store = await this.#getStore();
    // Persist only the disk-side fields; `ready` is runtime-only (it
    // reflects the keyring state and is recomputed on every load).
    const stored = this.list.map(({ id, url, alias, teamId, teamName }) => ({
      id,
      url,
      alias,
      teamId,
      teamName,
    }));
    await store.set(KEY_LIST, stored);
    await store.set(KEY_ACTIVE, this.activeId);
    await store.set(KEY_DEFAULT, this.defaultId);
    await store.save();
  }
}

export const instances = new InstancesStore();
