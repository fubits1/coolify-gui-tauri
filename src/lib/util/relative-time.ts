/**
 * Format an ISO date string as a short relative-time label
 * (e.g. `"2h ago"`, `"3d ago"`, `"just now"`). Returns `"—"` when the
 * input is `undefined`/empty, and an empty string for invalid dates.
 *
 * Granularity buckets: seconds, minutes, hours, days, weeks, months, years.
 * Always points to the past — future dates collapse to `"just now"` because
 * the only consumer (`last_deployed_at`) cannot legitimately be in the future.
 */
export function relativeTime(iso?: string): string {
  if (!iso) return "—";
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return "";

  const diffMs = Date.now() - then;
  if (diffMs < 0) return "just now";

  const sec = Math.floor(diffMs / 1000);
  if (sec < 45) return "just now";

  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;

  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;

  const day = Math.floor(hr / 24);
  if (day < 7) return `${day}d ago`;

  const week = Math.floor(day / 7);
  if (week < 5) return `${week}w ago`;

  const month = Math.floor(day / 30);
  if (month < 12) return `${month}mo ago`;

  const year = Math.floor(day / 365);
  return `${year}y ago`;
}
