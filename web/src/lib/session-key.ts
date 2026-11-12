/**
 * The visitor's session key: an ed25519 key made in this browser and added to their NEAR account
 * as a function-call key that may call only `propose` on the demo gate, with a small gas
 * allowance. It cannot transfer funds, call `withdraw_credit` or touch any other contract, so
 * keeping it in `localStorage` is a bounded risk the page states in plain words.
 */

/** 0.25 NEAR, the near-connect default. Each `propose` spends a fraction of a milliNEAR of it. */
export const SESSION_ALLOWANCE_YOCTO = 250_000_000_000_000_000_000_000n;
/** Below this much allowance left the page asks for a fresh key instead of risking a failed call. */
export const SESSION_ALLOWANCE_FLOOR_YOCTO = 10_000_000_000_000_000_000_000n;
export const SESSION_METHODS = ["propose"] as const;

export interface SessionKey {
  publicKey: string;
  /** `ed25519:<base58 of 64 bytes>`. Never log it, render it or put it in an error. */
  secretKey: string;
  createdAtMs: number;
}

const PUBLIC_KEY = /^ed25519:[1-9A-HJ-NP-Za-km-z]{43,44}$/;
const SECRET_KEY = /^ed25519:[1-9A-HJ-NP-Za-km-z]{86,88}$/;

export function sessionStorageKey(gate: string, accountId: string): string {
  return `constitution:session:${gate}:${accountId}`;
}

export function parseSessionKey(text: string | null): SessionKey | null {
  if (text === null) return null;
  try {
    const value: unknown = JSON.parse(text);
    if (typeof value !== "object" || value === null) return null;
    const { publicKey, secretKey, createdAtMs } = value as Record<string, unknown>;
    if (
      typeof publicKey !== "string" ||
      typeof secretKey !== "string" ||
      typeof createdAtMs !== "number" ||
      !PUBLIC_KEY.test(publicKey) ||
      !SECRET_KEY.test(secretKey)
    )
      return null;
    return { publicKey, secretKey, createdAtMs };
  } catch {
    return null;
  }
}

export function loadSessionKey(
  storage: Pick<Storage, "getItem">,
  gate: string,
  accountId: string,
): SessionKey | null {
  return parseSessionKey(storage.getItem(sessionStorageKey(gate, accountId)));
}

export function saveSessionKey(
  storage: Pick<Storage, "setItem">,
  gate: string,
  accountId: string,
  key: SessionKey,
): void {
  storage.setItem(sessionStorageKey(gate, accountId), JSON.stringify(key));
}

export function forgetSessionKey(
  storage: Pick<Storage, "removeItem">,
  gate: string,
  accountId: string,
): void {
  storage.removeItem(sessionStorageKey(gate, accountId));
}

/** A new random key. The SDK loads here, at the first click that needs it, not on page load. */
export async function generateSessionKey(now: () => number = Date.now): Promise<SessionKey> {
  const { KeyPairEd25519 } = await import("near-api-js");
  const pair = KeyPairEd25519.fromRandom();
  return { publicKey: pair.publicKey.toString(), secretKey: pair.toString(), createdAtMs: now() };
}
