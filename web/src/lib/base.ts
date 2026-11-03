import {
  createPublicClient,
  http,
  TransactionNotFoundError,
  TransactionReceiptNotFoundError,
  type Transport,
} from "viem";
import { baseSepolia } from "viem/chains";
import { InputError, NetworkError, RpcError, WireError } from "./errors.ts";
import { asHexFixed } from "./wire.ts";

/** `bytes32(uint256(keccak256("eip1967.proxy.implementation")) - 1)`, same as crates/cli audit.rs. */
export const ERC1967_IMPLEMENTATION_SLOT =
  "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";

export const BASE_SEPOLIA_CHAIN_ID = 84532;

export interface BaseClientOptions {
  /** Read endpoint, for example `https://sepolia.base.org`. Ignored when `transport` is given. */
  rpcUrl?: string;
  /** Injected for tests; a viem transport such as `custom({ request })`. */
  transport?: Transport;
  /** The chain the gate is configured for (`get_config().chain_id`). Reads fail on any other chain. */
  expectedChainId?: number;
  timeoutMs?: number;
}

/** One observed read of the ERC-1967 implementation slot. */
export interface ProxyRead {
  /** Lowercase `0x` address held in the slot. */
  implementation: string;
  /** The block the slot was read at. */
  block: number;
  block_hash: string;
}

/** The page's reads of the slot: one before the action and one after. A missing read is not observed. */
export interface ProxyReads {
  before?: ProxyRead;
  after?: ProxyRead;
}

export type TxObservation =
  | { status: "not_found" }
  | { status: "pending" }
  | { status: "included" | "reverted"; block: number; block_hash: string };

export interface BaseClient {
  /** Reads the implementation slot of `proxy` at the latest block, pinned to that block number. */
  readImplementation(proxy: string): Promise<ProxyRead>;
  /** Looks a transaction up by hash: not found, pending, or included with its execution status. */
  readTransaction(hash: string): Promise<TxObservation>;
}

function safeBlock(value: bigint | null | undefined, what: string): number {
  if (
    value === null ||
    value === undefined ||
    value < 0n ||
    value > BigInt(Number.MAX_SAFE_INTEGER)
  ) {
    throw new WireError(what, "expected a block number");
  }
  return Number(value);
}

export function createBaseClient(options: BaseClientOptions): BaseClient {
  if (options.transport === undefined && options.rpcUrl === undefined) {
    throw new InputError("createBaseClient needs rpcUrl or transport");
  }
  const expectedChainId = options.expectedChainId ?? BASE_SEPOLIA_CHAIN_ID;
  const client = createPublicClient({
    chain: baseSepolia,
    transport:
      options.transport ??
      http(options.rpcUrl, { timeout: options.timeoutMs ?? 10_000, retryCount: 0 }),
  });
  let chainVerified = false;

  async function verifyChain(): Promise<void> {
    if (chainVerified) return;
    const chainId = await client.getChainId();
    if (chainId !== expectedChainId) {
      throw new RpcError(
        `EVM RPC is chain ${chainId}, the gate is configured for ${expectedChainId}`,
      );
    }
    chainVerified = true;
  }

  /** Transport and RPC failures are `NetworkError`; domain errors pass through unchanged. */
  async function guarded<T>(run: () => Promise<T>): Promise<T> {
    try {
      await verifyChain();
      return await run();
    } catch (cause) {
      if (cause instanceof WireError || cause instanceof RpcError || cause instanceof InputError)
        throw cause;
      throw new NetworkError("Base Sepolia RPC is unreachable", { cause });
    }
  }

  return {
    async readImplementation(proxy) {
      const address = asHexFixed(proxy, 20, "proxy") as `0x${string}`;
      return guarded(async () => {
        const block = await client.getBlock({ blockTag: "latest" });
        const number = safeBlock(block.number, "block.number");
        const slot = await client.getStorageAt({
          address,
          slot: ERC1967_IMPLEMENTATION_SLOT,
          blockNumber: block.number,
        });
        const word = asHexFixed(slot, 32, "implementation slot");
        // An address occupies the low 20 bytes; anything above is not an address slot.
        if (!/^0x0{24}/.test(word))
          throw new WireError("implementation slot", "high 12 bytes are not zero");
        return { implementation: `0x${word.slice(26)}`, block: number, block_hash: block.hash };
      });
    },

    async readTransaction(hash) {
      const txHash = asHexFixed(hash, 32, "transaction hash") as `0x${string}`;
      return guarded(async () => {
        try {
          const receipt = await client.getTransactionReceipt({ hash: txHash });
          return {
            status: receipt.status === "success" ? "included" : "reverted",
            block: safeBlock(receipt.blockNumber, "receipt.blockNumber"),
            block_hash: receipt.blockHash,
          } as const;
        } catch (error) {
          if (!(error instanceof TransactionReceiptNotFoundError)) throw error;
        }
        try {
          await client.getTransaction({ hash: txHash });
          return { status: "pending" } as const;
        } catch (error) {
          if (error instanceof TransactionNotFoundError) return { status: "not_found" } as const;
          throw error;
        }
      });
    },
  };
}
