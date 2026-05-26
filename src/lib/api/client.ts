import { invoke } from "@tauri-apps/api/core";
import type {
  EnvVar,
  Resource,
  ResourceDetail,
  TestConnectionResult,
} from "./types";

export type ListResourcesResult = {
  resources: Resource[];
  errors: Record<string, string>;
};

/**
 * Thin typed wrappers around Tauri `invoke()`. Every Coolify HTTP call
 * lives Rust-side; the webview never sees the bearer token.
 *
 * Every per-instance call carries an `instanceId` that the Rust side
 * uses to route to the right CoolifyClient (and the right per-instance
 * cache). `testConnection` + `migrateLegacyToken` are the only stateless
 * exceptions.
 */
export const api = {
  testConnection: (url: string, token: string) =>
    invoke<TestConnectionResult>("test_connection", { url, token }),
  setCredentials: (instanceId: string, url: string, token: string) =>
    invoke<void>("set_credentials", { instanceId, url, token }),
  loadCredentials: (instanceId: string, url: string) =>
    invoke<boolean>("load_credentials", { instanceId, url }),
  clearCredentials: (instanceId: string) =>
    invoke<void>("clear_credentials", { instanceId }),
  listResources: (instanceId: string) =>
    invoke<ListResourcesResult>("list_resources", { instanceId }),
  getResourceDetail: (instanceId: string, uuid: string, kind: string) =>
    invoke<ResourceDetail>("get_resource_detail", { instanceId, uuid, kind }),
  getResourceEnvs: (instanceId: string, uuid: string, kind: string) =>
    invoke<EnvVar[]>("get_resource_envs", { instanceId, uuid, kind }),
  restart: (instanceId: string, uuid: string, kind: string) =>
    invoke<void>("restart_resource", { instanceId, uuid, kind }),
  stop: (instanceId: string, uuid: string, kind: string) =>
    invoke<void>("stop_resource", { instanceId, uuid, kind }),
  deploy: (instanceId: string, uuid: string, force: boolean) =>
    invoke<void>("deploy_resource", { instanceId, uuid, force }),
  tailLogs: (
    instanceId: string,
    uuid: string,
    kind: string,
    lines = 500,
    container?: string,
  ) =>
    invoke<string>("tail_logs", { instanceId, uuid, kind, lines, container }),
  debugDumpEndpoints: (instanceId: string) =>
    invoke<Record<string, string>>("debug_dump_endpoints", { instanceId }),
  /**
   * One-shot migration helper. Reads the legacy single-tenant keyring
   * entry (keyed by `alias`, default `"default"`) and removes it. The
   * caller is responsible for invoking `setCredentials` with the new
   * `instanceId` to re-save the token under the multi-instance scheme.
   * Returns `null` if no legacy entry exists.
   */
  migrateLegacyToken: (alias: string) =>
    invoke<string | null>("migrate_legacy_token_cmd", { alias }),
};
