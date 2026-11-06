import { describe, expect, it } from "vitest";
import raw from "../../rules/manifest.json";
import { manifestFacts, parseManifest } from "../src/data/manifest.ts";

describe("committed manifest facts", () => {
  it("projects the real committed compiler, bounds and AI policy", () => {
    expect(manifestFacts).toEqual({
      solc: "0.8.38",
      evm_version: "prague",
      tools: {
        foundry: "1.8.6",
        halmos: "0.3.3",
        upgrades_core: "1.47.0",
        z3: "4.12.6.0",
        yices: "2.6.4",
      },
      r2: {
        invariant_depth: 4,
        loop_bound: 2,
        fuzz_runs: 2000,
        fuzz_depth: 128,
        solver_timeout_assertion_s: 10,
      },
      r3: { forbidden_opcodes: ["SELFDESTRUCT", "CALLCODE", "DELEGATECALL"] },
      r4: { allow_empty: true, allowlist: ["0x5b4e128c"] },
      r5: { max_txs: 7 },
      ai: {
        required: true,
        model: "z-ai/glm-5.3-flash",
        signature_kind: "provider_tee",
        max_candidates: 4,
        timeout_s: 300,
      },
      max_cert_ttl_s: 300,
    });
    expect(parseManifest(raw)).toEqual(manifestFacts);
  });
  it.each(["solc", "evm_version", "tools", "r2", "r3", "r4", "r5", "ai", "max_cert_ttl_s"])(
    "fails closed when %s is missing",
    (key) => {
      const malformed = structuredClone(raw) as Record<string, unknown>;
      delete malformed[key];
      expect(() => parseManifest(malformed)).toThrow(/rules\/manifest.json/);
    },
  );
  it("rejects missing nested fields, unknown opcodes, invalid bounds and selector lengths", () => {
    expect(() => parseManifest({ ...raw, r2: {} })).toThrow(/invariant_depth/);
    expect(() => parseManifest({ ...raw, tools: {} })).toThrow(/foundry/);
    expect(() => parseManifest({ ...raw, r3: { forbidden_opcodes: [42] } })).toThrow(/opcode/);
    expect(() => parseManifest({ ...raw, r2: { ...raw.r2, fuzz_runs: -1 } })).toThrow(/fuzz_runs/);
    expect(() =>
      parseManifest({ ...raw, r4: { allow_empty: true, allowlist: [{ selector: "0x01" }] } }),
    ).toThrow(/selector/);
    expect(() => parseManifest({ ...raw, ai: { ...raw.ai, required: "true" } })).toThrow(
      /required/,
    );
  });
});
