import { describe, expect, it } from "vitest";
import {
  HttpStatusError,
  InputError,
  NetworkError,
  RpcError,
  WireError,
} from "../src/lib/errors.ts";
import { createGateClient } from "../src/lib/gate.ts";
import { hex } from "./support/fixtures.ts";

const GATE = "gate.constitution.testnet";
const RPC = "https://rpc.example/";

interface Call {
  url: string;
  body: {
    method: string;
    params: {
      request_type: string;
      finality: string;
      account_id: string;
      method_name: string;
      args_base64: string;
    };
  };
}

/** Fake NEAR RPC: answers `call_function` with `jsonText` encoded as the result byte array. */
function rpc(
  jsonText: string,
  block = { height: 123, hash: "Hq5vCnuBjvVPWLMMBHQBN9CnKbmGpGqPYTcVrYo7jCEV" },
) {
  const calls: Call[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    calls.push({ url: String(input), body: JSON.parse(String(init?.body)) });
    const result = Array.from(new TextEncoder().encode(jsonText));
    return new Response(
      JSON.stringify({
        jsonrpc: "2.0",
        id: "constitution",
        result: { result, logs: [], block_height: block.height, block_hash: block.hash },
      }),
      { status: 200 },
    );
  };
  return { fetch: fetchImpl, calls };
}

function client(fetchImpl: typeof fetch) {
  return createGateClient({ rpcUrl: RPC, gate: GATE, fetch: fetchImpl });
}

function decodeArgs(call: Call | undefined): unknown {
  if (call === undefined) throw new Error("no call was made");
  return JSON.parse(atob(call.body.params.args_base64));
}

// u64 nanosecond timestamps are written as bare JSON integers above 2^53, as NEAR serializes them.
const PROPOSAL_JSON = `{
  "id": 5, "proposer": "demo.testnet",
  "created_at_ns": 1793700000123456789, "expires_at_ns": 1793701800123456789,
  "rule_set_hash": "${hex(0xa1, 32)}", "new_impl": "${hex(0x4a, 20)}", "new_impl_codehash": "${hex(0xc0, 32)}",
  "expected_current_impl": "${hex(0x91, 20)}", "init_calldata": "0x5b4e128c", "source_commitment": "${hex(0xd0, 32)}",
  "reserved": "1000000000000000000000000", "state": "Signed", "certificate_hash": "${hex(0xce, 32)}",
  "rejection": null,
  "evm": {"nonce": 3, "deadline_s": 1793701800, "gas_limit": 300000, "max_fee_per_gas": "1009731712",
    "max_priority_fee_per_gas": "1000000", "signing_hash": "${hex(0x5e, 32)}", "signed_tx": "0x02ff", "attempts": 2}
}`;

const PARAMS_JSON = `{"proposal_deposit":"1000000000000000000000000","proposal_ttl_s":1800,"max_cert_ttl_s":3600,"sign_validity_s":1800,"gas_limit":300000,"max_fee_per_gas":"2000000000","max_priority_fee_per_gas":"1000000","fee_cap":"4000000000","sign_gas_tgas":50,"callback_gas_tgas":30}`;

