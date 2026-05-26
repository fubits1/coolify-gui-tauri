import { invoke } from "@tauri-apps/api/core";

/**
 * One cached entry produced by the Rust-side image-freshness checker.
 *
 * - `digest` is the manifest digest of the resource's *current* tag at the
 *   time of the last check.
 * - `latest_digest` (optional) is the manifest digest of the `:latest` tag,
 *   used to detect mutable-tag drift.
 * - `highest_semver_tag` (optional) is the highest valid semver tag seen
 *   when listing the repository's tags, used to surface "newer available"
 *   for pinned-version deployments.
 * - `checked_at` is epoch ms (Rust serializes `SystemTime` as a u64 ms).
 */
export interface ImageCacheEntry {
  digest: string;
  latest_digest?: string;
  highest_semver_tag?: string;
  /** Epoch ms when the registry's current `:latest` (or the highest-named
   *  tag, for pinned-version refs) was last pushed. Populated by the
   *  Docker Hub Hub API path. Used to detect `:latest` drift relative
   *  to the resource's deploy time. */
  latest_pushed_at?: number | null;
  checked_at: number;
}

/**
 * Thin typed wrappers around the Rust image-freshness commands. Mirrors the
 * pattern of `$lib/api/client.ts` so call sites stay uniform.
 */
export const images = {
  /** Force a check for one image reference (e.g. `nginx:1.27`). */
  check: (imageRef: string) =>
    invoke<ImageCacheEntry>("check_image", { imageRef }),
  /** Read the whole on-disk cache map (`imageRef → entry`). */
  readCache: () => invoke<Record<string, ImageCacheEntry>>("read_image_cache"),
};
