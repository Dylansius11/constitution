import { readFileSync } from "node:fs";
import type { ProxyRead } from "../../src/lib/base.ts";
import type { EvmTx, Proposal } from "../../src/lib/gate-types.ts";
import { type JsonValue, parseJson } from "../../src/lib/json.ts";
import type { VerifierRun } from "../../src/lib/verifier.ts";

/** Repeat one byte `length` times as lowercase 0x hex. */
export function hex(byte: number, length: number): string {
  return `0x${byte.toString(16).padStart(2, "0").repeat(length)}`;
}

export const OLD_IMPL = hex(0x91, 20);
export const NEW_IMPL = hex(0x4a, 20);
export const PROXY = hex(0x11, 20);

/** `signed_tx` bytes whose keccak-256 is a published constant: keccak256(0x00). */
export const SIGNED_TX = "0x00";
export const SIGNED_TX_HASH = "0xbc36789e7a1e281436464229828f817d6612f7b477d66591ff96a9e064bcc98a";

export function evmTx(over: Partial<EvmTx> = {}): EvmTx {
  return {
    nonce: 3,
    deadline_s: 1_793_701_800,
    gas_limit: 300_000,
    max_fee_per_gas: 1_009_731_712n,
    max_priority_fee_per_gas: 1_000_000n,
    signing_hash: hex(0x5e, 32),
    signed_tx: null,
    attempts: 1,
    ...over,
  };
}

export function proposal(over: Partial<Proposal> = {}): Proposal {
  return {
    id: 7,
    proposer: "demo.testnet",
    created_at_ns: 1_793_700_000_123_456_789n,
    expires_at_ns: 1_793_701_800_123_456_789n,
    rule_set_hash: hex(0xa1, 32),
    new_impl: NEW_IMPL,
    new_impl_codehash: hex(0xc0, 32),
    expected_current_impl: OLD_IMPL,
    init_calldata: "0x",
    source_commitment: hex(0xd0, 32),
    reserved: 1_000_000_000_000_000_000_000_000n,
    state: "Pending",
    certificate_hash: null,
    rejection: null,
    evm: null,
    ...over,
  };
}

export function run(over: Partial<VerifierRun> = {}): VerifierRun {
  return {
    stage: "queued",
    started_at: 1_793_700_005,
    stages: [],
    outcome: null,
    verdict: null,
    certificate_b64: null,
    signature_b64: null,
    near_tx_hash: null,
    evidence_root: null,
    elapsed_ms: null,
    error_code: null,
    ...over,
  };
}

export function proxyRead(implementation: string, block: number): ProxyRead {
  return { implementation, block, block_hash: hex(0xb0, 32) };
}

interface CertificateVector {
  name: string;
  certificate_b64: string;
  certificate_hash: string;
  decoded: JsonValue;
}

interface InvalidVector {
  name: string;
  why: string;
  bytes_b64: string;
}

/** vectors/certificate.json, parsed losslessly because its u64 fields exceed 2^53. */
export function certificateVectors(): { vectors: CertificateVector[]; invalid: InvalidVector[] } {
  const text = readFileSync(new URL("../../../vectors/certificate.json", import.meta.url), "utf8");
  return parseJson(text) as unknown as { vectors: CertificateVector[]; invalid: InvalidVector[] };
}

export function vectorByName(name: string): CertificateVector {
  const found = certificateVectors().vectors.find((v) => v.name === name);
  if (found === undefined)
    throw new Error(`vector ${name} is missing from vectors/certificate.json`);
  return found;
}
