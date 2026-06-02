# Releasing

Tag-driven, cross-OS, via GitHub Actions
([`.github/workflows/release.yml`](../.github/workflows/release.yml)).

## Cut a release

1. **Bump the version in three files. They must match.**
   - [`package.json`](../package.json) → `"version"`
   - [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json) → `"version"`
   - [`src-tauri/Cargo.toml`](../src-tauri/Cargo.toml) → `[package] version`

   For pre-releases (`alpha`, `beta`, `rc`) the MSI bundler ALSO needs a
   numeric-only override in `tauri.conf.json` → `bundle.windows.wix.version`
   (e.g. `"0.1.0.1"` for `0.1.0-alpha.1`). MSI's version field rejects
   anything beyond `MAJOR.MINOR.PATCH.BUILD` with all four parts numeric.

2. **Commit + tag + push.** The workflow trigger is `on.push.tags: v*` —
   it doesn't care which branch the tag points at. What matters is that
   the **tag itself** reaches the remote.

   ```bash
   git commit -am "release: v0.2.0"
   git tag v0.2.0
   # --follow-tags pushes annotated tags reachable from the pushed
   # commits. Lightweight tags (created without -a / -m / -s) are
   # ignored even with --follow-tags — push them explicitly.
   git push origin "$(git branch --show-current)" --follow-tags
   ```

   **Common mistake:** pushing the commit without the tag. The release
   commit shows up on the remote but no workflow fires. To verify:

   ```bash
   git ls-remote --tags origin v0.2.0   # must return a sha; empty = not pushed
   gh run list --workflow release.yml --limit 3
   ```

   If the tag is missing remotely, push it now:

   ```bash
   git push origin v0.2.0
   ```

3. **Watch the workflow** in the Actions tab or via CLI:

   ```bash
   gh run watch
   ```

4. After all three platforms succeed, GitHub creates a **draft release**.
   Review the assets, write release notes, click Publish.

   Tags with `-` (semver pre-release) are auto-flagged
   `prerelease: true` so they don't claim the "Latest" badge.

   **Gotcha — draft persistence across reruns.** `tauri-action` does
   "update if a draft for this tag already exists" rather than recreate.
   If a previous failed run created the draft with the wrong
   `prerelease` value (e.g. because the workflow's expression was
   hardcoded `false` at the time), subsequent successful runs upload
   assets to that same draft but DON'T retoggle the prerelease flag.
   Fix: either check the **"Set as a pre-release"** checkbox manually
   before clicking Publish, OR delete the stale draft on the Releases
   page and re-trigger the workflow.

## Re-trigger an already-pushed tag

