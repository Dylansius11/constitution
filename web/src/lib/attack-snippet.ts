import type { DemoAttackSource } from "./demo-config.ts";

/**
 * The few lines an attack adds to the current implementation, shown on its card. Computed at build
 * time from the sealed source itself, so the snippet can never disagree with what is proposed.
 */

const NOISE = /^(?:\/\/|\/\*|\*|pragma\b|import\b|\/\/ SPDX|$)/;

function meaningful(line: string): string | null {
  const text = line.trim();
  return text === "" || NOISE.test(text) ? null : text;
}

/** Added lines (trimmed of indentation but not of structure), at most `max`, with an elision mark. */
export function changeSnippet(
  files: readonly DemoAttackSource[],
  contract: string,
  max = 14,
): string[] {
  const selected = contract.split(":")[0];
  const changed = files.find((file) => file.path === selected);
  if (changed === undefined) return [];
  const baseline = new Map<string, number>();
  for (const file of files) {
    if (file === changed || !/VaultV1\.sol$/.test(file.path)) continue;
    for (const line of file.content.split("\n")) {
      const text = meaningful(line);
      if (text !== null) baseline.set(text, (baseline.get(text) ?? 0) + 1);
    }
  }
  const added: string[] = [];
  for (const line of changed.content.split("\n")) {
    const text = meaningful(line);
    if (text === null) continue;
    const seen = baseline.get(text) ?? 0;
    if (seen > 0) baseline.set(text, seen - 1);
    else added.push(line.replace(/\s+$/, ""));
  }
  if (added.length <= max) return added;
  return [...added.slice(0, max), "…"];
}
