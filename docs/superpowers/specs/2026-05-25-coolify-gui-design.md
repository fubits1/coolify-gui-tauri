# Coolify Desktop GUI — Design Spec

**Date:** 2026-05-25
**Status:** Draft — awaiting user review

## Context

Self-hosted Coolify users currently operate their instance through a browser dashboard. Day-to-day ops (status checks, restarts, redeploys, image freshness) require multiple clicks and tab juggling. A native cross-OS desktop app that lives in the dock/tray gives:

- Sortable/groupable single-pane overview of every resource.
- One-click Restart and Deploy (with `force_rebuild` opt-in).
- Drill-down detail (env, compose, logs, image freshness).
- Daily image-freshness check across all docker / docker-compose resources.

Companion documents:

- `PRD.md` — product requirements (origin doc)
- `CONTEXT.md` — domain glossary (Resource / Project / Instance / Status / Digest …)

This spec supersedes the PRD where they differ.

## Goals (v1, full scope per user)

1. Onboarding: paste Coolify URL + Bearer token, "Test connection" against `/health` + `/teams`, store token in OS keyring.
2. Overview screen: all Resources across Projects/Environments, two togglable view modes (table + cards), sortable A–Z + by last-deployment, groupable by Project.
3. Live-ish status per Resource via 5s polling while window focused.
4. Per-row actions: **Restart** (no confirm), **Deploy** (confirm dialog with `force_rebuild` checkbox).
5. Split-view detail pane (left list, right tabs): Overview / Env / Compose / Logs / Images.
6. Logs tab: poll last 500 lines, manual refresh button.
7. Image freshness check: scheduled daily at app start + manual override (per-row + "Check all"), 24h cache, badges on overview row and Images tab.
8. Connection status strip top-of-window + `svelte-sonner` toasts for action results.
9. Dark theme only.
10. Settings: single-instance UI (edit URL, rotate token) backed by multi-ready schema; registry tokens (Docker Hub PAT, GHCR PAT) in keyring; polling pause toggle.

## Non-Goals (v1)

