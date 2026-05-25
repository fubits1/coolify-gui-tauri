// Hand-written domain types mirroring the Rust side (see src-tauri/.../coolify/types.rs).
// The Rust layer normalises Coolify's OpenAPI shapes, so we do NOT re-export the
// generated OpenAPI types directly. Generated types live in ./coolify.openapi.ts
// for reference and lower-level use only.

export type ResourceKind = "Application" | "Service" | "Database";

export interface ResourceStatus {
  state: string;
  health?: string;
  raw: string;
}

export interface Resource {
  uuid: string;
  name: string;
  kind: ResourceKind;
  project_uuid?: string;
  project_name?: string;
  environment_name?: string;
  status: ResourceStatus;
  fqdn?: string;
  image_ref?: string;
  last_deployed_at?: string;
  build_pack?: string;
}

export interface EnvVar {
  key: string;
  value: string;
  is_secret: boolean;
}

export interface Healthcheck {
  path?: string;
  port?: number;
  interval?: number;
  retries?: number;
}

export interface ResourceDetail extends Resource {
  git_repository?: string;
  git_branch?: string;
  git_commit_sha?: string;
  ports_exposes?: string;
  docker_compose_raw?: string;
  env_vars: EnvVar[];
  healthcheck?: Healthcheck;
  server_name?: string;
}

export interface TestConnectionResult {
  ok: boolean;
  version?: string;
  team_name?: string;
  error?: string;
}
