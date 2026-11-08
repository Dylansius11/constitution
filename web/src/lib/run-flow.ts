import type { keccak256 } from "viem";
import type { BaseClient, ProxyRead, ProxyReads } from "./base.ts";
import type { SourceBundle } from "./bundle.ts";
import type { GateClient } from "./gate.ts";
import type { Config, EvmTx } from "./gate-types.ts";
import type { AcceptedProposal, DemoSignerConfig, ProposeArgs } from "./near-write.ts";
import type {
  reduceSurface,
  Source,
  SurfaceErrorCause,
  SurfaceInput,
  SurfaceState,
} from "./state.ts";
import type { SurfaceView, surfaceView } from "./surface-view.ts";
import type { VerifierClient } from "./verifier.ts";
import { isAccountId } from "./wire.ts";

export type FlowKind = "attack" | "honest";
export interface ProposalInput {
  newImpl: string;
  newImplCodehash: string;
  expectedCurrentImpl: string;
  /** Explicitly configured, including empty `0x`; never invent an initializer. */
  initCalldata: string;
  /** Omit to use the build-time embedded contracts/src fixture tree for this kind. */
  bundle?: SourceBundle;
}
export interface RunFlowConfig extends DemoSignerConfig {
  verifierUrl: string;
  baseRpc: string;
  proposals: Record<FlowKind, ProposalInput>;
}
export interface RunFlowUpdate {
  state: SurfaceState;
  view: SurfaceView;
  proposalId: number | null;
  nearTxHash: string | null;
}
export interface RunFlow {
  start(): Promise<void>;
  dispose(): void;
}

export type FlowTimer = number | NodeJS.Timeout;
export interface RunFlowDeps {
  gate: Pick<GateClient, "getConfig" | "getProposal">;
  verifier: Pick<VerifierClient, "identity" | "startRun" | "getRun">;
  base: BaseClient;
  propose(config: DemoSignerConfig, args: ProposeArgs): Promise<AcceptedProposal>;
  seal(bytes: Uint8Array, transportPubkey: string): Promise<Uint8Array>;
  relay(config: Config, evm: EvmTx): Promise<string>;
  storage: Pick<Storage, "getItem" | "setItem">;
  now(): number;
  setTimer(callback: () => void, ms: number): FlowTimer;
  clearTimer(timer: FlowTimer): void;
  isHidden(): boolean;
  subscribeVisibility(callback: () => void): () => void;
}

/** All deployment values are explicit. No production-like defaults or sample addresses. */
export function readFlowConfig(env: Record<string, string | undefined>): RunFlowConfig | null {
  const nearRpc = env.PUBLIC_NEAR_RPC;
  const gateAccount = env.PUBLIC_GATE_ACCOUNT;
  const verifierUrl = env.PUBLIC_VERIFIER_URL;
  const baseRpc = env.PUBLIC_BASE_RPC;
  const demoAccountId = env.PUBLIC_DEMO_ACCOUNT_ID;
  const demoSecretKey = env.PUBLIC_DEMO_SECRET_KEY;
  if (!nearRpc || !gateAccount || !verifierUrl || !baseRpc || !demoAccountId || !demoSecretKey)
    return null;
  if (!isAccountId(gateAccount) || !isAccountId(demoAccountId)) return null;
  // Base58-encoded 64-byte Ed25519 secret (seed + public key). Decode length without a crypto SDK.
  if (!/^ed25519:[1-9A-HJ-NP-Za-km-z]+$/.test(demoSecretKey)) return null;
  const key = demoSecretKey.slice(8);
  if (key.length < 64 || key.length > 88) return null;
  let integer = 0n;
  const alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  for (const char of key) integer = integer * 58n + BigInt(alphabet.indexOf(char));
  const leading = key.match(/^1*/)?.[0].length ?? 0;
  const byteLength = integer === 0n ? 0 : Math.ceil(integer.toString(16).length / 2);
  if (leading + byteLength !== 64) return null;
  for (const value of [nearRpc, verifierUrl, baseRpc]) {
    try {
      const url = new URL(value);
      if (!["http:", "https:"].includes(url.protocol) || url.username || url.password || url.hash)
        return null;
    } catch {
      return null;
    }
  }
  const proposals = {} as Record<FlowKind, ProposalInput>;
  for (const kind of ["attack", "honest"] as const) {
    const prefix = `PUBLIC_${kind.toUpperCase()}`;
    const newImpl = env[`${prefix}_IMPL`];
    const newImplCodehash = env[`${prefix}_CODEHASH`];
    const expectedCurrentImpl = env[`${prefix}_CURRENT_IMPL`];
    const initCalldata = env[`${prefix}_INIT_CALLDATA`];
    if (!newImpl || !newImplCodehash || !expectedCurrentImpl || initCalldata === undefined)
      return null;
    if (
      ![newImpl, expectedCurrentImpl].every(
        (value) => /^0x[0-9a-f]{40}$/.test(value) && !/^0x0{40}$/.test(value),
      )
    )
      return null;
    if (
      !/^0x[0-9a-f]{64}$/.test(newImplCodehash) ||
      !/^0x(?:[0-9a-f]{2}){0,1024}$/.test(initCalldata)
    )
      return null;
    proposals[kind] = { newImpl, newImplCodehash, expectedCurrentImpl, initCalldata };
  }
  return { nearRpc, gateAccount, verifierUrl, baseRpc, demoAccountId, demoSecretKey, proposals };
}

