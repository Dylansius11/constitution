import { WireError } from "./errors.ts";
import { asAccountId, asArray, asHexFixed, Obj } from "./wire.ts";

/**
 * Wire types of the constitution gate views (crates/constitution-types governance.rs and
 * proposal.rs, serde JSON). Field names stay snake_case so they read like the contract.
 * NEAR nanosecond timestamps are `bigint` (they exceed 2^53); amounts are `bigint` parsed from the
 * gate's decimal strings; hashes and addresses are lowercase `0x` hex.
 */

export const PROPOSAL_STATES = [
  "Pending",
  "Rejected",
  "Expired",
  "SignRequested",
  "Signed",
  "SignFailed",
] as const;
export type ProposalState = (typeof PROPOSAL_STATES)[number];

export const CHANGE_STATUSES = ["Pending", "Applied", "Cancelled"] as const;
export type ChangeStatus = (typeof CHANGE_STATUSES)[number];

export interface Config {
  chain_id: number;
  executor: string;
  proxy: string;
  proxy_admin: string;
  derived_eoa: string;
  mpc_account: string;
  mpc_path: string;
  mpc_domain_id: number;
}

export interface Params {
  proposal_deposit: bigint;
  proposal_ttl_s: number;
  max_cert_ttl_s: number;
  sign_validity_s: number;
  gas_limit: number;
  max_fee_per_gas: bigint;
  max_priority_fee_per_gas: bigint;
  fee_cap: bigint;
  sign_gas_tgas: number;
  callback_gas_tgas: number;
}

export interface Constitution {
  rule_set_hash: string;
  manifest_uri: string;
  ai_required: boolean;
  params: Params;
}

export interface VerifierRecord {
  release_hash: string;
  attestation_uri: string;
  added_at_ns: bigint;
}

/** `list_verifiers` yields `[pubkey_base64, record]` tuples. */
export interface ListedVerifier {
  pubkey_b64: string;
  record: VerifierRecord;
}

export interface EvmTx {
  nonce: number;
  deadline_s: number;
  gas_limit: number;
  max_fee_per_gas: bigint;
  max_priority_fee_per_gas: bigint;
  signing_hash: string;
  signed_tx: string | null;
  attempts: number;
}

export interface Rejection {
  rule_id: number;
  reason_code: number;
  evidence_root: string;
}

export interface Proposal {
  id: number;
  proposer: string;
  created_at_ns: bigint;
  expires_at_ns: bigint;
  rule_set_hash: string;
  new_impl: string;
  new_impl_codehash: string;
  expected_current_impl: string;
  init_calldata: string;
  source_commitment: string;
  reserved: bigint;
  state: ProposalState;
  certificate_hash: string | null;
  rejection: Rejection | null;
  evm: EvmTx | null;
}

export type ChangeKind =
  | { kind: "SetRuleSet"; rule_set_hash: string; manifest_uri: string; ai_required: boolean }
  | { kind: "AddVerifier"; pubkey_b64: string; release_hash: string; attestation_uri: string }
  | { kind: "RemoveVerifier"; pubkey_b64: string }
  | { kind: "SetParams"; params: Params }
  | { kind: "SetGovernor"; account: string }
  | { kind: "SetGuardian"; account: string }
  | { kind: "MigrateAuthority"; new_signer: string }
  | { kind: "Unpause" };

export interface PendingChange {
  id: number;
  kind: ChangeKind;
  scheduled_by: string;
  scheduled_at_ns: bigint;
  eta_ns: bigint;
  status: ChangeStatus;
  evm: EvmTx | null;
}

function parsePubkeyB64(value: unknown, path: string): string {
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]{43}=$/.test(value)) {
    throw new WireError(path, "expected a base64 32-byte key");
  }
  return value;
}

export function parseConfig(value: unknown, path = "config"): Config {
  const o = new Obj(value, path);
  return {
    chain_id: o.safe("chain_id"),
    executor: o.hex("executor", 20),
    proxy: o.hex("proxy", 20),
    proxy_admin: o.hex("proxy_admin", 20),
    derived_eoa: o.hex("derived_eoa", 20),
    mpc_account: o.account("mpc_account"),
    mpc_path: o.str("mpc_path"),
    mpc_domain_id: o.safe("mpc_domain_id", 0xffff_ffff),
  };
}

export function parseParams(value: unknown, path = "params"): Params {
  const o = new Obj(value, path);
  return {
    proposal_deposit: o.u128("proposal_deposit"),
    proposal_ttl_s: o.safe("proposal_ttl_s"),
    max_cert_ttl_s: o.safe("max_cert_ttl_s"),
    sign_validity_s: o.safe("sign_validity_s"),
    gas_limit: o.safe("gas_limit"),
    max_fee_per_gas: o.u128("max_fee_per_gas"),
    max_priority_fee_per_gas: o.u128("max_priority_fee_per_gas"),
    fee_cap: o.u128("fee_cap"),
    sign_gas_tgas: o.safe("sign_gas_tgas"),
    callback_gas_tgas: o.safe("callback_gas_tgas"),
  };
}

export function parseConstitution(value: unknown, path = "constitution"): Constitution {
  const o = new Obj(value, path);
  return {
    rule_set_hash: o.hex("rule_set_hash", 32),
    manifest_uri: o.str("manifest_uri"),
    ai_required: o.bool("ai_required"),
    params: parseParams(o.raw("params"), o.at("params")),
  };
}

