import { keccak256 } from "viem";
import type { ProxyReads, TxObservation } from "./base.ts";
import type { Proposal } from "./gate-types.ts";
import { implementationUnchanged } from "./receipt.ts";
import {
  isVerifierStage,
  type RunErrorCode,
  type RunVerdict,
  type VerifierRun,
  type VerifierStage,
} from "./verifier.ts";

/**
 * Pure reducer from what the page has observed to the surface states of docs/DESIGN.md 6.0.
 *
 * No fake progress: every field of a state comes from an observation, and anything the inputs do
 * not support becomes an explicit `unknown` state naming the reason. The reducer never reads a
 * clock; callers pass timestamps they recorded when they observed a value.
 *
 * [I] Decisions where DESIGN leaves room:
 * - The gate is the authority. A verifier run that reports `Reject` or `Pass` while the gate is
 *   still `Pending` stays `verifying` (the verdict is not recorded yet) and carries
 *   `pendingVerdict`. `rejected` needs gate state `Rejected`.
 * - The gate moves `Pending` to `SignRequested` in the same transaction that records a `Pass`,
 *   so `certified` and `signing` are one gate state. `certified` is the first observation of it,
 *   decided by the `previous` state kind the caller passes back in; later observations are `signing`.
 * - `executed` needs all of: gate `Signed`, an included successful transaction, and a read of the
 *   ERC-1967 slot at or after the inclusion block equal to the proposal's `new_impl`.
 * - A run in stage `failed` ends that run, never the proposal: while the gate is `Pending` the
 *   state is `verifier_failed` (retryable) and carries no verdict and no certificate. A later run
 *   replaces it, and the gate's own state always wins once it moves past `Pending`. It is kept
 *   apart from `review_incomplete` (a `done` run whose AI review is Incomplete) and from `error`
 *   `verifier_incomplete` (for example a `done` run whose `submit` stage is Incomplete).
 * - `runId` is the run the page started or remembered. With it set, a verifier that no longer
 *   knows the run (404, observed as `ok(null)`) means the run was evicted: an `error` state with
 *   cause `run_evicted`, not a network outage. The proposal stays `Pending` and may be retried.
 */

export type Source<T> =
  | { status: "idle" }
  | { status: "ok"; value: T; observedAtMs: number }
  | {
      status: "unavailable";
      /** Consecutive failed fetches. Three or more is an outage (DESIGN 6.15). */
      failures: number;
      error: string;
      /** The last good value, kept so stale data stays visible. */
      last?: { value: T; observedAtMs: number };
    };

export type SurfaceErrorCause =
  | "gate_refused"
  | "verifier_unreachable"
  | "broadcast_failed"
  | "receipt_schema"
  | "proposal_not_found"
  | "implementation_mismatch"
  | "verifier_incomplete"
  | "run_refused"
  | "run_evicted"
  | "session_key"
  | "wallet_declined"
  | "other";

/** The page's own write path: sealing the bundle, sending `propose`, posting the run. */
export type Submission =
  | { phase: "none" }
  | { phase: "sealing" }
  | { phase: "registering" }
  | { phase: "failed"; cause: SurfaceErrorCause; detail: string };

export interface SurfaceInput {
  submission: Submission;
  /** Gate view `get_proposal`; `null` means the gate has no such proposal. */
  proposal: Source<Proposal | null>;
  run: Source<VerifierRun | null>;
  /** The verifier run id the page holds, if any. See the module note on eviction. */
  runId?: number | null;
  /** Lookup of the gate's signed transaction on Base Sepolia. */
  evm: Source<TxObservation>;
  proxy: Source<ProxyReads>;
}

export type OutageSource = "near" | "verifier" | "base";

export type SurfaceState =
  | { kind: "idle" }
  | { kind: "submitting"; step: "sealing" | "registering" }
  | {
      kind: "verifying";
      stage: VerifierStage | null;
      runObserved: boolean;
      pendingVerdict: RunVerdict | null;
    }
  | {
      kind: "verifier_failed";
      /** `null` when the verifier named no cause. */
      errorCode: RunErrorCode | null;
      /** Always true: the proposal is still Pending at the gate and a new run may be started. */
      retryable: true;
    }
  | { kind: "ai_review"; stage: "ai_review" | "reproducing" }
  | { kind: "certified"; certificateHash: string }
  | { kind: "signing"; attempt: number; nonce: number }
  | { kind: "signing_retry"; attempt: number; nonce: number }
  | { kind: "relaying"; evm: "signed" | "pending" | "included_unverified"; txHash: string }
  | {
      kind: "executed";
      block: number;
      txHash: string;
      implementationBefore: string | null;
      implementationAfter: string;
    }
  | {
      kind: "rejected";
      ruleId: number;
      reasonCode: number;
      evidenceRoot: string;
      /** The gate rejected from `Pending`, so it never requested a signature. */
      noSignatureRequested: true;
      /** True only after two observed reads agree. */
      implementationUnchanged: boolean;
      unchangedAtBlock: number | null;
    }
  | { kind: "expired"; implementationUnchanged: boolean; unchangedAtBlock: number | null }
  | { kind: "review_incomplete"; stage: "ai_review" | "reproducing"; deadlineNs: bigint }
  | { kind: "reverted"; block: number; txHash: string }
  | { kind: "error"; cause: SurfaceErrorCause; detail: string }
  | {
      kind: "network_unavailable";
      source: OutageSource;
      failures: number;
      lastObservedAtMs: number | null;
      /** The state computed from last observed values, or `null` when nothing was ever observed. */
      lastKnown: Exclude<SurfaceState, { kind: "network_unavailable" }> | null;
    }
  | { kind: "unknown"; reason: string };

