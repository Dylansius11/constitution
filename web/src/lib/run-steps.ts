import type { SurfaceState } from "./state.ts";
import type { Stage } from "./surface-view.ts";

/**
 * The run panel's stepper (DESIGN 5.4): two wallet steps, nine pipeline steps the verifier really
 * reports, and the gate's verdict. Pure functions only, so every state is testable without a DOM.
 */

export type StepState = "pending" | "active" | "pass" | "refused" | "error" | "notrun";

export interface Step {
  id: string;
  label: string;
  state: StepState;
  /** The word beside the glyph; never colour alone. */
  status: string;
}

/** The nine pipeline steps, in the order `surfaceView` lists the same stages. */
export const PIPELINE = [
  { id: "register", label: "Register", done: "Recorded" },
  { id: "queued", label: "Queued", done: "Started" },
  { id: "unsealing", label: "Unsealing", done: "Opened" },
  { id: "compiling", label: "Compiling", done: "Built" },
  { id: "r1_layout", label: "R1 Layout", done: "Passed" },
  { id: "r2_solvency", label: "R2 Solvency", done: "Passed" },
  { id: "r3_filter", label: "R3 Filter", done: "Passed" },
  { id: "r4_initializer", label: "R4 Initializer", done: "Passed" },
  { id: "r5_replay", label: "R5 Replay", done: "Passed" },
] as const;

/** Index of the "Recording the verdict on NEAR" stage in `surfaceView().stages`. */
const SUBMITTING = 12;

export interface WalletSteps {
  connected: boolean;
  connecting: boolean;
  connectFailed: boolean;
  /** `null` until the account's standing on the gate has been read. */
  standing: "reading" | "unreadable" | "needs_credit" | "ready" | null;
}

type Pipeline = Exclude<SurfaceState, { kind: "network_unavailable" }>;

function settled(state: SurfaceState): Pipeline {
  return state.kind === "network_unavailable" ? (state.lastKnown ?? { kind: "idle" }) : state;
}

const FAILED = new Set(["error", "verifier_failed", "unknown", "reverted"]);
const AFTER_RULES = new Set([
  "ai_review",
  "reproducing",
  "certified",
  "signing",
  "signing_retry",
  "relaying",
  "review_incomplete",
]);

function walletSteps(wallet: WalletSteps): Step[] {
  const connect: Step = wallet.connected
    ? { id: "connect", label: "Connect", state: "pass", status: "Connected" }
    : wallet.connecting
      ? { id: "connect", label: "Connect", state: "active", status: "In your wallet" }
      : wallet.connectFailed
        ? { id: "connect", label: "Connect", state: "error", status: "Not connected" }
        : { id: "connect", label: "Connect", state: "active", status: "Your turn" };
  let enable: Step;
  if (!wallet.connected) enable = { id: "enable", label: "Enable", state: "pending", status: "Locked" };
  else if (wallet.standing === null || wallet.standing === "reading")
    enable = { id: "enable", label: "Enable", state: "active", status: "Reading" };
  else if (wallet.standing === "unreadable")
    enable = { id: "enable", label: "Enable", state: "error", status: "Unreadable" };
  else if (wallet.standing === "needs_credit")
    enable = { id: "enable", label: "Enable", state: "active", status: "Needs credit" };
  else enable = { id: "enable", label: "Enable", state: "pass", status: "Enabled" };
  return [connect, enable];
}

function stageStep(index: number, stage: Stage | undefined, failedAt: number): Step {
  const spec = PIPELINE[index];
  if (spec === undefined) throw new RangeError(`no pipeline step ${index}`);
  const base = { id: spec.id, label: spec.label };
  if (index === failedAt) return { ...base, state: "error", status: "Error" };
  if (failedAt >= 0 && index > failedAt) return { ...base, state: "notrun", status: "Not run" };
  switch (stage?.state) {
    case "active":
      return { ...base, state: "active", status: "Running" };
    case "done":
      return { ...base, state: "pass", status: spec.done };
    case "refused":
      return { ...base, state: "refused", status: "Refused" };
    case "skipped":
      return { ...base, state: "notrun", status: "Not run" };
    default:
      return { ...base, state: "pending", status: "Waiting" };
  }
}

