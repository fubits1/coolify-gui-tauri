import { images, type ImageCacheEntry } from "$lib/api/images";
import { parseSemver, compareSemver } from "$lib/util/semver";
import { toast } from "$lib/util/toast";

/**
 * Image-cache store — frontend mirror of the Rust-side digest cache.
 *
 * Owns:
 * - `entries`: `imageRef → ImageCacheEntry` (last known digest + optional
 *   `latest_digest` + optional `highest_semver_tag` + `checked_at`).
 * - `checking`: a set of image refs currently in flight, so UI rows can
 *   render a spinner without each row tracking its own loading state.
 *
 * `check(ref)` runs a single force-check via Tauri; `checkMany(refs)` runs
 * a concurrency-capped batch (4 in flight) to stay friendly to Docker Hub's
 * anonymous rate limit (100 / 6h).
 *
 * `isStale(ref)` is the single source of truth for "is there a newer image
 * than the one this resource is running?". UI badges, summary counts, and
 * the scheduler all derive from it.
 *
 * Same class-singleton pattern as the other stores in this folder.
 */

const CONCURRENCY = 4;

/** Tri-state result for a single image ref. */
export type StaleState = "unknown" | "fresh" | "newer-available";

class ImageCacheStore {
  entries: Record<string, ImageCacheEntry> = $state({});
  checking: Set<string> = $state(new Set());

  /** Read the persisted cache into reactive state. Safe to call repeatedly. */
  async load(): Promise<void> {
    try {
      this.entries = await images.readCache();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error("Failed to load image cache", msg);
    }
  }

  /**
   * Force a single check. Spinner is tracked via `checking`. On success the
   * entry is written into `entries` so dependent `$derived` reads update.
   */
  /**
   * Single-image check. Silent on success — callers (per-row "Check now",
   * batch "Check all") are responsible for any user-facing toast. Per-image
   * toasts caused 13 sticky popups when checking a full supabase compose.
   * Errors still toast individually so a partial failure surfaces.
   */
  async check(imageRef: string): Promise<void> {
    if (this.checking.has(imageRef)) return;
    // Set is reactive in Svelte 5 — reassign to trigger fine-grained updates
    // from callers using `imageCache.checking.has(ref)` as a $derived input.
    this.checking.add(imageRef);
    this.checking = new Set(this.checking);
    try {
      const entry = await images.check(imageRef);
      this.entries = { ...this.entries, [imageRef]: entry };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(`Image check failed: ${imageRef}`, msg);
    } finally {
      this.checking.delete(imageRef);
      this.checking = new Set(this.checking);
    }
  }

  /**
   * Concurrency-capped batch check. Runs at most `CONCURRENCY` calls in
   * flight at once. Resolves once every ref has either succeeded or failed
   * (individual failures don't reject the batch — each is toasted).
   */
  async checkMany(refs: string[]): Promise<void> {
    const queue = [...new Set(refs)].filter((r) => !this.checking.has(r));
    if (queue.length === 0) return;
    let cursor = 0;
    const worker = async (): Promise<void> => {
      while (cursor < queue.length) {
        const next = queue[cursor++];
        if (next === undefined) return;
        await this.check(next);
      }
    };
    const workers = Array.from(
      { length: Math.min(CONCURRENCY, queue.length) },
      worker,
    );
    await Promise.all(workers);
  }

  /**
   * Tri-state freshness verdict for a single image ref:
   * - `unknown` — never checked
   * - `newer-available` — `latest_digest` diverges OR `highest_semver_tag`
   *   parses higher than the current pinned tag
   * - `fresh` — checked, no newer image visible
   *
   * The current tag is parsed out of the image ref the same way `compose.ts`
   * splits images, so callers can pass either a full `name:tag` or a bare
   * `name` (which is treated as `:latest`).
   */
  isStale(imageRef: string, lastDeployedAt?: string | null): StaleState {
    const entry = this.entries[imageRef];
    if (!entry) return "unknown";
    // No useful upstream signal AT ALL → genuinely unknown.
    if (
      entry.latest_digest == null &&
      entry.highest_semver_tag == null &&
      entry.latest_pushed_at == null
    ) {
      return "unknown";
    }
    // For `:latest` tags, an additional publish-timestamp drift check:
    // when the registry pushed a newer `:latest` AFTER the resource was
    // deployed, the running container is stale — even if our
    // `current_digest` (sourced from the registry, not the running
    // container) currently matches `latest_digest`. Falls through to the
    // standard digest/semver compare below when timestamps are missing.
    const tag = parseTag(imageRef);
    if (tag === "latest" && entry.latest_pushed_at != null && lastDeployedAt) {
      const deployMs = Date.parse(lastDeployedAt);
      if (Number.isFinite(deployMs) && entry.latest_pushed_at > deployMs) {
        return "newer-available";
      }
    }
    return this.#stateFor(entry, imageRef);
  }

  /** Shared verdict logic for `isStale` and post-check toasts. */
  #stateFor(entry: ImageCacheEntry, imageRef: string): StaleState {
    if (entry.latest_digest && entry.latest_digest !== entry.digest) {
      return "newer-available";
    }
    if (entry.highest_semver_tag) {
      const currentTag = parseTag(imageRef);
      const current = parseSemver(currentTag);
      const highest = parseSemver(entry.highest_semver_tag);
      if (current && highest && compareSemver(highest, current) > 0) {
        return "newer-available";
      }
    }
    return "fresh";
  }
}

/**
 * Mirror of the `splitImage` heuristic in `$lib/util/compose.ts` so a bare
 * image ref like `nginx:1.27` or `registry:5000/foo:1.2.3` yields the tag
 * portion without misreading a registry port as a tag.
 */
function parseTag(imageRef: string): string {
  const atIdx = imageRef.indexOf("@");
  if (atIdx !== -1) return imageRef.slice(atIdx + 1);
  const lastColon = imageRef.lastIndexOf(":");
  if (lastColon === -1) return "latest";
  const candidate = imageRef.slice(lastColon + 1);
  if (candidate.includes("/")) return "latest";
  return candidate;
}

export const imageCache = new ImageCacheStore();
