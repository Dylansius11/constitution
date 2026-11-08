/**
 * Display rules for hashes, addresses and timestamps (DESIGN 7.5 and 6.1 rule 6).
 * Pure string functions shared by static components and live islands.
 */

export type HashKind = "hash" | "address" | "near";

/** Truncation for one breakpoint: characters kept after `0x` at the start, and at the end. */
const RULES: Record<Exclude<HashKind, "near">, { narrow: [number, number]; wide: [number, number] }> =
  {
    hash: { narrow: [6, 4], wide: [10, 8] },
    address: { narrow: [4, 4], wide: [6, 6] },
  };

function middle(value: string, head: number, tail: number): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

/** Shortened form at one width; the head count excludes the `0x` prefix. NEAR ids are kept whole up to 32 characters. */
export function shortValue(value: string, kind: HashKind, width: "narrow" | "wide"): string {
  if (kind === "near") return value.length <= 32 ? value : middle(value, 14, 14);
  const [head, tail] = RULES[kind][width];
  return middle(value, head + 2, tail);
}

/** Accessible name: "Transaction hash, starts 0x3fa9, ends e21e" instead of 66 characters aloud. */
export function spokenValue(label: string, value: string): string {
  if (value.length <= 16) return `${label}, ${value}`;
  const start = value.startsWith("0x") ? value.slice(0, 6) : value.slice(0, 4);
  return `${label}, starts ${start}, ends ${value.slice(-4)}`;
}

const pad = (n: number): string => String(n).padStart(2, "0");

/** `2026-11-03 03:04:47 UTC` from epoch milliseconds. */
export function utcStamp(ms: number): string {
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) throw new RangeError("invalid timestamp");
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ${pad(
    d.getUTCHours(),
  )}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())} UTC`;
}

/** `2026-11-03` from epoch milliseconds, UTC. */
export function utcDate(ms: number): string {
  return utcStamp(ms).slice(0, 10);
}

/** Remaining time as `6 d 23 h 41 m`; zero or negative becomes `0 d 0 h 0 m`. No seconds: it is not a countdown show. */
export function remaining(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 60_000));
  const days = Math.floor(total / 1440);
  const hours = Math.floor((total % 1440) / 60);
  const minutes = total % 60;
  return `${days} d ${hours} h ${minutes} m`;
}