describe("gate view client", () => {
  it("get_proposal: sends the documented call and parses u64 nanoseconds exactly", async () => {
    const fake = rpc(PROPOSAL_JSON);
    const result = await client(fake.fetch).getProposal(5);
    expect(fake.calls).toHaveLength(1);
    const call = fake.calls[0];
    expect(call?.url).toBe(RPC);
    expect(call?.body).toMatchObject({
      method: "query",
      params: {
        request_type: "call_function",
        finality: "final",
        account_id: GATE,
        method_name: "get_proposal",
      },
    });
    expect(decodeArgs(call)).toEqual({ id: 5 });
    expect(result.blockHeight).toBe(123);
    expect(result.blockHash).toBe("Hq5vCnuBjvVPWLMMBHQBN9CnKbmGpGqPYTcVrYo7jCEV");
    const p = result.value;
    expect(p?.created_at_ns).toBe(1_793_700_000_123_456_789n);
    expect(p?.expires_at_ns).toBe(1_793_701_800_123_456_789n);
    expect(p?.reserved).toBe(1_000_000_000_000_000_000_000_000n);
    expect(p?.state).toBe("Signed");
    expect(p?.evm?.max_fee_per_gas).toBe(1_009_731_712n);
    expect(p?.evm?.signed_tx).toBe("0x02ff");
    expect(p?.rejection).toBeNull();
  });

  it("get_proposal: null means the gate has no such proposal", async () => {
    const result = await client(rpc("null").fetch).getProposal(99);
    expect(result.value).toBeNull();
  });

  it("honours optimistic finality when configured", async () => {
    const fake = rpc("null");
    await createGateClient({
      rpcUrl: RPC,
      gate: GATE,
      finality: "optimistic",
      fetch: fake.fetch,
    }).getProposal(1);
    expect(fake.calls[0]?.body.params.finality).toBe("optimistic");
  });

  it("list_proposals: arguments and elements", async () => {
    const fake = rpc(`[${PROPOSAL_JSON}]`);
    const result = await client(fake.fetch).listProposals(4, 10);
    expect(decodeArgs(fake.calls[0])).toEqual({ from_id: 4, limit: 10 });
    expect(result.value.map((p) => p.id)).toEqual([5]);
  });

  it("get_config and get_constitution", async () => {
    const config = `{"chain_id":84532,"executor":"${hex(0xee, 20)}","proxy":"${hex(0x11, 20)}","proxy_admin":"${hex(0xad, 20)}","derived_eoa":"${hex(0xe0, 20)}","mpc_account":"v1.signer-prod.testnet","mpc_path":"constitution/base-sepolia/v1","mpc_domain_id":0}`;
    const c = await client(rpc(config).fetch).getConfig();
    expect(c.value).toMatchObject({
      chain_id: 84532,
      mpc_account: "v1.signer-prod.testnet",
      mpc_domain_id: 0,
    });
    const constitution = `{"rule_set_hash":"${hex(0xa1, 32)}","manifest_uri":"https://example/manifest.json","ai_required":true,"params":${PARAMS_JSON}}`;
    const k = await client(rpc(constitution).fetch).getConstitution();
    expect(k.value.ai_required).toBe(true);
    expect(k.value.params.fee_cap).toBe(4_000_000_000n);
  });

  it("list_verifiers: tuples of base64 key and record", async () => {
    const key = btoa(String.fromCharCode(...new Uint8Array(32).fill(7)));
    const json = `[["${key}",{"release_hash":"${hex(0x77, 32)}","attestation_uri":"https://att","added_at_ns":1793600000000000001}]]`;
    const fake = rpc(json);
    const result = await client(fake.fetch).listVerifiers(0, 50);
    expect(decodeArgs(fake.calls[0])).toEqual({ from_index: 0, limit: 50 });
    expect(result.value).toEqual([
      {
        pubkey_b64: key,
        record: {
          release_hash: hex(0x77, 32),
          attestation_uri: "https://att",
          added_at_ns: 1_793_600_000_000_000_001n,
        },
      },
    ]);
  });

  it("get_change and list_changes: every change kind parses", async () => {
    const pk = btoa(String.fromCharCode(...new Uint8Array(32).fill(9)));
    const kinds = [
      `{"SetRuleSet":{"rule_set_hash":"${hex(0xa2, 32)}","manifest_uri":"u","ai_required":false}}`,
      `{"AddVerifier":{"pubkey":"${pk}","release_hash":"${hex(0x77, 32)}","attestation_uri":"u"}}`,
      `{"RemoveVerifier":{"pubkey":"${pk}"}}`,
      `{"SetParams":{"params":${PARAMS_JSON}}}`,
      `{"SetGovernor":{"account":"gov.testnet"}}`,
      `{"SetGuardian":{"account":"guard.testnet"}}`,
      `{"MigrateAuthority":{"new_signer":"${hex(0xcd, 20)}"}}`,
      `"Unpause"`,
    ];
    const changes = kinds.map(
      (kind, i) =>
        `{"id":${i},"kind":${kind},"scheduled_by":"gov.testnet","scheduled_at_ns":1793700000000000000,"eta_ns":1794304800000000000,"status":"Pending","evm":null}`,
    );
    const listed = await client(rpc(`[${changes.join(",")}]`).fetch).listChanges(0, 50);
    expect(listed.value.map((c) => c.kind.kind)).toEqual([
      "SetRuleSet",
      "AddVerifier",
      "RemoveVerifier",
      "SetParams",
      "SetGovernor",
      "SetGuardian",
      "MigrateAuthority",
      "Unpause",
    ]);
    expect(listed.value[0]?.eta_ns).toBe(1_794_304_800_000_000_000n);
    const one = await client(rpc(changes[7] ?? "").fetch).getChange(7);
    expect(one.value?.kind).toEqual({ kind: "Unpause" });
    expect((await client(rpc("null").fetch).getChange(8)).value).toBeNull();
  });

  it("refuses invalid arguments before any request is made", async () => {
    const fake = rpc("null");
    const gate = client(fake.fetch);
    for (const bad of [-1, 1.5, Number.NaN, 2 ** 53, Number.POSITIVE_INFINITY]) {
      await expect(gate.getProposal(bad), String(bad)).rejects.toThrow(InputError);
      await expect(gate.getChange(bad), String(bad)).rejects.toThrow(InputError);
      await expect(gate.listProposals(bad, 1)).rejects.toThrow(InputError);
    }
    for (const bad of [0, 51, -1, 2.5, Number.NaN]) {
      await expect(gate.listProposals(0, bad), String(bad)).rejects.toThrow(InputError);
      await expect(gate.listChanges(0, bad)).rejects.toThrow(InputError);
      await expect(gate.listVerifiers(0, bad)).rejects.toThrow(InputError);
    }
    expect(fake.calls).toHaveLength(0);
  });

  it("refuses a bad endpoint or gate account at construction", () => {
    const f = rpc("null").fetch;
    expect(() => createGateClient({ rpcUrl: "not a url", gate: GATE, fetch: f })).toThrow(
      InputError,
    );
    expect(() => createGateClient({ rpcUrl: "ftp://rpc.example", gate: GATE, fetch: f })).toThrow(
      InputError,
    );
    expect(() => createGateClient({ rpcUrl: RPC, gate: "Gate.Testnet", fetch: f })).toThrow(
      InputError,
    );
    expect(() => createGateClient({ rpcUrl: RPC, gate: "", fetch: f })).toThrow(InputError);
  });

  it("rejects a malformed gate response with the offending field path", async () => {
    const upper = PROPOSAL_JSON.replace(
      hex(0x4a, 20),
      hex(0x4a, 20).toUpperCase().replace("0X", "0x"),
    );
    await expect(client(rpc(upper).fetch).getProposal(5)).rejects.toThrow(WireError);
    await expect(client(rpc(upper).fetch).getProposal(5)).rejects.toThrow(/new_impl/);
    const badState = PROPOSAL_JSON.replace('"Signed"', '"Teleported"');
    await expect(client(rpc(badState).fetch).getProposal(5)).rejects.toThrow(/state/);
    const badAmount = PROPOSAL_JSON.replace(
      '"1000000000000000000000000"',
      "1000000000000000000000000",
    );
    await expect(client(rpc(badAmount).fetch).getProposal(5)).rejects.toThrow(/reserved/);
    const tooBig = PROPOSAL_JSON.replace(
      '"1009731712"',
      '"340282366920938463463374607431768211456"',
    );
    await expect(client(rpc(tooBig).fetch).getProposal(5)).rejects.toThrow(/max_fee_per_gas/);
    await expect(client(rpc("not json").fetch).getProposal(5)).rejects.toThrow(WireError);
    await expect(client(rpc("[]").fetch).getConfig()).rejects.toThrow(WireError);
  });

  it("rejects a view result that is not UTF-8 or not bytes", async () => {
    const badBytes: typeof fetch = async () =>
      new Response(
        JSON.stringify({ result: { result: [255, 254], block_height: 1, block_hash: "h" } }),
      );
    await expect(client(badBytes).getConfig()).rejects.toThrow(WireError);
    const notBytes: typeof fetch = async () =>
      new Response(JSON.stringify({ result: { result: [256], block_height: 1, block_hash: "h" } }));
    await expect(client(notBytes).getConfig()).rejects.toThrow(WireError);
  });

  it("maps RPC errors, contract errors, HTTP failures and transport failures to distinct errors", async () => {
    const rpcError: typeof fetch = async () =>
      new Response(
        JSON.stringify({
          error: {
            name: "HANDLER_ERROR",
            cause: { name: "UNKNOWN_ACCOUNT" },
            message: "Server error",
          },
        }),
      );
    await expect(client(rpcError).getConfig()).rejects.toThrow(RpcError);
    await expect(client(rpcError).getConfig()).rejects.toThrow(/UNKNOWN_ACCOUNT/);
    const contractError: typeof fetch = async () =>
      new Response(
        JSON.stringify({
          result: { error: "wasm execution failed", logs: [], block_height: 1, block_hash: "h" },
        }),
      );
    await expect(client(contractError).getConfig()).rejects.toThrow(RpcError);
    const gateway: typeof fetch = async () => new Response("bad gateway", { status: 502 });
    const failure = await client(gateway)
      .getConfig()
      .catch((e: unknown) => e);
    expect(failure).toBeInstanceOf(HttpStatusError);
    expect((failure as HttpStatusError).status).toBe(502);
    const offline: typeof fetch = async () => {
      throw new TypeError("fetch failed");
    };
    await expect(client(offline).getConfig()).rejects.toThrow(NetworkError);
  });

  it("sends no credentials", async () => {
    let seen: RequestInit | undefined;
    const spy: typeof fetch = async (_input, init) => {
      seen = init;
      return new Response(
        JSON.stringify({
          result: {
            result: Array.from(new TextEncoder().encode("null")),
            block_height: 1,
            block_hash: "h",
          },
        }),
      );
    };
    await client(spy).getProposal(1);
    expect(seen?.credentials).toBe("omit");
  });
});
