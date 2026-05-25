# Coolify GUI

Desktop client for self-hosted Coolify instances. Glossary mirrors Coolify's own domain so client code, UI labels, and API talk match upstream terminology.

## Language

**Resource**:
A deployed unit operated via Coolify — one of: **Application**, **Service**, **Database**. Primary row type in the overview table.
_Avoid_: project (means org folder in Coolify), app (ambiguous), container, deployment (means a deploy event, not the thing).

**Application**:
A **Resource** built from a Git repo via nixpacks / dockerfile / static / dockercompose build pack.
_Avoid_: app, service (Service is a sibling type).

**Service**:
A **Resource** defined by a raw `docker-compose` YAML (`docker_compose_raw`). Generic multi-container deployment.
_Avoid_: stack, compose.

**Database**:
A **Resource** of a managed engine type (Postgres, MySQL, MariaDB, Mongo, Redis, ClickHouse, Dragonfly, KeyDB).
_Avoid_: db, datastore.

**Project**:
A Coolify organizational folder grouping **Resources**. Has no status, no actions. Shown as a tag/filter in the GUI.
_Avoid_: workspace, group.

**Environment**:
A sub-folder inside a **Project** (e.g. `production`, `staging`). Also shown as tag/filter.
_Avoid_: stage, tier.

**Team**:
A Coolify tenant boundary. The bearer token is scoped to one Team.
_Avoid_: org, account.

**Status**:
A combined string returned by Coolify per **Resource**: `running:healthy`, `exited:unhealthy`, `degraded`, `starting`, `excluded`. Parsed by splitting on `:`.

**Deploy**:
An action that pulls latest source / image and starts a new container for a **Resource**. Verb. The event record is a _Deployment_.
_Avoid_: redeploy (Coolify uses `deploy`), update.

**Restart**:
An action that recreates a **Resource**'s containers from the current image without pulling anything new.

**Instance**:
A single user-supplied Coolify deployment, addressed by `{url, token}`. The GUI talks to one Instance per session.
_Avoid_: server (Coolify uses _Server_ for the SSH host underneath a Resource — different concept).

**Image Reference**:
A `name:tag` string parsed out of a **Resource**'s compose YAML or image fields. Unit of the freshness check.

**Digest**:
The OCI content-addressable SHA256 from a registry's manifest. Two **Image References** with the same tag but different digests = the tag moved upstream.

## Relationships

- A **Team** contains many **Projects**.
- A **Project** contains many **Environments**.
- An **Environment** contains many **Resources**.
- A **Resource** is exactly one of: **Application**, **Service**, **Database**.
- A **Service** owns one or more **Image References** via its compose file.
- An **Application** with `build_pack = dockercompose` also owns **Image References**.
- The GUI talks to exactly one **Instance** at a time.

## Example dialogue

> **Dev:** "User clicks Restart on the overview row — what fires?"
> **Domain:** "`POST /api/v1/{applications|services}/{uuid}/restart` against the current **Instance**. Restart doesn't pull. Pull-latest is a separate **Deploy** action."
> **Dev:** "What if the row is a **Database**?"
> **Domain:** "Databases support start/stop/restart, not Deploy."

## Flagged ambiguities

- "Project" in everyday speech meant "the thing I deploy" — resolved: that's a **Resource**. A Coolify **Project** is just an org folder.
- "Server" is overloaded: Coolify uses it for the underlying SSH host. We don't surface that concept in v1.