interface SavedRun {
  version: 1;
  attempted: true;
  proposalId: number | null;
  runId: number | null;
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

/** Click-only state machine. start resolves after setup + first observation, not after finality. */
export function createRunFlow(
  kind: FlowKind,
  config: RunFlowConfig,
  onUpdate: (update: RunFlowUpdate) => void,
  injected: Partial<RunFlowDeps> = {},
): RunFlow {
  let disposed = false;
  let stopped = false;
  let polling = false;
  let started: Promise<void> | undefined;
  let timer: FlowTimer | undefined;
  let unsubscribe: (() => void) | undefined;
  let deps: RunFlowDeps;
  let gateConfig: Config;
  let runtime: {
    reduceSurface: typeof reduceSurface;
    surfaceView: typeof surfaceView;
    keccak256: typeof keccak256;
  };
  let current: SurfaceState = { kind: "idle" };
  let saved: SavedRun = {
    version: 1,
    attempted: true,
    proposalId: null,
    runId: null,
    nearTxHash: null,
  };
  const storageKey = `constitution:run:${config.gateAccount}:${kind}`;
  let relayedHash: string | null = null;
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
    onUpdate({
      state: current,
      view: runtime.surfaceView(current, {
        fixture: kind === "attack" ? "VaultV2Drain" : "VaultV2Honest",
        ...(saved.proposalId === null ? {} : { proposalId: saved.proposalId }),
        ...(observedRun ? { runStages: observedRun.stages } : {}),
        verifierObserved: observedRun !== undefined && observedRun !== null,
        ...(observedProposal ? { ruleSetHash: observedProposal.rule_set_hash } : {}),
      }),
      proposalId: saved.proposalId,
      nearTxHash: saved.nearTxHash,
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
    deps.storage.setItem(storageKey, JSON.stringify(saved));
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
        input.run = { status: "ok", value: run, observedAtMs: deps.now() };
      } catch {
        input.run = unavailable(input.run, "Verifier is unreachable");
      }
    }
    if (disposed) return;
    const proposal = last(input.proposal)?.value;
    if (kind === "attack" && proposal?.evm !== null && proposal?.evm !== undefined) {
      fail(
        "other",
        "The attack proposal unexpectedly has a signing record. Do not relay it; inspect the gate and its accepted certificate.",
      );
      return;
    }
    if (proposal?.state === "Signed" && proposal.evm?.signed_tx) {
      const hash = runtime.keccak256(proposal.evm.signed_tx as `0x${string}`);
      if (relayedHash !== hash) {
        try {
          await deps.relay(gateConfig, proposal.evm);
          relayedHash = hash;
        } catch {
          if (!disposed)
            fail(
              "broadcast_failed",
              "The signed transaction could not be broadcast. Inspect this proposal and relay with the CLI; do not repropose.",
            );
          return;
        }
      }
      if (disposed) return;
      try {
        const observation = await deps.base.readTransaction(hash);
        if (disposed) return;
        input.evm = { status: "ok", value: observation, observedAtMs: deps.now() };
      } catch {
        input.evm = unavailable(input.evm, "Base RPC is unreachable");
      }
    }
    if (disposed) return;
    // Rejection needs observed before/after reads too, not a claim inferred from no signature.
    if (
      proposal?.state === "Rejected" ||
      proposal?.state === "Expired" ||
      (proposal?.state === "Signed" && last(input.evm)?.value.status === "included")
    ) {
      try {
        const after = await deps.base.readImplementation(gateConfig.proxy);
        if (disposed) return;
        const reads: ProxyReads = { ...(saved.before ? { before: saved.before } : {}), after };
        input.proxy = { status: "ok", value: reads, observedAtMs: deps.now() };
      } catch {
        input.proxy = unavailable(input.proxy, "Base implementation read is unavailable");
        // Do not permanently stop at a rejection whose requested final observation failed.
        if (input.proxy.status === "unavailable" && input.proxy.failures < 3) {
          return;
        }
      }
    }
    if (disposed) return;
    emit();
  }

