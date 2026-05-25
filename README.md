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
3. Hit **Test connection**, then **Save**.

### Required token scope

In the Coolify dashboard, create a Personal Access Token with:

- `read:sensitive` — list resources + read environment variable values.
- `deploy` — Restart / Stop / Deploy actions.

`write` and `root` are **not** required. The token is stored in your OS keyring (Keychain / Credential Manager / Secret Service); the webview never holds it.

## Build from source

Prerequisites: [Rust](https://rustup.rs), Node 22, [pnpm](https://pnpm.io) 9, and the platform Tauri prereqs ([macOS](https://tauri.app/start/prerequisites/#macos) / [Linux](https://tauri.app/start/prerequisites/#linux) / [Windows](https://tauri.app/start/prerequisites/#windows)).

```bash
pnpm install
pnpm tauri dev      # dev loop with HMR
pnpm tauri build    # bundle for the current OS
```

Quick platform notes:

- **macOS** — Xcode CLT (`xcode-select --install`).
- **Linux** — `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`.
- **Windows** — WebView2 runtime + MSVC build tools.

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
