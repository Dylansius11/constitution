import { describe, expect, it } from "vitest";
import type { ProxyReads, TxObservation } from "../src/lib/base.ts";
import type { Proposal } from "../src/lib/gate-types.ts";
import {
  reduceSurface,
  type Source,
  type SurfaceInput,
  type SurfaceKind,
  type SurfaceState,
} from "../src/lib/state.ts";
import type { VerifierRun } from "../src/lib/verifier.ts";
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
} from "./support/fixtures.ts";

const T0 = 1_793_700_000_000;
const idle: Source<never> = { status: "idle" };
const ok = <T>(value: T, observedAtMs = T0): Source<T> => ({ status: "ok", value, observedAtMs });
const down = <T>(failures: number, last?: T): Source<T> =>
  last === undefined
    ? { status: "unavailable", failures, error: "fetch failed" }
    : {
        status: "unavailable",
        failures,
        error: "fetch failed",
        last: { value: last, observedAtMs: T0 },
      };

function input(over: Partial<SurfaceInput> = {}): SurfaceInput {
  return {
    submission: { phase: "none" },
    proposal: idle,
    run: idle,
    evm: idle,
    proxy: idle,
    ...over,
  };
}

const withProposal = (p: Proposal, over: Partial<SurfaceInput> = {}) =>
  input({ proposal: ok(p), ...over });
const withRun = (r: Partial<VerifierRun>) => ok<VerifierRun | null>(run(r));

const CERTIFIED = { certificate_hash: hex(0xce, 32), evm: evmTx() };
const SIGNED = {
  state: "Signed" as const,
  certificate_hash: hex(0xce, 32),
  evm: evmTx({ signed_tx: SIGNED_TX, attempts: 1 }),
};
const included: TxObservation = { status: "included", block: 42, block_hash: hex(0xb0, 32) };
const reads = (over: ProxyReads): Source<ProxyReads> => ok(over);

/** Every DESIGN 6.0 state, each produced by realistic observations. Keys must cover SurfaceKind. */
const scenarios: Record<SurfaceKind, () => SurfaceState> = {
  idle: () => reduceSurface(input()),
  submitting: () => reduceSurface(input({ submission: { phase: "registering" } })),
  verifying: () =>
    reduceSurface(withProposal(proposal(), { run: withRun({ stage: "r2_solvency" }) })),
  ai_review: () =>
    reduceSurface(withProposal(proposal(), { run: withRun({ stage: "ai_review" }) })),
  verifier_failed: () =>
    reduceSurface(
      withProposal(proposal(), { run: withRun({ stage: "failed", error_code: "worker_panic" }) }),
    ),
  certified: () => reduceSurface(withProposal(proposal({ state: "SignRequested", ...CERTIFIED }))),
  signing: () =>
    reduceSurface(withProposal(proposal({ state: "SignRequested", ...CERTIFIED })), "certified"),
  signing_retry: () =>
    reduceSurface(
      withProposal(proposal({ state: "SignFailed", ...CERTIFIED, evm: evmTx({ attempts: 2 }) })),
    ),
  relaying: () => reduceSurface(withProposal(proposal(SIGNED), { evm: ok({ status: "pending" }) })),
  executed: () =>
    reduceSurface(
      withProposal(proposal(SIGNED), {
        evm: ok(included),
        proxy: reads({ before: proxyRead(OLD_IMPL, 90), after: proxyRead(NEW_IMPL, 100) }),
      }),
    ),
  rejected: () =>
    reduceSurface(
      withProposal(
        proposal({
          state: "Rejected",
          rejection: { rule_id: 2, reason_code: 22, evidence_root: hex(0xe7, 32) },
        }),
      ),
    ),
  expired: () => reduceSurface(withProposal(proposal({ state: "Expired" }))),
  review_incomplete: () =>
    reduceSurface(
      withProposal(proposal(), {
        run: withRun({
          stage: "ai_review",
          stages: [{ name: "ai_review", status: "Incomplete", detail: "timeout", elapsed_ms: 1 }],
        }),
      }),
    ),
  reverted: () =>
    reduceSurface(
      withProposal(proposal(SIGNED), {
        evm: ok({ status: "reverted", block: 42, block_hash: hex(0xb0, 32) }),
      }),
    ),
  error: () =>
    reduceSurface(
      input({
        submission: {
          phase: "failed",
          cause: "gate_refused",
          detail: "E21_ACTIVE_PROPOSAL_EXISTS",
        },
      }),
    ),
  network_unavailable: () =>
    reduceSurface(withProposal(proposal(), { run: down(3, run({ stage: "r1_layout" })) })),
  unknown: () =>
    reduceSurface(withProposal(proposal(), { run: withRun({ stage: "from_the_future" }) })),
};

