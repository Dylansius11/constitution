import raw from "../../../rules/manifest.json";
import { WireError } from "../lib/errors.ts";
import { asHexFixed, asSafeUint, Obj } from "../lib/wire.ts";

/** Build-time projection, not a claim about an activated gate's current rule set. */
export interface ManifestFacts {
  solc: string;
  evm_version: string;
  tools: { foundry: string; halmos: string; upgrades_core: string; z3: string; yices: string };
  r2: {
    invariant_depth: number;
    loop_bound: number;
    fuzz_runs: number;
    fuzz_depth: number;
    solver_timeout_assertion_s: number;
  };
  r3: { forbidden_opcodes: string[] };
  r4: { allow_empty: boolean; allowlist: string[] };
  r5: { max_txs: number };
  ai: {
    required: boolean;
    model: string;
    signature_kind: string;
    max_candidates: number;
    timeout_s: number;
  };
  max_cert_ttl_s: number;
}

export function parseManifest(value: unknown): ManifestFacts {
  const o = new Obj(value, "rules/manifest.json");
  const tools = o.obj("tools");
  const r2 = o.obj("r2");
  const r3 = o.obj("r3");
  const r4 = o.obj("r4");
  const ai = o.obj("ai");
  const opcodeNames: Record<number, string> = {
    255: "SELFDESTRUCT",
    242: "CALLCODE",
    244: "DELEGATECALL",
  };
  return {
    solc: o.str("solc"),
    evm_version: o.str("evm_version"),
    tools: {
      foundry: tools.str("foundry"),
      halmos: tools.str("halmos"),
      upgrades_core: tools.str("upgrades_core"),
      z3: tools.str("z3"),
      yices: tools.str("yices"),
    },
    r2: {
      invariant_depth: r2.safe("invariant_depth"),
      loop_bound: r2.safe("loop_bound"),
      fuzz_runs: r2.safe("fuzz_runs"),
      fuzz_depth: r2.safe("fuzz_depth"),
      solver_timeout_assertion_s: r2.safe("solver_timeout_assertion_s"),
    },
    r3: {
      forbidden_opcodes: r3.arr("forbidden_opcodes").map((v, i) => {
        const path = `${r3.at("forbidden_opcodes")}[${i}]`;
        const name = opcodeNames[asSafeUint(v, path, 255)];
        if (name === undefined) throw new WireError(path, "unrecognized forbidden opcode");
        return name;
      }),
    },
    r4: {
      allow_empty: r4.bool("allow_empty"),
      allowlist: r4.arr("allowlist").map((v, i) => {
        const entry = new Obj(v, `${r4.at("allowlist")}[${i}]`);
        return asHexFixed(entry.raw("selector"), 4, entry.at("selector"));
      }),
    },
    r5: { max_txs: o.obj("r5").safe("max_txs") },
    ai: {
      required: ai.bool("required"),
      model: ai.str("model"),
      signature_kind: ai.str("signature_kind"),
      max_candidates: ai.safe("max_candidates"),
      timeout_s: ai.safe("timeout_s"),
    },
    max_cert_ttl_s: o.safe("max_cert_ttl_s"),
  };
}

export const manifestFacts = parseManifest(raw);
