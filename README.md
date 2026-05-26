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
pnpm tauri build    # bundle for the CURRENT OS only
```

`pnpm tauri build` produces installers under `src-tauri/target/release/bundle/` for whichever OS you ran the command on — macOS gives you `.dmg` + `.app`, Windows gives `.msi` + `.exe`, Linux gives `.deb` + `.AppImage`. Tauri does **not** cross-compile; building all three platforms requires either three machines or CI runners (see "Cross-OS releases" below).

Quick platform notes:

- **macOS** — Xcode CLT (`xcode-select --install`).
- **Linux** — `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`.
- **Windows** — WebView2 runtime + MSVC build tools.

## Release

Version + tag drives the release. The flow:

1. Bump the version in **both** files (they must match):
   - `package.json` → `"version"`
   - `src-tauri/tauri.conf.json` → `"version"`
   - `src-tauri/Cargo.toml` → `[package] version =`
2. Commit + tag:

   ```bash
   git commit -am "release: v0.2.0"
   git tag v0.2.0
   git push origin main --tags
   ```

3. The tag push triggers the cross-OS release workflow (see below).

### Cross-OS releases via GitHub Actions

Tauri builds are produced on three runners in parallel (`macos-latest`, `windows-latest`, `ubuntu-latest`) and the artifacts (`.dmg`, `.msi`, `.AppImage`, `.deb`) are attached to a draft GitHub Release. Tag-triggered.

Wire-up: add `.github/workflows/release.yml` using the official [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action). Minimal template:

```yaml
name: release
on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        platform: [macos-latest, ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with: { version: 9 }
      - uses: actions/setup-node@v4
        with: { node-version: 22, cache: pnpm }
      - uses: dtolnay/rust-toolchain@stable
      - if: matrix.platform == 'ubuntu-latest'
        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
      - run: pnpm install --frozen-lockfile
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: ${{ github.ref_name }}
          releaseDraft: true
          prerelease: false
```

After CI succeeds, the GitHub Release is in draft state — review the artifacts, write release notes, then click **Publish**.

### macOS code signing + notarization (optional)

Unsigned `.dmg` triggers Gatekeeper warnings (`"App is damaged"`). To sign:

1. Get an Apple Developer ID Application certificate.
2. Add these GitHub secrets and pass them into the `tauri-action` step:
   - `APPLE_CERTIFICATE` (base64-encoded .p12)
   - `APPLE_CERTIFICATE_PASSWORD`
   - `APPLE_SIGNING_IDENTITY` (e.g. `"Developer ID Application: Your Name (TEAMID)"`)
   - `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` for notarization
3. See [Tauri's signing guide](https://tauri.app/distribute/sign/macos/).

### Windows signing (optional)

Unsigned `.msi` triggers SmartScreen warnings. Requires an EV / OV code-signing certificate; see [Tauri's Windows signing guide](https://tauri.app/distribute/sign/windows/).

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
