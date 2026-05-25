import YAML from "yaml";

/**
 * One image reference extracted from a docker-compose file.
 * `tag` defaults to `'latest'` when no tag is present in the image string.
 */
export interface ImageRef {
  service: string;
  image: string;
  tag: string;
}

/**
 * Parse a docker-compose YAML string and return one `ImageRef` per service
 * that declares an `image:` field.
 *
 * - Services without an `image` (e.g. pure `build:` services) are skipped.
 * - Image strings containing a digest (`name@sha256:...`) keep the digest as `tag`.
 * - Image strings with registry/port like `registry:5000/foo:1.2.3` split on the
 *   last colon so the port is not mistaken for a tag.
 * - On malformed YAML or non-object `services`, returns an empty array.
 */
export function parseComposeImages(raw: string): ImageRef[] {
  let doc: unknown;
  try {
    doc = YAML.parse(raw);
  } catch {
    return [];
  }
  if (!doc || typeof doc !== "object") return [];

  const services = (doc as Record<string, unknown>).services;
  if (!services || typeof services !== "object") return [];

  const out: ImageRef[] = [];
  for (const [service, def] of Object.entries(
    services as Record<string, unknown>,
  )) {
    if (!def || typeof def !== "object") continue;
    const image = (def as Record<string, unknown>).image;
    if (typeof image !== "string" || image.length === 0) continue;
    const { name, tag } = splitImage(image);
    out.push({ service, image: name, tag });
  }
  return out;
}

function splitImage(image: string): { name: string; tag: string } {
  // Digest form: name@sha256:abc...
  const atIdx = image.indexOf("@");
  if (atIdx !== -1) {
    return { name: image.slice(0, atIdx), tag: image.slice(atIdx + 1) };
  }
  // Split on the LAST colon, but only if what follows looks like a tag
  // (no slash). Otherwise the colon belongs to a registry port.
  const lastColon = image.lastIndexOf(":");
  if (lastColon === -1) {
    return { name: image, tag: "latest" };
  }
  const candidateTag = image.slice(lastColon + 1);
  if (candidateTag.includes("/")) {
    return { name: image, tag: "latest" };
  }
  return { name: image.slice(0, lastColon), tag: candidateTag };
}
