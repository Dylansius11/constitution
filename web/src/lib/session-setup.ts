import type { SessionKey } from "./session-key.ts";
import { depositFor, type Standing } from "./standing.ts";
import { type WalletPort, WalletRejectedError } from "./wallet.ts";
import { addSessionKeyTx, depositCreditTx, type WalletTx } from "./wallet-actions.ts";

/**
 * "Enable live runs": the one wallet confirmation that funds the visitor's credit on the demo
 * gate and adds the session key to their account. The two transactions go to different receivers
 * (the gate and the account itself), so a batch needs a wallet that supports `signAndSendTransactions`;
 * otherwise the wallet confirms each. Success is read back from chain, never taken from the wallet.
 */

export interface SetupPlan {
  deposit: bigint;
  addKey: boolean;
  transactions: WalletTx[];
}

export function planSetup(input: {
  gate: string;
  accountId: string;
  standing: Standing;
  key: SessionKey;
  attempts: number;
}): SetupPlan {
  const deposit = depositFor(input.standing, input.attempts);
  const addKey = input.standing.key !== "active";
  const transactions: WalletTx[] = [];
  if (deposit > 0n) transactions.push(depositCreditTx(input.gate, deposit));
  if (addKey)
    transactions.push(addSessionKeyTx(input.accountId, input.gate, input.key.publicKey));
  return { deposit, addKey, transactions };
}

export type SetupResult =
  | { status: "done"; standing: Standing }
  /** The wallet confirmed or failed, but chain state still lacks part of the plan. */
  | { status: "partial"; standing: Standing; missing: Array<"credit" | "session_key"> };

function missing(plan: SetupPlan, standing: Standing, attempts: number): Array<"credit" | "session_key"> {
  const gaps: Array<"credit" | "session_key"> = [];
  if (plan.deposit > 0n && depositFor(standing, attempts) > 0n) gaps.push("credit");
  if (plan.addKey && standing.key !== "active") gaps.push("session_key");
  return gaps;
}

/**
 * Sends the plan as one batch. A non-decline failure does not prove nothing happened (a batch can
 * land in part), so the chain is re-read and only what is still missing is asked again, one
 * confirmation each. A decline stops everything and surfaces to the caller.
 */
export async function runSetup(input: {
  wallet: WalletPort;
  plan: SetupPlan;
  attempts: number;
  read: () => Promise<Standing>;
}): Promise<SetupResult> {
  const { wallet, plan, attempts, read } = input;
  if (plan.transactions.length === 0) return { status: "done", standing: await read() };
  let failed = false;
  try {
    await wallet.send(plan.transactions);
  } catch (error) {
    if (error instanceof WalletRejectedError) throw error;
    failed = true;
  }
  let standing = await read();
  if (failed) {
    for (const tx of plan.transactions) {
      const isDeposit = tx.actions[0]?.type === "FunctionCall";
      const gaps = missing(plan, standing, attempts);
      if (isDeposit ? !gaps.includes("credit") : !gaps.includes("session_key")) continue;
      try {
        await wallet.send([tx]);
      } catch (error) {
        if (error instanceof WalletRejectedError) throw error;
      }
      standing = await read();
    }
  }
  const gaps = missing(plan, standing, attempts);
  return gaps.length === 0 ? { status: "done", standing } : { status: "partial", standing, missing: gaps };
}
