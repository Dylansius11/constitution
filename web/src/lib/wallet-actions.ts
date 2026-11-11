import { PROPOSE_GAS, type ProposeArgs } from "./near-write.ts";
import { SESSION_ALLOWANCE_YOCTO, SESSION_METHODS } from "./session-key.ts";

/**
 * Transactions the visitor's wallet is asked to confirm, in the `ConnectorAction` shape of
 * `@hot-labs/near-connect` (gas and deposit are decimal strings). Nothing here signs or sends.
 */

export type WalletAction =
  | {
      type: "FunctionCall";
      params: { methodName: string; args: object; gas: string; deposit: string };
    }
  | {
      type: "AddKey";
      params: {
        publicKey: string;
        accessKey: {
          permission: { receiverId: string; allowance: string; methodNames: string[] };
        };
      };
    }
  | { type: "DeleteKey"; params: { publicKey: string } };

export interface WalletTx {
  receiverId: string;
  actions: WalletAction[];
}

const CALL_GAS = "30000000000000";
const WITHDRAW_GAS = "50000000000000";

/** `deposit_credit` for the caller's own account: the demo gate holds it until it is withdrawn. */
export function depositCreditTx(gate: string, amountYocto: bigint): WalletTx {
  return {
    receiverId: gate,
    actions: [
      {
        type: "FunctionCall",
        params: {
          methodName: "deposit_credit",
          args: {},
          gas: CALL_GAS,
          deposit: amountYocto.toString(),
        },
      },
    ],
  };
}

export function withdrawCreditTx(gate: string, amountYocto: bigint): WalletTx {
  return {
    receiverId: gate,
    actions: [
      {
        type: "FunctionCall",
        params: {
          methodName: "withdraw_credit",
          args: { amount: amountYocto.toString() },
          gas: WITHDRAW_GAS,
          deposit: "0",
        },
      },
    ],
  };
}

/** A function-call key limited to `propose` on the demo gate. Added to the visitor's own account. */
export function addSessionKeyTx(
  accountId: string,
  gate: string,
  publicKey: string,
  allowanceYocto: bigint = SESSION_ALLOWANCE_YOCTO,
): WalletTx {
  return {
    receiverId: accountId,
    actions: [
      {
        type: "AddKey",
        params: {
          publicKey,
          accessKey: {
            permission: {
              receiverId: gate,
              allowance: allowanceYocto.toString(),
              methodNames: [...SESSION_METHODS],
            },
          },
        },
      },
    ],
  };
}

export function removeSessionKeyTx(accountId: string, publicKey: string): WalletTx {
  return { receiverId: accountId, actions: [{ type: "DeleteKey", params: { publicKey } }] };
}

/** `expire` releases the reservation of a proposal whose deadline passed; any account may call it. */
export function expireProposalTx(gate: string, proposalId: number): WalletTx {
  return {
    receiverId: gate,
    actions: [
      {
        type: "FunctionCall",
        params: {
          methodName: "expire",
          args: { proposal_id: proposalId },
          gas: CALL_GAS,
          deposit: "0",
        },
      },
    ],
  };
}

/** The fallback when the wallet will not add a session key: one confirmation per `propose`. */
export function proposeTx(gate: string, args: ProposeArgs): WalletTx {
  return {
    receiverId: gate,
    actions: [
      {
        type: "FunctionCall",
        params: {
          methodName: "propose",
          args: { args },
          gas: PROPOSE_GAS.toString(),
          deposit: "0",
        },
      },
    ],
  };
}
