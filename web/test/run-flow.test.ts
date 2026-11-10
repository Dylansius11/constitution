import { afterEach, describe, expect, it, vi } from "vitest";
import type { TxObservation } from "../src/lib/base.ts";
import { GateWriteError } from "../src/lib/near-write.ts";
import {
  createRunFlow,
  type RunFlowConfig,
  type RunFlowDeps,
  type RunFlowUpdate,
  readFlowConfig,
} from "../src/lib/run-flow.ts";
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
  SIGNED_TX_HASH,
} from "./support/fixtures.ts";

const bundle = {
  contract_name: "src/A.sol:A",
  foundry_profile: "default",
  files: [["src/A.sol", new TextEncoder().encode("contract A {}")]] as const,
};
const candidate = {
  newImpl: NEW_IMPL,
  newImplCodehash: hex(0xc0, 32),
  expectedCurrentImpl: OLD_IMPL,
  initCalldata: "0x",
  bundle,
};
const config: RunFlowConfig = {
  nearRpc: "https://rpc.example",
  gateAccount: "gate.testnet",
  verifierUrl: "https://verifier.example",
  baseRpc: "https://base.example",
  demoAccountId: "demo.testnet",
  demoSecretKey: `ed25519:${"1".repeat(64)}`,
  proposals: { attack: candidate, honest: candidate },
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
  const propose = vi.fn(async () => ({
    proposalId: 7,
    nearTxHash: "11111111111111111111111111111111",
  }));
  const getProposal = vi.fn(async () => view(proposal()));
  const getRun = vi.fn(async () => run());
  const readImplementation = vi.fn(async () => proxyRead(OLD_IMPL, 1));
  const readTransaction = vi.fn(async (): Promise<TxObservation> => ({ status: "not_found" }));
  const relay = vi.fn(async () => SIGNED_TX_HASH);
  const startRun = vi.fn(async () => ({ runId: 9, gateAccount: "gate.testnet", proposalId: 7, bootId: "ab".repeat(32), label: null }));
  const deps: Partial<RunFlowDeps> = {
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
    base: { readImplementation, readTransaction },
    propose,
    seal: async (bytes) => bytes,
    relay,
  };
  return {
    deps,
    updates,
    propose,
    getProposal,
    getRun,
    startRun,
    readImplementation,
    readTransaction,
    relay,
    stored,
    last: () => updates.at(-1)?.state,
  };
}
afterEach(() => vi.useRealTimers());

