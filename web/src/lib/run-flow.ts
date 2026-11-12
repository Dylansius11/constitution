import type { BaseClient, ProxyRead, ProxyReads } from "./base.ts";
import type { SourceBundle } from "./bundle.ts";
import type { GateClient } from "./gate.ts";
import type { Config } from "./gate-types.ts";
import { type AcceptedProposal, GateWriteError, type ProposeArgs, type ProposeSigner } from "./near-write.ts";
import { reasonText } from "./reasons.ts";
import type {
  reduceSurface,
  Source,
  SurfaceErrorCause,
  SurfaceInput,
  SurfaceState,
} from "./state.ts";
import type { SurfaceView, surfaceView } from "./surface-view.ts";
import type { RunRefusedError, VerifierClient } from "./verifier.ts";

/** What one attack proposes. Every value comes from the gallery; nothing is invented here. */
export interface ProposalInput {
  newImpl: string;
  newImplCodehash: string;
  /** Explicitly configured, including empty `0x`; never invent an initializer. */
  initCalldata: string;
  bundle: SourceBundle;
}

export interface RunFlowConfig {
  nearRpc: string;
  gateAccount: string;
  verifierUrl: string;
  baseRpc: string;
  /** Contract name shown in the view, for example `VaultV2Drain`. */
  label: string;
  /** `localStorage` key that remembers this run across reloads. One key per account and attack. */
  storageKey: string;
  proposal: ProposalInput;
}

/** The verifier is occupied; the proposal is recorded and the page is waiting to start its run. */
export interface RunWaiting {
  reason: "verifier_busy";
  retryAtMs: number;
}

export interface RunFlowUpdate {
  state: SurfaceState;
  view: SurfaceView;
  proposalId: number | null;
  nearTxHash: string | null;
  waiting: RunWaiting | null;
  /** The gate's recorded rejection, once observed; the card compares it with the gallery's expectation. */
  rejection: { ruleId: number; reasonCode: number } | null;
}

export interface RunFlow {
  start(): Promise<void>;
  dispose(): void;
}

export type FlowTimer = number | NodeJS.Timeout;
export interface RunFlowDeps {
  gate: Pick<GateClient, "getConfig" | "getProposal">;
  verifier: Pick<VerifierClient, "identity" | "startRun" | "getRun">;
  base: Pick<BaseClient, "readImplementation">;
  propose: ProposeSigner;
  seal(bytes: Uint8Array, transportPubkey: string): Promise<Uint8Array>;
  storage: Pick<Storage, "getItem" | "setItem">;
  now(): number;
  setTimer(callback: () => void, ms: number): FlowTimer;
  clearTimer(timer: FlowTimer): void;
  isHidden(): boolean;
  subscribeVisibility(callback: () => void): () => void;
}

/** Refusals that mean "not now": the proposal stays recorded and the same run is posted again. */
const BUSY_CODES = ["queue_full", "busy", "rate_limited"] as const;
const DEFAULT_RETRY_S = 8;
const MAX_RETRY_S = 60;
/** The gate's proposal TTL is an hour; stop well inside it and leave recovery to the visitor. */
const MAX_BUSY_WAIT_MS = 15 * 60_000;

interface SavedRun {
  version: 1;
  attempted: true;
  proposalId: number | null;
  runId: number | null;
  bootId?: string | null;
  nearTxHash: string | null;
  before?: ProxyRead;
  beforeObservedAtMs?: number;
}

function last<T>(source: Source<T>): { value: T; observedAtMs: number } | undefined {
  if (source.status === "ok") return { value: source.value, observedAtMs: source.observedAtMs };
  return source.status === "unavailable" ? source.last : undefined;
}

function unavailable<T>(source: Source<T>, error: string): Source<T> {
  const observed = last(source);
  return {
    status: "unavailable",
    failures: source.status === "unavailable" ? source.failures + 1 : 1,
    error,
    ...(observed === undefined ? {} : { last: observed }),
  };
}

/**
 * Click-only state machine for one attack. `start` resolves after setup and the first
 * observation, not after finality. It never proposes twice: a stored proposal id is resumed, and a
 * proposal whose verifier run was never recorded gets its run posted again, nothing else.
 */
