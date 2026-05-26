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

class InstancesStore {
  list: Instance[] = $state([]);
  activeId: string | null = $state(null);

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
    type StoredInstance = Omit<Instance, "ready">;
    const stored = (await store.get<StoredInstance[]>(KEY_LIST)) ?? [];
    const activeId = (await store.get<string>(KEY_ACTIVE)) ?? null;

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
        return { ...record, ready };
      }),
    );

    this.list = hydrated;
    this.activeId =
      activeId && hydrated.find((instance) => instance.id === activeId)
        ? activeId
        : (hydrated[0]?.id ?? null);
    if (this.list.length === 0) {
      await this.#migrateLegacy();
    }
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
    const instance: Instance = { id, url, alias, ready: true };
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
   * `api.testConnection`. We generate the id here, write the token to
   * keyring under the new scheme, persist the metadata, and switch
   * active to the new entry.
   */
  async add(url: string, token: string, alias: string): Promise<Instance> {
    await this.load();
    const id = crypto.randomUUID();
    await api.setCredentials(id, url, token);
    connectionRegistry.ensure(id);
    resourcesRegistry.ensure(id);
    const instance: Instance = { id, url, alias, ready: true };
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
    const stored = this.list.map(({ id, url, alias }) => ({ id, url, alias }));
    await store.set(KEY_LIST, stored);
    await store.set(KEY_ACTIVE, this.activeId);
    await store.save();
  }
}

export const instances = new InstancesStore();
