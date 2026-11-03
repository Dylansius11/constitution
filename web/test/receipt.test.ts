import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { base64ToBytes, bytesToBase64 } from "../src/lib/bytes.ts";
import { InputError } from "../src/lib/errors.ts";
import { parseJson } from "../src/lib/json.ts";
import { buildObservedReceipt, implementationUnchanged, receiptJson } from "../src/lib/receipt.ts";
import {
  evmTx,
  hex,
  NEW_IMPL,
  OLD_IMPL,
  proposal,
  proxyRead,
  run,
  SIGNED_TX,
  SIGNED_TX_HASH,
  vectorByName,
} from "./support/fixtures.ts";

const PASS = vectorByName("pass_ai_clean_provider_tee");
const PASS_BYTES = base64ToBytes(PASS.certificate_b64, "certificate_b64");
const NEAR_TX = "9Wz4oKqkQyBN1Zr3nU7rJxk1L2m3n4o5p6q7r8s9t1u";

const sha256Hex = (bytes: Uint8Array) => `0x${createHash("sha256").update(bytes).digest("hex")}`;

/** A proposal the gate moved to SignRequested with the certificate in `PASS`. */
function certifiedProposal(over = {}) {
  return proposal({
    state: "SignRequested",
    certificate_hash: PASS.certificate_hash,
    evm: evmTx(),
    ...over,
  });
}

function passRun(over = {}) {
  return run({
    stage: "done",
    verdict: { kind: "Pass" },
    certificate_b64: PASS.certificate_b64,
    near_tx_hash: NEAR_TX,
    ...over,
  });
}

describe("Observed receipt: gate-only facts", () => {
  it("a pending proposal yields no verdict, verifier, evm detail or proxy facts", async () => {
    const { receipt, notes } = await buildObservedReceipt({
      proposal: proposal(),
      network: "testnet",
    });
    expect(notes).toEqual([]);
    expect(Object.keys(receipt)).toEqual(["schema", "proposal", "rule_set_hash", "evm", "labels"]);
    expect(receipt.schema).toBe("constitution.receipt/1");
    expect(receipt.evm).toEqual({ status: "not_signed" });
    expect(receipt.labels).toEqual({ network: "testnet" });
    expect(receipt.rule_set_hash).toBe(hex(0xa1, 32));
  });

  it("a rejected proposal carries the gate's verdict and evidence root, and nothing from a run", async () => {
    const rejected = proposal({
      state: "Rejected",
      rejection: { rule_id: 2, reason_code: 22, evidence_root: hex(0xe7, 32) },
    });
    const { receipt } = await buildObservedReceipt({ proposal: rejected, network: "testnet" });
    expect(receipt.verdict).toEqual({ kind: "Reject", rule_id: 2, reason_code: 22 });
    expect(receipt.evidence_root).toBe(hex(0xe7, 32));
    expect(receipt.verifier).toBeUndefined();
    expect(receipt.evm.status).toBe("not_signed");
  });

  it("Pass is stated only for states that follow a recorded certificate", async () => {
    for (const state of ["SignRequested", "SignFailed", "Signed"] as const) {
      const { receipt } = await buildObservedReceipt({
        proposal: certifiedProposal({ state }),
        network: "testnet",
      });
      expect(receipt.verdict, state).toEqual({ kind: "Pass" });
    }
    for (const state of ["Pending", "Expired"] as const) {
      const { receipt } = await buildObservedReceipt({
        proposal: proposal({ state }),
        network: "testnet",
      });
      expect(receipt.verdict, state).toBeUndefined();
    }
  });

  it("a signed transaction yields its hash and status signed, with the nonce", async () => {
    const signed = certifiedProposal({
      state: "Signed",
      evm: evmTx({ signed_tx: SIGNED_TX, nonce: 9 }),
    });
    const { receipt } = await buildObservedReceipt({ proposal: signed, network: "testnet" });
    expect(receipt.evm).toEqual({ status: "signed", nonce: 9, tx_hash: SIGNED_TX_HASH });
  });

  it("rejects an unknown network label and a non-URL explorer", async () => {
    await expect(
      buildObservedReceipt({ proposal: proposal(), network: "mainnet" as "testnet" }),
    ).rejects.toThrow(InputError);
    await expect(
      buildObservedReceipt({
        proposal: proposal(),
        network: "testnet",
        explorerBaseUrl: "javascript:alert(1)",
      }),
    ).rejects.toThrow(InputError);
  });
});

