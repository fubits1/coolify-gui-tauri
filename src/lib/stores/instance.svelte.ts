import { load, type Store } from "@tauri-apps/plugin-store";

/**
 * Instance store — the user's connection target ({url, alias}).
 *
 * v1 is single-instance UI but the schema is multi-ready: persisted via
 * `tauri-plugin-store` (file `instance.json` in `app_data_dir`). The bearer
 * token is NOT held here — it lives in the OS keyring on the Rust side,
 * out of webview reach. Only the URL + a user-facing alias persist here.
 *
 * Usage from a component:
 *
 * ```ts
 * import { instance } from '$lib/stores/instance.svelte.ts';
 * await instance.load();
 * if (!instance.url) goto('/onboarding');
 * ```
 *
 * Idiom note: same class-singleton pattern as `connection.svelte.ts`.
 */

const STORE_FILE = "instance.json";
const KEY_URL = "url";
const KEY_ALIAS = "alias";

class InstanceStore {
  url: string | null = $state(null);
  alias: string | null = $state(null);

  #store: Store | null = null;

  /** Lazy-init the plugin-store handle. */
  async #getStore(): Promise<Store> {
    if (!this.#store) {
      this.#store = await load(STORE_FILE, { autoSave: true, defaults: {} });
    }
    return this.#store;
  }

  /** Read persisted url + alias into reactive state. Safe to call repeatedly. */
  async load(): Promise<void> {
    const store = await this.#getStore();
    this.url = (await store.get<string>(KEY_URL)) ?? null;
    this.alias = (await store.get<string>(KEY_ALIAS)) ?? null;
  }

  /**
   * Persist a new {url, alias}. Updates reactive state immediately so the
   * UI doesn't lag behind disk I/O. The plugin auto-saves (debounced 100ms).
   */
  async save(url: string, alias: string): Promise<void> {
    this.url = url;
    this.alias = alias;
    const store = await this.#getStore();
    await store.set(KEY_URL, url);
    await store.set(KEY_ALIAS, alias);
  }
}

export const instance = new InstanceStore();