export type SurfaceKind = SurfaceState["kind"];

/** Three consecutive failed fetches make an outage (DESIGN 6.15). */
export const OUTAGE_FAILURES = 3;

const AI_STAGES = ["ai_review", "reproducing"] as const;
const SIGNING_FAMILY: readonly SurfaceKind[] = [
  "certified",
  "signing",
  "signing_retry",
  "relaying",
  "executed",
  "reverted",
];
const TERMINAL: readonly SurfaceKind[] = ["rejected", "expired", "executed", "reverted"];

function lastValue<T>(source: Source<T>): { value: T; observedAtMs: number } | undefined {
  if (source.status === "ok") return { value: source.value, observedAtMs: source.observedAtMs };
  if (source.status === "unavailable") return source.last;
  return undefined;
}

function proxyEvidence(input: SurfaceInput): {
  implementationUnchanged: boolean;
  unchangedAtBlock: number | null;
} {
  const reads = lastValue(input.proxy)?.value;
  const unchanged = implementationUnchanged(reads);
  return {
    implementationUnchanged: unchanged,
    unchangedAtBlock: unchanged ? (reads?.after?.block ?? null) : null,
  };
}

function fromPending(input: SurfaceInput, proposal: Proposal): SurfaceState {
  const observed = lastValue(input.run);
  const run = observed?.value ?? null;
  if (run === null) {
    if (observed !== undefined && input.runId !== undefined && input.runId !== null) {
      return {
        kind: "error",
        cause: "run_evicted",
        detail: `the verifier no longer has run ${input.runId}; the proposal is still Pending and a new run may be started`,
      };
    }
    return { kind: "verifying", stage: null, runObserved: false, pendingVerdict: null };
  }
  if (run.stage === "failed") {
    return { kind: "verifier_failed", errorCode: run.error_code, retryable: true };
  }
  const incomplete = run.stages.find((s) => s.status === "Incomplete");
  if (incomplete !== undefined) {
    const ai = AI_STAGES.find((stage) => stage === incomplete.name);
    if (ai !== undefined)
      return { kind: "review_incomplete", stage: ai, deadlineNs: proposal.expires_at_ns };
    return {
      kind: "error",
      cause: "verifier_incomplete",
      detail: `verifier stage ${incomplete.name} was incomplete: ${incomplete.detail}`,
    };
  }
  if (!isVerifierStage(run.stage)) {
    return {
      kind: "unknown",
      reason: `the verifier reported a stage this page does not know: ${run.stage}`,
    };
  }
  const ai = AI_STAGES.find((stage) => stage === run.stage);
  if (ai !== undefined) return { kind: "ai_review", stage: ai };
  return { kind: "verifying", stage: run.stage, runObserved: true, pendingVerdict: run.verdict };
}

function fromSigned(input: SurfaceInput, proposal: Proposal): SurfaceState {
  const signed = proposal.evm?.signed_tx;
  if (signed === null || signed === undefined) {
    return { kind: "unknown", reason: "the gate reports Signed but stores no signed transaction" };
  }
  const txHash = keccak256(signed as `0x${string}`);
  const observation = lastValue(input.evm)?.value;
  if (observation === undefined || observation.status === "not_found") {
    return { kind: "relaying", evm: "signed", txHash };
  }
  if (observation.status === "pending") return { kind: "relaying", evm: "pending", txHash };
  if (observation.status === "reverted")
    return { kind: "reverted", block: observation.block, txHash };
  const after = lastValue(input.proxy)?.value.after;
  if (after === undefined || after.block < observation.block) {
    return { kind: "relaying", evm: "included_unverified", txHash };
  }
  if (after.implementation !== proposal.new_impl) {
    return {
      kind: "error",
      cause: "implementation_mismatch",
      detail: `the transaction is included but the proxy implementation is ${after.implementation}, not ${proposal.new_impl}`,
    };
  }
  return {
    kind: "executed",
    block: observation.block,
    txHash,
    implementationBefore: lastValue(input.proxy)?.value.before?.implementation ?? null,
    implementationAfter: after.implementation,
  };
}