describe("Observed receipt: certificate anchoring", () => {
  it("admits verifier facts when the run's certificate bytes hash to the gate's certificate_hash", async () => {
    expect(sha256Hex(PASS_BYTES)).toBe(PASS.certificate_hash);
    const { receipt, notes } = await buildObservedReceipt({
      proposal: certifiedProposal(),
      network: "testnet",
      run: passRun(),
    });
    expect(notes).toEqual([]);
    expect(receipt.verifier?.pubkey).toMatch(/^0x[0-9a-f]{64}$/);
    expect(receipt.verifier?.release_hash).toMatch(/^0x[0-9a-f]{64}$/);
    expect(receipt.evidence_root).toMatch(/^0x[0-9a-f]{64}$/);
    expect(receipt.ai).toEqual({ signature_kind: "provider_tee" });
    expect(receipt.near).toEqual({ verdict_tx: NEAR_TX });
  });

  it("mismatched certificate hash yields no verifier facts at all", async () => {
    const wrong = certifiedProposal({ certificate_hash: hex(0xee, 32) });
    const { receipt, notes } = await buildObservedReceipt({
      proposal: wrong,
      network: "testnet",
      run: passRun(),
    });
    expect(notes.map((n) => n.code)).toEqual(["certificate_hash_mismatch"]);
    expect(receipt.verifier).toBeUndefined();
    expect(receipt.evidence_root).toBeUndefined();
    expect(receipt.ai).toBeUndefined();
    expect(receipt.near).toBeUndefined();
    // The gate's own fact survives: the gate recorded a certificate for this proposal.
    expect(receipt.verdict).toEqual({ kind: "Pass" });
  });

  it("a tampered certificate (one flipped bit) no longer anchors", async () => {
    const tampered = PASS_BYTES.slice();
    tampered[40] = (tampered[40] ?? 0) ^ 0x01;
    const { receipt, notes } = await buildObservedReceipt({
      proposal: certifiedProposal(),
      network: "testnet",
      run: passRun({ certificate_b64: bytesToBase64(tampered) }),
    });
    expect(notes.map((n) => n.code)).toEqual(["certificate_hash_mismatch"]);
    expect(receipt.verifier).toBeUndefined();
  });

  it("a certificate for a different proposal's hash is refused even if it decodes", async () => {
    const other = vectorByName("reject_r2_ai_vetoed");
    const { receipt, notes } = await buildObservedReceipt({
      proposal: certifiedProposal(),
      network: "testnet",
      run: passRun({ certificate_b64: other.certificate_b64 }),
    });
    expect(notes.map((n) => n.code)).toEqual(["certificate_hash_mismatch"]);
    expect(receipt.verifier).toBeUndefined();
  });

  it("uses no run facts while the gate has recorded no certificate", async () => {
    const { receipt, notes } = await buildObservedReceipt({
      proposal: proposal(),
      network: "testnet",
      run: passRun(),
    });
    expect(notes.map((n) => n.code)).toEqual(["certificate_not_recorded_on_gate"]);
    expect(receipt.verifier).toBeUndefined();
    expect(receipt.near).toBeUndefined();
  });

  it("a run without certificate bytes adds nothing and raises no note", async () => {
    const { receipt, notes } = await buildObservedReceipt({
      proposal: certifiedProposal(),
      network: "testnet",
      run: run({ stage: "r2_solvency" }),
    });
    expect(notes).toEqual([]);
    expect(receipt.verifier).toBeUndefined();
  });

  it("anchored but undecodable bytes are reported, not admitted", async () => {
    const garbage = Uint8Array.of(1, 2, 3, 4);
    const { receipt, notes } = await buildObservedReceipt({
      proposal: certifiedProposal({ certificate_hash: sha256Hex(garbage) }),
      network: "testnet",
      run: passRun({ certificate_b64: bytesToBase64(garbage) }),
    });
    expect(notes.map((n) => n.code)).toEqual(["certificate_undecodable"]);
    expect(receipt.verifier).toBeUndefined();
  });

  it("an unknown AI signature kind is reported and the AI fact is withheld", async () => {
    const odd = PASS_BYTES.slice();
    // Layout tail: ai kind (1), verdict Pass (1), issued_at_s (8), expires_at_s (8).
    odd[odd.length - 18] = 9;
    const { receipt, notes } = await buildObservedReceipt({
      proposal: certifiedProposal({ certificate_hash: sha256Hex(odd) }),
      network: "testnet",
      run: passRun({ certificate_b64: bytesToBase64(odd) }),
    });
    expect(notes.map((n) => n.code)).toEqual(["unknown_ai_signature_kind"]);
    expect(receipt.ai).toBeUndefined();
    expect(receipt.verifier).toBeDefined();
  });

  it("the gateway signature kind maps to gateway", async () => {
    const gateway = vectorByName("pass_ai_clean_gateway_boundary_values");
    const { receipt } = await buildObservedReceipt({
      proposal: certifiedProposal({ certificate_hash: gateway.certificate_hash }),
      network: "testnet",
      run: passRun({ certificate_b64: gateway.certificate_b64 }),
    });
    expect(receipt.ai).toEqual({ signature_kind: "gateway" });
  });
});

