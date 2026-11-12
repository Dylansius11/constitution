import type { SourceBundle } from "./bundle.ts";
import { type DemoAttack, type DemoGallery, parseDemoGallery } from "./demo-gallery.ts";
import { isAccountId, Obj } from "./wire.ts";

/**
 * Build-time resolution of the live demo gate. `/app` is runnable only when the gate account, the
 * verifier URL and `deployments/demo-gallery.json` all exist and agree. Anything missing or
 * inconsistent yields an explicit unavailable state naming what is missing; there is no fixture
 * fallback and nothing here is ever shown as live.
 */

export interface DemoAttackSource {
  path: string;
  content: string;
}

export interface DemoAttackConfig extends DemoAttack {
  /** The gallery's files, read from `contracts/` at build time, sorted by path. */
  files: readonly DemoAttackSource[];
}

export interface DemoConfig {
  gate: string;
  proxy: string;
  verifierUrl: string;
  nearRpc: string;
  baseRpc: string;
  attacks: readonly DemoAttackConfig[];
  upgrades: DemoGallery["upgrades"];
}

export type DemoState =
  | { status: "ready"; config: DemoConfig }
  | {
      status: "unavailable";
      /** What is absent: env variable names and repository files. Empty when something is invalid. */
      missing: readonly string[];
      /** Why present inputs were refused; `null` when only something is missing. */
      invalid: string | null;
    };

export interface DemoInputs {
  env: Readonly<Record<string, string | undefined>>;
  nearRpc: string;
  baseRpc: string;
  /** Parsed `deployments/demo-gallery.json`, or `null` when the file does not exist. */
  gallery: unknown | null;
  /** Parsed `deployments/demo.json`, or `null`. Optional; when present its gate must agree. */
  deployment: unknown | null;
  /** `contracts/` files by relative path, for example `src/demo/VaultV1.sol`. */
  sources: Readonly<Record<string, string>>;
}

function httpUrl(value: string): string | null {
  try {
    const url = new URL(value);
    if (!["http:", "https:"].includes(url.protocol) || url.username || url.password || url.hash)
      return null;
    return url.toString().replace(/\/$/, "");
  } catch {
    return null;
  }
}

export function resolveDemoState(input: DemoInputs): DemoState {
  const gate = input.env["PUBLIC_DEMO_GATE_ACCOUNT"]?.trim() || undefined;
  const verifier = input.env["PUBLIC_DEMO_VERIFIER_URL"]?.trim() || undefined;
  const missing: string[] = [];
  if (gate === undefined) missing.push("PUBLIC_DEMO_GATE_ACCOUNT");
  if (verifier === undefined) missing.push("PUBLIC_DEMO_VERIFIER_URL");
  if (input.gallery === null) missing.push("deployments/demo-gallery.json");
  if (missing.length > 0) return { status: "unavailable", missing, invalid: null };
  const invalid = (reason: string): DemoState => ({ status: "unavailable", missing: [], invalid: reason });

  if (gate === undefined || verifier === undefined) return invalid("incomplete configuration");
  if (!isAccountId(gate)) return invalid("PUBLIC_DEMO_GATE_ACCOUNT is not a NEAR account id");
  const verifierUrl = httpUrl(verifier);
  if (verifierUrl === null) return invalid("PUBLIC_DEMO_VERIFIER_URL is not a plain http(s) URL");

  let gallery: DemoGallery;
  try {
    gallery = parseDemoGallery(input.gallery);
  } catch (error) {
    return invalid(error instanceof Error ? error.message : "demo-gallery.json is malformed");
  }
  if (gallery.gate !== gate)
    return invalid(`demo-gallery.json is for ${gallery.gate}, but this build targets ${gate}`);
  if (input.deployment !== null) {
    try {
      const deployed = new Obj(input.deployment, "demo.json").str("gate");
      if (deployed !== gate) return invalid(`demo.json is for ${deployed}, but this build targets ${gate}`);
    } catch (error) {
      return invalid(error instanceof Error ? error.message : "demo.json is malformed");
    }
  }

  const attacks: DemoAttackConfig[] = [];
  for (const attack of gallery.attacks) {
    const files: DemoAttackSource[] = [];
    for (const path of [...attack.sourceFiles].sort()) {
      const content = input.sources[path];
      if (content === undefined) return invalid(`contracts/${path} is listed by the gallery but missing`);
      files.push({ path, content });
    }
    attacks.push({ ...attack, files });
  }
  return {
    status: "ready",
    config: {
      gate,
      proxy: gallery.proxy,
      verifierUrl,
      nearRpc: input.nearRpc,
      baseRpc: input.baseRpc,
      attacks,
      upgrades: gallery.upgrades,
    },
  };
}

/** The exact bundle the CLI would make for this attack: sorted paths, default profile. */
export function attackBundle(attack: Pick<DemoAttackConfig, "contract" | "files">): SourceBundle {
  const encoder = new TextEncoder();
  return {
    contract_name: attack.contract,
    foundry_profile: "default",
    files: attack.files.map(({ path, content }) => [path, encoder.encode(content)] as const),
  };
}