export function parseVerifierRecord(value: unknown, path = "verifier"): VerifierRecord {
  const o = new Obj(value, path);
  return {
    release_hash: o.hex("release_hash", 32),
    attestation_uri: o.str("attestation_uri"),
    added_at_ns: o.u64("added_at_ns"),
  };
}

export function parseVerifierList(value: unknown, path = "verifiers"): ListedVerifier[] {
  return asArray(value, path).map((entry, i) => {
    const at = `${path}[${i}]`;
    const tuple = asArray(entry, at);
    if (tuple.length !== 2) throw new WireError(at, "expected a [pubkey, record] pair");
    return {
      pubkey_b64: parsePubkeyB64(tuple[0], `${at}[0]`),
      record: parseVerifierRecord(tuple[1], `${at}[1]`),
    };
  });
}

export function parseEvmTx(value: unknown, path = "evm"): EvmTx {
  const o = new Obj(value, path);
  return {
    nonce: o.safe("nonce"),
    deadline_s: o.safe("deadline_s"),
    gas_limit: o.safe("gas_limit"),
    max_fee_per_gas: o.u128("max_fee_per_gas"),
    max_priority_fee_per_gas: o.u128("max_priority_fee_per_gas"),
    signing_hash: o.hex("signing_hash", 32),
    signed_tx: o.nullable("signed_tx", (v, p) => {
      if (typeof v !== "string" || !/^0x(?:[0-9a-f]{2})+$/.test(v)) {
        throw new WireError(p, "expected lowercase 0x hex bytes");
      }
      return v;
    }),
    attempts: o.safe("attempts", 0xffff_ffff),
  };
}

export function parseProposal(value: unknown, path = "proposal"): Proposal {
  const o = new Obj(value, path);
  return {
    id: o.safe("id"),
    proposer: o.account("proposer"),
    created_at_ns: o.u64("created_at_ns"),
    expires_at_ns: o.u64("expires_at_ns"),
    rule_set_hash: o.hex("rule_set_hash", 32),
    new_impl: o.hex("new_impl", 20),
    new_impl_codehash: o.hex("new_impl_codehash", 32),
    expected_current_impl: o.hex("expected_current_impl", 20),
    init_calldata: o.hexBytes("init_calldata"),
    source_commitment: o.hex("source_commitment", 32),
    reserved: o.u128("reserved"),
    state: o.oneOf("state", PROPOSAL_STATES),
    certificate_hash: o.nullable("certificate_hash", (v, p) => asHexFixed(v, 32, p)),
    rejection: o.nullable("rejection", (v, p) => {
      const r = new Obj(v, p);
      return {
        rule_id: r.safe("rule_id", 255),
        reason_code: r.safe("reason_code", 65535),
        evidence_root: r.hex("evidence_root", 32),
      };
    }),
    evm: o.nullable("evm", parseEvmTx),
  };
}

export function parseChangeKind(value: unknown, path = "kind"): ChangeKind {
  if (value === "Unpause") return { kind: "Unpause" };
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new WireError(path, "expected a change kind");
  }
  const keys = Object.keys(value);
  const tag = keys[0];
  if (keys.length !== 1 || tag === undefined) {
    throw new WireError(path, "expected exactly one variant key");
  }
  const body = new Obj((value as Record<string, unknown>)[tag], `${path}.${tag}`);
  switch (tag) {
    case "SetRuleSet":
      return {
        kind: "SetRuleSet",
        rule_set_hash: body.hex("rule_set_hash", 32),
        manifest_uri: body.str("manifest_uri"),
        ai_required: body.bool("ai_required"),
      };
    case "AddVerifier":
      return {
        kind: "AddVerifier",
        pubkey_b64: parsePubkeyB64(body.raw("pubkey"), body.at("pubkey")),
        release_hash: body.hex("release_hash", 32),
        attestation_uri: body.str("attestation_uri"),
      };
    case "RemoveVerifier":
      return {
        kind: "RemoveVerifier",
        pubkey_b64: parsePubkeyB64(body.raw("pubkey"), body.at("pubkey")),
      };
    case "SetParams":
      return { kind: "SetParams", params: parseParams(body.raw("params"), body.at("params")) };
    case "SetGovernor":
      return { kind: "SetGovernor", account: asAccountId(body.raw("account"), body.at("account")) };
    case "SetGuardian":
      return { kind: "SetGuardian", account: asAccountId(body.raw("account"), body.at("account")) };
    case "MigrateAuthority":
      return { kind: "MigrateAuthority", new_signer: body.hex("new_signer", 20) };
    default:
      throw new WireError(path, `unknown change kind ${tag}`);
  }
}

export function parsePendingChange(value: unknown, path = "change"): PendingChange {
  const o = new Obj(value, path);
  return {
    id: o.safe("id"),
    kind: parseChangeKind(o.raw("kind"), o.at("kind")),
    scheduled_by: o.account("scheduled_by"),
    scheduled_at_ns: o.u64("scheduled_at_ns"),
    eta_ns: o.u64("eta_ns"),
    status: o.oneOf("status", CHANGE_STATUSES),
    evm: o.nullable("evm", parseEvmTx),
  };
}
