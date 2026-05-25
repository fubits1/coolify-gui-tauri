# Coolify v1 API — endpoints used by this GUI

Verified against `coolify.io/docs/api-reference/api/` and
`github.com/coollabsio/coolify/openapi.json` (May 2026).

**Base:** `{instance_url}/api/v1` · **Auth:** `Authorization: Bearer {token}`

Token scopes needed for this GUI: **`read:sensitive` + `deploy`**. `write`
and `root` are NOT required.

## Resource lists do NOT carry project context

`GET /applications` / `/services` / `/databases` each return items with
**`environment_id` (integer)** but NO nested `environment` object and NO
`project_uuid` / `project_name`. Confirmed via runtime diagnostic against
a live instance (the `/applications[0]` keys we logged on 2026-05-25
explicitly lacked any `environment` or `project` field).

Project endpoints available:

| Method | Path | Returns |
|---|---|---|
| GET | `/projects` | bare array of `{id, uuid, name, description}` — no environments embedded |
| GET | `/projects/{uuid}` | single project, same flat shape |
| GET | `/projects/{uuid}/{env_name_or_uuid}` | environment object: `{id, name, project_id (int), created_at, updated_at, description}` — project as INT id, not uuid |

**There is no list-all-environments endpoint.** To map a resource's
`environment_id (int)` to a project name you must either:

1. Walk every project, fetch each environment by name (you don't know the
   names without prior knowledge — chicken/egg).
2. Try `GET /resources` (operation listed in API sidebar but schema is
   opaque; we added a one-shot diagnostic to dump its top-level keys at
   runtime — see `tracing::warn!("DIAG /resources[0] keys: …")`).

Until Coolify exposes a list-environments endpoint, "Group by Project"
falls back to `(NO PROJECT)`. Tracked under dex task `mot3vkfo` /
`ruu9wr1x`.

## Quirks that bit us (don't relearn the hard way)

1. **Datetimes are MySQL-style**, NOT RFC 3339: `"2026-05-25 17:57:07"` (space separator, no `Z`). chrono's default `parse_from_rfc3339` rejects → serde_json reports the failure as `premature end of input at line 1 column N` (misleading). Use `parse_loose_datetime`.
2. **Coolify ships compressed responses**. reqwest must be built with `gzip + brotli + deflate` features — without them, large list bodies look truncated.
3. **`last_online_at` is a heartbeat**, not a deploy timestamp — refreshed continuously while a container is running. Use `/deployments/applications/{uuid}` for real deploy times.
4. **Service detail nests containers under `applications` + `databases`** (NOT `service_applications` / `service_databases` despite the schema name). Each entry has its own coolify UUID.
5. **No logs endpoint for Services or Databases.** Only `/applications/{uuid}/logs`. The dashboard streams container logs via Soketi WebSocket out-of-band.
6. **Service top-level `fqdn` is often null.** For Coolify service templates that declare `SERVICE_URL_<NAME>_<PORT>` or `SERVICE_FQDN_<NAME>` as env passthroughs, the actual URL only appears in `/services/{uuid}/envs` (key value), not in `docker_compose_raw`.
7. **Coolify env vars duplicate keys across scopes** (production vs preview, build vs runtime). A keyed `{#each}` keyed on var name will explode with `each_key_duplicate`. Either key by index or include the entry's own uuid.
8. **Per-tag digests not in OCI v2 tag-list response.** Use Docker Hub Hub API (`hub.docker.com/v2/repositories/{ns}/{repo}/tags?ordering=last_updated`) for Hub images — one call returns digest + `last_updated` per tag.

## Endpoint reference (used + planned)

### Health + auth

| Method | Path | Notes |
|---|---|---|
| GET | `/health` | No auth. Returns `{version}` or bare `"OK"`. Used for connection probe. |
| GET | `/teams` | Returns `[{name, ...}]`. First entry's name → ConnectionStrip label. |

### Resources — list

| Method | Path | Used for |
|---|---|---|
| GET | `/applications` | Overview rows (kind=Application) |
| GET | `/services` | Overview rows (kind=Service) |
| GET | `/databases` | Overview rows (kind=Database) |

All return **bare arrays** (no `{data:[…]}` wrapper).

### Resources — detail

| Method | Path | Notes |
|---|---|---|
| GET | `/applications/{uuid}` | Detail pane source for Applications |
| GET | `/services/{uuid}` | Detail pane source for Services. Includes nested `applications`/`databases` arrays — each item has its own UUID we surface as `service_containers`. |
| GET | `/databases/{uuid}` | Detail pane source for Databases |

