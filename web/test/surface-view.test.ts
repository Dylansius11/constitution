import { describe, expect, it } from "vitest";
import { reduceSurface, type SurfaceInput, type SurfaceState } from "../src/lib/state.ts";
import { surfaceView, VERIFIER_LABELS } from "../src/lib/surface-view.ts";
import { NEW_IMPL, OLD_IMPL, proposal, proxyRead } from "./support/fixtures.ts";

const rejected: SurfaceState = {
  kind: "rejected",
  ruleId: 2,
  reasonCode: 1,
  evidenceRoot: "0x01",
  noSignatureRequested: true,
  implementationUnchanged: false,
  unchangedAtBlock: null,
};
const states = [
  { kind: "idle" },
  { kind: "submitting", step: "registering" },
  { kind: "verifying", stage: "r2_solvency", runObserved: true, pendingVerdict: null },
  { kind: "verifier_failed", errorCode: "tool_unavailable", retryable: true },
  { kind: "ai_review", stage: "reproducing" },
  { kind: "certified", certificateHash: "0x01" },
  { kind: "signing", attempt: 1, nonce: 0 },
  { kind: "signing_retry", attempt: 1, nonce: 0 },
  { kind: "relaying", evm: "signed", txHash: "0x01" },
  {
    kind: "executed",
    block: 12,
    txHash: "0x01",
    implementationBefore: "VaultV1",
    implementationAfter: "VaultV2Honest",
  },
  rejected,
  { kind: "expired", implementationUnchanged: false, unchangedAtBlock: null },
  { kind: "review_incomplete", stage: "ai_review", deadlineNs: 1793701800000000000n },
  { kind: "reverted", block: 12, txHash: "0x01" },
  { kind: "error", cause: "proposal_not_found", detail: "No proposal with this id." },
  {
    kind: "network_unavailable",
    source: "base",
    failures: 3,
    lastObservedAtMs: 1793675087000,
    lastKnown: rejected,
  },
  { kind: "unknown", reason: "New gate state." },
] as const satisfies readonly SurfaceState[];