If the workflow file changed but the tag commit is old (workflow runs
against the workflow file AT THE TAG'S COMMIT, not main), re-cut:

```bash
git tag -d v0.2.0                         # delete local
git push origin :refs/tags/v0.2.0         # delete remote
git tag v0.2.0                            # retag at current HEAD
git push origin v0.2.0                    # re-push
```

Or trigger manually via the Actions tab → "release" workflow → "Run
workflow" button (passes the tag name as an input).

## macOS code signing + notarization

Without signing, the `.dmg` triggers Gatekeeper warnings
(`"App is damaged and can't be opened"`). Two-step setup:

### 1. Get a Developer ID Application certificate

1. [developer.apple.com](https://developer.apple.com/account/resources/certificates) →
   Certificates → `+` → Developer ID Application.
2. Generate a Certificate Signing Request from Keychain Access
   (Keychain Access → Certificate Assistant → Request a Certificate
   from a Certificate Authority).
3. Upload the CSR, download the `.cer`, double-click to install in
   your local Keychain.
4. Export from Keychain: right-click the certificate → Export → save
   as `.p12` with a password.

### 2. Add GitHub secrets

Convert the `.p12` to base64 for the secret value:

```bash
base64 -i Certificates.p12 | pbcopy
```

Repo → Settings → Secrets and variables → Actions → New repository
secret. Add all six:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64-encoded `.p12` contents |
| `APPLE_CERTIFICATE_PASSWORD` | password used when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | full identity string, e.g. `"Developer ID Application: Your Name (TEAMID)"` (find with `security find-identity -v -p codesigning`) |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | [app-specific password](https://support.apple.com/en-us/HT204397) (NOT your real Apple ID password) |
| `APPLE_TEAM_ID` | 10-character team ID (find at appleid.apple.com under "Memberships") |

### 3. Re-enable the signing env block in the workflow

Open [`.github/workflows/release.yml`](../.github/workflows/release.yml)
and add to the `tauri-apps/tauri-action@v0` step's `env`:

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
  APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
  APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
  APPLE_ID: ${{ secrets.APPLE_ID }}
  APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
  APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
```

Important: these env vars must NOT be set to empty strings when secrets
are missing — `tauri-action` interprets an empty `APPLE_CERTIFICATE` as
"attempt signing" and fails on `security import`. Either add ALL six
secrets, or omit the env block entirely.

References:

- [Tauri macOS signing guide](https://tauri.app/distribute/sign/macos/)
- [Apple notarization overview](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)

## Windows code signing

Optional but recommended. Without it, SmartScreen shows the
"Windows protected your PC" warning. Requires an OV or EV code-signing
certificate from a trusted CA (typically $200–500/year).

Once you have the cert:

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | base64-encoded `.pfx` or `.p12` |
| `WINDOWS_CERTIFICATE_PASSWORD` | password for the cert |

Add to the workflow's `env`:

```yaml
env:
  WINDOWS_CERTIFICATE: ${{ secrets.WINDOWS_CERTIFICATE }}
  WINDOWS_CERTIFICATE_PASSWORD: ${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}
```

Reference: [Tauri Windows signing guide](https://tauri.app/distribute/sign/windows/).

## Linux

`.deb` + `.AppImage` produced by `tauri-action` are unsigned. AppImage
supports embedded signatures via `gpg`, but most distribution channels
(direct download, GitHub Release) don't enforce it. Skip for now.

## Known gotchas

### MSI version must be numeric-only

The Windows MSI bundler rejects pre-release identifiers like
`alpha.1` in the app version. Symptom in CI:

```
failed to bundle project `optional pre-release identifier in app version
must be numeric-only and cannot be greater than 65535 for msi target`
```

Workaround: set a separate MSI-only numeric version in
`tauri.conf.json`:

```json
"bundle": {
  "windows": {
    "wix": {
      "version": "0.1.0.1"
    }
  }
}
```

Keep this in sync with the semver version on each bump (e.g. `0.2.0` →
`"version": "0.2.0.0"`, `0.2.0-alpha.1` → `"version": "0.2.0.1"`).

### Empty Apple signing env breaks the macOS build

`tauri-action` treats `APPLE_CERTIFICATE` set to an empty string as
"attempt signing" and dies on `security import`:

```
security: SecKeychainItemImport: One or more parameters passed to a
function were not valid.
failed to bundle project failed codesign application
```

If you don't have a Developer ID yet, OMIT the entire `APPLE_*` env
block from the workflow step. Don't leave the keys in with empty
secret references — `${{ secrets.APPLE_CERTIFICATE }}` returns `""`
when the secret is unset, which still triggers the signing path.
Either all six secrets are populated, or none of the env keys are
present.

### `codeload.github.com` outages cause "action could not be found"

Symptom: the "Set up job" step fails with

```
##[error]An action could not be found at the URI
'https://codeload.github.com/...'
##[error]Failed to download archive ... after 1 attempts.
```

This is a transient GitHub infrastructure issue — your workflow is
fine. Re-run the failed jobs:

```bash
gh run rerun <run-id> --failed
```

### Node.js 20 deprecation warning

GitHub Actions is migrating actions from Node 20 to Node 24 on
June 2nd, 2026. Until then, suppress the deprecation warning by
opting in early at workflow scope:

```yaml
env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
```

(Already wired in this repo's `release.yml`.)

## Auto-updater (future)

Tauri ships a built-in updater. Enabling it requires:

1. A keypair (`tauri signer generate`)
2. `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   secrets
3. `updater` field in `tauri.conf.json` pointing at a `latest.json`
   endpoint (`tauri-action`'s `includeUpdaterJson: true` already
   generates one alongside the release assets)

Defer until the project has real users.
