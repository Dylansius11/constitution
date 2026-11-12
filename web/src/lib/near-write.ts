import { base64ToBytes } from "./bytes.ts";
import type { SurfaceErrorCause } from "./state.ts";

/**
 * The page's only NEAR write: `propose` on the demo gate, signed locally with the visitor's own
 * restricted session key (a function-call key limited to `propose` on the gate). No secret key
 * ever appears in an error, a log or a DOM node.
 */
export interface SessionSignerConfig {
  nearRpc: string;
  gateAccount: string;
  /** The visitor's NEAR account; it pays gas from the key's allowance and owns the credit. */
  accountId: string;
  /** Session secret key, `ed25519:<base58>`. Never include it in errors or logs. */
  secretKey: string;
}

export interface ProposeArgs {
  new_impl: string;
  new_impl_codehash: string;
  expected_current_impl: string;
  init_calldata: string;
  source_commitment: string;
}

export interface AcceptedProposal {
  proposalId: number;
  nearTxHash: string;
}

/** Signs and sends one `propose`; the run flow never knows whether a key or a wallet popup signed it. */
export type ProposeSigner = (args: ProposeArgs) => Promise<AcceptedProposal>;

export interface ProposeRequest {
  receiverId: string;
  methodName: "propose";
  args: { args: ProposeArgs };
  gas: bigint;
  deposit: bigint;
}

export const PROPOSE_GAS = 30_000_000_000_000n;

export class GateWriteError extends Error {
  override readonly cause: SurfaceErrorCause;
  readonly detail: string;

  constructor(cause: SurfaceErrorCause, detail: string) {
    super(detail);
    this.name = "GateWriteError";
    this.cause = cause;
    this.detail = detail;
  }
}

function object(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
}

function id(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

/** Reads only the bounded stable panic code, never an RPC message containing signing inputs. */
function panicCode(value: unknown): string | null {
  const text = value instanceof Error ? value.message : JSON.stringify(value);
  return text?.match(/\bE\d{2}_[A-Z_]+\b/)?.[0] ?? null;
}

/**
 * Failures of the session key itself, recognised from the RPC's named error kinds. The match reads
 * the error text only to choose one of these fixed messages; the text is never shown.
 */
function sessionKeyFailure(value: unknown): GateWriteError | null {
  const text = value instanceof Error ? value.message : JSON.stringify(value);
  if (text === undefined) return null;
  if (/NotEnoughAllowance/.test(text))
    return new GateWriteError(
      "session_key",
      "The session key has used up its gas allowance. Remove it and enable live runs again to get a fresh one.",
    );
  if (/AccessKeyNotFound|UnknownAccessKey|does not exist while viewing/.test(text))
    return new GateWriteError(
      "session_key",
      "The session key is not on your account any more. Enable live runs again to add a new one.",
    );
  if (/NotEnoughBalance|LackBalanceForState/.test(text))
    return new GateWriteError(
      "session_key",
      "Your account does not have enough NEAR to pay gas for this call.",
    );
  return null;
}

/**
 * Reads a final `propose` outcome. Never guesses: a missing hash, a missing id or an id that
 * disagrees with the gate's `proposal_created` event is an error, not a default.
 */
export function parseProposeOutcome(outcome: unknown, gateAccount: string): AcceptedProposal {
  const result = object(outcome);
  const status = object(result.status);
  if ("Failure" in status) {
    throw new GateWriteError(
      "gate_refused",
      panicCode(status.Failure) ?? "Gate refused the proposal",
    );
  }
  if (typeof status.SuccessValue !== "string") {
    throw new GateWriteError(
      "other",
      "NEAR has not reported successful final execution; inspect the account before trying another proposal",
    );
  }
  const hash = object(result.transaction).hash;
  if (typeof hash !== "string" || !/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(hash)) {
    throw new GateWriteError(
      "other",
      "NEAR returned no valid transaction hash; inspect the account before trying another proposal",
    );
  }
  let proposalId: number | null = null;
  if (status.SuccessValue !== "") {
    const returned = new TextDecoder("utf-8", { fatal: true }).decode(
      base64ToBytes(status.SuccessValue, "propose result"),
    );
    if (!/^(0|[1-9]\d*)$/.test(returned) || id(Number(returned)) === null) {
      throw new GateWriteError(
        "other",
        "NEAR proposal id exceeds the page's supported integer range",
      );
    }
    proposalId = Number(returned);
  }
  const receipts = Array.isArray(result.receipts_outcome) ? result.receipts_outcome : [];
  for (const receipt of receipts) {
    const outcomeBody = object(object(receipt).outcome);
    if (outcomeBody.executor_id !== gateAccount || !Array.isArray(outcomeBody.logs)) continue;
    for (const line of outcomeBody.logs) {
      if (typeof line !== "string" || !line.startsWith("EVENT_JSON:")) continue;
      const event = object(JSON.parse(line.slice(11)));
      if (
        event.standard !== "constitution" ||
        event.version !== "1.0.0" ||
        event.event !== "proposal_created" ||
        !Array.isArray(event.data)
      )
        continue;
      for (const data of event.data) {
        const eventId = id(object(data).proposal_id);
        if (eventId === null || (proposalId !== null && proposalId !== eventId)) {
          throw new GateWriteError("other", "NEAR proposal return and event identifiers disagree");
        }
        proposalId = eventId;
      }
    }
  }
  if (proposalId === null)
    throw new GateWriteError(
      "other",
      "NEAR returned no accepted proposal id; inspect the account before trying another proposal",
    );
  return { proposalId, nearTxHash: hash };
}

/** Maps any thrown value to a `GateWriteError` with a bounded, secret-free message. */
export function classifyWriteError(error: unknown): GateWriteError {
  if (error instanceof GateWriteError) return error;
  const code = panicCode(error);
  if (code !== null) return new GateWriteError("gate_refused", code);
  return (
    sessionKeyFailure(error) ??
    new GateWriteError(
      "other",
      "NEAR proposal submission failed; inspect the account before trying another proposal",
    )
  );
}

export async function proposeWithSessionKey(
  config: SessionSignerConfig,
  args: ProposeArgs,
  send?: (request: ProposeRequest) => Promise<unknown>,
): Promise<AcceptedProposal> {
  const request: ProposeRequest = {
    receiverId: config.gateAccount,
    methodName: "propose",
    args: { args },
    gas: PROPOSE_GAS,
    deposit: 0n,
  };
  try {
    if (send === undefined) {
      // Loaded only at the click boundary: the SDK is large and nothing renders without a click.
      const { Account, JsonRpcProvider, actions } = await import("near-api-js");
      const account = new Account(
        config.accountId,
        new JsonRpcProvider({ url: config.nearRpc }),
        config.secretKey as `ed25519:${string}`,
      );
      send = async (call) =>
        account.signAndSendTransaction({
          receiverId: call.receiverId,
          actions: [actions.functionCall(call.methodName, call.args, call.gas, call.deposit)],
          waitUntil: "FINAL",
          throwOnFailure: false,
          retries: 0,
        });
    }
    return parseProposeOutcome(await send(request), config.gateAccount);
  } catch (error) {
    throw classifyWriteError(error);
  }
}
