# AGENTS.md — Working in this repo

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

HTTP runs Rust-side via `tauri-plugin-http` (reqwest). The webview never holds the Coolify bearer token. Token is stored via `tauri-plugin-keyring-api`.

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
