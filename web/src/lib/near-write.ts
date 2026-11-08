import { base64ToBytes } from "./bytes.ts";
import type { SurfaceErrorCause } from "./state.ts";

export interface DemoSignerConfig {
  nearRpc: string;
  gateAccount: string;
  demoAccountId: string;
  /** Intentionally public D17 restricted testnet key. Never include it in errors or logs. */
  demoSecretKey: string;
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

export interface ProposeRequest {
  receiverId: string;
  methodName: "propose";
  args: { args: ProposeArgs };
  gas: bigint;
  deposit: bigint;
}

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

export async function proposeWithDemoKey(
  config: DemoSignerConfig,
  args: ProposeArgs,
  send?: (request: ProposeRequest) => Promise<unknown>,
): Promise<AcceptedProposal> {
  const request: ProposeRequest = {
    receiverId: config.gateAccount,
    methodName: "propose",
    args: { args },
    gas: 30_000_000_000_000n,
    deposit: 0n,
  };
  try {
    if (send === undefined) {
      const { Account, JsonRpcProvider, actions } = await import("near-api-js");
      const account = new Account(
        config.demoAccountId,
        new JsonRpcProvider({ url: config.nearRpc }),
        config.demoSecretKey as `ed25519:${string}`,
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
    const result = object(await send(request));
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
    if (typeof status.SuccessValue === "string" && status.SuccessValue !== "") {
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
      const outcome = object(object(receipt).outcome);
      if (outcome.executor_id !== config.gateAccount || !Array.isArray(outcome.logs)) continue;
      for (const line of outcome.logs) {
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
            throw new GateWriteError(
              "other",
              "NEAR proposal return and event identifiers disagree",
            );
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
  } catch (error) {
    if (error instanceof GateWriteError) throw error;
    const code = panicCode(error);
    if (code !== null) throw new GateWriteError("gate_refused", code);
    throw new GateWriteError(
      "other",
      "NEAR proposal submission failed; inspect the account before trying another proposal",
    );
  }
}
