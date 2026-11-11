import { describe, expect, it, vi } from "vitest";
import { parseDemoGallery } from "../src/lib/demo-gallery.ts";
import { reasonText, ruleLabel } from "../src/lib/reasons.ts";
import {
  generateSessionKey,
  loadSessionKey,
  parseSessionKey,
  SESSION_ALLOWANCE_FLOOR_YOCTO,
  saveSessionKey,
} from "../src/lib/session-key.ts";
import { planSetup, runSetup } from "../src/lib/session-setup.ts";
import {
  attemptsAffordable,
  depositFor,
  formatNear,
  readiness,
  readStanding,
  type Standing,
} from "../src/lib/standing.ts";
import { WalletError, type WalletPort, WalletRejectedError } from "../src/lib/wallet.ts";
import {
  addSessionKeyTx,
  depositCreditTx,
  expireProposalTx,
  proposeTx,
  removeSessionKeyTx,
  withdrawCreditTx,
} from "../src/lib/wallet-actions.ts";
import { hex } from "./support/fixtures.ts";

const NEAR = 10n ** 24n;
const GATE = "demo.testnet";
const ME = "alice.testnet";
const KEY = {
  publicKey: `ed25519:${"3".repeat(44)}`,
  secretKey: `ed25519:${"4".repeat(88)}`,
  createdAtMs: 1,
};

const attack = (over: Record<string, unknown> = {}) => ({
  id: "drain",
  title: "Drain dressed as a fee update",
  summary: "One address can withdraw the whole vault.",
  expected_rule: 2,
  expected_reason: 20,
  contract: "src/demo/VaultV2Drain.sol:VaultV2Drain",
  source_files: ["src/demo/VaultV1.sol", "src/demo/VaultV2Drain.sol"],
  impl: hex(0x4a, 20),
  codehash: hex(0xc0, 32),
  init_calldata: "0x",
  observed: null,
  ...over,
});
const gallery = (over: Record<string, unknown> = {}) => ({
  version: 1,
  gate: GATE,
  proxy: hex(0x11, 20),
  attacks: [attack()],
  upgrades: [{ from: "VaultV1", to: "VaultV2Honest", observed: null }],
  ...over,
});

describe("demo gallery", () => {
  it("parses the attacks and keeps unobserved runs as null", () => {
    const parsed = parseDemoGallery(gallery());
    expect(parsed.gate).toBe(GATE);
    expect(parsed.attacks[0]).toMatchObject({
      id: "drain",
      expectedRule: 2,
      expectedReason: 20,
      sourceFiles: ["src/demo/VaultV1.sol", "src/demo/VaultV2Drain.sol"],
      observed: null,
    });
  });

  it("reads an observed run, and requires a Base tx on an observed upgrade", () => {
    const observed = {
      proposal_id: 4,
      near_tx: "11111111111111111111111111111111",
      observed_at: "2026-11-12T10:00:00Z",
    };
    expect(parseDemoGallery(gallery({ attacks: [attack({ observed })] })).attacks[0]?.observed).toEqual({
      proposalId: 4,
      nearTx: observed.near_tx,
      observedAt: observed.observed_at,
    });
    const upgrade = { from: "VaultV1", to: "VaultV2Honest", observed };
    expect(() => parseDemoGallery(gallery({ upgrades: [upgrade] }))).toThrow(/base_tx/);
    const withBase = parseDemoGallery(
      gallery({ upgrades: [{ ...upgrade, observed: { ...observed, base_tx: hex(9, 32) } }] }),
    );
    expect(withBase.upgrades[0]?.observed?.baseTx).toBe(hex(9, 32));
  });

  it("refuses anything that could build a wrong bundle or a wrong target", () => {
    const bad: Array<Record<string, unknown>> = [
      { version: 2 },
      { attacks: [] },
      { attacks: [attack(), attack()] },
      { attacks: [attack({ id: "Not A Slug" })] },
      { attacks: [attack({ impl: hex(0, 20) })] },
      { attacks: [attack({ codehash: "0xff" })] },
      { attacks: [attack({ contract: "VaultV2Drain" })] },
      { attacks: [attack({ source_files: ["src/demo/VaultV1.sol"] })] },
      { attacks: [attack({ source_files: ["src/../secret.sol", "src/demo/VaultV2Drain.sol"] })] },
      { attacks: [attack({ source_files: ["/etc/passwd.sol", "src/demo/VaultV2Drain.sol"] })] },
      { attacks: [attack({ init_calldata: `0x${"00".repeat(1025)}` })] },
      { attacks: [attack({ expected_rule: 9 })] },
      { gate: "Not A Gate" },
    ];
    for (const over of bad) expect(() => parseDemoGallery(gallery(over))).toThrow();
  });
});

describe("rule and reason vocabulary", () => {
  it("names known ids and never invents a meaning for an unknown code", () => {
    expect(ruleLabel(2)).toBe("R2 Solvency");
    expect(ruleLabel(0)).toBe("Pre-check");
    expect(ruleLabel(42)).toBe("Rule 42");
    expect(reasonText(30)).toContain("SELFDESTRUCT");
    expect(reasonText(9999)).toBeNull();
  });
});

