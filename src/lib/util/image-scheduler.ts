import type { Resource, ResourceDetail } from "$lib/api/types";
import { api } from "$lib/api/client";
import { imageCache } from "$lib/stores/image-cache.svelte";
import { parseComposeImages } from "$lib/util/compose";
import { toast } from "$lib/util/toast";

/**
 * Image-freshness scheduler — runs once at app boot.
 *
 * Per design decision #5: daily-at-startup + manual override, with a 24h
 * cache so we don't burn Docker Hub's anon rate limit (100/6h) on every
 * launch. Each image ref is checked only if:
 *   1. it's missing from the cache, or
 *   2. its `checked_at` is older than 24h.
 *
 * For compose-backed Application/Service resources we have to fetch the
 * detail to read `docker_compose_raw`. Failures are silent (toast at the
 * end) — the scheduler must never block the UI.
 */

const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * Walk every resource, collect its image refs, then fire a
 * concurrency-capped batch check for the subset whose cache entries are
 * missing or stale (> 24h old).
 *
 * Caller responsibility:
 * - Call this *after* `resources.start()` so `resources.list` is populated.
 * - Do NOT await it from a layout/page bootstrap if you care about TTFB —
 *   the inner `checkMany` is the slow part.
 */
export async function runStartupCheck(resources: Resource[]): Promise<void> {
  if (resources.length === 0) return;

  await imageCache.load();

  const refs = await collectImageRefs(resources);
  if (refs.length === 0) return;

  const now = Date.now();
  const stale = refs.filter((ref) => {
    const entry = imageCache.entries[ref];
    if (!entry) return true;
    return now - entry.checked_at > DAY_MS;
  });

  if (stale.length === 0) {
    toast.info(`Image cache fresh (${refs.length} images, < 24h old)`);
    return;
  }

  toast.info(
    `Checking ${stale.length} image${stale.length === 1 ? "" : "s"} for updates…`,
  );
  await imageCache.checkMany(stale);

  const newer = stale.filter(
    (ref) => imageCache.isStale(ref) === "newer-available",
  ).length;
  if (newer > 0) {
    toast.info(
      `${newer} image${newer === 1 ? " has" : "s have"} a newer version available`,
    );
  } else {
    toast.success("All images are up to date");
  }
}

/**
 * Pull image refs out of each resource:
 * - Application / Service with compose → fetch detail, parse YAML.
 * - Application non-compose / Database → use `image_ref` if present.
 * Returns a de-duplicated list.
 */
async function collectImageRefs(resources: Resource[]): Promise<string[]> {
  const out = new Set<string>();

  for (const r of resources) {
    // Fast path: a bare image ref is already known on the list view.
    if (r.image_ref) {
      out.add(r.image_ref);
      continue;
    }

    // Compose-backed resources need a detail fetch to see the YAML.
    if (r.kind === "Application" || r.kind === "Service") {
      const detail = await fetchDetailSafe(r);
      if (!detail) continue;
      if (
        detail.docker_compose_raw &&
        detail.docker_compose_raw.trim().length > 0
      ) {
        for (const img of parseComposeImages(detail.docker_compose_raw)) {
          out.add(`${img.image}:${img.tag}`);
        }
      } else if (detail.image_ref) {
        out.add(detail.image_ref);
      }
    }
  }

  return [...out];
}

/** Detail fetch that swallows errors — a missing detail must not abort the scheduler. */
async function fetchDetailSafe(r: Resource): Promise<ResourceDetail | null> {
  try {
    return await api.getResourceDetail(r.uuid, r.kind);
  } catch {
    return null;
  }
}
