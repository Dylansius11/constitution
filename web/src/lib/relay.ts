import type { Config, EvmTx } from "./gate-types.ts";

export type RelayConfig = Pick<Config, "chain_id" | "executor" | "derived_eoa">;
export type RelayTx = Pick<EvmTx, "nonce" | "signing_hash" | "signed_tx">;
export interface RelayTransport {
  chainId(): Promise<number>;
  knows(hash: string): Promise<boolean>;
  send(raw: string): Promise<string>;
}

/** The gate already assembled the signed raw bytes; reproduce CLI signed_tx::verify checks. */
export async function verifyRelay(
  config: RelayConfig,
  evm: RelayTx,
): Promise<{ raw: `0x${string}`; hash: `0x${string}` }> {
  // Click-only crypto: this module is also safe to import without loading viem.
  const { parseTransaction, serializeTransaction, recoverTransactionAddress, keccak256 } =
    await import("viem");
  const raw = evm.signed_tx;
  if (raw === null || !/^0x02(?:[a-f0-9]{2})+$/.test(raw))
    throw new Error("No signed EIP-1559 transaction");
  const serialized = raw as `0x02${string}`;
  const tx = parseTransaction(serialized);
  if (tx.type !== "eip1559" || tx.r === undefined || tx.s === undefined || tx.yParity === undefined)
    throw new Error("Malformed signed transaction");
  if (BigInt(tx.s) > 0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0n)
    throw new Error("Non-canonical high-s signature");
  if (serializeTransaction(tx) !== raw)
    throw new Error("Non-canonical or trailing transaction bytes");
  const { r: _r, s: _s, v: _v, yParity: _parity, ...unsigned } = tx;
  if (keccak256(serializeTransaction(unsigned)) !== evm.signing_hash)
    throw new Error("Signing hash differs from the gate");
  if (
    tx.chainId !== config.chain_id ||
    tx.to?.toLowerCase() !== config.executor ||
    tx.nonce !== evm.nonce
  )
    throw new Error("Transaction chain, executor or nonce differs from the gate");
  const sender = await recoverTransactionAddress({ serializedTransaction: serialized });
  if (sender.toLowerCase() !== config.derived_eoa)
    throw new Error("Recovered sender is not the derived EOA");
  return { raw: serialized, hash: keccak256(serialized) };
}

export async function relaySignedTransaction(
  rpcUrl: string,
  config: RelayConfig,
  evm: RelayTx,
  transport?: RelayTransport,
): Promise<string> {
  const { raw, hash } = await verifyRelay(config, evm);
  if (transport === undefined) {
    // Network and crypto stay off the eager page graph; the flow invokes this only after Signed.
    const { createPublicClient, http, TransactionNotFoundError } = await import("viem");
    const client = createPublicClient({
      transport: http(rpcUrl, { retryCount: 0, timeout: 10_000 }),
    });
    transport = {
      chainId: () => client.getChainId(),
      knows: async (value) => {
        try {
          await client.getTransaction({ hash: value as `0x${string}` });
          return true;
        } catch (error) {
          if (error instanceof TransactionNotFoundError) return false;
          throw error;
        }
      },
      send: (value) => client.sendRawTransaction({ serializedTransaction: value as `0x${string}` }),
    };
  }
  if ((await transport.chainId()) !== config.chain_id)
    throw new Error("Base RPC serves a different chain");
  if (await transport.knows(hash)) return hash;
  try {
    if ((await transport.send(raw)) !== hash)
      throw new Error("RPC returned a different transaction hash");
  } catch {
    // Including nonce-too-low/already-known: only the SAME hash observed on chain is success.
    if (!(await transport.knows(hash)))
      throw new Error(
        "Signed transaction broadcast failed; inspect the gate and relay with the CLI",
      );
  }
  return hash;
}
