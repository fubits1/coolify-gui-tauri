import type { Resource } from "$lib/api/types";
import { imageCache } from "$lib/stores/image-cache.svelte";
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
 * Pull every image ref out of `resources[*].image_refs` — the backend now
 * populates this for us via compose-scrape + single-image-name+tag, so no
 * per-resource detail fetch is needed. De-duplicate before returning.
 */
async function collectImageRefs(resources: Resource[]): Promise<string[]> {
  const out = new Set<string>();
  for (const r of resources) {
    for (const ref of r.image_refs ?? []) {
      if (ref && ref.trim().length > 0) {
        out.add(ref);
      }
    }
  }
  return [...out];
}
