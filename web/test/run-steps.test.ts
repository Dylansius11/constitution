import { describe, expect, it } from "vitest";
import {
  advanceClock,
  buildSteps,
  formatElapsed,
  newClock,
  PIPELINE,
  runFinished,
  runSeconds,
  stageSeconds,
  type WalletSteps,
} from "../src/lib/run-steps.ts";
import type { SurfaceState } from "../src/lib/state.ts";
import { type Stage, surfaceView } from "../src/lib/surface-view.ts";

const ready: WalletSteps = { connected: true, connecting: false, connectFailed: false, standing: "ready" };
const anon: WalletSteps = { connected: false, connecting: false, connectFailed: false, standing: null };

const rejected = (ruleId: number): SurfaceState => ({
  kind: "rejected",
  ruleId,
  reasonCode: 20,
  evidenceRoot: `0x${"ab".repeat(32)}`,
  noSignatureRequested: true,
  implementationUnchanged: true,
  unchangedAtBlock: 1,
});

/** Stages in the order the verifier reports them, with the first `done` passed and the next one in `rest`. */
function stagesFor(done: number, next: Stage["state"] | null): Stage[] {
  const all = surfaceView({ kind: "idle" }).stages;
  return all.map((stage, i) => ({
    ...stage,
    state: i < done ? "done" : i === done && next !== null ? next : "pending",
  }));
}

describe("stepper before a run", () => {
  it("has twelve steps, starts at the wallet and locks the rest", () => {
    const steps = buildSteps(anon, null);
    expect(steps).toHaveLength(2 + PIPELINE.length + 1);
    expect(steps.map((s) => s.state)).toEqual(["active", "pending", ...PIPELINE.map(() => "pending"), "pending"]);
    expect(steps[0]).toMatchObject({ status: "Your turn" });
    expect(steps[1]).toMatchObject({ status: "Locked" });
  });

  it("marks the wallet steps done once the account can pay", () => {
    const steps = buildSteps(ready, null);
    expect(steps[0]).toMatchObject({ state: "pass", status: "Connected" });
    expect(steps[1]).toMatchObject({ state: "pass", status: "Enabled" });
  });

  it("shows credit as the current step, and a read failure as an error", () => {
    expect(buildSteps({ ...ready, standing: "needs_credit" }, null)[1]).toMatchObject({
      state: "active",
      status: "Needs credit",
    });
    expect(buildSteps({ ...ready, standing: "unreadable" }, null)[1]).toMatchObject({ state: "error" });
    expect(buildSteps({ ...anon, connectFailed: true }, null)[0]).toMatchObject({ state: "error" });
  });
});

describe("stepper during and after a run", () => {
  it("follows the verifier stage that is running", () => {
    const state: SurfaceState = { kind: "verifying", stage: "compiling", runObserved: true, pendingVerdict: null };
    const steps = buildSteps(ready, { state, stages: stagesFor(3, "active") });
    expect(steps.slice(2, 6).map((s) => s.state)).toEqual(["pass", "pass", "pass", "active"]);
    expect(steps[5]).toMatchObject({ label: "Compiling", status: "Running" });
    expect(steps.at(-1)).toMatchObject({ state: "pending" });
    expect(runFinished(steps)).toBe(false);
  });

  it("presents a refusal as the gate working: R2 refused, R3 to R5 not run, verdict refused", () => {
    const stages = stagesFor(5, "refused").map((s, i) => (i > 5 && i < 9 ? { ...s, state: "skipped" as const } : s));
    const steps = buildSteps(ready, { state: rejected(2), stages });
    const byId = Object.fromEntries(steps.map((s) => [s.id, s]));
    expect(byId["r1_layout"]).toMatchObject({ state: "pass", status: "Passed" });
    expect(byId["r2_solvency"]).toMatchObject({ state: "refused", status: "Refused" });
    expect(byId["r3_filter"]).toMatchObject({ state: "notrun", status: "Not run" });
    expect(byId["r5_replay"]).toMatchObject({ state: "notrun" });
    expect(byId["verdict"]).toMatchObject({ state: "refused" });
    expect(runFinished(steps)).toBe(true);
  });

  it("marks the first unfinished step as an error when the run breaks", () => {
    const state: SurfaceState = { kind: "verifier_failed", errorCode: null, retryable: true };
    const steps = buildSteps(ready, { state, stages: stagesFor(2, "active") });
    const states = steps.slice(2, 11).map((s) => s.state);
    expect(states).toEqual(["pass", "pass", "error", "notrun", "notrun", "notrun", "notrun", "notrun", "notrun"]);
    expect(steps.at(-1)).toMatchObject({ state: "error", status: "No verdict" });
    expect(runFinished(steps)).toBe(true);
  });

  it("reads the last known state while the network is down instead of resetting", () => {
    const lastKnown: SurfaceState = { kind: "verifying", stage: "r1_layout", runObserved: true, pendingVerdict: null };
    const state: SurfaceState = {
      kind: "network_unavailable",
      source: "near",
      failures: 3,
      lastObservedAtMs: 1,
      lastKnown: lastKnown as Exclude<SurfaceState, { kind: "network_unavailable" }>,
    };
    const steps = buildSteps(ready, { state, stages: stagesFor(4, "active") });
    expect(steps[6]).toMatchObject({ label: "R1 Layout", state: "active" });
  });
});

describe("clocks", () => {
  it("formats minutes and seconds", () => {
    expect(formatElapsed(0)).toBe("0:00");
    expect(formatElapsed(7_900)).toBe("0:07");
    expect(formatElapsed(63_000)).toBe("1:03");
    expect(formatElapsed(-5)).toBe("0:00");
  });

  it("times each stage as the page saw it and stops at the verdict", () => {
    const clock = newClock(1_000);
    const view = (done: number, next: Stage["state"] | null, state: SurfaceState) =>
      buildSteps(ready, { state, stages: stagesFor(done, next) });
    const verifying = (stage: "queued" | "compiling"): SurfaceState => ({
      kind: "verifying",
      stage,
      runObserved: true,
      pendingVerdict: null,
    });

    advanceClock(clock, view(1, "active", verifying("queued")), 3_000);
    advanceClock(clock, view(3, "active", verifying("compiling")), 13_000);
    expect(stageSeconds(clock, 1, 13_000)).toBe(10);
    expect(stageSeconds(clock, 3, 18_000)).toBe(5);
    expect(runSeconds(clock, 18_000)).toBe(17);

    const stages = stagesFor(5, "refused").map((s, i) => (i > 5 && i < 9 ? { ...s, state: "skipped" as const } : s));
    advanceClock(clock, buildSteps(ready, { state: rejected(2), stages }), 40_000);
    expect(runSeconds(clock, 99_000)).toBe(39);
    expect(stageSeconds(clock, 3, 99_000)).toBe(27);
    // Seen only after it finished, so no duration is claimed.
    expect(stageSeconds(clock, 5, 99_000)).toBeNull();
    expect(stageSeconds(clock, 6, 99_000)).toBeNull();
  });

  it("reports nothing when the run was resumed after a reload", () => {
    const clock = newClock(1_000, false);
    expect(runSeconds(clock, 50_000)).toBeNull();
    expect(stageSeconds(clock, 0, 50_000)).toBeNull();
  });
});
