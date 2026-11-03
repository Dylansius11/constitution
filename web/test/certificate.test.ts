import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { base64ToBytes, bytesToBase64, hexToBytes } from "../src/lib/bytes.ts";
import {
  aiSignatureKindName,
  type Certificate,
  certificateHash,
  decodeCertificate,
} from "../src/lib/certificate.ts";
import { WireError } from "../src/lib/errors.ts";
import type { JsonValue } from "../src/lib/json.ts";
import { certificateVectors } from "./support/fixtures.ts";

/** serde keeps u64 exact in JSON; the page parser returns a number when safe and a bigint otherwise. */
function u64(value: bigint): JsonValue {
  return value <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(value) : value;
}

/** Rebuild the Rust serde form from the page's decoded certificate, so every field is compared. */
function serdeShape(c: Certificate): JsonValue {
  return {
    domain: Array.from(hexToBytes(c.domain, "domain")),
    gate: c.gate,
    proposal_id: u64(c.proposal_id),
    attempt: c.attempt,
    chain_id: u64(c.chain_id),
    proxy: c.proxy,
    executor: c.executor,
    current_impl: c.current_impl,
    current_impl_codehash: c.current_impl_codehash,
    new_impl: c.new_impl,
    new_impl_codehash: c.new_impl_codehash,
    init_calldata_hash: c.init_calldata_hash,
    source_commitment: c.source_commitment,
    rule_set_hash: c.rule_set_hash,
    verifier_pubkey: bytesToBase64(hexToBytes(c.verifier_pubkey, "verifier_pubkey")),
    verifier_release_hash: c.verifier_release_hash,
    evidence_block_number: u64(c.evidence_block_number),
    evidence_block_hash: c.evidence_block_hash,
    evidence_root: c.evidence_root,
    ai:
      c.ai.kind === "NotRun"
        ? "NotRun"
        : c.ai.kind === "Clean"
          ? { Clean: { evidence_hash: c.ai.evidence_hash, signature_kind: c.ai.signature_kind } }
          : {
              Vetoed: {
                evidence_hash: c.ai.evidence_hash,
                counterexample_hash: c.ai.counterexample_hash,
              },
            },
    verdict:
      c.verdict.kind === "Pass"
        ? "Pass"
        : { Reject: { rule_id: c.verdict.rule_id, reason_code: c.verdict.reason_code } },
    issued_at_s: u64(c.issued_at_s),
    expires_at_s: u64(c.expires_at_s),
  };
}

const { vectors, invalid } = certificateVectors();

describe("T-ENC-2 certificate vectors (Rust Borsh, decoded independently)", () => {
  it("covers Pass, Reject and every AI variant", () => {
    const decoded = vectors.map((v) => decodeCertificate(base64ToBytes(v.certificate_b64, v.name)));
    expect(vectors.length).toBeGreaterThanOrEqual(3);
    expect(decoded.some((c) => c.verdict.kind === "Pass")).toBe(true);
    expect(decoded.some((c) => c.verdict.kind === "Reject")).toBe(true);
    expect(new Set(decoded.map((c) => c.ai.kind))).toEqual(new Set(["Clean", "Vetoed", "NotRun"]));
  });

  for (const vector of vectors) {
    it(`${vector.name}: every decoded field and the hash match Rust`, async () => {
      const bytes = base64ToBytes(vector.certificate_b64, "certificate_b64");
      expect(serdeShape(decodeCertificate(bytes))).toEqual(vector.decoded);
      expect(await certificateHash(bytes)).toBe(vector.certificate_hash);
      // A second implementation of SHA-256, so the hash check does not rest on WebCrypto alone.
      expect(`0x${createHash("sha256").update(bytes).digest("hex")}`).toBe(vector.certificate_hash);
    });
  }

  it("preserves u64 values above 2^53 exactly", () => {
    const max = vectors.find((v) => v.name === "pass_ai_clean_gateway_boundary_values");
    if (max === undefined) throw new Error("boundary vector missing");
    const c = decodeCertificate(base64ToBytes(max.certificate_b64, "x"));
    expect(c.proposal_id).toBe(18_446_744_073_709_551_615n);
    expect(c.attempt).toBe(4_294_967_295);
    expect(c.gate).toBe("a".repeat(64));
  });

  for (const vector of invalid) {
    it(`refuses ${vector.name} (${vector.why})`, () => {
      expect(() => decodeCertificate(base64ToBytes(vector.bytes_b64, "bytes_b64"))).toThrow(
        WireError,
      );
    });
  }

  it("refuses every proper prefix of every valid certificate", () => {
    for (const vector of vectors) {
      const bytes = base64ToBytes(vector.certificate_b64, vector.name);
      for (let length = 0; length < bytes.length; length += 1) {
        expect(
          () => decodeCertificate(bytes.subarray(0, length)),
          `${vector.name}[0..${length}]`,
        ).toThrow(WireError);
      }
    }
  });

  it("never throws anything but WireError on arbitrary single-byte corruption", () => {
    for (const vector of vectors) {
      const bytes = base64ToBytes(vector.certificate_b64, vector.name);
      for (let at = 0; at < bytes.length; at += 1) {
        const corrupted = bytes.slice();
        corrupted[at] = (corrupted[at] ?? 0) ^ 0xff;
        try {
          decodeCertificate(corrupted);
        } catch (error) {
          expect(error, `${vector.name} byte ${at}`).toBeInstanceOf(WireError);
        }
      }
    }
  });

  it("decodes a view into a larger buffer", () => {
    const first = vectors[0];
    if (first === undefined) throw new Error("no vectors");
    const bytes = base64ToBytes(first.certificate_b64, "x");
    const padded = new Uint8Array(bytes.length + 10);
    padded.set(bytes, 5);
    expect(decodeCertificate(padded.subarray(5, 5 + bytes.length))).toEqual(
      decodeCertificate(bytes),
    );
  });

  it("names the AI signature kinds the verifier writes", () => {
    expect(aiSignatureKindName(1)).toBe("provider_tee");
    expect(aiSignatureKindName(2)).toBe("gateway");
    expect(aiSignatureKindName(0)).toBeNull();
    expect(aiSignatureKindName(9)).toBeNull();
  });
});
