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
  }: {
    dockerComposeRaw?: string;
    imageRef?: string;
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
    rows.filter((r) => imageCache.isStale(r.ref) === "newer-available").length,
  );

  function shortDigest(d: string | undefined): string {
    if (!d) return "—";
    // strip `sha256:` prefix if present, then first 12 hex chars.
    const hex = d.startsWith("sha256:") ? d.slice(7) : d;
    return hex.slice(0, 12);
  }

  function badgeFor(state: StaleState): { label: string; class: string } {
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
        {#if staleCount === 0}
          All {rows.length} image{rows.length === 1 ? "" : "s"} up to date.
        {:else}
          {staleCount} of {rows.length} image{rows.length === 1 ? "" : "s"} have a newer version available.
        {/if}
      </div>
      <Button size="sm" variant="outline" onclick={checkAll}>
        Check all images on this resource
      </Button>
    </div>

    <div class="rounded-md border border-border">
      <table class="w-full text-sm">
        <thead class="bg-muted/40 text-xs text-muted-foreground">
          <tr>
            <th class="px-3 py-2 text-left font-medium">Service</th>
            <th class="px-3 py-2 text-left font-medium">Image</th>
            <th class="px-3 py-2 text-left font-medium">Tag</th>
            <th class="px-3 py-2 text-left font-mono">Digest</th>
            <th class="px-3 py-2 text-left font-mono">Latest</th>
            <th class="px-3 py-2 text-left font-medium">State</th>
            <th class="px-3 py-2 text-right font-medium">Action</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as row (row.ref)}
            {@const entry = imageCache.entries[row.ref]}
            {@const state = imageCache.isStale(row.ref)}
            {@const view = badgeFor(state)}
            {@const isChecking = imageCache.checking.has(row.ref)}
            <tr class="border-t border-border">
              <td class="px-3 py-2">{row.service}</td>
              <td class="px-3 py-2 font-mono text-xs">{row.image}</td>
              <td class="px-3 py-2 font-mono text-xs">{row.tag}</td>
              <td class="px-3 py-2 font-mono text-xs" title={entry?.digest ?? ""}>
                {shortDigest(entry?.digest)}
              </td>
              <td
                class="px-3 py-2 font-mono text-xs"
                title={entry?.latest_digest ?? ""}
              >
                {shortDigest(entry?.latest_digest)}
              </td>
              <td class="px-3 py-2">
                <Badge variant="default" class={view.class}>
                  {view.label}
                </Badge>
              </td>
              <td class="px-3 py-2 text-right">
                <Button
                  size="xs"
                  variant="outline"
                  disabled={isChecking}
                  onclick={() => imageCache.check(row.ref)}
                >
                  {isChecking ? "Checking…" : "Check now"}
                </Button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}
