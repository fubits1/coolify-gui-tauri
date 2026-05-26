<!--
@component
ImagesTab — per-resource image freshness rows.

Renders one row per image reference declared by the selected resource:
- Compose-based resources (Application with compose / Service) → parsed
  via `parseComposeImages(dockerComposeRaw)`.
- Plain Application/Database → single row driven by `imageRef`.
- Neither → empty state.

Each row shows: image name · current tag · current digest (short) · latest
digest (short) · state badge · "Check now" button (spinner while in flight).

The tab header summarises stale count and exposes "Check all images on
this resource" which delegates to `imageCache.checkMany`.

Props:
- `dockerComposeRaw?: string` — raw docker-compose YAML (if any).
- `imageRef?: string` — single-image ref (if any).
-->
<script lang="ts">
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { imageCache, type StaleState } from "$lib/stores/image-cache.svelte";
  import { parseComposeImages, type ImageRef } from "$lib/util/compose";

  let {
    dockerComposeRaw,
    imageRef,
    lastDeployedAt = null,
  }: {
    dockerComposeRaw?: string;
    imageRef?: string;
    /** Resource's last-deploy timestamp; lets isStale do publish-time
     *  drift checks for `:latest` tags. */
    lastDeployedAt?: string | null;
  } = $props();

  /**
   * Normalised list of {service, image, tag, ref} for rendering. `ref` is
   * the canonical key (`name:tag`) used by the cache + Rust commands.
   */
  type Row = ImageRef & { ref: string };

  const rows: Row[] = $derived.by(() => {
    if (dockerComposeRaw && dockerComposeRaw.trim().length > 0) {
      return parseComposeImages(dockerComposeRaw).map((r) => ({
        ...r,
        ref: `${r.image}:${r.tag}`,
      }));
    }
    if (imageRef && imageRef.trim().length > 0) {
      const { name, tag } = splitImage(imageRef);
      return [{ service: name, image: name, tag, ref: imageRef }];
    }
    return [];
  });

  const staleCount = $derived(
    rows.filter((r) => imageCache.isStale(r.ref, lastDeployedAt) === "newer-available").length,
  );
  const unknownCount = $derived(
    rows.filter((r) => imageCache.isStale(r.ref, lastDeployedAt) === "unknown").length,
  );
  const freshCount = $derived(rows.length - staleCount - unknownCount);

  function shortDigest(d: string | undefined): string {
    if (!d) return "—";
    // strip `sha256:` prefix if present, then first 12 hex chars.
    const hex = d.startsWith("sha256:") ? d.slice(7) : d;
    return hex.slice(0, 12);
  }

  function badgeFor(
    state: StaleState,
    tag: string,
  ): { label: string; class: string; title?: string } {
    switch (state) {
      case "fresh":
        return {
          label: "fresh",
          class: "bg-green-600/20 text-green-400 border-green-600/30",
        };
      case "newer-available":
        return {
          label: "newer available",
          class: "bg-amber-600/20 text-amber-400 border-amber-600/30",
        };
      case "unknown":
        if (tag === "latest") {
          return {
            label: "unchecked (:latest)",
            class: "",
            title:
              "Image pinned to :latest. Click Check now — we'll compare the registry's :latest publish time against this resource's last deploy.",
          };
        }
        return { label: "unchecked", class: "" };
    }
  }

  function splitImage(ref: string): { name: string; tag: string } {
    const atIdx = ref.indexOf("@");
    if (atIdx !== -1) return { name: ref.slice(0, atIdx), tag: ref.slice(atIdx + 1) };
    const lastColon = ref.lastIndexOf(":");
    if (lastColon === -1) return { name: ref, tag: "latest" };
    const candidate = ref.slice(lastColon + 1);
    if (candidate.includes("/")) return { name: ref, tag: "latest" };
    return { name: ref.slice(0, lastColon), tag: candidate };
  }

  function checkAll() {
    void imageCache.checkMany(rows.map((r) => r.ref));
  }
</script>

{#if rows.length === 0}
  <div class="p-6 text-sm text-muted-foreground">
    No image references found for this resource.
  </div>
{:else}
  <div class="flex flex-col gap-3 p-4">
    <div class="flex items-center justify-between">
      <div class="text-sm text-muted-foreground">
        {#if staleCount > 0}
          {staleCount} of {rows.length} image{rows.length === 1 ? " has" : "s have"} a newer version available.
        {:else if unknownCount === rows.length}
          {rows.length === 1 ? "Image" : "All images"} pinned to a floating tag ({rows.length === 1 ? ":latest" : "e.g. :latest"}) — drift can't be determined.
        {:else if unknownCount > 0}
          {freshCount} up to date · {unknownCount} with unknown drift (floating tag).
        {:else}
          All {rows.length} image{rows.length === 1 ? "" : "s"} up to date.
        {/if}
      </div>
      <Button size="sm" variant="outline" onclick={checkAll}>
        Check all images on this resource
      </Button>
    </div>

    <div class="flex flex-col gap-2">
      {#each rows as row (row.ref)}
        {@const entry = imageCache.entries[row.ref]}
        {@const state = imageCache.isStale(row.ref, lastDeployedAt)}
        {@const view = badgeFor(state, row.tag)}
        {@const isChecking = imageCache.checking.has(row.ref)}
        {@const latestTag =
          entry?.highest_semver_tag && entry.highest_semver_tag !== row.tag
            ? entry.highest_semver_tag
            : null}
        <div
          class="flex flex-col gap-1.5 rounded-md border border-border bg-muted/10 px-3 py-2.5"
        >
          <!-- Row 1: service name + full image:tag -->
          <div class="flex items-baseline justify-between gap-2">
            <span class="text-xs font-semibold">{row.service}</span>
            <span class="font-mono text-xs text-muted-foreground truncate" title={row.ref}>
              {row.image}:{row.tag}
            </span>
          </div>

          <!-- Row 2: current digest → latest digest / latest tag -->
          <div class="flex items-center justify-between gap-2 text-[0.7rem] font-mono text-muted-foreground">
            <span title={entry?.digest ?? "not checked"}>
              current: {shortDigest(entry?.digest)}
            </span>
            <span title={entry?.latest_digest ?? "not checked"}>
              latest: {shortDigest(entry?.latest_digest)}
              {#if latestTag}
                <span class="text-amber-400"> ({latestTag})</span>
              {/if}
            </span>
          </div>

          <!-- Row 3: state badge + check-now -->
          <div class="flex items-center justify-between gap-2">
            <Badge variant="default" class={view.class} title={view.title ?? ""}>
              {view.label}
            </Badge>
            <Button
              size="xs"
              variant="outline"
              disabled={isChecking}
              onclick={() => imageCache.check(row.ref)}
            >
              {isChecking ? "Checking…" : "Check now"}
            </Button>
          </div>
        </div>
      {/each}
    </div>
  </div>
{/if}
