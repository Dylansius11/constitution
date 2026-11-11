import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { GateWriteError, type ProposeArgs, proposeWithSessionKey } from "../src/lib/near-write.ts";
import { relaySignedTransaction, verifyRelay } from "../src/lib/relay.ts";
import { hex } from "./support/fixtures.ts";

const args: ProposeArgs = {
  new_impl: hex(1, 20),
  new_impl_codehash: hex(2, 32),
  expected_current_impl: hex(3, 20),
  init_calldata: "0x",
  source_commitment: hex(4, 32),
};
const config = {
  nearRpc: "https://rpc.example",
  gateAccount: "gate.testnet",
  accountId: "demo.testnet",
  secretKey: "ed25519:never-print-this",
};
const txHash = "11111111111111111111111111111111";
const event = (id: number, executor = "gate.testnet") => ({
  outcome: {
    executor_id: executor,
    logs: [
      `EVENT_JSON:${JSON.stringify({ standard: "constitution", version: "1.0.0", event: "proposal_created", data: [{ proposal_id: id }] })}`,
    ],
  },
});
const response = (value: string, receipts = [event(7)]) => ({
  transaction: { hash: txHash },
  status: { SuccessValue: btoa(value) },
  receipts_outcome: receipts,
});

describe("session-key NEAR propose", () => {
  it("uses only propose, zero attachment and no automatic resubmission", async () => {
    let observed: unknown;
    const accepted = await proposeWithSessionKey(config, args, async (request) => {
      observed = request;
      return response("7");
    });
    expect(observed).toEqual({
      receiverId: "gate.testnet",
      methodName: "propose",
      args: { args },
      gas: 30_000_000_000_000n,
      deposit: 0n,
    });
    expect(accepted).toEqual({ proposalId: 7, nearTxHash: txHash });
  });

  it("can read the gate event when the final return is empty", async () => {
    expect(await proposeWithSessionKey(config, args, async () => response(""))).toEqual({
      proposalId: 7,
      nearTxHash: txHash,
    });
  });

  it("rejects imprecise ids, conflicting result/log ids and non-gate logs", async () => {
    for (const result of [
      response("9007199254740993", []),
      response("8"),
      response("", [event(7, "other.testnet")]),
    ]) {
      await expect(proposeWithSessionKey(config, args, async () => result)).rejects.toThrow();
    }
  });

  it("does not treat an event as acceptance without a successful final execution status", async () => {
    await expect(
      proposeWithSessionKey(config, args, async () => ({
        ...response("7"),
        status: "Started",
      })),
    ).rejects.toThrow();
  });

  it("maps recorded panic failures to gate_refused with only the stable code", async () => {
    try {
      await proposeWithSessionKey(config, args, async () => ({
        status: {
          Failure: {
            ActionError: {
              kind: {
                FunctionCallError: {
                  ExecutionError: "Smart contract panicked: E11_INSUFFICIENT_CREDIT",
                },
              },
            },
          },
        },
      }));
      expect.fail("expected refusal");
    } catch (error) {
      expect(error).toBeInstanceOf(GateWriteError);
      expect(error).toMatchObject({ cause: "gate_refused", detail: "E11_INSUFFICIENT_CREDIT" });
    }
  });

  it("never exposes a secret from a thrown transport error", async () => {
    await expect(
      proposeWithSessionKey(config, args, async () => {
        throw new Error(config.secretKey);
      }),
    ).rejects.toThrow("NEAR proposal submission failed");
  });
});

describe("verified permissionless relay", () => {
  const spike = JSON.parse(
    readFileSync(new URL("../../vectors/evm-tx-spike.json", import.meta.url), "utf8"),
  );
  const signed = spike.cases[0].signed;
  const config = {
    chain_id: 84532,
    executor: spike.deployment.addresses.executor,
    derived_eoa: spike.signer,
  };
  const evm = { nonce: 0, signing_hash: signed.signing_hash, signed_tx: signed.raw_tx };

  it("recovers the real MPC vector and derives the identical transaction hash", async () => {
    expect(await verifyRelay(config, evm)).toEqual({ raw: signed.raw_tx, hash: signed.tx_hash });
    for (const wrong of [
      { ...config, derived_eoa: hex(9, 20) },
      { ...config, chain_id: 8453 },
      { ...config, executor: hex(9, 20) },
    ]) {
      await expect(verifyRelay(wrong, evm)).rejects.toThrow();
    }
    await expect(verifyRelay(config, { ...evm, nonce: 1 })).rejects.toThrow();
    await expect(verifyRelay(config, { ...evm, signing_hash: hex(9, 32) })).rejects.toThrow();
    await expect(
      verifyRelay(config, { ...evm, signed_tx: `${signed.raw_tx}00` }),
    ).rejects.toThrow();
  });

  it("sends only verified raw bytes and accepts a relay race only when the identical hash is known", async () => {
    const calls: string[] = [];
    const transport = {
      chainId: async () => 84532,
      knows: async () => calls.length > 0,
      send: async (raw: string) => {
        calls.push(raw);
        throw new Error("nonce too low");
      },
    };
    expect(await relaySignedTransaction("https://base.example", config, evm, transport)).toBe(
      signed.tx_hash,
    );
    expect(calls).toEqual([signed.raw_tx]);
    await expect(
      relaySignedTransaction("https://base.example", config, evm, {
        ...transport,
        knows: async () => false,
      }),
    ).rejects.toThrow();
    await expect(
      relaySignedTransaction("https://base.example", config, evm, {
        ...transport,
        chainId: async () => 8453,
      }),
    ).rejects.toThrow();
  });
});
