/**
 * Page configuration from `PUBLIC_*` build variables (DESIGN 5). Every value is optional: a missing
 * gate account means the live gate surfaces render `Awaiting activation`, never a sample value.
 */

function optional(value: string | undefined): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function optionalUrl(name: string, value: string | undefined): string | null {
  const raw = optional(value);
  if (raw === null) return null;
  const url = new URL(raw);
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error(`${name} must be an http(s) URL`);
  }
  return url.toString().replace(/\/$/, "");
}

const env = import.meta.env;

export const site = {
  title: "Constitution: upgrades only through the rules",
  description:
    "A keyless NEAR gate holds the upgrade authority of one Base Sepolia proxy and signs an upgrade only when an attested verifier certifies it against owner-committed rules.",
  /** Gate account of the activated production deployment. `null` until activation. */
  gateAccount: optional(env.PUBLIC_GATE_ACCOUNT),
  nearRpc: optionalUrl("PUBLIC_NEAR_RPC", env.PUBLIC_NEAR_RPC) ?? "https://test.rpc.fastnear.com",
  verifierUrl: optionalUrl("PUBLIC_VERIFIER_URL", env.PUBLIC_VERIFIER_URL),
  baseRpc: optionalUrl("PUBLIC_BASE_RPC", env.PUBLIC_BASE_RPC) ?? "https://sepolia.base.org",
  baseExplorer: "https://sepolia.basescan.org",
  nearExplorer: "https://testnet.nearblocks.io",
  repoUrl: optionalUrl("PUBLIC_REPO_URL", env.PUBLIC_REPO_URL),
  xPostUrl: optionalUrl("PUBLIC_X_POST_URL", env.PUBLIC_X_POST_URL),
  builtAt: new Date(),
} as const;
