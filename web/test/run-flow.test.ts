import { afterEach, describe, expect, it, vi } from "vitest";
import { GateWriteError } from "../src/lib/near-write.ts";
import { createRunFlow, type RunFlowConfig, type RunFlowDeps, type RunFlowUpdate } from "../src/lib/run-flow.ts";
import { RunRefusedError } from "../src/lib/verifier.ts";
import {
  evmTx,
  hex,
  NEW_IMPL,
  OLD_IMPL,
  PROXY,
  proposal,
  proxyRead,
  run,
  SIGNED_TX,
} from "./support/fixtures.ts";

const bundle = {
  contract_name: "src/A.sol:A",
  foundry_profile: "default",
  files: [["src/A.sol", new TextEncoder().encode("contract A {}")]] as const,
};
const config: RunFlowConfig = {
  nearRpc: "https://rpc.example",
  gateAccount: "gate.testnet",
  verifierUrl: "https://verifier.example",
  baseRpc: "https://base.example",
  label: "VaultV2Drain",
  storageKey: "constitution:run:gate.testnet:alice.testnet:drain",
  proposal: {
    newImpl: NEW_IMPL,
    newImplCodehash: hex(0xc0, 32),
    initCalldata: "0x",
    bundle,
  },
};
const gateConfig = {
  chain_id: 84532,
  executor: hex(0x22, 20),
  proxy: PROXY,
  proxy_admin: hex(0x33, 20),
  derived_eoa: hex(0x44, 20),
  mpc_account: "v1.signer-prod.testnet",
  mpc_path: "constitution/base-sepolia/v1",
  mpc_domain_id: 0,
  governance_delay_days: 3,
};
const view = <T>(value: T) => ({ value, blockHeight: 1, blockHash: "block" });
const PROPOSE_TX = "11111111111111111111111111111111";

function harness() {
  vi.useFakeTimers();
  const stored = new Map<string, string>();
  const storage = {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => {
      stored.set(key, value);
    },
  };
  const updates: RunFlowUpdate[] = [];
  const propose = vi.fn(async () => ({ proposalId: 7, nearTxHash: PROPOSE_TX }));
  const getProposal = vi.fn(async () => view(proposal()));
  const getRun = vi.fn(async () => run());
  const readImplementation = vi.fn(async () => proxyRead(OLD_IMPL, 1));
  const startRun = vi.fn(async () => ({
    runId: 9,
    gateAccount: "gate.testnet",
    proposalId: 7,
    bootId: "ab".repeat(32),
    label: null,
  }));
  const deps: Partial<RunFlowDeps> & Pick<RunFlowDeps, "propose"> = {
    storage,
    gate: { getConfig: async () => view(gateConfig), getProposal },
    verifier: {
      identity: async () => ({
        verifier_pubkey: hex(1, 32),
        transport_pubkey: hex(2, 32),
        release_hash: hex(3, 32),
        report_data_preimage: "0x",
        quote_hex: "0x",
        event_log: [],
        compose_hash: hex(4, 32),
        supported_rule_set_hashes: [],
        label: null,
      }),
      startRun,
      getRun,
    },
    base: { readImplementation },
    propose,
    seal: async (bytes) => bytes,
  };
  return {
    deps,
    updates,
    propose,
    getProposal,
    getRun,
    startRun,
    readImplementation,
    stored,
    last: () => updates.at(-1)?.state,
  };
}
afterEach(() => vi.useRealTimers());

