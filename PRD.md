# PRD — Coolify Desktop GUI

## Context

Cross-OS desktop client for self-hosted Coolify instances. Replace browser-tab juggling with native app: project overview, status at a glance, one-click restart/redeploy, drill-down config, and image-freshness check for docker/compose workloads.

**Stack**: Tauri 2 + Svelte 5 (runes) + TypeScript + Vite + **shadcn-svelte** UI + Rust backend (HTTP via `tauri-plugin-http`, secrets via `tauri-plugin-keyring`, settings via `tauri-plugin-store`).

**Target users**: self-hosters running Coolify who want a native ops panel.

## Goals (v1)

1. Connect to user-supplied Coolify URL with Bearer token.
2. Sortable overview of all resources (applications + services + databases) across projects/environments.
3. Live-ish status per resource (`running` / `exited` / `degraded` / `starting`).
4. One-click **Restart** and **Redeploy (pull latest)**.
5. Detail view per resource: FQDN/URL, env vars, build pack, git ref, ports, health-check config, raw docker-compose (if present), logs (last N lines).
6. **Outdated image check** for docker / docker-compose resources: compare image digests against remote registries (Docker Hub, GHCR, quay.io).

## Non-Goals (v1)

- Creating/deleting resources.
- Editing env vars (read-only first).
- Server provisioning, GitHub-app setup, cloud-token mgmt.
- Real-time log streaming (poll last N lines is enough).
- Multi-account / multi-instance switching (single instance v1).
- Auto-updater (defer).

## Coolify API Reference (key endpoints)

Base: `{user_url}/api/v1` — header `Authorization: Bearer {token}`.

OpenAPI spec: `https://raw.githubusercontent.com/coollabsio/coolify/main/openapi.json` — generate TS types from this.

| Purpose | Endpoint |
|---|---|
| List apps | `GET /applications` |
| List services | `GET /services` |
| List databases | `GET /databases` |
| List projects | `GET /projects` |
| App detail | `GET /applications/{uuid}` |
| Service detail | `GET /services/{uuid}` |
| Restart app | `POST /applications/{uuid}/restart` |
| Restart service | `POST /services/{uuid}/restart` |
| Redeploy app | `POST /applications/{uuid}/deploy` or `/redeploy` |
| Logs | `GET /applications/{uuid}/logs` |
| Health probe | `GET /health` (no auth) |

**Status field**: combined string `running:healthy`, `exited:unhealthy`, `degraded`, `starting`, `excluded`. Parse `:` split.

**Resource detail fields used**: `uuid`, `name`, `fqdn`, `build_pack`, `git_repository`, `git_branch`, `git_commit_sha`, `docker_compose_raw`, `docker_registry_image_name`, `docker_registry_image_tag`, `ports_exposes`, env vars, `config_hash`.

## Architecture

```
src/                  Svelte 5 frontend (runes)
  lib/
    api/              typed Coolify client (calls Rust commands)
    components/       shadcn-svelte primitives + app components
    stores/           $state-based reactive stores
    types/            OpenAPI-generated TS types
  routes/             SvelteKit filesystem routes
    +page.svelte              (overview)
    resource/[uuid]/+page.svelte
    settings/+page.svelte

src-tauri/
  src/
    lib.rs            tauri commands entrypoint
    coolify.rs        HTTP client (reqwest via plugin-http), bearer injection
    registry.rs       oci-distribution: digest fetch + compare
    secrets.rs        keyring wrappers
  capabilities/       allowlist for http://*/api/v1 patterns
  tauri.conf.json
```

**Why Rust-side HTTP**: avoids CORS, hides token from webview, lets us add retry/timeout/cache logic centrally.

**Why shadcn-svelte**: matches user preference, Svelte 5 native, includes Table/Dialog/Button/Card/Input — covers entire UI need without extra deps.

## Outdated Image Check

For each `docker_compose_raw` or `docker_registry_image_name:tag`:

1. Parse compose YAML → list of `image:tag` refs (Rust `serde_yaml`).
2. For each ref: fetch `Docker-Content-Digest` via OCI v2 manifest API using `oci-distribution` crate.
3. Compare against `:latest` tag digest **and** highest semver tag digest (`/v2/<name>/tags/list`).
4. Cache `{image:tag → digest, checked_at}` in `tauri-plugin-store` (`image-digests.json`) for offline diffing and rate-limit friendliness.
5. UI: badge per resource — green (current), yellow (newer semver), red (latest moved). Detail view lists per-image status.

**Rate limit**: Docker Hub anon = 100 pulls / 6h. Cache aggressively; show last-checked timestamp; manual "Check now" button (not auto-poll all).

**Auth**: anonymous by default. Settings allow per-registry tokens (GHCR PAT, Docker Hub PAT) stored via keyring.

## Data Flow

```
[Svelte component] --invoke('list_resources')--> [Rust command]
                                                       |
                                                       v
                                            reqwest -> Coolify /api/v1
                                                       |
                                                       v
                                                 typed JSON -> Svelte $state
```

Polling: overview page refreshes every 15s while focused; manual refresh button always available.

## UI Pages

1. **Onboarding** — Coolify URL + token input, "Test connection" calls `/health` + `/teams`. Save via keyring.
2. **Overview** — sortable table of resources. Columns: name, project/env, type (app/service/db), status badge, FQDN, last-deploy, image-freshness badge, actions (restart / redeploy / open).
3. **Detail** — tabs: Overview / Env / Compose / Logs / Images.
4. **Settings** — Coolify URL, token rotation, registry tokens, polling interval, theme.

## Verification Plan

- `pnpm tauri dev` boots, app window opens on macOS first (primary dev OS).
- Onboarding: paste valid token → green "Connected" + team name.
- Overview lists ≥1 resource from a real Coolify instance; sort by status/name works.
- Click Restart on a sandbox app → Coolify dashboard confirms restart event within 5s.
- Detail page renders FQDN, env vars (masked), compose YAML.
- Image-check on a known-stale image (e.g. `nginx:1.20`) shows yellow badge.
- Build artifacts: `pnpm tauri build` produces `.dmg` (mac), `.msi` (win), `.AppImage` (linux). Test mac first.

## Open Questions

1. Token scope — does Coolify let us create a read+deploy token, or is it all-or-nothing?
2. Logs endpoint — streaming or polling? Confirm against actual instance.
3. Compose-only services: do we get `docker_compose_raw` for **all** service types or only the "generic" ones?

## Build Sequence (issues → dex tasks)

1. Scaffold Tauri 2 + Svelte 5 + TS + shadcn-svelte.
2. Add plugins: http, keyring, store.
3. Onboarding screen + token storage.
4. Rust `coolify.rs` HTTP wrapper + typed commands.
5. Generate TS types from OpenAPI.
6. Overview page with sortable table (shadcn DataTable).
7. Status polling + manual refresh.
8. Restart + Redeploy buttons (with confirm dialog).
9. Detail page (tabs).
10. Logs tab (poll last 500 lines).
11. `registry.rs` — oci-distribution digest fetch + cache.
12. Image-freshness badges in overview + detail.
13. Settings page (URL rotation, registry tokens).
14. Bundle config — mac/win/linux targets.
15. README + screenshots.
