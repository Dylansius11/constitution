import { custom } from "viem";
import { describe, expect, it } from "vitest";
import { createBaseClient, ERC1967_IMPLEMENTATION_SLOT } from "../src/lib/base.ts";
import { InputError, NetworkError, RpcError, WireError } from "../src/lib/errors.ts";
import { hex, PROXY } from "./support/fixtures.ts";

type Handler = (params: unknown[]) => unknown;

/** A viem EIP-1193 transport answering from a method table; unknown methods fail loudly. */
function chain(methods: Record<string, Handler>) {
  const log: { method: string; params: unknown[] }[] = [];
  const transport = custom({
    async request({ method, params }: { method: string; params?: unknown }) {
      const list = Array.isArray(params) ? params : [];
      log.push({ method, params: list });
      const handler = methods[method];
      if (handler === undefined) throw new Error(`unexpected RPC method ${method}`);
      return handler(list);
    },
  });
  return { transport, log };
}

const BLOCK_HASH = hex(0xb0, 32);

function block(number: number) {
  return {
    number: `0x${number.toString(16)}`,
    hash: BLOCK_HASH,
    parentHash: hex(0x00, 32),
    timestamp: "0x65",
    nonce: "0x0000000000000000",
    difficulty: "0x0",
    totalDifficulty: "0x0",
    gasLimit: "0x1c9c380",
    gasUsed: "0x0",
    miner: hex(0x00, 20),
    extraData: "0x",
    logsBloom: `0x${"00".repeat(256)}`,
    mixHash: hex(0x00, 32),
    receiptsRoot: hex(0x00, 32),
    sha3Uncles: hex(0x00, 32),
    stateRoot: hex(0x00, 32),
    transactionsRoot: hex(0x00, 32),
    size: "0x1",
    baseFeePerGas: "0x1",
    transactions: [],
    uncles: [],
  };
}

const TX_HASH = hex(0xf0, 32);

function receipt(status: "0x1" | "0x0") {
  return {
    transactionHash: TX_HASH,
    transactionIndex: "0x0",
    blockHash: BLOCK_HASH,
    blockNumber: "0x2a",
    from: hex(0xe0, 20),
    to: hex(0xee, 20),
    cumulativeGasUsed: "0x5208",
    gasUsed: "0x5208",
    effectiveGasPrice: "0x1",
    contractAddress: null,
    logs: [],
    logsBloom: `0x${"00".repeat(256)}`,
    status,
    type: "0x2",
  };
}

function pendingTransaction() {
  return {
    hash: TX_HASH,
    nonce: "0x3",
    blockHash: null,
    blockNumber: null,
    transactionIndex: null,
    from: hex(0xe0, 20),
    to: hex(0xee, 20),
    value: "0x0",
    gas: "0x493e0",
    gasPrice: "0x1",
    maxFeePerGas: "0x1",
    maxPriorityFeePerGas: "0x1",
    input: "0x",
    type: "0x2",
    chainId: "0x14a34",
    accessList: [],
    v: "0x0",
    r: hex(0x01, 32),
    s: hex(0x02, 32),
  };
}

const IMPL_WORD = `0x${"00".repeat(12)}${"4a".repeat(20)}`;

function baseline(extra: Record<string, Handler> = {}) {
  return chain({
    eth_chainId: () => "0x14a34",
    eth_getBlockByNumber: () => block(100),
    eth_getStorageAt: () => IMPL_WORD,
    ...extra,
  });
}

describe("Base Sepolia reads", () => {
  it("reads the ERC-1967 slot pinned to the block it reports", async () => {
    const { transport, log } = baseline();
    const read = await createBaseClient({ transport }).readImplementation(PROXY);
    expect(read).toEqual({ implementation: hex(0x4a, 20), block: 100, block_hash: BLOCK_HASH });
    const storage = log.find((l) => l.method === "eth_getStorageAt");
    expect(storage?.params).toEqual([PROXY, ERC1967_IMPLEMENTATION_SLOT, "0x64"]);
  });

  it("refuses a slot whose high 12 bytes are not zero", async () => {
    const { transport } = baseline({
      eth_getStorageAt: () => `0x01${"00".repeat(11)}${"4a".repeat(20)}`,
    });
    await expect(createBaseClient({ transport }).readImplementation(PROXY)).rejects.toThrow(
      WireError,
    );
  });

  it("refuses a malformed slot word and a malformed proxy address", async () => {
    const { transport } = baseline({ eth_getStorageAt: () => "0x1234" });
    const client = createBaseClient({ transport });
    await expect(client.readImplementation(PROXY)).rejects.toThrow(WireError);
    await expect(client.readImplementation("0xABC")).rejects.toThrow(WireError);
  });

  it("refuses an RPC on a different chain than the gate's", async () => {
    const { transport } = baseline({ eth_chainId: () => "0x1" });
    await expect(createBaseClient({ transport }).readImplementation(PROXY)).rejects.toThrow(
      RpcError,
    );
    const ok = baseline();
    await expect(
      createBaseClient({ transport: ok.transport, expectedChainId: 1 }).readImplementation(PROXY),
    ).rejects.toThrow(/84532/);
  });

  it("reports transport failures as NetworkError", async () => {
    const { transport } = chain({
      eth_chainId: () => {
        throw new Error("socket hang up");
      },
    });
    await expect(createBaseClient({ transport }).readImplementation(PROXY)).rejects.toThrow(
      NetworkError,
    );
  });

  it("needs a transport or a URL", () => {
    expect(() => createBaseClient({})).toThrow(InputError);
  });

  it("transaction lookup: included, reverted, pending and not found", async () => {
    const included = baseline({ eth_getTransactionReceipt: () => receipt("0x1") });
    expect(
      await createBaseClient({ transport: included.transport }).readTransaction(TX_HASH),
    ).toEqual({
      status: "included",
      block: 42,
      block_hash: BLOCK_HASH,
    });
    const reverted = baseline({ eth_getTransactionReceipt: () => receipt("0x0") });
    expect(
      await createBaseClient({ transport: reverted.transport }).readTransaction(TX_HASH),
    ).toMatchObject({
      status: "reverted",
      block: 42,
    });
    const pending = baseline({
      eth_getTransactionReceipt: () => null,
      eth_getTransactionByHash: () => pendingTransaction(),
    });
    expect(
      await createBaseClient({ transport: pending.transport }).readTransaction(TX_HASH),
    ).toEqual({ status: "pending" });
    const absent = baseline({
      eth_getTransactionReceipt: () => null,
      eth_getTransactionByHash: () => null,
    });
    expect(
      await createBaseClient({ transport: absent.transport }).readTransaction(TX_HASH),
    ).toEqual({ status: "not_found" });
  });

  it("transaction lookup: a malformed hash never reaches the network", async () => {
    const { transport, log } = baseline();
    const client = createBaseClient({ transport });
    for (const bad of ["0x", "0xABC", TX_HASH.toUpperCase().replace("0X", "0x"), `${TX_HASH}00`]) {
      await expect(client.readTransaction(bad), bad).rejects.toThrow(WireError);
    }
    expect(log).toHaveLength(0);
  });
});
