import { describe, expect, it } from "vitest";
import raw from "../../spikes/gate-min/base-sepolia-evidence.json";
import { parseSpikeEvidence, spikeEvidence } from "../src/data/evidence.ts";

describe("recorded Stage B spike evidence", () => {
  it("projects actual chain facts without carrying RPC bodies or signing material", () => {
    expect(spikeEvidence.nearAccount).toBe("constitution-spike.testnet");
    expect(spikeEvidence.observedAtUtc).toBe("2026-11-03T03:04:47.707934+00:00");
    expect(spikeEvidence.honest).toMatchObject({
      nearTxHash: "55bekU9mqsWr8VP5TQp5xzViHxGLTUH5b55mduo3azSy",
      baseTxHash: "0xc232188629763bec4eb9a72be7fd08f66ab5690a2ad997ac6ed20697ff0729d8",
      status: 1,
      gasUsed: 105159,
      block: 53766464,
      revertReason: null,
    });
    expect(spikeEvidence.expired).toMatchObject({
      baseTxHash: "0x4e095a6be399e32cd5e72dc5140a433c128716e6dfe6f2c92191f4ca2c0afee3",
      status: 0,
      gasUsed: 28900,
      revertReason: "DeadlinePassed",
    });
    expect(spikeEvidence.addresses.vaultV2Honest).toBe(
      "0x92aba426dc77a914b07dfefbed3f57e020436e96",
    );
    expect(parseSpikeEvidence(raw)).toEqual(spikeEvidence);
    const text = JSON.stringify(spikeEvidence);
    expect(text.length).toBeLessThan(2500);
    expect(text).not.toMatch(/raw_tx|mpc_response|logsBloom|bootstrap_output/);
  });
  it("finds cases by name rather than their position", () => {
    expect(parseSpikeEvidence({ ...raw, cases: [...raw.cases].reverse() })).toEqual(spikeEvidence);
  });
  it("rejects missing cases, malformed receipts, and an unrecognized revert", () => {
    expect(() => parseSpikeEvidence({ ...raw, cases: [] })).toThrow(/honest_upgrade/);
    expect(() => parseSpikeEvidence({ ...raw, signer: "0x1" })).toThrow(/signer/);
    const cases = structuredClone(raw.cases);
    const honest = cases[0];
    if (!honest) throw new Error("Recorded honest case is missing");
    honest.receipt.status = "0x2";
    expect(() => parseSpikeEvidence({ ...raw, cases })).toThrow(/status/);
    const unknown = structuredClone(raw.cases);
    const revert = unknown[1]?.revert_call_response;
    if (!revert) throw new Error("Recorded expired revert is missing");
    revert.error.data = "0x00000000";
    expect(() => parseSpikeEvidence({ ...raw, cases: unknown })).toThrow(/revert/);
  });
  it("rejects a truncated DeadlinePassed payload instead of treating its selector as proof", () => {
    const cases = structuredClone(raw.cases);
    const revert = cases[1]?.revert_call_response;
    if (!revert) throw new Error("Recorded expired revert is missing");
    revert.error.data = "0x3dee80f2";
    expect(() => parseSpikeEvidence({ ...raw, cases })).toThrow(/revert/);
  });
  it("refuses a receipt whose transaction hash disagrees with the signed hash", () => {
    const cases = structuredClone(raw.cases);
    const honest = cases[0];
    if (!honest) throw new Error("Recorded honest case is missing");
    honest.receipt.transactionHash = `0x${"00".repeat(32)}`;
    expect(() => parseSpikeEvidence({ ...raw, cases })).toThrow(/hash/);
  });
});
