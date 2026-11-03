import { InputError, RpcError, WireError } from "./errors.ts";
import {
  type Config,
  type Constitution,
  type ListedVerifier,
  type PendingChange,
  type Proposal,
  parseConfig,
  parseConstitution,
  parsePendingChange,
  parseProposal,
  parseVerifierList,
} from "./gate-types.ts";
import { type HttpOptions, parseHttpUrl, requestText } from "./http.ts";
import { type JsonValue, parseJson } from "./json.ts";
import { asArray, asSafeUint, isAccountId, Obj } from "./wire.ts";

/** The gate's list views cap `limit` at 50 (SMART-CONTRACT views). */
export const MAX_LIST_LIMIT = 50;

export interface GateClientOptions extends HttpOptions {
  /** NEAR JSON-RPC endpoint, for example `https://test.rpc.fastnear.com`. */
  rpcUrl: string;
  /** The gate account id. */
  gate: string;
  /** `final` is the default: the page only shows state that cannot be reorganized away. */
  finality?: "final" | "optimistic";
}

/** A value together with the NEAR block it was read at. */
export interface View<T> {
  value: T;
  blockHeight: number;
  blockHash: string;
}

export interface GateClient {
  getConfig(): Promise<View<Config>>;
  getConstitution(): Promise<View<Constitution>>;
  /** `null` when the gate has no proposal with this id. */
  getProposal(id: number): Promise<View<Proposal | null>>;
  listProposals(fromId: number, limit: number): Promise<View<Proposal[]>>;
  getChange(id: number): Promise<View<PendingChange | null>>;
  listChanges(fromId: number, limit: number): Promise<View<PendingChange[]>>;
  listVerifiers(fromIndex: number, limit: number): Promise<View<ListedVerifier[]>>;
}

function requireId(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new InputError(`${name} must be an unsigned safe integer`);
  }
  return value;
}

function requireLimit(value: number): number {
  if (!Number.isSafeInteger(value) || value < 1 || value > MAX_LIST_LIMIT) {
    throw new InputError(`limit must be an integer from 1 to ${MAX_LIST_LIMIT}`);
  }
  return value;
}

function utf8Base64(json: JsonValue): string {
  const bytes = new TextEncoder().encode(JSON.stringify(json));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

/** Typed `call_function` views over NEAR JSON-RPC. Reads only; nothing here can sign or send. */
export function createGateClient(options: GateClientOptions): GateClient {
  const url = parseHttpUrl(options.rpcUrl, "rpcUrl");
  if (!isAccountId(options.gate)) throw new InputError("gate must be a NEAR account id");
  const finality = options.finality ?? "final";

  async function view<T>(
    method: string,
    args: JsonValue,
    parse: (value: unknown) => T,
  ): Promise<View<T>> {
    const body = JSON.stringify({
      jsonrpc: "2.0",
      id: "constitution",
      method: "query",
      params: {
        request_type: "call_function",
        finality,
        account_id: options.gate,
        method_name: method,
        args_base64: utf8Base64(args),
      },
    });
    const { text } = await requestText(
      url,
      { method: "POST", headers: { "content-type": "application/json" }, body },
      options,
      `NEAR RPC (${method})`,
    );
    const envelope = new Obj(parseJson(text), "rpc");
    if (envelope.has("error")) throw new RpcError(describeRpcError(envelope.raw("error")));
    const result = new Obj(envelope.raw("result"), "rpc.result");
    // Older RPC nodes report a contract failure as a string in result.error instead of an error object.
    if (result.has("error")) throw new RpcError(`${method} failed: ${String(result.raw("error"))}`);
    const raw = asArray(result.raw("result"), "rpc.result.result").map((byte, i) =>
      asSafeUint(byte, `rpc.result.result[${i}]`, 255),
    );
    let decoded: string;
    try {
      decoded = new TextDecoder("utf-8", { fatal: true }).decode(Uint8Array.from(raw));
    } catch {
      throw new WireError("rpc.result.result", "view result is not UTF-8");
    }
    return {
      value: parse(parseJson(decoded)),
      blockHeight: result.safe("block_height"),
      blockHash: result.str("block_hash"),
    };
  }

  const list =
    <T>(parse: (value: unknown, path: string) => T) =>
    (value: unknown): T[] =>
      asArray(value, "list").map((item, i) => parse(item, `list[${i}]`));

  return {
    getConfig: async () => view("get_config", {}, (v) => parseConfig(v)),
    getConstitution: async () => view("get_constitution", {}, (v) => parseConstitution(v)),
    getProposal: async (id) =>
      view("get_proposal", { id: requireId(id, "id") }, (v) =>
        v === null ? null : parseProposal(v),
      ),
    listProposals: async (fromId, limit) =>
      view(
        "list_proposals",
        { from_id: requireId(fromId, "fromId"), limit: requireLimit(limit) },
        list(parseProposal),
      ),
    getChange: async (id) =>
      view("get_change", { id: requireId(id, "id") }, (v) =>
        v === null ? null : parsePendingChange(v),
      ),
    listChanges: async (fromId, limit) =>
      view(
        "list_changes",
        { from_id: requireId(fromId, "fromId"), limit: requireLimit(limit) },
        list(parsePendingChange),
      ),
    listVerifiers: async (fromIndex, limit) =>
      view(
        "list_verifiers",
        { from_index: requireId(fromIndex, "fromIndex"), limit: requireLimit(limit) },
        (v) => parseVerifierList(v),
      ),
  };
}

function describeRpcError(error: unknown): string {
  if (typeof error === "object" && error !== null) {
    const record = error as Record<string, unknown>;
    const parts = [
      record["name"],
      record["cause"] && (record["cause"] as Record<string, unknown>)["name"],
      record["message"],
      record["data"],
    ].filter((part): part is string => typeof part === "string");
    if (parts.length > 0) return parts.join(": ");
  }
  return "NEAR RPC returned an error";
}
