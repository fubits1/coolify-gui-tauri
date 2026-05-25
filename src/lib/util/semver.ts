/**
 * Pure semver helpers. No deps.
 *
 * Supports the subset Coolify release tags actually use:
 *   - optional leading `v`
 *   - MAJOR.MINOR.PATCH
 *   - optional `-<prerelease>` suffix (anything after the dash, dot-separated)
 *
 * Non-matching tags are skipped, never thrown on.
 */

export interface ParsedSemver {
  major: number;
  minor: number;
  patch: number;
  pre?: string;
  raw: string;
}

const SEMVER_RE = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;

export function parseSemver(tag: string): ParsedSemver | null {
  const m = tag.trim().match(SEMVER_RE);
  if (!m) return null;
  const [, maj, min, pat, pre] = m;
  return {
    major: Number(maj),
    minor: Number(min),
    patch: Number(pat),
    pre: pre ?? undefined,
    raw: tag,
  };
}

/**
 * Returns negative if `a < b`, positive if `a > b`, 0 if equal.
 *
 * Prerelease ordering rules (simplified semver):
 *   - A version without prerelease ranks higher than one with prerelease.
 *   - Otherwise prerelease strings compare by dot-separated identifiers
 *     (numeric identifiers compared numerically).
 */
export function compareSemver(a: ParsedSemver, b: ParsedSemver): number {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  if (a.patch !== b.patch) return a.patch - b.patch;

  if (a.pre === undefined && b.pre === undefined) return 0;
  if (a.pre === undefined) return 1;
  if (b.pre === undefined) return -1;

  const aParts = a.pre.split(".");
  const bParts = b.pre.split(".");
  const len = Math.max(aParts.length, bParts.length);
  for (let i = 0; i < len; i++) {
    const ap = aParts[i];
    const bp = bParts[i];
    if (ap === undefined) return -1;
    if (bp === undefined) return 1;
    const an = /^\d+$/.test(ap) ? Number(ap) : null;
    const bn = /^\d+$/.test(bp) ? Number(bp) : null;
    if (an !== null && bn !== null) {
      if (an !== bn) return an - bn;
    } else if (an !== null) {
      return -1;
    } else if (bn !== null) {
      return 1;
    } else if (ap !== bp) {
      return ap < bp ? -1 : 1;
    }
  }
  return 0;
}

/**
 * Pick the highest valid semver tag from a list, returning its original `raw` form.
 * Non-semver tags are silently dropped. Returns `null` if no valid tag is found.
 */
export function pickHighestSemverTag(tags: string[]): string | null {
  const parsed = tags
    .map(parseSemver)
    .filter((p): p is ParsedSemver => p !== null);
  if (parsed.length === 0) return null;
  parsed.sort((a, b) => compareSemver(b, a));
  return parsed[0].raw;
}

// example: parseSemver('v4.0.0-beta.391')      yields { major: 4, minor: 0, patch: 0, pre: 'beta.391', raw: 'v4.0.0-beta.391' }
// example: parseSemver('4.0.0')                yields { major: 4, minor: 0, patch: 0, pre: undefined, raw: '4.0.0' }
// example: parseSemver('latest')               yields null
// example: compareSemver(parseSemver('v4.0.0')!, parseSemver('v4.0.0-beta.391')!) is positive  (release beats prerelease)
// example: compareSemver(parseSemver('v4.0.1')!, parseSemver('v4.0.0')!)          is positive
// example: pickHighestSemverTag(['v3.12.0', 'v4.0.0-beta.391', 'v4.0.0', 'latest']) yields 'v4.0.0'