  async function start(): Promise<void> {
    // state→verifier→seal currently has static heavy dependencies. Keep the ENTIRE runtime
    // behind the click boundary, not just the NEAR SDK. Type imports above erase completely.
    const [state, view, viem] = await Promise.all([
      import("./state.ts"),
      import("./surface-view.ts"),
      import("viem"),
    ]);
    runtime = {
      reduceSurface: state.reduceSurface,
      surfaceView: view.surfaceView,
      keccak256: viem.keccak256,
    };
    if (disposed) return;
    const [gate, verifier, base, writer, sealing] = await Promise.all([
      injected.gate ? undefined : import("./gate.ts"),
      import("./verifier.ts"),
      injected.base ? undefined : import("./base.ts"),
      import("./near-write.ts"),
      injected.seal ? undefined : import("./seal.ts"),
    ]);
    if (disposed) return;
    try {
      const gateClient =
        injected.gate ??
        gate?.createGateClient({ rpcUrl: config.nearRpc, gate: config.gateAccount });
      const verifierClient =
        injected.verifier ?? verifier?.createVerifierClient({ baseUrl: config.verifierUrl });
      const baseClient = injected.base ?? base?.createBaseClient({ rpcUrl: config.baseRpc });
      const seal = injected.seal ?? sealing?.sealBundle;
      if (!gateClient || !verifierClient || !baseClient || !seal)
        throw new Error("Flow runtime is unavailable");
      deps = {
        gate: gateClient,
        verifier: verifierClient,
        base: baseClient,
        propose: injected.propose ?? writer.proposeWithDemoKey,
        seal,
        relay:
          injected.relay ??
          (async (cfg, evm) => {
            const { relaySignedTransaction } = await import("./relay.ts");
            return relaySignedTransaction(config.baseRpc, cfg, evm);
          }),
        storage: injected.storage ?? globalThis.sessionStorage,
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
      const stored = deps.storage.getItem(storageKey);
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
        if (saved.proposalId === null) {
          fail(
            "other",
            "The earlier NEAR submission is uncertain. Inspect the demo account before starting a new run; this page will not submit it again.",
          );
          return;
        }
        if (saved.runId === null)
          input.submission = {
            phase: "failed",
            cause: "verifier_unreachable",
            detail:
              "The accepted proposal has no recorded verifier run. Inspect it with the CLI; this page will not re-post or repropose automatically.",
          };
      } else {
        input.submission = { phase: "sealing" };
        emit();
      }
      if (disposed) return;
      gateConfig = (await deps.gate.getConfig()).value;
      if (disposed) return;
      if (gateConfig.chain_id !== 84532) throw new Error("Gate is not configured for Base Sepolia");
      if (saved.proposalId === null) {
        const candidate = config.proposals[kind];
        const { encodeSourceBundle, sourceCommitment } = await import("./bundle.ts");
        const bundle =
          candidate.bundle ?? (await import("../data/flow-sources.ts")).fixtureBundle(kind);
        const bytes = encodeSourceBundle(bundle);
        const commitment = await sourceCommitment(bytes);
        if (disposed) return;
        const identity = await deps.verifier.identity();
        if (disposed) return;
        const sealed = await deps.seal(bytes, identity.transport_pubkey);
        if (disposed) return;
        saved.before = await deps.base.readImplementation(gateConfig.proxy);
        if (disposed) return;
        if (saved.before.implementation !== candidate.expectedCurrentImpl)
          throw new Error("Current implementation differs from configured proposal");
        saved.beforeObservedAtMs = deps.now();
        input.proxy = {
          status: "ok",
          value: { before: saved.before },
          observedAtMs: saved.beforeObservedAtMs,
        };
        // Refuse to write without durable session storage; uncertainty survives reload.
        persist();
        input.submission = { phase: "registering" };
        emit();
        if (disposed) return;
        const accepted = await deps.propose(config, {
          new_impl: candidate.newImpl,
          new_impl_codehash: candidate.newImplCodehash,
          expected_current_impl: candidate.expectedCurrentImpl,
          init_calldata: candidate.initCalldata,
          source_commitment: commitment,
        });
        saved.proposalId = accepted.proposalId;
        saved.nearTxHash = accepted.nearTxHash;
        persist();
        if (disposed) return;
        input.submission = { phase: "none" };
        try {
          const run = await deps.verifier.startRun({
            gate: config.gateAccount,
            proposalId: accepted.proposalId,
            sealedBundle: sealed,
          });
          saved.runId = run.runId;
          persist();
        } catch (error) {
          if (disposed) return;
          const refused = error instanceof verifier.RunRefusedError;
          fail(
            refused ? "run_refused" : "verifier_unreachable",
            "Your proposal is recorded on NEAR, but the verifier did not accept a recorded run. Inspect this proposal with the CLI; do not repropose.",
          );
          return;
        }
      }
      if (disposed) return;
      if (saved.before !== undefined && saved.beforeObservedAtMs !== undefined)
        input.proxy = {
          status: "ok",
          value: { before: saved.before },
          observedAtMs: saved.beforeObservedAtMs,
        };
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
      if (error instanceof writer.GateWriteError) fail(error.cause, error.detail);
      else
        fail(
          "other",
          "The run could not start. Check deployment configuration and session storage, then inspect the demo account before submitting again.",
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