describe("browser run flow", () => {
  it("drops a verifier run bound to another proposal", async () => {
    const h = harness();
    h.getRun.mockResolvedValue(run({ proposal_id: 8, stages: [{ name: "compiling", status: "Pass", detail: "wrong proposal", elapsed_ms: 0 }] }));
    await createRunFlow("attack", config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()?.kind).toBe("error");
    expect(JSON.parse([...h.stored.values()][0] ?? "null").runId).toBeNull();
    expect(h.updates.every((update) => update.view.stages.every((stage) => stage.detail !== "wrong proposal"))).toBe(true);
  });

  it("ends attack only on gate Rejected, without requesting or broadcasting a signature", async () => {
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
    const flow = createRunFlow("attack", config, (u) => h.updates.push(u), h.deps);
    await flow.start();
    expect(h.last()).toMatchObject({
      kind: "rejected",
      noSignatureRequested: true,
      implementationUnchanged: true,
    });
    expect(h.relay).not.toHaveBeenCalled();
    expect(h.readTransaction).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("requires Signed, successful inclusion and candidate slot readback for execution", async () => {
    const h = harness();
    const signed = proposal({
      state: "Signed",
      certificate_hash: hex(5, 32),
      evm: evmTx({ signed_tx: SIGNED_TX }),
    });
    h.getProposal.mockResolvedValueOnce(view(proposal())).mockResolvedValue(view(signed));
    const flow = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await flow.start();
    expect(h.last()?.kind).toBe("verifying");
    expect(h.relay).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2000);
    expect(h.last()?.kind).toBe("relaying");
    expect(h.relay).toHaveBeenCalledTimes(1);
    h.readTransaction.mockImplementation(async () => ({
      status: "included",
      block: 42,
      block_hash: hex(1, 32),
    }));
    h.readImplementation.mockResolvedValue(proxyRead(NEW_IMPL, 41));
    await vi.advanceTimersByTimeAsync(2000);
    expect(h.last()).toMatchObject({ kind: "relaying", evm: "included_unverified" });
    h.readImplementation.mockResolvedValue(proxyRead(NEW_IMPL, 42));
    await vi.advanceTimersByTimeAsync(2000);
    expect(h.last()).toMatchObject({ kind: "executed", implementationAfter: NEW_IMPL });
    expect(vi.getTimerCount()).toBe(0);
  });

  it("surfaces a gate refusal without leaking credentials", async () => {
    const h = harness();
    h.propose.mockRejectedValue(new GateWriteError("gate_refused", "E11_INSUFFICIENT_CREDIT"));
    await createRunFlow("attack", config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toEqual({
      kind: "error",
      cause: "gate_refused",
      detail: "E11_INSUFFICIENT_CREDIT",
    });
    expect(h.startRun).not.toHaveBeenCalled();
  });

  it("shows verifier outage after three consecutive polling failures and recovers", async () => {
    const h = harness();
    h.getRun.mockRejectedValue(new Error("offline"));
    const flow = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await flow.start();
    await vi.advanceTimersByTimeAsync(4000);
    expect(h.last()).toMatchObject({
      kind: "network_unavailable",
      source: "verifier",
      failures: 3,
    });
    h.getRun.mockResolvedValue(run({ stage: "ai_review" }));
    await vi.advanceTimersByTimeAsync(2000);
    expect(h.last()?.kind).toBe("ai_review");
    flow.dispose();
  });

  it("resumes persisted proposal and run ids without reproposing or re-posting", async () => {
    const h = harness();
    const first = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await first.start();
    first.dispose();
    const second = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await second.start();
    expect(h.propose).toHaveBeenCalledTimes(1);
    expect(h.startRun).toHaveBeenCalledTimes(1);
    expect(h.updates.at(-1)).toMatchObject({
      proposalId: 7,
      nearTxHash: "11111111111111111111111111111111",
    });
    expect(h.getRun).toHaveBeenLastCalledWith(9);
    second.dispose();
  });

  it("preserves the original observation time of the persisted baseline across reload", async () => {
    const h = harness();
    h.deps.now = () => 123;
    const first = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await first.start();
    first.dispose();
    h.deps.now = () => 999;
    h.getProposal.mockResolvedValue(
      view(
        proposal({
          state: "Signed",
          certificate_hash: hex(5, 32),
          evm: evmTx({ signed_tx: SIGNED_TX }),
        }),
      ),
    );
    h.readTransaction.mockRejectedValue(new Error("Base unavailable"));
    const second = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await second.start();
    await vi.advanceTimersByTimeAsync(4000);
    expect(h.last()).toMatchObject({
      kind: "network_unavailable",
      source: "base",
      lastObservedAtMs: 123,
    });
    second.dispose();
  });

  it("retains acceptance before a failed verifier POST and never reproposes on reload", async () => {
    const h = harness();
    h.startRun.mockRejectedValue(new Error("offline"));
    await createRunFlow("honest", config, (u) => h.updates.push(u), h.deps).start();
    const resumed = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await resumed.start();
    expect(h.propose).toHaveBeenCalledTimes(1);
    expect(h.startRun).toHaveBeenCalledTimes(1);
    expect(h.updates.at(-1)?.proposalId).toBe(7);
    resumed.dispose();
  });

  it("keeps an explicit verifier refusal distinct from a transport outage with injected clients", async () => {
    const h = harness();
    h.startRun.mockRejectedValue(new RunRefusedError("queue_full", 429, "queue_full", 10));
    await createRunFlow("honest", config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "error", cause: "run_refused" });
    expect(h.updates.at(-1)?.proposalId).toBe(7);
  });

  it("stops before further I/O when disposed by a state update callback", async () => {
    const h = harness();
    const getConfig = vi.fn(async () => view(gateConfig));
    h.deps.gate = { getConfig, getProposal: h.getProposal };
    const flow = createRunFlow(
      "honest",
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
    const flow = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
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
    await createRunFlow("honest", config, (u) => h.updates.push(u), h.deps).start();
    await createRunFlow("honest", config, (u) => h.updates.push(u), h.deps).start();
    expect(h.propose).toHaveBeenCalledTimes(1);
    expect(h.last()).toMatchObject({ kind: "error", cause: "other" });
    expect(JSON.stringify(h.updates)).not.toContain(config.demoSecretKey);
  });

  it("does not broadcast an attack that unexpectedly obtained a signature", async () => {
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
    await createRunFlow("attack", config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "error", cause: "other" });
    expect(h.relay).not.toHaveBeenCalled();
  });

  it("maps broadcast failures and refuses included transactions with divergent slot state", async () => {
    for (const broadcastFailure of [true, false]) {
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
      if (broadcastFailure) h.relay.mockRejectedValue(new Error("send failed"));
      h.readTransaction.mockResolvedValue({
        status: "included",
        block: 42,
        block_hash: hex(1, 32),
      });
      h.readImplementation
        .mockResolvedValueOnce(proxyRead(OLD_IMPL, 1))
        .mockResolvedValue(proxyRead(OLD_IMPL, 42));
      await createRunFlow("honest", config, (u) => h.updates.push(u), h.deps).start();
      expect(h.last()).toMatchObject({
        kind: "error",
        cause: broadcastFailure ? "broadcast_failed" : "implementation_mismatch",
      });
      expect(vi.getTimerCount()).toBe(0);
    }
  });

  it("persists an accepted result after disposal without starting any further network work", async () => {
    const h = harness();
    const pending = Promise.withResolvers<{ proposalId: number; nearTxHash: string }>();
    const sending = Promise.withResolvers<void>();
    h.propose.mockImplementation(async () => {
      sending.resolve();
      return pending.promise;
    });
    const flow = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    const started = flow.start();
    await sending.promise;
    flow.dispose();
    const count = h.updates.length;
    pending.resolve({ proposalId: 7, nearTxHash: "11111111111111111111111111111111" });
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
    await createRunFlow("honest", config, (u) => h.updates.push(u), h.deps).start();
    expect(h.last()).toMatchObject({ kind: "error", cause: "other" });
    expect(h.propose).not.toHaveBeenCalled();
  });

  it("disposes pending timers and prevents later callbacks", async () => {
    const h = harness();
    const flow = createRunFlow("honest", config, (u) => h.updates.push(u), h.deps);
    await flow.start();
    flow.dispose();
    const count = h.updates.length;
    await vi.advanceTimersByTimeAsync(10000);
    expect(h.updates).toHaveLength(count);
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe("activation config", () => {
  const env = {
    PUBLIC_NEAR_RPC: config.nearRpc,
    PUBLIC_GATE_ACCOUNT: config.gateAccount,
    PUBLIC_VERIFIER_URL: config.verifierUrl,
    PUBLIC_BASE_RPC: config.baseRpc,
    PUBLIC_DEMO_ACCOUNT_ID: config.demoAccountId,
    PUBLIC_DEMO_SECRET_KEY: config.demoSecretKey,
    PUBLIC_ATTACK_IMPL: NEW_IMPL,
    PUBLIC_ATTACK_CODEHASH: hex(1, 32),
    PUBLIC_ATTACK_CURRENT_IMPL: OLD_IMPL,
    PUBLIC_ATTACK_INIT_CALLDATA: "0x",
    PUBLIC_HONEST_IMPL: NEW_IMPL,
    PUBLIC_HONEST_CODEHASH: hex(2, 32),
    PUBLIC_HONEST_CURRENT_IMPL: OLD_IMPL,
    PUBLIC_HONEST_INIT_CALLDATA: "0x",
  };
  it("is dormant with empty or incomplete env and accepts explicit valid deployment values", () => {
    expect(readFlowConfig({})).toBeNull();
    expect(readFlowConfig({ PUBLIC_GATE_ACCOUNT: "gate.testnet" })).toBeNull();
    expect(readFlowConfig(env)).toMatchObject({
      gateAccount: "gate.testnet",
      proposals: { attack: { newImpl: NEW_IMPL } },
    });
    for (const key of Object.keys(env)) {
      expect(readFlowConfig({ ...env, [key]: undefined })).toBeNull();
    }
  });
  it("refuses invalid accounts, addresses, hashes, keys, urls and calldata", () => {
    for (const [key, value] of Object.entries({
      PUBLIC_GATE_ACCOUNT: "Gate.testnet",
      PUBLIC_DEMO_ACCOUNT_ID: "a",
      PUBLIC_DEMO_SECRET_KEY: "ed25519:abc",
      PUBLIC_ATTACK_IMPL: hex(0, 20),
      PUBLIC_ATTACK_CODEHASH: "0xff",
      PUBLIC_HONEST_CURRENT_IMPL: "0xBAD",
      PUBLIC_NEAR_RPC: "javascript:alert(1)",
      PUBLIC_VERIFIER_URL: "https://secret@example.com",
      PUBLIC_HONEST_INIT_CALLDATA: "0x0",
    })) {
      expect(readFlowConfig({ ...env, [key]: value })).toBeNull();
    }
  });
});
