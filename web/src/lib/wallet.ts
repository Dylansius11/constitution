import type { WalletTx } from "./wallet-actions.ts";

/**
 * The page's wallet boundary. `WalletPort` is wallet-agnostic: the NEAR Connect adapter below is
 * the only implementation, and tests inject their own. The page never trusts a wallet's reply as
 * proof of state; after every confirmation it re-reads the gate and the account.
 */

/** The visitor closed or declined the wallet's confirmation. Nothing was sent. */
export class WalletRejectedError extends Error {
  constructor() {
    super("The wallet confirmation was cancelled.");
    this.name = "WalletRejectedError";
  }
}

/** The wallet failed for a reason other than the visitor declining. The state is unknown until re-read. */
export class WalletError extends Error {
  constructor() {
    super("The wallet reported an error.");
    this.name = "WalletError";
  }
}

export interface WalletPort {
  /** The account a previous visit left connected, or `null`. */
  restore(): Promise<string | null>;
  /** Opens the wallet picker and resolves the chosen account id. */
  connect(): Promise<string>;
  disconnect(): Promise<void>;
  /** One confirmation for the whole list when the wallet supports batches. Returns final outcomes. */
  send(transactions: readonly WalletTx[]): Promise<unknown[]>;
  /** Called when the wallet signs the visitor out from its own side. */
  onSignOut(listener: () => void): void;
}

/** Wallet errors carry free text; only a decline is told apart, by the words wallets use for it. */
export function classifyWalletError(error: unknown): WalletRejectedError | WalletError {
  const text = error instanceof Error ? error.message : typeof error === "string" ? error : "";
  return /reject|declin|denied|cancel|closed|dismiss|abort/i.test(text)
    ? new WalletRejectedError()
    : new WalletError();
}

/**
 * NEAR Connect (`@hot-labs/near-connect`, testnet): a sandboxed wallet picker that lists the
 * wallets (Meteor, Intear, Nightly and others) that declare testnet and transaction support in
 * the connector's remote manifest. Loaded on the first wallet click, never on page load.
 */
export async function createNearConnectWallet(): Promise<WalletPort> {
  // Loaded at the first wallet click only: the connector touches window and IndexedDB.
  const { NearConnector } = await import("@hot-labs/near-connect");
  const connector = new NearConnector({
    network: "testnet",
    features: { testnet: true, signAndSendTransaction: true },
  });
  const signOutListeners: Array<() => void> = [];
  connector.on("wallet:signOut", () => {
    for (const listener of signOutListeners) listener();
  });

  return {
    async restore() {
      await connector.whenManifestLoaded;
      try {
        const connected = await connector.getConnectedWallet();
        return connected.accounts[0]?.accountId ?? null;
      } catch {
        return null;
      }
    },
    async connect() {
      await connector.whenManifestLoaded;
      try {
        const wallet = await connector.connect();
        const accounts = await wallet.getAccounts({ network: "testnet" });
        const account = accounts[0]?.accountId;
        if (account === undefined) throw new WalletError();
        return account;
      } catch (error) {
        throw error instanceof WalletError ? error : classifyWalletError(error);
      }
    },
    async disconnect() {
      try {
        await connector.disconnect();
      } catch {
        // The wallet may already have signed out; the page forgets the account either way.
      }
    },
    async send(transactions) {
      try {
        const wallet = await connector.wallet();
        if (transactions.length === 1) {
          const [only] = transactions;
          if (only === undefined) return [];
          return [await wallet.signAndSendTransaction({ network: "testnet", ...only })];
        }
        if (wallet.manifest.features.signAndSendTransactions)
          return await wallet.signAndSendTransactions({
            network: "testnet",
            transactions: [...transactions],
          });
        const outcomes: unknown[] = [];
        for (const tx of transactions)
          outcomes.push(await wallet.signAndSendTransaction({ network: "testnet", ...tx }));
        return outcomes;
      } catch (error) {
        throw classifyWalletError(error);
      }
    },
    onSignOut(listener) {
      signOutListeners.push(listener);
    },
  };
}