describe("surface state matrix", () => {
  for (const [kind, make] of Object.entries(scenarios)) {
    it(`reaches ${kind}`, () => {
      expect(make().kind).toBe(kind);
    });
  }

  it("covers every DESIGN state exactly once", () => {
    expect(Object.keys(scenarios).sort()).toEqual(
      [
        "idle",
        "submitting",
        "verifying",
        "verifier_failed",
        "ai_review",
        "certified",
        "signing",
        "signing_retry",
        "relaying",
        "executed",
        "rejected",
        "expired",
        "review_incomplete",
        "reverted",
        "error",
        "network_unavailable",
        "unknown",
      ].sort(),
    );
  });
});

describe("idle and submitting", () => {
  it("idle needs no submission and no proposal", () => {
    expect(reduceSurface(input())).toEqual({ kind: "idle" });
  });

  it("sealing and registering are submitting while no proposal id is observed", () => {
    expect(reduceSurface(input({ submission: { phase: "sealing" } }))).toEqual({
      kind: "submitting",
      step: "sealing",
    });
    expect(reduceSurface(input({ submission: { phase: "registering" } }))).toEqual({
      kind: "submitting",
      step: "registering",
    });
    // A fetch for a proposal the gate does not have yet is still just submitting.
    expect(
      reduceSurface(input({ submission: { phase: "registering" }, proposal: ok(null) })).kind,
    ).toBe("submitting");
  });

  it("once the proposal is observed the gate wins over a stale submitting phase", () => {
    expect(
      reduceSurface(withProposal(proposal(), { submission: { phase: "registering" } })).kind,
    ).toBe("verifying");
  });
});