### Env vars

| Method | Path | Notes |
|---|---|---|
| GET | `/applications/{uuid}/envs` | Env tab for Applications. Bare array. |
| GET | `/services/{uuid}/envs` | Env tab for Services. Bare array. ALSO scraped for `SERVICE_URL_*` / `SERVICE_FQDN_*` to recover the missing top-level fqdn. |

Per-env-var fields: `key`, `value` (masked), `real_value` (full), `is_secret`, `is_preview`, `is_buildtime`, `is_runtime`, `is_shared`, `is_shown_once`, plus its own `uuid`.

### Actions

| Method | Path | Notes |
|---|---|---|
| GET | `/applications/{uuid}/restart` | Restart button |
| GET | `/applications/{uuid}/stop` | Stop button (accepts `?docker_cleanup=`) |
| GET | `/applications/{uuid}/start` | Start (accepts `?force=`, `?instant_deploy=`) |
| GET | `/services/{uuid}/restart` | Restart button for Services |
| GET | `/services/{uuid}/stop` | Stop for Services |
| GET | `/services/{uuid}/start` | Start for Services |
| GET | `/databases/{uuid}/restart` | Restart for Databases |
| GET | `/deploy?uuid=<uuid>&force=<true\|false>` | Deploy button. `force=true` skips Docker cache. |

### Deployments

| Method | Path | Notes |
|---|---|---|
| GET | `/deployments` | "Currently running" deployments (not history) |
| GET | `/deployments/applications/{uuid}?take=1&skip=0` | **Per-app deploy history.** First entry's `created_at` = last deploy timestamp. Used by overview "Last deploy" column (60s cached). |

### Logs (Applications only)

| Method | Path | Notes |
|---|---|---|
| GET | `/applications/{uuid}/logs?lines=N` | Returns `{"logs": "string"}` JSON envelope. |

**Services + Databases have NO logs endpoint.** LogsTab shows an explanatory empty-state for those kinds.

### Image-freshness (out-of-band registries)

Not Coolify endpoints — we call registries directly for image staleness:

| Source | URL | Notes |
|---|---|---|
| Docker Hub Hub API | `hub.docker.com/v2/repositories/{ns}/{repo}/tags?page_size=100&ordering=last_updated` | One call → digest + `last_updated` per tag. Lighter rate limit than registry API. Used for any Docker Hub `image:tag`. |
| OCI Distribution v2 | `{registry}/v2/{name}/manifests/{tag}` + `/v2/{name}/tags/list` | Used for non-Docker-Hub registries (GHCR, quay). 3 round-trips per image. |

`registry::hub::is_docker_hub_ref` decides routing.

## Response field cheatsheet

### Application (relevant fields)

```
uuid, name, status, fqdn, build_pack,
git_repository, git_branch, git_commit_sha,
docker_registry_image_name, docker_registry_image_tag,
docker_compose_raw, ports_exposes,
last_online_at (HEARTBEAT, not deploy), updated_at,
environment { name, project { uuid, name } },
destination { server { name } }
```

### Service (relevant fields)

```
uuid, name, status, fqdn (often null),
docker_compose_raw, docker_compose (parsed object form),
service_type, server_status,
applications: [{ uuid, name, image, fqdn, ... }],   // ← per-container handles
databases: [{ uuid, name, image, ... }],
last_online_at, updated_at,
environment { name, project { uuid, name } },
server, destination_id
```

### Deployment

```
id, deployment_uuid, application_id, application_name,
docker_registry_image_tag, force_rebuild, commit, commit_message,
status (queued | in_progress | success | failed | cancelled),
created_at, updated_at,
is_webhook, is_api, rollback, restart_only, only_this_server,
deployment_url, server_id, server_name
```

### EnvironmentVariable

```
id, uuid, resourceable_type, resourceable_id,
key, value (masked when is_secret), real_value (full),
is_literal, is_multiline, is_preview, is_runtime, is_buildtime,
is_shared, is_shown_once,
comment, version, created_at, updated_at
```

## Sources

- [Coolify API reference](https://coolify.io/docs/api-reference/api/)
- [openapi.json](https://raw.githubusercontent.com/coollabsio/coolify/main/openapi.json) (often truncated in WebFetch — read directly)
- [Authorization docs](https://coolify.io/docs/api-reference/authorization)
- [DeepWiki — Authentication & Authorization](https://deepwiki.com/coollabsio/coolify/8.1-authentication-and-authorization)
- [Docker Hub Hub API](https://docs.docker.com/reference/api/hub/latest/)
