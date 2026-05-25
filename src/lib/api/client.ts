import { invoke } from "@tauri-apps/api/core";
import type { Resource, ResourceDetail, TestConnectionResult } from "./types";

// Thin typed wrappers around Tauri `invoke()`. Every Coolify HTTP call lives
// Rust-side; the webview never sees the bearer token.
export const api = {
  testConnection: (url: string, token: string) =>
    invoke<TestConnectionResult>("test_connection", { url, token }),
  setCredentials: (url: string, token: string) =>
    invoke<void>("set_credentials", { url, token }),
  listResources: () => invoke<Resource[]>("list_resources"),
  getResourceDetail: (uuid: string, kind: string) =>
    invoke<ResourceDetail>("get_resource_detail", { uuid, kind }),
  restart: (uuid: string, kind: string) =>
    invoke<void>("restart_resource", { uuid, kind }),
  stop: (uuid: string, kind: string) =>
    invoke<void>("stop_resource", { uuid, kind }),
  deploy: (uuid: string, force: boolean) =>
    invoke<void>("deploy_resource", { uuid, force }),
  tailLogs: (uuid: string, kind: string, lines = 500) =>
    invoke<string>("tail_logs", { uuid, kind, lines }),
};