describe("verifying and AI review", () => {
  it("a pending proposal with no run observed is verifying with no stage claimed", () => {
    expect(reduceSurface(withProposal(proposal()))).toEqual({
      kind: "verifying",
      stage: null,
      runObserved: false,
      pendingVerdict: null,
    });
    expect(reduceSurface(withProposal(proposal(), { run: ok(null) })).kind).toBe("verifying");
  });

  it("every deterministic and tail stage is verifying with that stage", () => {
    for (const stage of [
      "queued",
      "unsealing",
      "compiling",
      "r1_layout",
      "r2_solvency",
      "r3_filter",
      "r4_initializer",
      "r5_replay",
      "signing",
      "submitting",
      "done",
    ]) {
      expect(reduceSurface(withProposal(proposal(), { run: withRun({ stage }) })), stage).toEqual({
        kind: "verifying",
        stage,
        runObserved: true,
        pendingVerdict: null,
      });
    }
  });

  it("ai_review and reproducing are the AI state", () => {
    for (const stage of ["ai_review", "reproducing"] as const) {
      expect(reduceSurface(withProposal(proposal(), { run: withRun({ stage }) }))).toEqual({
        kind: "ai_review",
        stage,
      });
    }
  });

  it("a run verdict the gate has not recorded does not become rejected or certified", () => {
    const reject = reduceSurface(
      withProposal(proposal(), {
        run: withRun({
          stage: "submitting",
          verdict: { kind: "Reject", rule_id: 2, reason_code: 22 },
        }),
      }),
    );
    expect(reject).toMatchObject({
      kind: "verifying",
      stage: "submitting",
      pendingVerdict: { kind: "Reject", rule_id: 2, reason_code: 22 },
    });
    const pass = reduceSurface(
      withProposal(proposal(), {
        run: withRun({ stage: "submitting", verdict: { kind: "Pass" } }),
      }),
    );
    expect(pass).toMatchObject({ kind: "verifying", pendingVerdict: { kind: "Pass" } });
  });

  it("an incomplete AI stage is review_incomplete with the proposal deadline, not a pass or a refusal", () => {
    const state = reduceSurface(
      withProposal(proposal(), {
        run: withRun({
          stage: "reproducing",
          stages: [
            {
              name: "reproducing",
              status: "Incomplete",
              detail: "wrong signature kind",
              elapsed_ms: null,
            },
          ],
        }),
      }),
    );
    expect(state).toEqual({
      kind: "review_incomplete",
      stage: "reproducing",
      deadlineNs: 1_793_701_800_123_456_789n,
    });
  });

  it("an incomplete deterministic stage is an error naming the stage, never review_incomplete", () => {
    const state = reduceSurface(
      withProposal(proposal(), {
        run: withRun({
          stage: "r2_solvency",
          stages: [
            { name: "r2_solvency", status: "Incomplete", detail: "solver timeout", elapsed_ms: 9 },
          ],
        }),
      }),
    );
    expect(state).toMatchObject({ kind: "error", cause: "verifier_incomplete" });
    expect((state as { detail: string }).detail).toContain("r2_solvency");
  });
});