describe("session key", () => {
  it("round-trips through storage and refuses malformed or foreign records", () => {
    const store = new Map<string, string>();
    const storage = {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
    };
    expect(loadSessionKey(storage, GATE, ME)).toBeNull();
    saveSessionKey(storage, GATE, ME, KEY);
    expect(loadSessionKey(storage, GATE, ME)).toEqual(KEY);
    expect(loadSessionKey(storage, GATE, "bob.testnet")).toBeNull();
    expect(parseSessionKey("not json")).toBeNull();
    expect(parseSessionKey(JSON.stringify({ ...KEY, secretKey: "ed25519:abc" }))).toBeNull();
    expect(parseSessionKey(JSON.stringify({ ...KEY, publicKey: "secp256k1:abc" }))).toBeNull();
  });

  it("generates a distinct, well-formed ed25519 pair each time", async () => {
    const [a, b] = [await generateSessionKey(() => 5), await generateSessionKey(() => 5)];
    expect(parseSessionKey(JSON.stringify(a))).toEqual(a);
    expect(a.publicKey).not.toBe(b.publicKey);
    expect(a.createdAtMs).toBe(5);
  });
});

describe("wallet transactions", () => {
  it("add a key limited to propose on the gate, with an allowance, on the visitor's own account", () => {
    const tx = addSessionKeyTx(ME, GATE, KEY.publicKey);
    expect(tx.receiverId).toBe(ME);
    expect(tx.actions).toEqual([
      {
        type: "AddKey",
        params: {
          publicKey: KEY.publicKey,
          accessKey: {
            permission: {
              receiverId: GATE,
              allowance: "250000000000000000000000",
              methodNames: ["propose"],
            },
          },
        },
      },
    ]);
  });

  it("carry amounts as exact decimal strings and never attach a deposit to a withdrawal or a proposal", () => {
    const deposit = depositCreditTx(GATE, 123_456_789_012_345_678_901_234_567n);
    expect(deposit.actions[0]).toMatchObject({
      params: { methodName: "deposit_credit", deposit: "123456789012345678901234567" },
    });
    expect(withdrawCreditTx(GATE, 7n).actions[0]).toMatchObject({
      params: { methodName: "withdraw_credit", args: { amount: "7" }, deposit: "0" },
    });
    expect(expireProposalTx(GATE, 3).actions[0]).toMatchObject({
      params: { methodName: "expire", args: { proposal_id: 3 }, deposit: "0" },
    });
    expect(removeSessionKeyTx(ME, KEY.publicKey).actions[0]).toEqual({
      type: "DeleteKey",
      params: { publicKey: KEY.publicKey },
    });
    const propose = proposeTx(GATE, {
      new_impl: hex(1, 20),
      new_impl_codehash: hex(2, 32),
      expected_current_impl: hex(3, 20),
      init_calldata: "0x",
      source_commitment: hex(4, 32),
    });
    expect(propose.actions[0]).toMatchObject({ params: { methodName: "propose", deposit: "0" } });
  });
});

const standing = (over: Partial<Standing> = {}): Standing => ({
  credit: 0n,
  required: NEAR / 10n,
  fee: NEAR / 100n,
  entryFee: NEAR / 50n,
  active: null,
  key: "none",
  allowance: null,
  ...over,
});