function fromProposal(
  input: SurfaceInput,
  proposal: Proposal,
  previous: SurfaceKind | undefined,
): SurfaceState {
  switch (proposal.state) {
    case "Rejected":
      if (proposal.rejection === null) {
        return { kind: "unknown", reason: "the gate reports Rejected without rejection data" };
      }
      return {
        kind: "rejected",
        ruleId: proposal.rejection.rule_id,
        reasonCode: proposal.rejection.reason_code,
        evidenceRoot: proposal.rejection.evidence_root,
        noSignatureRequested: true,
        ...proxyEvidence(input),
      };
    case "Expired":
      return { kind: "expired", ...proxyEvidence(input) };
    case "Pending":
      return fromPending(input, proposal);
    case "SignRequested":
    case "SignFailed": {
      if (proposal.certificate_hash === null || proposal.evm === null) {
        return {
          kind: "unknown",
          reason: `the gate reports ${proposal.state} without a certificate and transaction`,
        };
      }
      const { attempts, nonce } = proposal.evm;
      if (proposal.state === "SignFailed")
        return { kind: "signing_retry", attempt: attempts, nonce };
      if (previous === undefined || !SIGNING_FAMILY.includes(previous)) {
        return { kind: "certified", certificateHash: proposal.certificate_hash };
      }
      return { kind: "signing", attempt: attempts, nonce };
    }
    case "Signed":
      return fromSigned(input, proposal);
  }
}

function reduceCore(input: SurfaceInput, previous: SurfaceKind | undefined): SurfaceState {
  const { submission } = input;
  const proposal = lastValue(input.proposal)?.value;
  let state: SurfaceState;
  if (proposal === undefined) {
    if (submission.phase === "sealing" || submission.phase === "registering") {
      return { kind: "submitting", step: submission.phase };
    }
    if (submission.phase === "failed")
      return { kind: "error", cause: submission.cause, detail: submission.detail };
    if (input.proposal.status === "unavailable") {
      return { kind: "unknown", reason: "the gate proposal has not been observed yet" };
    }
    return { kind: "idle" };
  }
  if (proposal === null) {
    if (submission.phase === "sealing" || submission.phase === "registering") {
      return { kind: "submitting", step: submission.phase };
    }
    state = {
      kind: "error",
      cause: "proposal_not_found",
      detail: "the gate has no proposal with this id",
    };
  } else {
    state = fromProposal(input, proposal, previous);
  }
  // A failed write (for example a refused run post) shows until the chain moves past it.
  if (submission.phase === "failed" && !TERMINAL.includes(state.kind) && state.kind !== "error") {
    return { kind: "error", cause: submission.cause, detail: submission.detail };
  }
  return state;
}

function relevantSources(kind: SurfaceKind): readonly OutageSource[] {
  switch (kind) {
    case "idle":
      return [];
    case "submitting":
    case "certified":
    case "signing":
    case "signing_retry":
      return ["near"];
    case "verifying":
    case "verifier_failed":
    case "ai_review":
    case "review_incomplete":
      return ["near", "verifier"];
    case "rejected":
    case "expired":
    case "relaying":
    case "executed":
    case "reverted":
      return ["near", "base"];
    case "error":
    case "unknown":
    case "network_unavailable":
      return ["near", "verifier", "base"];
  }
}

function sourceFailures(
  input: SurfaceInput,
  source: OutageSource,
): { failures: number; lastObservedAtMs: number | null } {
  const parts: Source<unknown>[] =
    source === "near"
      ? [input.proposal]
      : source === "verifier"
        ? [input.run]
        : [input.evm, input.proxy];
  let failures = 0;
  let lastObservedAtMs: number | null = null;
  for (const part of parts) {
    if (part.status === "unavailable") failures = Math.max(failures, part.failures);
    const last = lastValue(part);
    if (last !== undefined) lastObservedAtMs = Math.max(lastObservedAtMs ?? 0, last.observedAtMs);
  }
  return { failures, lastObservedAtMs };
}

/**
 * Reduce observations to one surface state. `previous` is the kind this surface showed last time;
 * it only decides `certified` versus `signing` (see the module note).
 */
export function reduceSurface(input: SurfaceInput, previous?: SurfaceKind): SurfaceState {
  const core = reduceCore(input, previous);
  if (core.kind === "network_unavailable") return core;
  for (const source of relevantSources(core.kind)) {
    const { failures, lastObservedAtMs } = sourceFailures(input, source);
    if (failures >= OUTAGE_FAILURES) {
      const sources: Source<unknown>[] = [input.proposal, input.run, input.evm, input.proxy];
      const everObserved = sources.some((s) => lastValue(s) !== undefined);
      return {
        kind: "network_unavailable",
        source,
        failures,
        lastObservedAtMs,
        lastKnown: everObserved && core.kind !== "unknown" ? core : null,
      };
    }
  }
  return core;
}
