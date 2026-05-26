# AGENTS.md — Working in this repo

> **MANDATORY for every agent + main thread:**
>
> - **`/nogrep`** — use Read/Glob/fff/Grep tools. NEVER Bash `cat/grep/find/head/tail/sed/awk/wc/ls -R`. Bash is only for `cargo`, `pnpm`, `dex`, `jq`, `gh`, `mkdir`, `cp`, `git mv`, `which`.
> - **`/dex`** — track every task via dex. **NEW /dex requests = QUEUE them with `dex create`. Do NOT context-switch to them mid-flight. Finish the in-progress fix first, then dequeue.**
> - **`/playwright`** — VERIFY EVERY VISIBLE CHANGE in the browser yourself. "It compiles" ≠ "it works".
>
>   **Dev server commands** (the SvelteKit frontend serves at **<http://localhost:1420/>**):
>
>   | Command | What runs | When to use |
>   |---|---|---|
>   | `pnpm tauri dev` | Vite (1420) + Rust backend + native webview | Full app — Tauri IPC works, real Coolify API calls, secrets via keyring. Required for any test that depends on backend data. |
>   | `pnpm dev` | Vite (1420) only — no Rust, no IPC | Pure-frontend visual smoke test. `invoke()` calls fail; stores stay empty. Use only when verifying static layout, routing, CSS, keybinds, or pure-UI logic. |
>
>   The user is expected to keep `pnpm tauri dev` running while you work. If `<http://localhost:1420/>` refuses connection, ASK the user to start it — do NOT spawn `pnpm tauri dev` in the background yourself (it opens a native window and grabs focus). `pnpm dev` is safe to background for frontend-only smoke tests.
>
>   Flow: `mcp__playwright__browser_navigate http://localhost:1420/` → `browser_snapshot` → confirm the change rendered. Then `browser_take_screenshot` for the user to eyeball when needed.
>
> Skipping either rule wastes the user's time on permission prompts.
>
> ## How to use `/dex` WITHOUT tripping `/nogrep`
>
> The dex CLI is allowed; what trips nogrep is **piping dex output through banned tools**. Do this:
>
> | Allowed | Banned |
> |---|---|
> | `dex create "Title" --parent mn0zcb89 --description "what + why"` | `dex list \| grep ...` |
> | `dex complete <id> --result "what landed"` | `dex list \| awk '{print $2}'` |
> | `dex show <id>` | `$(dex list \| head -1)` |
> | `dex list mn0zcb89` (read the output yourself in scrollback) | piping ANY dex output |
>
> Lifecycle: **create → (work) → complete**. One dex command per Bash call. Read IDs back from the previous output with your eyes — never extract them with `grep`/`awk`/`head`/`tail`. If you need to find a task by name, run `dex list mn0zcb89` and scan the output in the conversation.
>
> Mark `complete` ONLY after the fix is actually applied + verified (`pnpm check` / `cargo check` green). Marking a task complete while the bug still ships is a lie.

Conventions for any agent (or human) touching this codebase.

## Package manager

**pnpm only.** Never `npm` / `npx` / `yarn`.

For docs lookups via the npm scaffold/CLIs, use `pnpm dlx <pkg>`.

## Socket Firewall (supply-chain safety)

This machine has [Socket Firewall Free](https://docs.socket.dev/docs/socket-firewall-free) installed as `sfw`. It is a single binary that **wraps** package manager calls and blocks confirmed-malware installs at network time.

**Do not get stuck looking for `socket-pnpm` / `sfw pnpm` subcommands.** `sfw` is a transparent prefix — you call your normal package manager command **after** `sfw`.

### Use it like this

```bash
sfw pnpm add <pkg>
sfw pnpm add -D <pkg>
sfw pnpm install
sfw pnpm dlx shadcn-svelte@latest init
sfw pnpm dlx shadcn-svelte@latest add button input dialog
```

`sfw` accepts `pnpm`, `npm`, `yarn`, `pip`, `uv`, `cargo` after it.

### When to use `sfw`

- Adding new dependencies (`add`, `install <pkg>`).
- Running `dlx` against unknown / unfamiliar publishers.

### When you can skip `sfw`

- `pnpm install` / `pnpm i` to hydrate an existing lockfile — the lockfile pins versions; `sfw` is most useful at the moment of dependency introduction.
- `pnpm <script>` (e.g. `pnpm check`, `pnpm tauri ...`) — no network.
- `pnpm dlx` for well-known first-party tools (e.g. `pnpm dlx shadcn-svelte`).

If in doubt, prefix with `sfw` — there is no downside other than a few seconds of scan time.

## Tauri docs

A Tauri-docs MCP server is configured in `.mcp.json` (`tauri-docs`). Use it before guessing Tauri 2 APIs. Local llms.txt fallback at `docs/external/tauri-llms.txt`.

## Svelte 5

This is a Svelte 5 + SvelteKit project. **Runes only.**

- `$state`, `$derived`, `$effect`, `$props`, `$bindable`.
- No `export let` — use `let { ... } = $props()`.
- No `$:` — use `$derived` / `$effect`.
- No `on:click` — use `onclick`.
- Layout child rendering: `{@render children?.()}` after `let { children } = $props()`.

A Svelte MCP server is configured. Use `get-documentation` / `list-sections` / `svelte-autofixer` before guessing component syntax.

## Theme

**Dark only.** No light variants, no system-follow, no toggle. `<html class="dark">` always set in `src/app.html`.

## Routing

SvelteKit filesystem routes. Not `svelte-spa-router`.

- `/` → `src/routes/+page.svelte`
- `/settings` → `src/routes/settings/+page.svelte`
- Global layout + `<Toaster />` mount → `src/routes/+layout.svelte`

## Tauri commands

HTTP runs Rust-side via `tauri-plugin-http` (reqwest). The webview never holds the Coolify bearer token. Token is stored via the `keyring` crate (apple-native + windows-native + sync-secret-service + crypto-rust) — direct Rust integration, no community Tauri plugin. Linux requires a desktop session (D-Bus + Secret Service daemon — GNOME Keyring or KWallet); headless Linux is out of scope for this desktop GUI app.

reqwest is built with `gzip + brotli + deflate` features. Coolify (Laravel + nginx/Traefik) ships compressed responses on list endpoints; without these features `text()` reads raw compressed bytes and serde_json errors with "premature end of input".

## macOS Keychain prompts in dev mode

`pnpm tauri dev` recompiles the Rust binary on every change. Each rebuild produces a new file hash → macOS Keychain treats it as a new application → re-prompts for "Allow access to `coolify_token_*`" on every relaunch, even after clicking "Always Allow". This is a Keychain + unsigned-binary limitation, **not a bug in the keyring wiring**.

Production builds via `pnpm tauri build` are codesigned (with `APPLE_CERTIFICATE` env or ad-hoc) → "Always Allow" sticks across launches.

Workarounds during dev:

- Hit Return on the prompt — token loads, app proceeds.
- For sustained testing of the auth path, build once via `pnpm tauri build` and run the bundle directly.

## Dev loop

- `pnpm check` — typecheck (run after any change).
- `pnpm tauri dev` — **the user runs this**, not agents.
- `pnpm tauri build` — bundles for the current OS.

## Reference docs

- `PRD.md` — what we're building (origin).
- `CONTEXT.md` — domain glossary.
- `docs/superpowers/specs/2026-05-25-coolify-gui-design.md` — locked design + 30-step build sequence.

## Task tracking

dex tasks live under epic `mn0zcb89`. Run `dex list mn0zcb89` to see the tree. Update with `dex complete <id>` when finishing a leaf.