export function createRunFlow(
  config: RunFlowConfig,
  onUpdate: (update: RunFlowUpdate) => void,
  injected: Partial<RunFlowDeps> & Pick<RunFlowDeps, "propose">,
): RunFlow {
  let disposed = false;
  let stopped = false;
  let polling = false;
  let started: Promise<void> | undefined;
  let timer: FlowTimer | undefined;
  let unsubscribe: (() => void) | undefined;
  let deps: RunFlowDeps;
  let RunRefused: typeof RunRefusedError;
  let runtime: { reduceSurface: typeof reduceSurface; surfaceView: typeof surfaceView };
  let current: SurfaceState = { kind: "idle" };
  let waiting: RunWaiting | null = null;
  let saved: SavedRun = {
    version: 1,
    attempted: true,
    proposalId: null,
    runId: null,
    bootId: null,
    nearTxHash: null,
  };
  const input: SurfaceInput = {
    submission: { phase: "none" },
    proposal: { status: "idle" },
    run: { status: "idle" },
    evm: { status: "idle" },
    proxy: { status: "idle" },
  };

  function emit(): void {
    if (disposed) return;
    input.runId = saved.runId;
    current = runtime.reduceSurface(input, current.kind);
    const observedRun = last(input.run)?.value;
    const observedProposal = last(input.proposal)?.value;
    const rejection = observedProposal?.rejection ?? null;
    const reason = rejection === null ? null : reasonText(rejection.reason_code);
    onUpdate({
      state: current,
      view: runtime.surfaceView(current, {
        fixture: config.label,
        ...(saved.proposalId === null ? {} : { proposalId: saved.proposalId }),
        ...(observedRun ? { runStages: observedRun.stages } : {}),
        verifierObserved: observedRun !== undefined && observedRun !== null,
        ...(observedProposal ? { ruleSetHash: observedProposal.rule_set_hash } : {}),
        ...(reason === null ? {} : { ruleReason: reason }),
      }),
      proposalId: saved.proposalId,
      nearTxHash: saved.nearTxHash,
      waiting,
      rejection:
        rejection === null
          ? null
          : { ruleId: rejection.rule_id, reasonCode: rejection.reason_code },
    });
    if (["executed", "rejected", "expired", "reverted", "error"].includes(current.kind)) {
      stopped = true;
      unsubscribe?.();
    }
  }

  function fail(cause: SurfaceErrorCause, detail: string): void {
    input.submission = { phase: "failed", cause, detail };
    emit();
  }

  function persist(): void {
    deps.storage.setItem(config.storageKey, JSON.stringify(saved));
  }

  function schedule(): void {
    if (disposed || stopped || polling || deps.isHidden() || timer !== undefined) return;
    timer = deps.setTimer(() => {
      timer = undefined;
      void poll();
    }, 2000);
  }

  async function poll(): Promise<void> {
    if (polling || disposed || stopped) return;
    polling = true;
    try {
      await observe();
    } finally {
      polling = false;
      schedule();
    }
  }

  /** Resolves `false` when the flow was disposed while waiting. */
  function sleep(ms: number): Promise<boolean> {
    const { promise, resolve } = Promise.withResolvers<boolean>();
    const wake = deps.setTimer(() => resolve(!disposed), ms);
    if (disposed) {
      deps.clearTimer(wake);
      resolve(false);
    }
    return promise;
  }

  async function observe(): Promise<void> {
    if (disposed || stopped || saved.proposalId === null) return;
    try {
      const result = await deps.gate.getProposal(saved.proposalId);
      if (disposed) return;
      input.proposal = { status: "ok", value: result.value, observedAtMs: deps.now() };
    } catch {
      input.proposal = unavailable(input.proposal, "NEAR RPC is unreachable");
    }
    if (disposed) return;
    if (saved.runId !== null) {
      try {
        const run = await deps.verifier.getRun(saved.runId);
        if (disposed) return;
        if (
          run !== null &&
          (run.gate_account !== config.gateAccount ||
            run.proposal_id !== saved.proposalId ||
            run.boot_id !== saved.bootId)
        ) {
          saved.runId = null;
          saved.bootId = null;
          persist();
          input.run = { status: "idle" };
          fail(
            "verifier_unreachable",
            "The saved verifier run belongs to a different proposal or verifier boot. Inspect the proposal with the CLI.",
          );
          return;
        }
        input.run = { status: "ok", value: run, observedAtMs: deps.now() };
      } catch {
        input.run = unavailable(input.run, "Verifier is unreachable");
      }
    }
    if (disposed) return;
    const proposal = last(input.proposal)?.value;
    // Every demo run is an attack: a signing record means a rule failed to hold. Never relay it.
    if (proposal?.evm !== null && proposal?.evm !== undefined) {
      fail(
        "other",
        "The attack proposal unexpectedly has a signing record. Do not relay it; inspect the gate and its accepted certificate.",
      );
      return;
    }
    // Rejection needs observed before/after reads too, not a claim inferred from no signature.
    if (proposal?.state === "Rejected" || proposal?.state === "Expired") {
      try {
        const after = await deps.base.readImplementation(gateConfig.proxy);
        if (disposed) return;
        const reads: ProxyReads = { ...(saved.before ? { before: saved.before } : {}), after };
        input.proxy = { status: "ok", value: reads, observedAtMs: deps.now() };
      } catch {
        input.proxy = unavailable(input.proxy, "Base implementation read is unavailable");
        // Do not permanently stop at a rejection whose requested final observation failed.
        if (input.proxy.status === "unavailable" && input.proxy.failures < 3) return;
      }
    }
    if (disposed) return;
    emit();
  }

  let gateConfig: Config;

  /**
   * Seals the bundle and posts the run for `proposalId`. A busy verifier is waited out in place;
   * any other refusal ends the attempt with a recovery that never involves proposing again.
   */
  async function postRun(proposalId: number, sealed?: Uint8Array): Promise<boolean> {
    let bytes = sealed;
    if (bytes === undefined) {
      const { encodeSourceBundle } = await import("./bundle.ts");
      const identity = await deps.verifier.identity();
      if (disposed) return false;
      bytes = await deps.seal(encodeSourceBundle(config.proposal.bundle), identity.transport_pubkey);
    }
    const waitStart = deps.now();
    for (;;) {
      if (disposed) return false;
      try {
        const run = await deps.verifier.startRun({
          gate: config.gateAccount,
          proposalId,
          sealedBundle: bytes,
        });
        if (
          run.gateAccount !== config.gateAccount ||
          run.proposalId !== proposalId ||
          !/^[0-9a-f]{64}$/.test(run.bootId)
        )
          throw new Error("Verifier run binding mismatch");
        saved.bootId = run.bootId;
        saved.runId = run.runId;
        persist();
        waiting = null;
        return true;
      } catch (error) {
        if (disposed) return false;
        if (!(error instanceof RunRefused)) {
          waiting = null;
          fail(
            "verifier_unreachable",
            "Your proposal is recorded on NEAR, but the verifier did not accept a recorded run. Check the demo verifier below and try again; this page will not propose twice.",
          );
          return false;
        }
        if ((BUSY_CODES as readonly string[]).includes(error.code)) {
          const seconds = Math.min(error.retryAfterSeconds ?? DEFAULT_RETRY_S, MAX_RETRY_S);
          if (deps.now() - waitStart + seconds * 1000 > MAX_BUSY_WAIT_MS) {
            waiting = null;
            fail(
              "run_refused",
              "The verifier stayed busy. Your proposal is recorded and still pending; reload this page to try posting its run again",
            );
            return false;
          }
          waiting = { reason: "verifier_busy", retryAtMs: deps.now() + seconds * 1000 };
          emit();
          if (!(await sleep(seconds * 1000))) return false;
          continue;
        }
        waiting = null;
        // The proposal already moved on (for example it was decided): reading the gate is enough.
        if (error.code === "proposal_not_pending") return true;
        fail(
          "run_refused",
          error.code === "cooling_down"
            ? `The verifier is cooling down after earlier failed runs${
                error.retryAfterSeconds === null
                  ? ""
                  : `; it accepts this proposal again in about ${Math.ceil(error.retryAfterSeconds / 60)} min`
              }`
            : `The verifier refused the run (${error.code})`,
        );
        return false;
      }
    }
  }

  async function start(): Promise<void> {
    // The state machine and the verifier client pull in heavy modules; keep them behind the click.
    const [state, view, verifier] = await Promise.all([
      import("./state.ts"),
      import("./surface-view.ts"),
      import("./verifier.ts"),
    ]);
    runtime = { reduceSurface: state.reduceSurface, surfaceView: view.surfaceView };
    RunRefused = verifier.RunRefusedError;
    if (disposed) return;
    const [gate, base, sealing] = await Promise.all([
      injected.gate ? undefined : import("./gate.ts"),
      injected.base ? undefined : import("./base.ts"),
      injected.seal ? undefined : import("./seal.ts"),
    ]);
    if (disposed) return;
    try {
      const gateClient =
        injected.gate ?? gate?.createGateClient({ rpcUrl: config.nearRpc, gate: config.gateAccount });
      const verifierClient =
        injected.verifier ?? verifier.createVerifierClient({ baseUrl: config.verifierUrl });
      const baseClient = injected.base ?? base?.createBaseClient({ rpcUrl: config.baseRpc });
      const seal = injected.seal ?? sealing?.sealBundle;
      if (!gateClient || !baseClient || !seal) throw new Error("Flow runtime is unavailable");
      deps = {
        gate: gateClient,
        verifier: verifierClient,
        base: baseClient,
        propose: injected.propose,
        seal,
        storage: injected.storage ?? globalThis.localStorage,
        now: injected.now ?? Date.now,
        setTimer: injected.setTimer ?? ((callback, ms) => setTimeout(callback, ms)),
        clearTimer: injected.clearTimer ?? clearTimeout,
        isHidden: injected.isHidden ?? (() => typeof document !== "undefined" && document.hidden),
        subscribeVisibility:
          injected.subscribeVisibility ??
          ((callback) => {
            if (typeof document === "undefined") return () => {};
            document.addEventListener("visibilitychange", callback);
            return () => document.removeEventListener("visibilitychange", callback);
          }),
      };
      const stored = deps.storage.getItem(config.storageKey);
      if (stored !== null) {
        const value: SavedRun = JSON.parse(stored);
        const validId = (id: unknown) =>
          id === null || (typeof id === "number" && Number.isSafeInteger(id) && id >= 0);
        if (
          value.version !== 1 ||
          value.attempted !== true ||
          !validId(value.proposalId) ||
          !validId(value.runId) ||
          (value.nearTxHash !== null &&
            (typeof value.nearTxHash !== "string" ||
              !/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(value.nearTxHash)))
        )
          throw new Error("Invalid saved run");
        saved = value;
        if (
          saved.runId !== null &&
          (typeof saved.bootId !== "string" || !/^[0-9a-f]{64}$/.test(saved.bootId))
        ) {
          saved.runId = null;
          saved.bootId = null;
          persist();
        }
        if (saved.proposalId === null) {
          fail(
            "other",
            "The earlier NEAR submission is uncertain. Check your account's active proposal below before starting a new run; this page will not submit it again.",
          );
          return;
        }
      } else {
        input.submission = { phase: "sealing" };
        emit();
      }
      if (disposed) return;
      gateConfig = (await deps.gate.getConfig()).value;
      if (disposed) return;
      if (gateConfig.chain_id !== 84532) throw new Error("Gate is not configured for Base Sepolia");
      let sealed: Uint8Array | undefined;
      if (saved.proposalId === null) {
        const candidate = config.proposal;
        const { encodeSourceBundle, sourceCommitment } = await import("./bundle.ts");
        const bytes = encodeSourceBundle(candidate.bundle);
        const commitment = await sourceCommitment(bytes);
        if (disposed) return;
        const identity = await deps.verifier.identity();
        if (disposed) return;
        sealed = await deps.seal(bytes, identity.transport_pubkey);
        if (disposed) return;
        saved.before = await deps.base.readImplementation(gateConfig.proxy);
        if (disposed) return;
        saved.beforeObservedAtMs = deps.now();
        // Refuse to write without durable storage; uncertainty survives reload.
        persist();
        input.submission = { phase: "registering" };
        emit();
        if (disposed) return;
        const args: ProposeArgs = {
          new_impl: candidate.newImpl,
          new_impl_codehash: candidate.newImplCodehash,
          expected_current_impl: saved.before.implementation,
          init_calldata: candidate.initCalldata,
          source_commitment: commitment,
        };
        const accepted: AcceptedProposal = await deps.propose(args);
        saved.proposalId = accepted.proposalId;
        saved.nearTxHash = accepted.nearTxHash;
        persist();
        if (disposed) return;
        input.submission = { phase: "none" };
      }
      if (saved.before !== undefined && saved.beforeObservedAtMs !== undefined)
        input.proxy = {
          status: "ok",
          value: { before: saved.before },
          observedAtMs: saved.beforeObservedAtMs,
        };
      if (saved.runId === null) {
        input.submission = { phase: "none" };
        emit();
        const posted = await postRun(saved.proposalId, sealed);
        if (disposed || stopped || !posted) return;
      }
      if (disposed) return;
      unsubscribe = deps.subscribeVisibility(() => {
        if (disposed || stopped) return;
        if (deps.isHidden()) {
          if (timer !== undefined) deps.clearTimer(timer);
          timer = undefined;
        } else schedule();
      });
      await poll();
    } catch (error) {
      if (disposed) return;
      if (error instanceof GateWriteError) fail(error.cause, error.detail);
      else
        fail(
          "other",
          "The run could not start. Check your connection and account state below, then try again; nothing was proposed unless a proposal id is shown.",
        );
    }
  }

  return {
    start() {
      if (disposed) return Promise.resolve();
      started ??= start();
      return started;
    },
    dispose() {
      disposed = true;
      if (timer !== undefined) deps.clearTimer(timer);
      timer = undefined;
      unsubscribe?.();
    },
  };
}