describe("surface state copy and honest ladders", () => {
  it.each(states)("renders $kind without guessing an outcome", (state) => {
    const view = surfaceView(state, { fixture: "VaultV2Honest", proposalId: 7 });
    expect(view.title.length).toBeGreaterThan(0);
    expect(view.body.length).toBeGreaterThan(0);
    expect(view.stages.length).toBeGreaterThan(0);
    expect(["neutral", "pass", "refuse", "pending"]).toContain(view.tone);
    expect(view.body).not.toMatch(/\d+%|step \d|estimated/i);
  });
  it("maps the exact verifier labels including verdict signing, not MPC signing", () => {
    expect(VERIFIER_LABELS.r2_solvency.label).toBe("R2 Solvency");
    expect(VERIFIER_LABELS.signing.label).toBe("Signing the verdict");
    expect(VERIFIER_LABELS.submitting.label).toBe("Recording the verdict on NEAR");
    const view = surfaceView(states[2], { fixture: "VaultV2Honest" });
    expect(view.stages.find((s) => s.label === "R2 Solvency")?.state).toBe("active");
    expect(view.stages.find((s) => s.label === "R1 Storage layout")?.state).not.toBe("done");
  });
  it("marks AI Not run after an observed deterministic refusal and downstream Not reached", () => {
    const view = surfaceView(rejected, { fixture: "VaultV2Drain" });
    expect(view.stages.find((s) => s.label === "Private red-team review")).toMatchObject({
      state: "skipped",
      status: "Not run",
      detail: "A deterministic rule refused first.",
    });
    expect(view.stages.find((s) => s.label === "Chain Signature")?.status).toBe("Not reached");
    expect(view.body).toContain("No signature was requested.");
    expect(view.body).not.toContain("unchanged");
  });
  it("says unchanged only with two agreeing observed reads", () => {
    expect(
      surfaceView({ ...rejected, implementationUnchanged: true, unchangedAtBlock: 42 }, {}).body,
    ).toContain("The proxy implementation is unchanged.");
    expect(
      surfaceView({ kind: "expired", implementationUnchanged: false, unchangedAtBlock: null }, {})
        .body,
    ).not.toContain("unchanged");
    expect(surfaceView({ kind: "reverted", block: 1, txHash: "0x01" }, {}).body).not.toContain(
      "unchanged",
    );
  });
  it("does not say unchanged when the reducer's observed before and after disagree", () => {
    const input: SurfaceInput = {
      submission: { phase: "none" },
      proposal: {
        status: "ok",
        observedAtMs: 1,
        value: proposal({
          state: "Rejected",
          rejection: { rule_id: 2, reason_code: 1, evidence_root: "0x01" },
        }),
      },
      run: { status: "idle" },
      evm: { status: "idle" },
      proxy: {
        status: "ok",
        observedAtMs: 2,
        value: { before: proxyRead(OLD_IMPL, 40), after: proxyRead(NEW_IMPL, 41) },
      },
    };
    expect(surfaceView(reduceSurface(input), {}).body).not.toContain("unchanged");
  });
  it("shows executed before and after, but does not invent a missing before read", () => {
    expect(surfaceView(states[9], {}).body).toContain("from VaultV1 to VaultV2Honest");
    expect(
      surfaceView(
        {
          kind: "executed",
          block: 12,
          txHash: "0x01",
          implementationBefore: null,
          implementationAfter: "VaultV2Honest",
        },
        {},
      ).body,
    ).toContain("The previous implementation was not observed by this page.");
  });
  it("keeps incomplete review neutral to verdict colors", () => {
    const view = surfaceView(states[12], {});
    expect(view.tone).toBe("pending");
    expect(view.body).toContain("not a pass and not a refusal");
    expect(view.stages.find((s) => s.label === "Private red-team review")?.status).toBe(
      "Incomplete",
    );
  });
  it("retains the last known ladder without stale pass/refusal styling", () => {
    const view = surfaceView(states[15], {});
    expect(view.title).toBe("Cannot reach Base Sepolia");
    expect(view.body).toContain("last observed state");
    expect(view.body).toContain("Refused under R2 Solvency");
    expect(view.tone).toBe("neutral");
    expect(view.stages.some((s) => s.state === "refused" || s.state === "done")).toBe(false);
    expect(view.stages[0]?.status).toBe("Done");
  });
  it("does not claim verifier observations when only gate data is known", () => {
    const view = surfaceView(
      { kind: "certified", certificateHash: "0x01" },
      { verifierObserved: false },
    );
    expect(view.stages.find((s) => s.label === "R1 Storage layout")?.status).toBe(
      "Not observed by this page",
    );
  });
  it("does not infer an AI skip when no verifier run was observed", () => {
    const view = surfaceView(rejected, { verifierObserved: false });
    expect(view.stages.find((s) => s.label === "Private red-team review")?.status).toBe(
      "Not observed by this page",
    );
  });
  it("preserves an observed AI review on an AI-origin refusal", () => {
    const view = surfaceView(rejected, {
      runStages: [
        { name: "ai_review", status: "Fail", detail: "A candidate reproduced.", elapsed_ms: 10 },
      ],
    });
    expect(view.stages.find((s) => s.label === "Private red-team review")).toMatchObject({
      status: "Refused",
      detail: "A candidate reproduced.",
    });
  });
  it.each(["Pass", "Incomplete"] as const)(
    "renders the backend's terminal submit result (%s) on the verdict stage",
    (status) => {
      for (const state of [rejected, states[5]]) {
        const view = surfaceView(state, {
          runStages: [
            { name: "submit", status, detail: "Observed submission result.", elapsed_ms: 1 },
            { name: "done", status, detail: "Run complete.", elapsed_ms: 0 },
          ],
        });
        const stage = view.stages.find(
          (s) => s.label === VERIFIER_LABELS.submitting.label,
        );
        expect(stage?.state).toBe(status === "Pass" ? "done" : "skipped");
        expect(stage?.status).toBe(status === "Pass" ? "Done" : "Incomplete");
      }
    },
  );
  it("distinguishes a registration refusal from a signed broadcast failure", () => {
    const registration = surfaceView(
      { kind: "error", cause: "gate_refused", detail: "Wrong target." },
      { proposalId: 7 },
    );
    expect(registration.body).toContain("Nothing was recorded.");
    const broadcast = surfaceView(
      { kind: "error", cause: "broadcast_failed", detail: "RPC refused the broadcast." },
      { proposalId: 7 },
    );
    expect(broadcast.body).toContain("still signed and held by the gate");
    expect(broadcast.body).not.toContain("Nothing was recorded.");
  });
  it("uses observed stage results instead of assuming earlier stages passed", () => {
    const view = surfaceView(states[2], {
      runStages: [
        { name: "r1_layout", status: "Pass", detail: "Append-only layout.", elapsed_ms: 1 },
      ],
    });
    expect(view.stages.find((s) => s.label === "R1 Storage layout")).toMatchObject({
      state: "done",
      detail: "Append-only layout.",
    });
    expect(
      surfaceView({ kind: "relaying", evm: "included_unverified", txHash: "0x01" }, {}).title,
    ).not.toBe("Executed on Base Sepolia");
  });
});
