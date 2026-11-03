import { keccak256 } from "viem";
import type { ProxyReads, TxObservation } from "./base.ts";
import { base64ToBytes } from "./bytes.ts";
import {
  aiSignatureKindName,
  type Certificate,
  certificateHash,
  decodeCertificate,
} from "./certificate.ts";
import { InputError } from "./errors.ts";
import type { Proposal } from "./gate-types.ts";
import { parseHttpUrl } from "./http.ts";
import { type JsonValue, stringifyJson } from "./json.ts";
import type { VerifierRun } from "./verifier.ts";

/**
 * `Observed` receipts: `constitution.receipt/1` (ARCHITECTURE, Receipt JSON) assembled in the
 * browser from public reads. This mirrors crates/cli/src/receipt.rs:
 *
 * - A field exists only after it was observed. Nothing is defaulted, estimated or inferred.
 * - Facts that come from a verifier run (verifier key and release, evidence root, AI signature
 *   kind, verdict transaction) are admitted only when the run's certificate bytes hash to the
 *   `certificate_hash` the gate stored. A run is never an authority on its own.
 * - Where the CLI aborts on a mismatch, the page keeps the receipt it can prove and reports the
 *   refusal in `notes`.
 *
 * [I] Not yet admitted: evidence-manifest facts (`ai.reproduced_candidates`). The CLI anchors them
 * by re-encoding the manifest to Borsh and hashing it to `evidence_root`; the page has no such
 * encoder yet, so it omits them rather than trust the verifier's JSON.
 */

export type ReceiptNetwork = "testnet" | "local";
export type EvmStatus = "not_signed" | "signed" | "pending" | "included" | "reverted";

export type ReceiptVerdict =
  | { kind: "Pass" }
  | { kind: "Reject"; rule_id: number; reason_code: number };

export type ObservedReceipt = {
  schema: "constitution.receipt/1";
  proposal: JsonValue;
  rule_set_hash: string;
  verifier?: { pubkey: string; release_hash: string };
  verdict?: ReceiptVerdict;
  evidence_root?: string;
  ai?: { signature_kind: "provider_tee" | "gateway" };
  near?: { propose_tx?: string; verdict_tx?: string; sign_tx?: string };
  evm: { status: EvmStatus; nonce?: number; tx_hash?: string; block?: number; explorer?: string };
  proxy?: {
    implementation_before?: string;
    implementation_after?: string;
    observed_at_block?: number;
  };
  labels: { network: ReceiptNetwork };
};

export type ReceiptNoteCode =
  | "certificate_not_recorded_on_gate"
  | "certificate_hash_mismatch"
  | "certificate_undecodable"
  | "unknown_ai_signature_kind";

/** Something the builder refused to admit, with the reason. Show it; do not hide it. */
export interface ReceiptNote {
  code: ReceiptNoteCode;
  message: string;
}

export interface ReceiptInput {
  proposal: Proposal;
  network: ReceiptNetwork;
  /** The verifier run for this proposal, if the page has one. Unanchored, so facts are gated. */
  run?: VerifierRun | null;
  /** The page's own lookup of the gate's signed transaction by hash. Absent when not looked up. */
  evm?: TxObservation;
  /** The page's own reads of the proxy implementation slot. */
  proxy?: ProxyReads;
  /** NEAR transaction hashes the page itself observed (`verdict_tx` comes from the anchored run). */
  nearTxs?: { propose_tx?: string; sign_tx?: string };
  /** Explorer base URL, configuration not constant. Defaults to https://sepolia.basescan.org. */
  explorerBaseUrl?: string;
}

export interface ReceiptResult {
  receipt: ObservedReceipt;
  notes: ReceiptNote[];
}

const DEFAULT_EXPLORER = "https://sepolia.basescan.org";

/** The gate proposal in its serde wire form (u128 as decimal strings, nanoseconds as integers). */
export function proposalView(p: Proposal): JsonValue {
  return {
    id: p.id,
    proposer: p.proposer,
    created_at_ns: p.created_at_ns,
    expires_at_ns: p.expires_at_ns,
    rule_set_hash: p.rule_set_hash,
    new_impl: p.new_impl,
    new_impl_codehash: p.new_impl_codehash,
    expected_current_impl: p.expected_current_impl,
    init_calldata: p.init_calldata,
    source_commitment: p.source_commitment,
    reserved: p.reserved.toString(),
    state: p.state,
    certificate_hash: p.certificate_hash,
    rejection:
      p.rejection === null
        ? null
        : {
            rule_id: p.rejection.rule_id,
            reason_code: p.rejection.reason_code,
            evidence_root: p.rejection.evidence_root,
          },
    evm:
      p.evm === null
        ? null
        : {
            nonce: p.evm.nonce,
            deadline_s: p.evm.deadline_s,
            gas_limit: p.evm.gas_limit,
            max_fee_per_gas: p.evm.max_fee_per_gas.toString(),
            max_priority_fee_per_gas: p.evm.max_priority_fee_per_gas.toString(),
            signing_hash: p.evm.signing_hash,
            signed_tx: p.evm.signed_tx,
            attempts: p.evm.attempts,
          },
  };
}