describe("account standing", () => {
  it("counts the attacks the credit pays for: a reservation that returns plus a fee that does not", () => {
    expect(attemptsAffordable(standing({ credit: 0n }))).toBe(0);
    expect(attemptsAffordable(standing({ credit: NEAR / 10n }))).toBe(0);
    expect(attemptsAffordable(standing({ credit: NEAR / 10n + NEAR / 100n }))).toBe(1);
    expect(attemptsAffordable(standing({ credit: NEAR / 10n + (NEAR / 100n) * 5n - 1n }))).toBe(4);
    expect(attemptsAffordable(standing({ fee: 0n, credit: NEAR }))).toBe(0);
  });

  it("deposits only the shortfall, and adds the entry fee only for a zero balance", () => {
    const fresh = standing();
    expect(depositFor(fresh, 10)).toBe(NEAR / 10n + (NEAR / 100n) * 10n + NEAR / 50n);
    const funded = standing({ credit: NEAR / 10n + (NEAR / 100n) * 4n });
    expect(depositFor(funded, 4)).toBe(0n);
    expect(depositFor(funded, 10)).toBe((NEAR / 100n) * 6n);
  });

  it("orders readiness: an active proposal first, then credit, then how the run will be signed", () => {
    expect(readiness(standing({ active: 3, credit: NEAR }))).toEqual({
      kind: "active_proposal",
      proposalId: 3,
    });
    expect(readiness(standing())).toEqual({ kind: "needs_credit" });
    expect(readiness(standing({ credit: NEAR, key: "active" }))).toMatchObject({
      kind: "ready",
      signing: "session",
    });
    for (const key of ["none", "not_on_chain", "low_allowance"] as const)
      expect(readiness(standing({ credit: NEAR, key }))).toMatchObject({ signing: "wallet" });
  });

  it("formats NEAR for display without rounding a tiny amount to zero", () => {
    expect(formatNear(0n)).toBe("0");
    expect(formatNear(NEAR / 10n)).toBe("0.1");
    expect(formatNear(NEAR + (NEAR * 142n) / 10_000n)).toBe("1.0142");
    expect(formatNear(1n)).toBe("<0.0001");
  });

  it("reads the standing from the gate and classifies the session key on the account", async () => {
    const reads = (key: unknown) => ({
      getCredit: async () => ({ value: 5n, blockHeight: 1, blockHash: "h" }),
      getRequiredCredit: async () => ({ value: 2n, blockHeight: 1, blockHash: "h" }),
      getProposalFee: async () => ({ value: 1n, blockHeight: 1, blockHash: "h" }),
      getCreditEntryFee: async () => ({ value: 3n, blockHeight: 1, blockHash: "h" }),
      getActive: async () => ({ value: null, blockHeight: 1, blockHash: "h" }),
      getAccessKey: async () => key as never,
    });
    const info = (allowance: bigint | null) => ({
      value: { allowance, receiverId: GATE, methodNames: ["propose"], fullAccess: false },
      blockHeight: 1,
      blockHash: "h",
    });
    expect((await readStanding(reads(null), ME, null)).key).toBe("none");
    expect((await readStanding(reads(null), ME, KEY.publicKey)).key).toBe("not_on_chain");
    expect((await readStanding(reads(info(SESSION_ALLOWANCE_FLOOR_YOCTO)), ME, KEY.publicKey)).key).toBe("active");
    expect(
      (await readStanding(reads(info(SESSION_ALLOWANCE_FLOOR_YOCTO - 1n)), ME, KEY.publicKey)).key,
    ).toBe("low_allowance");
    expect((await readStanding(reads(info(null)), ME, KEY.publicKey)).key).toBe("active");
    const all = await readStanding(reads(info(null)), ME, KEY.publicKey);
    expect(all).toMatchObject({ credit: 5n, required: 2n, fee: 1n, entryFee: 3n, active: null });
  });
});

function wallet(send: WalletPort["send"]): WalletPort {
  return {
    restore: async () => null,
    connect: async () => ME,
    disconnect: async () => {},
    send,
    onSignOut: () => {},
  };
}

describe("enabling live runs", () => {
  const plan = planSetup({ gate: GATE, accountId: ME, standing: standing(), key: KEY, attempts: 10 });

  it("asks for a deposit and a session key together, and for nothing that is already in place", () => {
    expect(plan.transactions.map((tx) => tx.receiverId)).toEqual([GATE, ME]);
    const ready = planSetup({
      gate: GATE,
      accountId: ME,
      standing: standing({ credit: NEAR, key: "active" }),
      key: KEY,
      attempts: 10,
    });
    expect(ready.transactions).toEqual([]);
    const keyOnly = planSetup({
      gate: GATE,
      accountId: ME,
      standing: standing({ credit: NEAR, key: "not_on_chain" }),
      key: KEY,
      attempts: 10,
    });
    expect(keyOnly.transactions.map((tx) => tx.receiverId)).toEqual([ME]);
  });

  it("sends one batch and reports done only from the re-read chain state", async () => {
    const send = vi.fn(async () => []);
    const read = vi.fn(async () => standing({ credit: NEAR, key: "active" }));
    const result = await runSetup({ wallet: wallet(send), plan, attempts: 10, read });
    expect(result.status).toBe("done");
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith(plan.transactions);
  });

  it("does not take the wallet's success for proof when the chain still lacks the key", async () => {
    const send = vi.fn(async () => []);
    const read = async () => standing({ credit: NEAR, key: "none" });
    const result = await runSetup({ wallet: wallet(send), plan, attempts: 10, read });
    expect(result).toMatchObject({ status: "partial", missing: ["session_key"] });
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("stops at once on a decline and never retries", async () => {
    const send = vi.fn(async () => {
      throw new WalletRejectedError();
    });
    await expect(
      runSetup({ wallet: wallet(send), plan, attempts: 10, read: async () => standing() }),
    ).rejects.toBeInstanceOf(WalletRejectedError);
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("after a failed batch asks again only for what the chain still lacks, never twice for the deposit", async () => {
    const send = vi.fn(async (txs: readonly { receiverId: string }[]) => {
      if (txs.length > 1) throw new WalletError();
      return [];
    });
    // The batch landed its deposit but not its key.
    const reads = [
      standing({ credit: NEAR, key: "none" }),
      standing({ credit: NEAR, key: "active" }),
    ];
    const read = vi.fn(async () => reads.shift() ?? standing({ credit: NEAR, key: "active" }));
    const result = await runSetup({ wallet: wallet(send), plan, attempts: 10, read });
    expect(result.status).toBe("done");
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.calls[1]?.[0]).toHaveLength(1);
    expect(send.mock.calls[1]?.[0]?.[0]?.receiverId).toBe(ME);
  });
});