describe("Observed receipt: EVM and proxy observations", () => {
  const signed = certifiedProposal({ state: "Signed", evm: evmTx({ signed_tx: SIGNED_TX }) });

  it("included: status, block and a testnet explorer link built from configuration", async () => {
    const { receipt } = await buildObservedReceipt({
      proposal: signed,
      network: "testnet",
      evm: { status: "included", block: 42, block_hash: hex(0xb0, 32) },
      explorerBaseUrl: "https://base-sepolia.blockscout.com/",
    });
    expect(receipt.evm).toEqual({
      status: "included",
      nonce: 3,
      tx_hash: SIGNED_TX_HASH,
      block: 42,
      explorer: `https://base-sepolia.blockscout.com/tx/${SIGNED_TX_HASH}`,
    });
  });

  it("reverted records the block; local receipts never get an explorer link", async () => {
    const testnet = await buildObservedReceipt({
      proposal: signed,
      network: "testnet",
      evm: { status: "reverted", block: 42, block_hash: hex(0xb0, 32) },
    });
    expect(testnet.receipt.evm.status).toBe("reverted");
    expect(testnet.receipt.evm.explorer).toBe(`https://sepolia.basescan.org/tx/${SIGNED_TX_HASH}`);
    const local = await buildObservedReceipt({
      proposal: signed,
      network: "local",
      evm: { status: "included", block: 42, block_hash: hex(0xb0, 32) },
    });
    expect(local.receipt.evm.explorer).toBeUndefined();
    expect(local.receipt.labels.network).toBe("local");
  });

  it("pending adds no block or link; not_found leaves the status at signed", async () => {
    const pending = await buildObservedReceipt({
      proposal: signed,
      network: "testnet",
      evm: { status: "pending" },
    });
    expect(pending.receipt.evm).toEqual({ status: "pending", nonce: 3, tx_hash: SIGNED_TX_HASH });
    const absent = await buildObservedReceipt({
      proposal: signed,
      network: "testnet",
      evm: { status: "not_found" },
    });
    expect(absent.receipt.evm.status).toBe("signed");
  });

  it("an on-chain observation cannot appear for a proposal with no signed transaction", async () => {
    const { receipt } = await buildObservedReceipt({
      proposal: certifiedProposal(),
      network: "testnet",
      evm: { status: "included", block: 42, block_hash: hex(0xb0, 32) },
    });
    expect(receipt.evm).toEqual({ status: "not_signed", nonce: 3 });
  });

  it("proxy reads: after only, before only, both, none", async () => {
    const before = proxyRead(OLD_IMPL, 90);
    const after = proxyRead(NEW_IMPL, 100);
    const both = await buildObservedReceipt({
      proposal: signed,
      network: "testnet",
      proxy: { before, after },
    });
    expect(both.receipt.proxy).toEqual({
      implementation_before: OLD_IMPL,
      implementation_after: NEW_IMPL,
      observed_at_block: 100,
    });
    const afterOnly = await buildObservedReceipt({
      proposal: signed,
      network: "testnet",
      proxy: { after },
    });
    expect(afterOnly.receipt.proxy).toEqual({
      implementation_after: NEW_IMPL,
      observed_at_block: 100,
    });
    const beforeOnly = await buildObservedReceipt({
      proposal: signed,
      network: "testnet",
      proxy: { before },
    });
    expect(beforeOnly.receipt.proxy).toEqual({ implementation_before: OLD_IMPL });
    const none = await buildObservedReceipt({ proposal: signed, network: "testnet", proxy: {} });
    expect(none.receipt.proxy).toBeUndefined();
  });

  it("NEAR transactions keep the schema order and only what was observed", async () => {
    const { receipt } = await buildObservedReceipt({
      proposal: certifiedProposal(),
      network: "testnet",
      run: passRun(),
      nearTxs: { propose_tx: "ProposeTx", sign_tx: "SignTx" },
    });
    expect(Object.keys(receipt.near ?? {})).toEqual(["propose_tx", "verdict_tx", "sign_tx"]);
    const proposeOnly = await buildObservedReceipt({
      proposal: proposal(),
      network: "testnet",
      nearTxs: { propose_tx: "ProposeTx" },
    });
    expect(proposeOnly.receipt.near).toEqual({ propose_tx: "ProposeTx" });
  });

  it("'unchanged' needs two reads that agree", () => {
    const a = proxyRead(OLD_IMPL, 90);
    expect(implementationUnchanged(undefined)).toBe(false);
    expect(implementationUnchanged({})).toBe(false);
    expect(implementationUnchanged({ before: a })).toBe(false);
    expect(implementationUnchanged({ after: a })).toBe(false);
    expect(implementationUnchanged({ before: a, after: proxyRead(NEW_IMPL, 100) })).toBe(false);
    expect(implementationUnchanged({ before: a, after: proxyRead(OLD_IMPL, 100) })).toBe(true);
  });
});

describe("Observed receipt JSON", () => {
  it("renders nanosecond timestamps exactly and amounts as decimal strings, and parses back", async () => {
    const { receipt } = await buildObservedReceipt({
      proposal: certifiedProposal({ state: "Signed", evm: evmTx({ signed_tx: SIGNED_TX }) }),
      network: "testnet",
    });
    const text = receiptJson(receipt);
    expect(text.endsWith("\n")).toBe(true);
    expect(text).toContain('"created_at_ns": 1793700000123456789');
    expect(text).toContain('"reserved": "1000000000000000000000000"');
    expect(text).toContain('"max_fee_per_gas": "1009731712"');
    const parsed = parseJson(text) as {
      proposal: { created_at_ns: bigint; evm: { signed_tx: string } };
      evm: { tx_hash: string };
    };
    expect(parsed.proposal.created_at_ns).toBe(1_793_700_000_123_456_789n);
    expect(parsed.evm.tx_hash).toBe(SIGNED_TX_HASH);
  });
});