describe("failed verifier runs", () => {
  const failedRun = (over: Partial<VerifierRun> = {}) =>
    withRun({ stage: "failed", error_code: "rpc_unavailable", ...over });

  it("verifying -> failed is verifier_failed with the error code, retryable, and still no verdict", () => {
    const verifying = reduceSurface(
      withProposal(proposal(), { run: withRun({ stage: "r5_replay" }) }),
    );
    expect(verifying.kind).toBe("verifying");
    const failed = reduceSurface(withProposal(proposal(), { run: failedRun() }), verifying.kind);
    expect(failed).toEqual({
      kind: "verifier_failed",
      errorCode: "rpc_unavailable",
      retryable: true,
    });
  });

  it("a failed run without a named cause keeps errorCode null; an unknown code stays unknown_code", () => {
    expect(
      reduceSurface(withProposal(proposal(), { run: failedRun({ error_code: null }) })),
    ).toEqual({ kind: "verifier_failed", errorCode: null, retryable: true });
    expect(
      reduceSurface(withProposal(proposal(), { run: failedRun({ error_code: "unknown_code" }) })),
    ).toMatchObject({ kind: "verifier_failed", errorCode: "unknown_code" });
  });

  it("a failed run beats an Incomplete stage it recorded, so it is never review_incomplete", () => {
    const state = reduceSurface(
      withProposal(proposal(), {
        run: failedRun({
          error_code: "tool_output_invalid",
          stages: [{ name: "ai_review", status: "Incomplete", detail: "x", elapsed_ms: 1 }],
        }),
      }),
    );
    expect(state).toMatchObject({ kind: "verifier_failed", errorCode: "tool_output_invalid" });
  });

  it("done runs with AI withheld or submit Incomplete stay distinct from failed", () => {
    const withheld = reduceSurface(
      withProposal(proposal(), {
        run: withRun({
          stage: "done",
          outcome: "withheld: AI review incomplete; no Pass without provider_tee Clean",
          stages: [{ name: "ai_review", status: "Incomplete", detail: "timeout", elapsed_ms: 1 }],
        }),
      }),
    );
    expect(withheld.kind).toBe("review_incomplete");
    const submit = reduceSurface(
      withProposal(proposal(), {
        run: withRun({
          stage: "done",
          verdict: { kind: "Pass" },
          stages: [{ name: "submit", status: "Incomplete", detail: "relay lost", elapsed_ms: 1 }],
        }),
      }),
    );
    expect(submit).toMatchObject({
      kind: "error",
      cause: "verifier_incomplete",
      detail: expect.stringContaining("submit"),
    });
  });

  it("the proposal stays Pending: a failed run never makes it rejected, expired or certified", () => {
    const state = reduceSurface(withProposal(proposal({ state: "Pending" }), { run: failedRun() }));
    expect(state.kind).toBe("verifier_failed");
    expect(JSON.stringify(state)).not.toMatch(/certificate|verdict|rejected/i);
  });

  it("a failed run reports no certificate even when the run object were to carry one", () => {
    const state = reduceSurface(
      withProposal(proposal(), {
        run: failedRun({ certificate_b64: "AAEC", verdict: { kind: "Pass" } }),
      }),
    );
    expect(state).toEqual({
      kind: "verifier_failed",
      errorCode: "rpc_unavailable",
      retryable: true,
    });
  });

  it("failed, then a later successful run replaces it and the gate wins once it moves on", () => {
    const base = proposal();
    const failed = reduceSurface(withProposal(base, { run: failedRun() }));
    const retry = reduceSurface(
      withProposal(base, { run: withRun({ stage: "compiling" }) }),
      failed.kind,
    );
    expect(retry).toMatchObject({ kind: "verifying", stage: "compiling", runObserved: true });
    const certified = reduceSurface(
      withProposal(proposal({ state: "SignRequested", ...CERTIFIED }), {
        run: withRun({ stage: "done", verdict: { kind: "Pass" } }),
      }),
      retry.kind,
    );
    expect(certified).toEqual({ kind: "certified", certificateHash: hex(0xce, 32) });
    // An older failed run left in the input does not hide the gate's later outcome.
    const rejected = reduceSurface(
      withProposal(
        proposal({
          state: "Rejected",
          rejection: { rule_id: 2, reason_code: 22, evidence_root: hex(0xe7, 32) },
        }),
        { run: failedRun() },
      ),
    );
    expect(rejected.kind).toBe("rejected");
  });

  it("a refused retry shows over the failed run until the gate moves", () => {
    const refused = {
      phase: "failed",
      cause: "run_refused",
      detail: "rate_limited; retry after 7s",
    } as const;
    expect(
      reduceSurface(withProposal(proposal(), { run: failedRun(), submission: refused })),
    ).toEqual({ kind: "error", cause: "run_refused", detail: "rate_limited; retry after 7s" });
  });

  it("three failed fetches while showing a failed run is an outage that keeps it as last known", () => {
    const state = reduceSurface(
      withProposal(proposal(), {
        run: down(3, run({ stage: "failed", error_code: "storage_error" })),
      }),
    );
    expect(state).toMatchObject({
      kind: "network_unavailable",
      source: "verifier",
      lastKnown: { kind: "verifier_failed", errorCode: "storage_error" },
    });
  });
});

describe("evicted runs", () => {
  it("a 404 for a run the page holds is run_evicted, not a network error, and the proposal stays Pending", () => {
    const state = reduceSurface(withProposal(proposal(), { run: ok(null), runId: 12 }));
    expect(state).toMatchObject({
      kind: "error",
      cause: "run_evicted",
      detail: expect.stringContaining("12"),
    });
  });

  it("without a held run id, no run is just verifying; a never-polled run is not evicted", () => {
    expect(reduceSurface(withProposal(proposal(), { run: ok(null), runId: null })).kind).toBe(
      "verifying",
    );
    expect(reduceSurface(withProposal(proposal(), { run: idle, runId: 12 })).kind).toBe(
      "verifying",
    );
  });

  it("an evicted run does not hide the gate's outcome", () => {
    const state = reduceSurface(
      withProposal(proposal({ state: "SignRequested", ...CERTIFIED }), {
        run: ok(null),
        runId: 12,
      }),
    );
    expect(state.kind).toBe("certified");
  });
});

