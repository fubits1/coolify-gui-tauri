import { invoke } from "@tauri-apps/api/core";
import type {
  EnvVar,
  Resource,
  ResourceDetail,
  TestConnectionResult,
} from "./types";

export interface ListResourcesResult {
  resources: Resource[];
  errors: Record<string, string>;
}

// Thin typed wrappers around Tauri `invoke()`. Every Coolify HTTP call lives
// Rust-side; the webview never sees the bearer token.
export const api = {
  testConnection: (url: string, token: string) =>
    invoke<TestConnectionResult>("test_connection", { url, token }),
  setCredentials: (url: string, token: string, alias?: string) =>
    invoke<void>("set_credentials", { url, token, alias }),
  loadCredentials: (url: string, alias?: string) =>
    invoke<boolean>("load_credentials", { url, alias }),
  clearCredentials: (alias?: string) =>
    invoke<void>("clear_credentials", { alias }),
  listResources: () => invoke<ListResourcesResult>("list_resources"),
  getResourceDetail: (uuid: string, kind: string) =>
    invoke<ResourceDetail>("get_resource_detail", { uuid, kind }),
  getResourceEnvs: (uuid: string, kind: string) =>
    invoke<EnvVar[]>("get_resource_envs", { uuid, kind }),
  restart: (uuid: string, kind: string) =>
    invoke<void>("restart_resource", { uuid, kind }),
  stop: (uuid: string, kind: string) =>
    invoke<void>("stop_resource", { uuid, kind }),
  deploy: (uuid: string, force: boolean) =>
    invoke<void>("deploy_resource", { uuid, force }),
  tailLogs: (uuid: string, kind: string, lines = 500, container?: string) =>
    invoke<string>("tail_logs", { uuid, kind, lines, container }),
  debugDumpEndpoints: () =>
    invoke<Record<string, string>>("debug_dump_endpoints"),
};