/** Facts available from the gate alone (`assemble` in the CLI). */
function assemble(p: Proposal, network: ReceiptNetwork): ObservedReceipt {
  const receipt: ObservedReceipt = {
    schema: "constitution.receipt/1",
    proposal: proposalView(p),
    rule_set_hash: p.rule_set_hash,
    evm: { status: "not_signed" },
    labels: { network },
  };
  if (p.rejection !== null) {
    receipt.verdict = {
      kind: "Reject",
      rule_id: p.rejection.rule_id,
      reason_code: p.rejection.reason_code,
    };
    receipt.evidence_root = p.rejection.evidence_root;
  } else if (
    p.certificate_hash !== null &&
    (p.state === "SignRequested" || p.state === "SignFailed" || p.state === "Signed")
  ) {
    receipt.verdict = { kind: "Pass" };
  }
  if (p.evm !== null) {
    receipt.evm.nonce = p.evm.nonce;
    if (p.evm.signed_tx !== null) {
      receipt.evm.status = "signed";
      receipt.evm.tx_hash = keccak256(p.evm.signed_tx as `0x${string}`);
    }
  }
  return receipt;
}

async function admitRun(
  receipt: ObservedReceipt,
  p: Proposal,
  run: VerifierRun,
  notes: ReceiptNote[],
): Promise<void> {
  if (run.certificate_b64 === null) return;
  if (p.certificate_hash === null) {
    notes.push({
      code: "certificate_not_recorded_on_gate",
      message:
        "the gate has not recorded a certificate for this proposal, so the run's certificate is not used",
    });
    return;
  }
  const bytes = base64ToBytes(run.certificate_b64, "run.certificate_b64");
  if ((await certificateHash(bytes)) !== p.certificate_hash) {
    notes.push({
      code: "certificate_hash_mismatch",
      message:
        "the run's certificate does not hash to the certificate the gate accepted; no verifier facts are shown",
    });
    return;
  }
  let certificate: Certificate;
  try {
    certificate = decodeCertificate(bytes);
  } catch (error) {
    notes.push({
      code: "certificate_undecodable",
      message: `the anchored certificate bytes could not be decoded: ${error instanceof Error ? error.message : String(error)}`,
    });
    return;
  }
  receipt.verifier = {
    pubkey: certificate.verifier_pubkey,
    release_hash: certificate.verifier_release_hash,
  };
  receipt.evidence_root = certificate.evidence_root;
  if (certificate.ai.kind === "Clean") {
    const kind = aiSignatureKindName(certificate.ai.signature_kind);
    if (kind === null) {
      notes.push({
        code: "unknown_ai_signature_kind",
        message: `certificate carries AI signature kind ${certificate.ai.signature_kind}, which this page does not know`,
      });
    } else {
      receipt.ai = { signature_kind: kind };
    }
  }
  if (run.near_tx_hash !== null) receipt.near = { ...receipt.near, verdict_tx: run.near_tx_hash };
}

function admitEvm(
  receipt: ObservedReceipt,
  observation: TxObservation,
  explorerBase: string,
): void {
  // The lookup key is keccak256(raw signed tx); without a stored signed tx there is nothing to look up.
  if (receipt.evm.tx_hash === undefined) return;
  if (observation.status === "pending") {
    receipt.evm.status = "pending";
  } else if (observation.status === "included" || observation.status === "reverted") {
    receipt.evm.status = observation.status;
    receipt.evm.block = observation.block;
    if (receipt.labels.network === "testnet") {
      receipt.evm.explorer = `${explorerBase}/tx/${receipt.evm.tx_hash}`;
    }
  }
}

export async function buildObservedReceipt(input: ReceiptInput): Promise<ReceiptResult> {
  const explorerBase = parseHttpUrl(
    input.explorerBaseUrl ?? DEFAULT_EXPLORER,
    "explorerBaseUrl",
  ).href.replace(/\/+$/, "");
  if (input.network !== "testnet" && input.network !== "local") {
    throw new InputError("network must be testnet or local");
  }
  const { proposal } = input;
  const notes: ReceiptNote[] = [];
  const receipt = assemble(proposal, input.network);
  if (input.run) await admitRun(receipt, proposal, input.run, notes);
  if (input.evm) admitEvm(receipt, input.evm, explorerBase);
  const near: NonNullable<ObservedReceipt["near"]> = {};
  if (input.nearTxs?.propose_tx !== undefined) near.propose_tx = input.nearTxs.propose_tx;
  if (receipt.near?.verdict_tx !== undefined) near.verdict_tx = receipt.near.verdict_tx;
  if (input.nearTxs?.sign_tx !== undefined) near.sign_tx = input.nearTxs.sign_tx;
  if (Object.keys(near).length > 0) receipt.near = near;
  if (input.proxy?.before !== undefined || input.proxy?.after !== undefined) {
    const proxy: NonNullable<ObservedReceipt["proxy"]> = {};
    if (input.proxy.before !== undefined)
      proxy.implementation_before = input.proxy.before.implementation;
    if (input.proxy.after !== undefined) {
      proxy.implementation_after = input.proxy.after.implementation;
      proxy.observed_at_block = input.proxy.after.block;
    }
    receipt.proxy = proxy;
  }
  return { receipt, notes };
}

/**
 * `unchanged` may be claimed only after two observed reads agree (DESIGN 6.10). One read, or a
 * read pair that differs, is not evidence of "unchanged".
 */
export function implementationUnchanged(proxy: ProxyReads | undefined): boolean {
  return (
    proxy?.before !== undefined &&
    proxy.after !== undefined &&
    proxy.before.implementation === proxy.after.implementation
  );
}

/** The exact bytes the page renders, downloads and copies. */
export function receiptJson(receipt: ObservedReceipt): string {
  return `${stringifyJson(receipt as JsonValue)}\n`;
}