describe("gate-recorded outcomes", () => {
  it("rejected takes the rule, reason and evidence root from the gate, and claims no signature was requested", () => {
    const state = reduceSurface(
      withProposal(
        proposal({
          state: "Rejected",
          rejection: { rule_id: 2, reason_code: 22, evidence_root: hex(0xe7, 32) },
        }),
      ),
    );
    expect(state).toEqual({
      kind: "rejected",
      ruleId: 2,
      reasonCode: 22,
      evidenceRoot: hex(0xe7, 32),
      noSignatureRequested: true,
      implementationUnchanged: false,
      unchangedAtBlock: null,
    });
  });

  it("rejected claims 'unchanged' only after two equal proxy reads", () => {
    const rejected = proposal({
      state: "Rejected",
      rejection: { rule_id: 2, reason_code: 22, evidence_root: hex(0xe7, 32) },
    });
    const one = reduceSurface(
      withProposal(rejected, { proxy: reads({ after: proxyRead(OLD_IMPL, 100) }) }),
    );
    expect(one).toMatchObject({ kind: "rejected", implementationUnchanged: false });
    const differs = reduceSurface(
      withProposal(rejected, {
        proxy: reads({ before: proxyRead(OLD_IMPL, 90), after: proxyRead(NEW_IMPL, 100) }),
      }),
    );
    expect(differs).toMatchObject({ kind: "rejected", implementationUnchanged: false });
    const equal = reduceSurface(
      withProposal(rejected, {
        proxy: reads({ before: proxyRead(OLD_IMPL, 90), after: proxyRead(OLD_IMPL, 100) }),
      }),
    );
    expect(equal).toMatchObject({
      kind: "rejected",
      implementationUnchanged: true,
      unchangedAtBlock: 100,
    });
  });

  it("a Rejected proposal with no rejection data is unknown, not a fabricated refusal", () => {
    expect(reduceSurface(withProposal(proposal({ state: "Rejected" }))).kind).toBe("unknown");
  });

  it("expired follows the same unchanged rule", () => {
    const expired = proposal({ state: "Expired" });
    expect(reduceSurface(withProposal(expired))).toEqual({
      kind: "expired",
      implementationUnchanged: false,
      unchangedAtBlock: null,
    });
    expect(
      reduceSurface(
        withProposal(expired, {
          proxy: reads({ before: proxyRead(OLD_IMPL, 1), after: proxyRead(OLD_IMPL, 2) }),
        }),
      ),
    ).toEqual({ kind: "expired", implementationUnchanged: true, unchangedAtBlock: 2 });
  });
});