function verdictStep(state: Pipeline, stages: readonly Stage[]): Step {
  const base = { id: "verdict", label: "Verdict" };
  if (state.kind === "rejected") return { ...base, state: "refused", status: "Refused" };
  if (state.kind === "executed") return { ...base, state: "pass", status: "Executed" };
  if (FAILED.has(state.kind)) return { ...base, state: "error", status: "No verdict" };
  if (state.kind === "expired") return { ...base, state: "notrun", status: "Expired" };
  if (AFTER_RULES.has(state.kind) || stages[SUBMITTING]?.state === "active")
    return { ...base, state: "active", status: "Recording" };
  return { ...base, state: "pending", status: "Waiting" };
}

/**
 * Twelve steps for one attack. `stages` is `surfaceView(...).stages` of that attack's run, or `null`
 * before the visitor has started it. The wallet steps always reflect the account, never the run.
 */
export function buildSteps(
  wallet: WalletSteps,
  run: { state: SurfaceState; stages: readonly Stage[] } | null,
): Step[] {
  const steps = walletSteps(wallet);
  const state = run === null ? ({ kind: "idle" } as const) : settled(run.state);
  const stages = run?.stages ?? [];
  // A failed run stops at the first stage that never finished; everything after it did not run.
  const failedAt = FAILED.has(state.kind) ? PIPELINE.findIndex((_, i) => stages[i]?.state !== "done") : -1;
  const started = run !== null && state.kind !== "idle";
  for (let i = 0; i < PIPELINE.length; i += 1) {
    steps.push(stageStep(i, started ? stages[i] : undefined, failedAt));
  }
  steps.push(verdictStep(state, stages));
  return steps;
}

/** True once the run can change no further: the clock stops and the controls unlock. */
export function runFinished(steps: readonly Step[]): boolean {
  const verdict = steps.at(-1);
  return verdict !== undefined && ["pass", "refused", "error", "notrun"].includes(verdict.state);
}

/* ---------- clocks ---------- */

export interface StageClock {
  startMs: number | null;
  endMs: number | null;
}

export interface RunClock {
  startMs: number;
  endMs: number | null;
  /** False when the page reloaded mid-run, so durations before this page saw the run are unknown. */
  known: boolean;
  stages: StageClock[];
}

export function newClock(startMs: number, known = true): RunClock {
  return { startMs, endMs: null, known, stages: PIPELINE.map(() => ({ startMs: null, endMs: null })) };
}

/**
 * Records when this page first saw each pipeline step running and finished. A step the page only
 * ever saw finished has no duration: it ran between two polls, and guessing would invent a number.
 * Times are as observed, so each is accurate to the poll interval, not to the verifier's clock.
 */
export function advanceClock(clock: RunClock, steps: readonly Step[], nowMs: number): void {
  const first = 2;
  PIPELINE.forEach((_, i) => {
    const step = steps[first + i];
    const stage = clock.stages[i];
    if (step === undefined || stage === undefined) return;
    const ended = step.state === "pass" || step.state === "refused" || step.state === "error";
    if (stage.startMs === null && step.state === "active") stage.startMs = nowMs;
    if (stage.endMs === null && ended) stage.endMs = nowMs;
  });
  const verdict = steps.at(-1);
  if (clock.endMs === null && verdict !== undefined && runFinished(steps)) clock.endMs = nowMs;
}

/** `0:07`, `1:03`, `12:00`. */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/** Seconds a pipeline step took, as observed; `null` when it never ran or the time is unknown. */
export function stageSeconds(clock: RunClock, index: number, nowMs: number): number | null {
  const stage = clock.stages[index];
  if (stage === undefined || !clock.known || stage.startMs === null) return null;
  return Math.max(0, Math.round(((stage.endMs ?? nowMs) - stage.startMs) / 1000));
}

/** Elapsed run time, live until the verdict; `null` when it cannot be known. */
export function runSeconds(clock: RunClock, nowMs: number): number | null {
  return clock.known ? Math.max(0, Math.floor(((clock.endMs ?? nowMs) - clock.startMs) / 1000)) : null;
}
