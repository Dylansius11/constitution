import type { GateClient } from "./gate.ts";
import { SESSION_ALLOWANCE_FLOOR_YOCTO } from "./session-key.ts";

/**
 * What the demo gate says about one visitor account, and what that allows. Every number is read
 * from the gate or from the account's access keys; nothing is assumed. `propose` needs credit of
 * `required` (a reservation that returns when the proposal ends) plus `fee` (spent for good), so
 * `n` more attacks need `required + n * fee` in credit.
 */

export type KeyState =
  /** The visitor has no local session key. */
  | "none"
  /** A local key exists but the account does not have it: removed elsewhere, or never added. */
  | "not_on_chain"
  | "low_allowance"
  | "active";

export interface Standing {
  credit: bigint;
  required: bigint;
  fee: bigint;
  entryFee: bigint;
  active: number | null;
  key: KeyState;
  /** Remaining gas allowance of the session key, when it has one. */
  allowance: bigint | null;
}

export type StandingReader = Pick<
  GateClient,
  "getCredit" | "getRequiredCredit" | "getProposalFee" | "getCreditEntryFee" | "getActive" | "getAccessKey"
>;

export async function readStanding(
  gate: StandingReader,
  accountId: string,
  sessionPublicKey: string | null,
): Promise<Standing> {
  const [credit, required, fee, entryFee, active, key] = await Promise.all([
    gate.getCredit(accountId),
    gate.getRequiredCredit(),
    gate.getProposalFee(),
    gate.getCreditEntryFee(accountId),
    gate.getActive(accountId),
    sessionPublicKey === null ? Promise.resolve(undefined) : gate.getAccessKey(accountId, sessionPublicKey),
  ]);
  let keyState: KeyState = "none";
  let allowance: bigint | null = null;
  if (key !== undefined) {
    if (key === null || key.value.fullAccess) keyState = "not_on_chain";
    else {
      allowance = key.value.allowance;
      keyState =
        allowance !== null && allowance < SESSION_ALLOWANCE_FLOOR_YOCTO ? "low_allowance" : "active";
    }
  }
  return {
    credit: credit.value,
    required: required.value,
    fee: fee.value,
    entryFee: entryFee.value,
    active: active.value,
    key: keyState,
    allowance,
  };
}

/** How many more `propose` calls the current credit pays for. */
export function attemptsAffordable(standing: Standing): number {
  if (standing.fee === 0n || standing.credit < standing.required) return 0;
  const attempts = (standing.credit - standing.required) / standing.fee;
  return attempts > 1000n ? 1000 : Number(attempts);
}

/**
 * Yocto to deposit so credit covers `attempts` attacks. A zero balance may mean the account has no
 * credit entry yet, whose one-time storage fee comes out of the first deposit, so it is added then.
 */
export function depositFor(standing: Standing, attempts: number): bigint {
  const target = standing.required + standing.fee * BigInt(attempts);
  const missing = target > standing.credit ? target - standing.credit : 0n;
  if (missing === 0n) return 0n;
  return standing.credit === 0n ? missing + standing.entryFee : missing;
}

export type Readiness =
  | { kind: "active_proposal"; proposalId: number }
  | { kind: "needs_credit" }
  | { kind: "ready"; signing: "session" | "wallet"; attempts: number };

export function readiness(standing: Standing): Readiness {
  if (standing.active !== null) return { kind: "active_proposal", proposalId: standing.active };
  const attempts = attemptsAffordable(standing);
  if (attempts < 1) return { kind: "needs_credit" };
  return { kind: "ready", signing: standing.key === "active" ? "session" : "wallet", attempts };
}

/** NEAR with at most 4 decimals: `0.1`, `0.0142`, `<0.0001`. Display only, never parsed back. */
export function formatNear(yocto: bigint): string {
  if (yocto === 0n) return "0";
  const unit = 10n ** 24n;
  const tenThousandths = (yocto * 10_000n) / unit;
  if (tenThousandths === 0n) return "<0.0001";
  const whole = tenThousandths / 10_000n;
  const fraction = (tenThousandths % 10_000n).toString().padStart(4, "0").replace(/0+$/, "");
  return fraction === "" ? whole.toString() : `${whole}.${fraction}`;
}
