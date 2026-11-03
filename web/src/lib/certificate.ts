import { bytesToHex, sha256 } from "./bytes.ts";
import { WireError } from "./errors.ts";
import { isAccountId } from "./wire.ts";

/**
 * Independent Borsh decoder for `constitution_types::Certificate`
 * (crates/constitution-types/src/certificate.rs). It re-derives the layout from the Rust field and
 * variant order and shares no code with Rust; vectors/certificate.json (T-ENC-2) pins agreement.
 *
 * Field order: domain[16], gate (u32 LE length + UTF-8), proposal_id u64, attempt u32, chain_id u64,
 * proxy[20], executor[20], current_impl[20], current_impl_codehash[32], new_impl[20],
 * new_impl_codehash[32], init_calldata_hash[32], source_commitment[32], rule_set_hash[32],
 * verifier_pubkey[32], verifier_release_hash[32], evidence_block_number u64, evidence_block_hash[32],
 * evidence_root[32], ai (u8 tag), verdict (u8 tag), issued_at_s u64, expires_at_s u64.
 * Every integer is little endian. Byte arrays are returned as lowercase `0x` hex.
 */

export type AiReview =
  | { kind: "Clean"; evidence_hash: string; signature_kind: number }
  | { kind: "Vetoed"; evidence_hash: string; counterexample_hash: string }
  | { kind: "NotRun" };

export type Verdict = { kind: "Pass" } | { kind: "Reject"; rule_id: number; reason_code: number };

export interface Certificate {
  domain: string;
  gate: string;
  proposal_id: bigint;
  attempt: number;
  chain_id: bigint;
  proxy: string;
  executor: string;
  current_impl: string;
  current_impl_codehash: string;
  new_impl: string;
  new_impl_codehash: string;
  init_calldata_hash: string;
  source_commitment: string;
  rule_set_hash: string;
  verifier_pubkey: string;
  verifier_release_hash: string;
  evidence_block_number: bigint;
  evidence_block_hash: string;
  evidence_root: string;
  ai: AiReview;
  verdict: Verdict;
  issued_at_s: bigint;
  expires_at_s: bigint;
}

/** `AiReview::Clean.signature_kind` values the verifier writes (ARCHITECTURE, AI review). */
export function aiSignatureKindName(kind: number): "provider_tee" | "gateway" | null {
  if (kind === 1) return "provider_tee";
  if (kind === 2) return "gateway";
  return null;
}

class Reader {
  readonly #bytes: Uint8Array;
  readonly #view: DataView;
  #at = 0;

  constructor(bytes: Uint8Array) {
    this.#bytes = bytes;
    this.#view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  #need(count: number, what: string): number {
    if (count > this.#bytes.length - this.#at) {
      throw new WireError(`certificate.${what}`, `truncated at offset ${this.#at}`);
    }
    const start = this.#at;
    this.#at += count;
    return start;
  }

  u8(what: string): number {
    return this.#view.getUint8(this.#need(1, what));
  }

  u16(what: string): number {
    return this.#view.getUint16(this.#need(2, what), true);
  }

  u32(what: string): number {
    return this.#view.getUint32(this.#need(4, what), true);
  }

  u64(what: string): bigint {
    return this.#view.getBigUint64(this.#need(8, what), true);
  }

  hex(count: number, what: string): string {
    const start = this.#need(count, what);
    return bytesToHex(this.#bytes.subarray(start, start + count));
  }

  accountId(what: string): string {
    const length = this.u32(what);
    const start = this.#need(length, what);
    let text: string;
    try {
      text = new TextDecoder("utf-8", { fatal: true }).decode(
        this.#bytes.subarray(start, start + length),
      );
    } catch {
      throw new WireError(`certificate.${what}`, "not valid UTF-8");
    }
    // near-account-id validates while deserializing, so an invalid id is not a decodable certificate.
    if (!isAccountId(text))
      throw new WireError(`certificate.${what}`, "not a valid NEAR account id");
    return text;
  }

  finish(): void {
    if (this.#at !== this.#bytes.length) {
      throw new WireError("certificate", `${this.#bytes.length - this.#at} trailing bytes`);
    }
  }
}

function decodeAi(r: Reader): AiReview {
  const tag = r.u8("ai");
  switch (tag) {
    case 0:
      return {
        kind: "Clean",
        evidence_hash: r.hex(32, "ai.evidence_hash"),
        signature_kind: r.u8("ai.signature_kind"),
      };
    case 1:
      return {
        kind: "Vetoed",
        evidence_hash: r.hex(32, "ai.evidence_hash"),
        counterexample_hash: r.hex(32, "ai.counterexample_hash"),
      };
    case 2:
      return { kind: "NotRun" };
    default:
      throw new WireError("certificate.ai", `unknown variant ${tag}`);
  }
}

function decodeVerdict(r: Reader): Verdict {
  const tag = r.u8("verdict");
  switch (tag) {
    case 0:
      return { kind: "Pass" };
    case 1:
      return {
        kind: "Reject",
        rule_id: r.u8("verdict.rule_id"),
        reason_code: r.u16("verdict.reason_code"),
      };
    default:
      throw new WireError("certificate.verdict", `unknown variant ${tag}`);
  }
}

/** Decode exactly one certificate; truncated input, trailing bytes and unknown variants throw. */
export function decodeCertificate(bytes: Uint8Array): Certificate {
  const r = new Reader(bytes);
  const certificate: Certificate = {
    domain: r.hex(16, "domain"),
    gate: r.accountId("gate"),
    proposal_id: r.u64("proposal_id"),
    attempt: r.u32("attempt"),
    chain_id: r.u64("chain_id"),
    proxy: r.hex(20, "proxy"),
    executor: r.hex(20, "executor"),
    current_impl: r.hex(20, "current_impl"),
    current_impl_codehash: r.hex(32, "current_impl_codehash"),
    new_impl: r.hex(20, "new_impl"),
    new_impl_codehash: r.hex(32, "new_impl_codehash"),
    init_calldata_hash: r.hex(32, "init_calldata_hash"),
    source_commitment: r.hex(32, "source_commitment"),
    rule_set_hash: r.hex(32, "rule_set_hash"),
    verifier_pubkey: r.hex(32, "verifier_pubkey"),
    verifier_release_hash: r.hex(32, "verifier_release_hash"),
    evidence_block_number: r.u64("evidence_block_number"),
    evidence_block_hash: r.hex(32, "evidence_block_hash"),
    evidence_root: r.hex(32, "evidence_root"),
    ai: decodeAi(r),
    verdict: decodeVerdict(r),
    issued_at_s: r.u64("issued_at_s"),
    expires_at_s: r.u64("expires_at_s"),
  };
  r.finish();
  return certificate;
}

/** SHA-256 of the canonical certificate bytes as `0x` hex: the value the gate stores as `certificate_hash`. */
export async function certificateHash(bytes: Uint8Array): Promise<string> {
  return bytesToHex(await sha256(bytes));
}
