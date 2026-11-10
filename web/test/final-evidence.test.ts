import { getAddress } from "viem";
import { describe, expect, it } from "vitest";
import {
  baseTxUrl,
  finalEvm,
  finalGate,
  finalRuns,
  finalVerifier,
  nearTxUrl,
} from "../src/data/final-evidence.ts";

const BASE58_TX = /^[1-9A-HJ-NP-Za-km-z]{43,44}$/;

describe("recorded final deployment constants", () => {
  it("holds well-formed NEAR transaction hashes", () => {
    for (const hash of [
      finalGate.deployTx,
      finalRuns.drain.proposeTx,
      finalRuns.drain.verdictTx,
      finalRuns.governance.scheduleTx,
      finalRuns.governance.earlyApplyTx,
      finalRuns.honest.proposeTx,
      finalRuns.honest.verdictTx,
    ]) {
      expect(hash).toMatch(BASE58_TX);
    }
    expect(finalGate.codeHash).toMatch(BASE58_TX);
  });

  it("holds well-formed Base Sepolia hashes and addresses that survive EIP-55", () => {
    expect(finalRuns.honest.baseTx).toMatch(/^0x[0-9a-f]{64}$/);
    for (const hash of [finalGate.wasmSha256, finalVerifier.ruleSetHash, finalVerifier.releaseHash, finalVerifier.composeHash]) {
      expect(hash).toMatch(/^0x[0-9a-f]{64}$/);
    }
    for (const address of Object.values(finalEvm)) {
      expect(address).toMatch(/^0x[0-9a-f]{40}$/);
      expect(getAddress(address).toLowerCase()).toBe(address);
    }
  });

  it("keeps the verifier key a 32-byte base64 value", () => {
    expect(finalVerifier.publicKeyB64).toMatch(/^[A-Za-z0-9+/]{43}=$/);
  });

  it("names the six appraisal checks", () => {
    expect(finalVerifier.appraisal).toHaveLength(6);
  });

  it("builds explorer links from the configured bases", () => {
    expect(nearTxUrl("https://testnet.nearblocks.io", finalRuns.drain.proposeTx)).toBe(
      `https://testnet.nearblocks.io/txns/${finalRuns.drain.proposeTx}`,
    );
    expect(baseTxUrl("https://sepolia.basescan.org", finalRuns.honest.baseTx)).toBe(
      `https://sepolia.basescan.org/tx/${finalRuns.honest.baseTx}`,
    );
  });

  it("records the honest upgrade as one executed block with the three expected facts", () => {
    expect(finalRuns.honest.block).toBe(57042237);
    expect(finalRuns.honest.gasUsed).toBe(105099);
    expect(finalRuns.honest.aiReproduced).toBe(0);
  });
});