describe("demo attack run flow", () => {
  it("drops a verifier run bound to another proposal", async () => {
    const h = harness();
    h.getRun.mockResolvedValue(
      run({
        proposal_id: 8,
        stages: [{ name: "compiling", status: "Pass", detail: "wrong proposal", elapsed_ms: 0 }],
      }),
    );
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()?.kind).toBe("error");
    expect(JSON.parse([...h.stored.values()][0] ?? "null").runId).toBeNull();
    expect(
      h.updates.every((update) => update.view.stages.every((stage) => stage.detail !== "wrong proposal")),
    ).toBe(true);
  });

  it("proposes against the implementation it just read, so an upgraded proxy is never contradicted", async () => {
    const h = harness();
    h.readImplementation.mockResolvedValue(proxyRead(NEW_IMPL, 5));
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.propose).toHaveBeenCalledWith(
      expect.objectContaining({ expected_current_impl: NEW_IMPL }),
    );
  });

  it("ends on gate Rejected with the recorded rule and a plain reason, without any signature", async () => {
    const h = harness();
    h.getProposal.mockResolvedValue(
      view(
        proposal({
          state: "Rejected",
          rejection: { rule_id: 2, reason_code: 20, evidence_root: hex(7, 32) },
        }),
      ),
    );
    h.readImplementation
      .mockResolvedValueOnce(proxyRead(OLD_IMPL, 1))
      .mockResolvedValue(proxyRead(OLD_IMPL, 2));
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({
      kind: "rejected",
      noSignatureRequested: true,
      implementationUnchanged: true,
    });
    const final = h.updates.at(-1);
    expect(final?.rejection).toEqual({ ruleId: 2, reasonCode: 20 });
    expect(final?.view.body).toContain("owing more than it holds");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("does not relay or accept an attack that unexpectedly obtained a signature", async () => {
    const h = harness();
    h.getProposal.mockResolvedValue(
      view(
        proposal({
          state: "Signed",
          certificate_hash: hex(5, 32),
          evm: evmTx({ signed_tx: SIGNED_TX }),
        }),
      ),
    );
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "error", cause: "other" });
  });

  it("surfaces a gate refusal and never posts a verifier run for it", async () => {
    const h = harness();
    h.propose.mockRejectedValue(new GateWriteError("gate_refused", "E11_INSUFFICIENT_CREDIT"));
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toEqual({
      kind: "error",
      cause: "gate_refused",
      detail: "E11_INSUFFICIENT_CREDIT",
    });
    expect(h.startRun).not.toHaveBeenCalled();
  });

  it("surfaces a missing or exhausted session key as its own cause", async () => {
    const h = harness();
    h.propose.mockRejectedValue(new GateWriteError("session_key", "The session key is gone."));
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toEqual({
      kind: "error",
      cause: "session_key",
      detail: "The session key is gone.",
    });
  });

  it("shows verifier outage after three consecutive polling failures and recovers", async () => {
    const h = harness();
    h.getRun.mockRejectedValue(new Error("offline"));
    const flow = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    await flow.start();
    await vi.advanceTimersByTimeAsync(4000);
    expect(h.last()).toMatchObject({ kind: "network_unavailable", source: "verifier", failures: 3 });
    h.getRun.mockResolvedValue(run({ stage: "ai_review" }));
    await vi.advanceTimersByTimeAsync(2000);
    expect(h.last()?.kind).toBe("ai_review");
    flow.dispose();
  });

  it("resumes persisted proposal and run ids without reproposing or re-posting", async () => {
    const h = harness();
    const first = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    await first.start();
    first.dispose();
    const second = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    await second.start();
    expect(h.propose).toHaveBeenCalledTimes(1);
    expect(h.startRun).toHaveBeenCalledTimes(1);
    expect(h.updates.at(-1)).toMatchObject({ proposalId: 7, nearTxHash: PROPOSE_TX });
    expect(h.getRun).toHaveBeenLastCalledWith(9);
    second.dispose();
  });

  it("posts only the verifier run again when a recorded proposal has none, never a second proposal", async () => {
    const h = harness();
    h.startRun.mockRejectedValueOnce(new Error("offline"));
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "error", cause: "verifier_unreachable" });
    const resumed = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    await resumed.start();
    expect(h.propose).toHaveBeenCalledTimes(1);
    expect(h.startRun).toHaveBeenCalledTimes(2);
    expect(h.updates.at(-1)?.proposalId).toBe(7);
    expect(JSON.parse([...h.stored.values()][0] ?? "null").runId).toBe(9);
    resumed.dispose();
  });

  it("waits out a busy verifier in place and then records the run", async () => {
    const h = harness();
    h.startRun.mockRejectedValueOnce(new RunRefusedError("queue_full", 429, "queue_full", 10));
    const flow = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    const started = flow.start();
    await vi.waitFor(() => expect(h.updates.some((update) => update.waiting !== null)).toBe(true));
    const busy = h.updates.find((update) => update.waiting !== null);
    expect(busy?.waiting?.reason).toBe("verifier_busy");
    expect(busy?.proposalId).toBe(7);
    expect(h.startRun).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(10_000);
    await started;
    expect(h.startRun).toHaveBeenCalledTimes(2);
    expect(h.propose).toHaveBeenCalledTimes(1);
    expect(h.updates.at(-1)?.waiting).toBeNull();
    expect(h.getRun).toHaveBeenCalledWith(9);
    flow.dispose();
  });

  it("ends a verifier cool-down as a refusal with a recovery and does not keep retrying", async () => {
    const h = harness();
    h.startRun.mockRejectedValue(new RunRefusedError("cooling_down", 429, "cooling_down", 600));
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "error", cause: "run_refused" });
    expect(JSON.stringify(h.updates.at(-1))).toContain("10 min");
    expect(h.startRun).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("treats a proposal the verifier says is no longer pending as readable from the gate", async () => {
    const h = harness();
    h.startRun.mockRejectedValue(new RunRefusedError("proposal_not_pending", 409, "proposal_not_pending"));
    h.getProposal.mockResolvedValue(
      view(
        proposal({
          state: "Rejected",
          rejection: { rule_id: 1, reason_code: 10, evidence_root: hex(7, 32) },
        }),
      ),
    );
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "rejected", ruleId: 1 });
  });

  it("stops before further I/O when disposed by a state update callback", async () => {
    const h = harness();
    const getConfig = vi.fn(async () => view(gateConfig));
    h.deps.gate = { getConfig, getProposal: h.getProposal };
    const flow = createRunFlow(
      config,
      (u) => {
        h.updates.push(u);
        flow.dispose();
      },
      h.deps,
    );
    await flow.start();
    expect(getConfig).not.toHaveBeenCalled();
    expect(h.propose).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("never overlaps observations when visibility changes during a slow RPC call", async () => {
    const h = harness();
    let hidden = false;
    let visibility = () => {};
    h.deps.isHidden = () => hidden;
    h.deps.subscribeVisibility = (callback) => {
      visibility = callback;
      return () => {};
    };
    const flow = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    await flow.start();
    const deferred = Promise.withResolvers<void>();
    h.getProposal.mockImplementationOnce(async () => {
      await deferred.promise;
      return view(proposal());
    });
    await vi.advanceTimersByTimeAsync(2000);
    hidden = true;
    visibility();
    hidden = false;
    visibility();
    await vi.advanceTimersByTimeAsync(2000);
    expect(h.getProposal).toHaveBeenCalledTimes(2);
    deferred.resolve();
    await vi.advanceTimersByTimeAsync(0);
    flow.dispose();
  });

  it("retains uncertain submissions across reload and never guesses another proposal", async () => {
    const h = harness();
    h.propose.mockRejectedValue(new Error("uncertain network result"));
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.propose).toHaveBeenCalledTimes(1);
    expect(h.last()).toMatchObject({ kind: "error", cause: "other" });
  });

  it("persists an accepted result after disposal without starting any further network work", async () => {
    const h = harness();
    const pending = Promise.withResolvers<{ proposalId: number; nearTxHash: string }>();
    const sending = Promise.withResolvers<void>();
    h.propose.mockImplementation(async () => {
      sending.resolve();
      return pending.promise;
    });
    const flow = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    const started = flow.start();
    await sending.promise;
    flow.dispose();
    const count = h.updates.length;
    pending.resolve({ proposalId: 7, nearTxHash: PROPOSE_TX });
    await started;
    expect(h.startRun).not.toHaveBeenCalled();
    expect(h.updates).toHaveLength(count);
    expect([...h.stored.values()].map((value) => JSON.parse(value))).toContainEqual(
      expect.objectContaining({ proposalId: 7, runId: null }),
    );
    expect(vi.getTimerCount()).toBe(0);
  });

  it("refuses to write when durable recovery storage is unavailable", async () => {
    const h = harness();
    h.deps.storage = {
      getItem: () => null,
      setItem: () => {
        throw new Error("storage disabled");
      },
    };
    await createRunFlow(config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "error", cause: "other" });
    expect(h.propose).not.toHaveBeenCalled();
  });

  it("disposes pending timers and prevents later callbacks", async () => {
    const h = harness();
    const flow = createRunFlow(config, (u) => h.updates.push(u), h.deps);
    await flow.start();
    flow.dispose();
    const count = h.updates.length;
    await vi.advanceTimersByTimeAsync(10000);
    expect(h.updates).toHaveLength(count);
    expect(vi.getTimerCount()).toBe(0);
  });
});
