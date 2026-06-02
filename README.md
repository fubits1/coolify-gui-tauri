# Coolify GUI

Native cross-OS desktop client for self-hosted Coolify. Single-pane overview of every Resource across all your Projects and Environments, one-click Restart / Deploy, drill-down detail (env, compose, logs), and a daily image-freshness check across docker / docker-compose resources — without juggling browser tabs.

## Screenshots

Screenshots live under `docs/screenshots/` (placeholder — not yet populated).

## Download

Pre-built binaries are published on the [GitHub Releases](https://github.com/fubits/coolify-gui/releases) page (placeholder — point to your fork until upstream releases exist):

- macOS: `.dmg` (Apple Silicon + Intel)
- Windows: `.msi`
- Linux: `.AppImage`, `.deb`

## Quickstart

1. Install and launch.
2. Paste your **Coolify URL** (e.g. `https://acme.coolify.dev`) and a **Bearer token**.
3. Hit **Test connection**, pick the team from the dropdown, then **Save**.

### Required token scope

In the Coolify dashboard, create a Personal Access Token with:

- `read:sensitive` — list resources + read environment variable values.
- `deploy` — Restart / Stop / Deploy actions.

`write` and `root` are **not** required. The token is stored in your OS keyring (Keychain / Credential Manager / Secret Service); the webview never holds it.

### Multiple teams = multiple tokens

Coolify PATs are **team-scoped at creation**: each token's `team_id` column is stamped from your Coolify web session's currently-active team at the moment you click "Create New Token". Coolify's API (`/applications`, `/services`, `/databases`) hard-filters every list response by that team_id — there is no header, query param, or scope that lets one token span teams.

Consequences for this app:

- One tab per `(instance, team)` pair. Each tab needs its own PAT.
- A token that was created under team A will **only ever** return team A's resources, regardless of what the Coolify dashboard's "Keys & Tokens" page shows when you switch teams (the UI lists all your user-owned tokens under each team you switch to — that's a display quirk, not a rebind).
- The team dropdown shown after a successful "Test connection" lists every team your user belongs to, but only the **token's bound team** is selectable. The others appear grayed out with `— needs separate PAT`.

To add a second team:

1. In the Coolify dashboard, **switch the active team** to the second team (top-right team picker — verify the switch took by refreshing the page).
2. Open **Keys & Tokens** → **+ Create New Token** (do *not* reuse an existing token).
3. The new token's row in `personal_access_tokens` gets `team_id = <second team>.id`.
4. In the app, click **+ Add** in the tab strip and paste the new token. The team dropdown now offers the second team as selectable.

## Build from source

Prerequisites: [Rust](https://rustup.rs), Node 22, [pnpm](https://pnpm.io) 9, and the platform Tauri prereqs ([macOS](https://tauri.app/start/prerequisites/#macos) / [Linux](https://tauri.app/start/prerequisites/#linux) / [Windows](https://tauri.app/start/prerequisites/#windows)).

```bash
pnpm install
pnpm tauri dev      # dev loop with HMR
pnpm tauri build    # bundle for the CURRENT OS only
```

`pnpm tauri build` produces installers under `src-tauri/target/release/bundle/` for whichever OS you ran the command on — macOS gives you `.dmg` + `.app`, Windows gives `.msi` + `.exe`, Linux gives `.deb` + `.AppImage`. Tauri does **not** cross-compile; building all three platforms requires either three machines or CI runners (see "Cross-OS releases" below).

Quick platform notes:

- **macOS** — Xcode CLT (`xcode-select --install`).
- **Linux** — `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`.
- **Windows** — WebView2 runtime + MSVC build tools.

## Release

Tag-driven cross-OS builds via GitHub Actions
([`.github/workflows/release.yml`](./.github/workflows/release.yml)).
Push a `v*` tag → matrix runs on macOS / Linux / Windows → draft GitHub
Release with `.dmg` + `.msi` + `.AppImage` + `.deb` attached.

See [`docs/releasing.md`](./docs/releasing.md) for the full procedure,
including code-signing setup (macOS Developer ID + notarization,
Windows OV/EV).

## Tech stack

- [Tauri 2](https://tauri.app) — Rust backend + native webview shell
- [SvelteKit](https://svelte.dev) + [Svelte 5 runes](https://svelte.dev/docs/svelte/what-are-runes) — frontend
- [TypeScript](https://www.typescriptlang.org) + [Vite](https://vite.dev)
- [shadcn-svelte](https://www.shadcn-svelte.com) — UI primitives (dark theme only)
- [svelte-sonner](https://svelte-sonner.vercel.app) — toasts
- [tauri-plugin-http](https://tauri.app/plugin/http-client/) — Rust-side reqwest (CORS-free, token never enters the webview)
- [tauri-plugin-keyring](https://github.com/tauri-apps/plugins-workspace) — OS-native secret storage
- [oci-distribution](https://crates.io/crates/oci-distribution) — registry digest fetch for image-freshness checks

## Reference docs

- [`PRD.md`](./PRD.md) — product requirements (origin)
- [`CONTEXT.md`](./CONTEXT.md) — domain glossary (Resource / Project / Instance / Status / Digest …)
- [`docs/superpowers/specs/2026-05-25-coolify-gui-design.md`](./docs/superpowers/specs/2026-05-25-coolify-gui-design.md) — locked design + 30-step build sequence
- [`AGENTS.md`](./AGENTS.md) — conventions for agents and humans working in this repo
- [`docs/releasing.md`](./docs/releasing.md) — release process + code-signing