describe("certified, signing and relaying", () => {
  const requested = proposal({ state: "SignRequested", ...CERTIFIED });

  it("the first observation of SignRequested is certified; later ones are signing", () => {
    expect(reduceSurface(withProposal(requested))).toEqual({
      kind: "certified",
      certificateHash: hex(0xce, 32),
    });
    for (const previous of [
      undefined,
      "idle",
      "submitting",
      "verifying",
      "ai_review",
      "unknown",
    ] as const) {
      expect(reduceSurface(withProposal(requested), previous).kind, String(previous)).toBe(
        "certified",
      );
    }
    for (const previous of ["certified", "signing", "signing_retry", "relaying"] as const) {
      expect(reduceSurface(withProposal(requested), previous), previous).toEqual({
        kind: "signing",
        attempt: 1,
        nonce: 3,
      });
    }
  });

  it("SignFailed is signing_retry with its attempt count", () => {
    const failed = proposal({ state: "SignFailed", ...CERTIFIED, evm: evmTx({ attempts: 2 }) });
    expect(reduceSurface(withProposal(failed))).toEqual({
      kind: "signing_retry",
      attempt: 2,
      nonce: 3,
    });
  });

  it("SignRequested without a certificate or transaction is unknown", () => {
    expect(reduceSurface(withProposal(proposal({ state: "SignRequested" }))).kind).toBe("unknown");
  });

  it("Signed relays: signed, pending and not-found all stay relaying with the transaction hash", () => {
    for (const evm of [idle, ok<TxObservation>({ status: "not_found" })]) {
      expect(reduceSurface(withProposal(proposal(SIGNED), { evm }))).toEqual({
        kind: "relaying",
        evm: "signed",
        txHash: SIGNED_TX_HASH,
      });
    }
    expect(
      reduceSurface(withProposal(proposal(SIGNED), { evm: ok({ status: "pending" }) })),
    ).toEqual({
      kind: "relaying",
      evm: "pending",
      txHash: SIGNED_TX_HASH,
    });
  });

  it("an included transaction is not executed until the slot is read at or after its block", () => {
    const base = withProposal(proposal(SIGNED), { evm: ok(included) });
    expect(reduceSurface(base)).toMatchObject({ kind: "relaying", evm: "included_unverified" });
    const stale = reduceSurface({ ...base, proxy: reads({ after: proxyRead(NEW_IMPL, 41) }) });
    expect(stale).toMatchObject({ kind: "relaying", evm: "included_unverified" });
    const fresh = reduceSurface({
      ...base,
      proxy: reads({ before: proxyRead(OLD_IMPL, 30), after: proxyRead(NEW_IMPL, 42) }),
    });
    expect(fresh).toEqual({
      kind: "executed",
      block: 42,
      txHash: SIGNED_TX_HASH,
      implementationBefore: OLD_IMPL,
      implementationAfter: NEW_IMPL,
    });
    const noBefore = reduceSurface({ ...base, proxy: reads({ after: proxyRead(NEW_IMPL, 42) }) });
    expect(noBefore).toMatchObject({ kind: "executed", implementationBefore: null });
  });

  it("an included transaction whose slot differs from the proposal's implementation is an error, never executed", () => {
    const state = reduceSurface(
      withProposal(proposal(SIGNED), {
        evm: ok(included),
        proxy: reads({ after: proxyRead(OLD_IMPL, 100) }),
      }),
    );
    expect(state).toMatchObject({ kind: "error", cause: "implementation_mismatch" });
  });

  it("a reverted transaction is reverted with its block", () => {
    const state = reduceSurface(
      withProposal(proposal(SIGNED), {
        evm: ok({ status: "reverted", block: 42, block_hash: hex(0xb0, 32) }),
      }),
    );
    expect(state).toEqual({ kind: "reverted", block: 42, txHash: SIGNED_TX_HASH });
  });

  it("Signed with no stored transaction is unknown", () => {
    expect(reduceSurface(withProposal(proposal({ ...SIGNED, evm: evmTx() }))).kind).toBe("unknown");
  });
});

describe("errors", () => {
  it("a failed registration is an error with its cause", () => {
    expect(
      reduceSurface(
        input({
          submission: { phase: "failed", cause: "gate_refused", detail: "E24_DUPLICATE_ACTIVE" },
        }),
      ),
    ).toEqual({
      kind: "error",
      cause: "gate_refused",
      detail: "E24_DUPLICATE_ACTIVE",
    });
  });

  it("a failed run post shows over a pending proposal but never over a terminal gate outcome", () => {
    const failed = { phase: "failed", cause: "verifier_unreachable", detail: "down" } as const;
    expect(reduceSurface(withProposal(proposal(), { submission: failed })).kind).toBe("error");
    const rejected = proposal({
      state: "Rejected",
      rejection: { rule_id: 1, reason_code: 10, evidence_root: hex(0xe7, 32) },
    });
    expect(reduceSurface(withProposal(rejected, { submission: failed })).kind).toBe("rejected");
    expect(
      reduceSurface(withProposal(proposal({ state: "Expired" }), { submission: failed })).kind,
    ).toBe("expired");
    const executed = withProposal(proposal(SIGNED), {
      submission: { phase: "failed", cause: "broadcast_failed", detail: "x" },
      evm: ok(included),
      proxy: reads({ after: proxyRead(NEW_IMPL, 100) }),
    });
    expect(reduceSurface(executed).kind).toBe("executed");
    expect(reduceSurface({ ...executed, evm: idle, proxy: idle }).kind).toBe("error");
  });

  it("a proposal the gate does not have is an error unless a submission is still in flight", () => {
    expect(reduceSurface(input({ proposal: ok(null) }))).toMatchObject({
      kind: "error",
      cause: "proposal_not_found",
    });
  });
});

