import { describe, expect, it } from "vitest";
import { HttpStatusError, RpcError, WireError } from "../src/lib/errors.ts";
import { accessKeyState } from "../src/lib/display.ts";
import { createGateClient } from "../src/lib/gate.ts";
import { parseAccessKeyList } from "../src/lib/gate-types.ts";

const GATE = "constitution-gate.testnet";
const RPC = "https://rpc.example/";
const KEY = "ed25519:34d3Rpzzp4MGSVKY3DchtL6iHS9nUfimgjc8sDyaNuJX";
const FULL = { nonce: 7, permission: "FullAccess" };

interface Call {
  body: { method: string; params: Record<string, unknown> };
}

/** Fake NEAR RPC: answers a `view_access_key_list` query with `result` as the JSON-RPC result. */
function rpc(result: unknown) {
  const calls: Call[] = [];
  const fetchImpl: typeof fetch = async (_input, init) => {
    calls.push({ body: JSON.parse(String(init?.body)) });
    return new Response(JSON.stringify({ jsonrpc: "2.0", id: "constitution", result }));
  };
  return { fetch: fetchImpl, calls };
}

function client(fetchImpl: typeof fetch) {
  return createGateClient({ rpcUrl: RPC, gate: GATE, fetch: fetchImpl });
}

const listing = (keys: unknown[]) => ({
  keys,
  block_height: 321,
  block_hash: "Hq5vCnuBjvVPWLMMBHQBN9CnKbmGpGqPYTcVrYo7jCEV",
});

describe("gate access key count", () => {
  it("asks for the final access key list of the gate account", async () => {
    const fake = rpc(listing([]));
    await client(fake.fetch).getAccessKeyCount();
    expect(fake.calls).toHaveLength(1);
    expect(fake.calls[0]?.body).toMatchObject({
      method: "query",
      params: { request_type: "view_access_key_list", finality: "final", account_id: GATE },
    });
  });

  it("reads zero keys as zero, with the block it was read at", async () => {
    const result = await client(rpc(listing([])).fetch).getAccessKeyCount();
    expect(result.value).toBe(0);
    expect(result.blockHeight).toBe(321);
    expect(result.blockHash).toBe("Hq5vCnuBjvVPWLMMBHQBN9CnKbmGpGqPYTcVrYo7jCEV");
  });

  it("counts one full-access key and keeps no key material", async () => {
    const result = await client(rpc(listing([{ public_key: KEY, access_key: FULL }])).fetch)
      .getAccessKeyCount();
    expect(result.value).toBe(1);
    expect(JSON.stringify(result)).not.toContain(KEY);
  });

  it("counts function-call keys too", async () => {
    const call = {
      nonce: 0,
      permission: { FunctionCall: { allowance: "1", receiver_id: GATE, method_names: ["propose"] } },
    };
    const keys = [
      { public_key: KEY, access_key: FULL },
      { public_key: "ed25519:11111111111111111111111111111111", access_key: call },
    ];
    expect((await client(rpc(listing(keys)).fetch).getAccessKeyCount()).value).toBe(2);
  });

  it("rejects a malformed response instead of reporting a count", () => {
    for (const bad of [
      {},
      { keys: "none" },
      { keys: null },
      { keys: [null] },
      { keys: ["ed25519:abc"] },
      { keys: [{ access_key: FULL }] },
      { keys: [{ public_key: 5, access_key: FULL }] },
      { keys: [{ public_key: KEY }] },
      [],
    ]) {
      expect(() => parseAccessKeyList(bad), JSON.stringify(bad)).toThrow(WireError);
    }
  });

  it("rejects a malformed RPC envelope and maps transport and account errors", async () => {
    await expect(client(rpc({ keys: "none" }).fetch).getAccessKeyCount()).rejects.toThrow(
      WireError,
    );
    await expect(
      client(rpc({ keys: [], block_height: "x", block_hash: "h" }).fetch).getAccessKeyCount(),
    ).rejects.toThrow(WireError);
    const unknown: typeof fetch = async () =>
      new Response(
        JSON.stringify({
          error: { name: "HANDLER_ERROR", cause: { name: "UNKNOWN_ACCOUNT" }, message: "x" },
        }),
      );
    await expect(client(unknown).getAccessKeyCount()).rejects.toThrow(RpcError);
    const gateway: typeof fetch = async () => new Response("bad gateway", { status: 502 });
    await expect(client(gateway).getAccessKeyCount()).rejects.toThrow(HttpStatusError);
  });
});

describe("gate access key state copy", () => {
  it("zero keys: nobody can redeploy or reconfigure the gate", () => {
    expect(accessKeyState(0)).toEqual({
      tone: "pass",
      text: "No access keys: nobody can redeploy or reconfigure this gate",
    });
  });

  it("one or more keys: a warning that names the count and says not keyless", () => {
    expect(accessKeyState(1)).toEqual({
      tone: "warning",
      text: "1 access key(s) still present: this gate can still be redeployed. Not keyless yet.",
    });
    expect(accessKeyState(3).text).toMatch(/^3 access key\(s\) still present/);
  });
});