- Multi-instance switcher UI (schema ready, deferred).
- Creating/deleting/editing Resources, env vars, or compose.
- Realtime websocket subscriptions (Coolify's Pusher channels are undocumented for 3rd parties — v2 spike).
- Auto-updater.
- Server provisioning, GitHub-app setup, cloud-token mgmt.

## Locked Design Decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | Instance schema multi-ready, v1 UI single | Avoid retrofit later; cheap insurance |
| 2 | Full PRD scope in v1 (broken into dex tasks) | User preference |
| 3 | Split master-detail layout (list left, detail right) | Keyboard-driven ops scan |
| 4 | 5s polling while focused, pause when blurred | No documented Coolify rate limit; safe |
| 5 | Image freshness: daily on launch + manual, 24h cache | Docker Hub anon limit 100/6h, manageable |
| 6 | Logs: poll last 500 lines, manual refresh | Simple, low risk |
| 7 | Confirm only on Deploy (with `force_rebuild` checkbox); Restart no confirm | Deploy mutates, Restart idempotent |
| 8 | Overview: two view modes (dense table + cards), togglable | User wants both |
| 9 | Dark theme only | shadcn-svelte dark default; ops convention |
| 10 | Connection-status strip + `svelte-sonner` toasts | Persistent state + per-action feedback |
| 11 | Actions: **Restart** → `/restart`; **Deploy** → `/deploy?uuid=&force=` | Verified against Coolify OpenAPI |
| 12 | Token storage: `tauri-plugin-keyring` (OS-native) | Encrypted at rest, cross-OS |
| 13 | HTTP from Rust side via `tauri-plugin-http` (reqwest) | Bypass CORS, hide token from webview |
| 14 | Registry digest fetch via `oci-distribution` crate | Maintained, OCI-compliant, supports Docker Hub / GHCR / quay |

## Architecture

```
src/                       Svelte 5 + TS + Vite
  lib/
    api/
      client.ts            invoke wrappers around Rust commands
      types.ts             types generated from Coolify OpenAPI
    components/
      ui/                  shadcn-svelte primitives
      overview/
        TableView.svelte
        CardsView.svelte
        ViewToggle.svelte
        StatusBadge.svelte
        ImageBadge.svelte
      detail/
        DetailPane.svelte
        tabs/
          OverviewTab.svelte
          EnvTab.svelte
          ComposeTab.svelte
          LogsTab.svelte
          ImagesTab.svelte
      shell/
        AppShell.svelte
        ConnectionStrip.svelte
        Sidebar.svelte
      onboarding/
        ConnectScreen.svelte
      settings/
        SettingsPage.svelte
    stores/
      instance.svelte.ts    $state — current Instance {url, alias}
      resources.svelte.ts   $state — list + poll loop + selection
      connection.svelte.ts  $state — online/reconnecting/last-ping
      image-cache.svelte.ts $state — {imageRef → {digest, checkedAt}}
    util/
      semver.ts             tag comparison
      compose.ts            parse docker-compose YAML for image refs
  routes/                  svelte-spa-router (overview is /, settings is /settings)
  App.svelte
  app.css                  shadcn theme tokens

src-tauri/
  src/
    lib.rs                 #[tauri::command] registrations
    coolify/
      mod.rs
      client.rs            reqwest client, bearer injection, retry
      types.rs             Rust mirror of Coolify schemas (serde)
      ops.rs               list_resources / restart / deploy / logs
    registry/
      mod.rs
      digest.rs            oci-distribution wrappers
      tags.rs              tag listing + semver pick
      cache.rs             read/write tauri-plugin-store cache file
    secrets.rs             keyring helpers (coolify_token, registry_*_token)
    settings.rs            instance config via tauri-plugin-store
  capabilities/
    default.json           plugin allowlist
  tauri.conf.json
```

### Data flow

```
[Svelte $state store] ──invoke('list_resources')──▶ [Rust ops::list_resources]
                                                         │
                                                         ▼
                                            reqwest → {coolify_url}/api/v1/applications
                                            reqwest → /services, /databases (parallel)
                                                         │
                                                         ▼
                                              merge → Vec<Resource> → JSON
                                                         │
                                                         ▼
                                              Svelte store updates ──▶ table/cards rerender
```

### Polling loop

`stores/resources.svelte.ts` owns one polling loop. Cadence 5s. Pauses on `window.blur`, resumes on `window.focus`. Manual `refresh()` always available. Failures don't unmount the loop — they bump `connection` store to "reconnecting" and retry.

### Image freshness

On app launch:

1. Read cache (`tauri-plugin-store` file `image-digests.json`).
2. If any entry's `checkedAt > 24h ago` OR missing, queue background `check_image_digest(image_ref)` per-image.
3. Concurrency cap: 4 in flight (avoid Docker Hub anon throttle).
4. Per-image: `oci-distribution::pull_manifest()` for current tag digest → compare against last stored digest. If unchanged but checkedAt stale, also fetch `:latest` digest and highest semver tag from `/v2/<name>/tags/list` for "newer available" signal.
5. Result written to cache + emitted via Tauri event → frontend store updates badges.

Manual "Check all" / per-row "Check now" bypass the 24h gate.

### Auth flow

1. Onboarding screen: URL field + token field + "Test" button.
2. Test calls Rust command `test_connection(url, token)` → `GET {url}/api/v1/health` (no auth) + `GET /teams` (with token).
3. Success → keyring `set("coolify_token_default", token)` + store URL + alias in `tauri-plugin-store`.
4. App boot: read URL from store, read token from keyring, init client.

Token never enters the Svelte side. Webview only sees it during onboarding paste, then discards.

## UI Specifications

### Connection strip (top, 24px)

`● Connected to acme.coolify.dev` (green) / `● Reconnecting in 4s` (amber, countdown) / `● Offline — check VPN` (red).

### Overview screen

**Header bar**: Search input · Group dropdown (None / Project / Environment / Status) · Sort dropdown (Name A–Z / Last deploy ⇣ / Status) · View toggle (Table / Cards) · "Check all images" button (badge with stale count).

**Table view**: columns — Name · Type · Project · Env · Status · FQDN · Last deploy · Images · Actions (↻ ⬇). Click row → select + open right pane.

**Cards view**: grid 2-up (3-up on wide). Each card: name + status badge, type · project, FQDN/image-ref, image badge + last-deploy, inline action buttons.

### Detail pane (right, when row selected)

Top: name + combined-status badge + Restart + Deploy buttons + project/env breadcrumb + FQDN link.
Tab bar: Overview · Env (count) · Compose · Logs · Images (stale-count badge).
Bottom strip: keyboard hints (⌘R restart, ⌘D deploy, ⌘I check images, ⌘L logs) + last-refresh.

**Database resources**: Compose tab hidden; Logs tab maps to db engine logs (same endpoint).
**Application non-compose**: Compose tab → "Build config" (Dockerfile / nixpacks summary).

### Settings page

Sections:

- **Instance**: URL (editable) + token (re-enter to rotate) + "Test" + alias.
- **Registries**: rows for Docker Hub, GHCR, etc. — each: "Set token" → keyring.
- **Polling**: pause toggle, cadence read-only display.
- **About**: version, links.

## Error Handling

- HTTP 401 → connection strip red + onboarding prompt to re-paste token. Don't wipe keyring automatically.
- HTTP 5xx / network error → connection strip "Reconnecting", exponential backoff (1s, 2s, 4s, 8s, max 30s).
- Per-action failure → toast (error variant) with raw Coolify error message + "Retry" button.
- Registry rate-limit (Docker Hub 429) → toast warning, mark image as "rate-limited, retry later", skip remaining checks for that registry.

## Testing Strategy

- **Rust unit**: `coolify::ops` against a mock reqwest layer (wiremock crate). `registry::digest` against a mock OCI registry.
- **Frontend unit**: store reducers (sorting, grouping, polling pause/resume) via vitest.
- **E2E smoke**: Playwright drives the built Tauri app against a real Coolify instance (env: `COOLIFY_URL`, `COOLIFY_TOKEN`). One test: onboard → see ≥1 resource → restart a sandbox app → confirm status returns to running within 30s.

## Build & Bundling

- macOS: `.dmg`, codesigned if `APPLE_CERTIFICATE` env present.
- Windows: `.msi`, unsigned v1.
- Linux: `.AppImage` + `.deb`.

GitHub Actions: build all three on tag push. Local dev: `pnpm tauri dev`.

## Implementation Sequence

Translates 1:1 into dex tasks. Each item is one ticket.

1. Scaffold: `pnpm create tauri-app` → Svelte + TS, add `pnpm` lockfile, commit.
2. Add Tauri plugins: `http`, `store`, `keyring`. Wire capabilities.
3. Add shadcn-svelte, init theme, dark mode lock.
4. Add `svelte-spa-router`, two routes: `/` and `/settings`.
5. Onboarding screen + Rust `test_connection` + token storage.
6. Generate TS types from Coolify OpenAPI (`openapi-typescript`).
7. Rust `coolify::client` (reqwest + bearer injection + base URL).
8. Rust `coolify::ops::list_resources` (apps + services + dbs in parallel, merge).
9. `stores/resources.svelte.ts` + polling loop + focus pause.
10. Overview table view (sortable, groupable, search).
11. Overview cards view + view toggle.
12. Status badge component + image badge component.
13. Connection strip + `connection.svelte.ts`.
14. `svelte-sonner` integration + action toasts.
15. Restart button → `/restart` → optimistic + toast.
16. Deploy confirm dialog + force_rebuild checkbox → `/deploy?uuid=&force=`.
17. Split-view layout + selection state.
18. DetailTab: Overview content.
19. DetailTab: Env (masked + reveal on click).
20. DetailTab: Compose (read-only highlighted YAML).
21. DetailTab: Logs (poll last 500 lines + refresh).
22. Rust `registry::digest` + `oci-distribution` wiring + cache file.
23. Rust `registry::tags` (list tags, semver-pick latest).
24. DetailTab: Images + per-image freshness rows.
25. Header "Check all images" + per-row "Check now".
26. Daily-at-startup auto-check scheduler.
27. Settings page (instance + polling + registries).
28. Keyboard shortcuts (⌘R / ⌘D / ⌘I / ⌘L).
29. Tauri build config for mac/win/linux + GitHub Actions release workflow.
30. README + screenshots.

## Verification

End-to-end on a real Coolify instance (user's sandbox):

1. `pnpm tauri dev` opens window, onboarding renders.
2. Paste real URL + token → "Connected" green within 2s.
3. Overview lists ≥1 resource; sort + group + search work.
4. Click Restart on a sandbox app → Coolify dashboard shows restart event within 5s.
5. Click row → detail pane opens. Cycle every tab; no console errors.
6. Click "Check all images" → at least one resource shows a freshness badge within 30s.
7. Kill network (turn off wifi) → connection strip flips red within 6s; restore → green within 5s.
8. `pnpm tauri build` produces signed `.dmg` (mac) without errors.

## Open Items (to confirm during implementation, not blockers)

- Coolify `/teams` shape — confirm we can get current team name for the connection strip.
- Logs endpoint exact path + response shape — verify against running instance.
- `force_rebuild` query param name — verified as `force` per Coolify OpenAPI; double-check on first integration.
- Compose YAML for `Service` resources: confirm field is `docker_compose_raw` (parsed available too).
