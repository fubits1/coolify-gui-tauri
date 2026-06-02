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
  environment_uuid?: string;
  environment_name?: string;
  environment_id?: number;
  status: ResourceStatus;
  fqdn?: string;
  /** Primary image:tag (single-image resources). */
  image_ref?: string;
  /** All image:tag refs to watch for freshness (compose + single image). */
  image_refs: string[];
  /** Heartbeat — Coolify's `last_online_at`. Constantly refreshed for running
   *  containers, only useful for non-running rows ("died X ago"). */
  last_online_at?: string;
  /** True last-deploy timestamp from `/deployments/applications/{uuid}`.
   *  Only populated for Applications. */
  last_deployed_at?: string;
  build_pack?: string;
}

export interface EnvVar {
  key: string;
  value: string;
  is_secret: boolean;
  /** Preview-deploy scope. The same key can exist in both production and
   *  preview with different values. */
  is_preview: boolean;
  /** Build-time only (not present at runtime). */
  is_buildtime: boolean;
  /** Runtime container env (default). */
  is_runtime: boolean;
  /** Team-shared variable, not resource-specific. */
  is_shared: boolean;
}

export interface Healthcheck {
  path?: string;
  port?: number;
  interval?: number;
  retries?: number;
}

export interface ServiceContainer {
  uuid: string;
  name: string;
  image?: string;
  fqdn?: string;
}

export interface ResourceDetail extends Resource {
  git_repository?: string;
  git_branch?: string;
  git_commit_sha?: string;
  ports_exposes?: string;
  docker_compose_raw?: string;
  /** Build-pack-specific config — populated for nixpacks/railpack/dockerfile Applications. */
  install_command?: string;
  build_command?: string;
  start_command?: string;
  base_directory?: string;
  publish_directory?: string;
  dockerfile?: string;
  dockerfile_location?: string;
  dockerfile_target_build?: string;
  watch_paths?: string;
  pre_deployment_command?: string;
  pre_deployment_command_container?: string;
  post_deployment_command?: string;
  post_deployment_command_container?: string;
  custom_docker_run_options?: string;
  static_image?: string;
  env_vars: EnvVar[];
  healthcheck?: Healthcheck;
  server_name?: string;
  /** Empty for Application + Database; populated for compose Services. */
  service_containers: ServiceContainer[];
}

export interface TeamRef {
  id: number;
  name: string;
}

export interface TestConnectionResult {
  ok: boolean;
  version?: string;
  /** Every team `/teams` returned for this token. Onboarding renders a
   *  dropdown over this list. */
  teams: TeamRef[];
  /** `/teams/current` id, when Coolify returned one. Used to pre-select
   *  the dropdown — not required for save. */
  current_team_id?: number;
  error?: string;
}

export interface CurrentTeam {
  team_id: number;
  team_name: string;
}