describe("network unavailable and unknown", () => {
  const pendingRun = run({ stage: "r1_layout" });

  it("fewer than three failures keep showing the last known state", () => {
    for (const failures of [1, 2]) {
      expect(
        reduceSurface(withProposal(proposal(), { run: down(failures, pendingRun) })).kind,
        String(failures),
      ).toBe("verifying");
    }
  });

  it("three consecutive failures wrap the last known state and name the source and last observation", () => {
    const state = reduceSurface(withProposal(proposal(), { run: down(3, pendingRun) }));
    expect(state).toEqual({
      kind: "network_unavailable",
      source: "verifier",
      failures: 3,
      lastObservedAtMs: T0,
      lastKnown: { kind: "verifying", stage: "r1_layout", runObserved: true, pendingVerdict: null },
    });
  });

  it("NEAR, Base and verifier outages are each attributed", () => {
    const near = reduceSurface(input({ proposal: down(4, proposal()) }));
    expect(near).toMatchObject({
      kind: "network_unavailable",
      source: "near",
      failures: 4,
      lastKnown: { kind: "verifying" },
    });
    const base = reduceSurface(
      withProposal(proposal(SIGNED), { evm: down(3, { status: "pending" }) }),
    );
    expect(base).toMatchObject({
      kind: "network_unavailable",
      source: "base",
      lastKnown: { kind: "relaying", evm: "pending" },
    });
    const proxy = reduceSurface(
      withProposal(proposal(SIGNED), { evm: ok(included), proxy: down(5) }),
    );
    expect(proxy).toMatchObject({ kind: "network_unavailable", source: "base", failures: 5 });
  });

  it("an outage with nothing ever observed has no last known state", () => {
    expect(reduceSurface(input({ proposal: down(3) }))).toEqual({
      kind: "network_unavailable",
      source: "near",
      failures: 3,
      lastObservedAtMs: null,
      lastKnown: null,
    });
  });

  it("a verifier outage does not disturb a state the verifier no longer feeds", () => {
    const rejected = proposal({
      state: "Rejected",
      rejection: { rule_id: 2, reason_code: 22, evidence_root: hex(0xe7, 32) },
    });
    expect(reduceSurface(withProposal(rejected, { run: down(9, pendingRun) })).kind).toBe(
      "rejected",
    );
    expect(
      reduceSurface(
        withProposal(proposal({ state: "SignRequested", ...CERTIFIED }), {
          run: down(9, pendingRun),
        }),
        "certified",
      ).kind,
    ).toBe("signing");
  });

  it("recovery: a fresh ok observation after failures returns to the live state", () => {
    const state = reduceSurface(
      withProposal(proposal(), { run: ok<VerifierRun | null>(pendingRun) }),
    );
    expect(state.kind).toBe("verifying");
  });

  it("a first failed fetch with no data yet is unknown, not idle", () => {
    expect(reduceSurface(input({ proposal: down(1) }))).toEqual({
      kind: "unknown",
      reason: "the gate proposal has not been observed yet",
    });
  });

  it("an unrecognized verifier stage is unknown and says which", () => {
    const state = reduceSurface(
      withProposal(proposal(), { run: withRun({ stage: "teleporting" }) }),
    );
    expect(state).toMatchObject({ kind: "unknown" });
    expect((state as { reason: string }).reason).toContain("teleporting");
  });
});
