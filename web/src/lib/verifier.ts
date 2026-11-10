import { base64ToBytes, bytesToBase64 } from "./bytes.ts";
import { HttpStatusError, InputError, WireError } from "./errors.ts";
import { type HttpOptions, parseHttpUrl, requestText } from "./http.ts";
import { parseJson } from "./json.ts";
import { MAX_BUNDLE_BYTES, SEAL_OVERHEAD_BYTES } from "./seal.ts";
import { asArray, asBoolean, asHexFixed, asSafeUint, asString, isAccountId, Obj } from "./wire.ts";

/** Verifier stages in execution order (ARCHITECTURE, Verifier HTTP API). */
export const VERIFIER_STAGES = [
  "queued",
  "unsealing",
  "compiling",
  "r1_layout",
  "r2_solvency",
  "r3_filter",
  "r4_initializer",
  "r5_replay",
  "ai_review",
  "reproducing",
  "signing",
  "submitting",
  "done",
  "failed",
] as const;
export type VerifierStage = (typeof VERIFIER_STAGES)[number];

export function isVerifierStage(value: string): value is VerifierStage {
  return (VERIFIER_STAGES as readonly string[]).includes(value);
}

export const STAGE_STATUSES = ["Pass", "Fail", "Incomplete", "NotRun", "Skipped"] as const;
export type StageStatus = (typeof STAGE_STATUSES)[number];

export interface VerifierIdentity {
  verifier_pubkey: string;
  transport_pubkey: string;
  /** `null` while the verifier has no measured release (local mode). */
  release_hash: string | null;
  report_data_preimage: string;
  quote_hex: string;
  event_log: string[];
  compose_hash: string | null;
  supported_rule_set_hashes: string[];
  /** Free-text self description, for example "local, not attested". Display it; never parse it. */
  label: string | null;
}

export interface RunStage {
  name: string;
  status: StageStatus;
  detail: string;
  elapsed_ms: number | null;
}

export type RunVerdict =
  | { kind: "Pass" }
  | { kind: "Reject"; rule_id: number; reason_code: number };

/**
 * Stable `error_code` values a `failed` run carries (LOCAL.md, outcome classification). A code this
 * page does not know becomes `unknown_code`: a newer verifier must not break the page.
 */
export const RUN_ERROR_CODES = [
  "worker_panic",
  "sandbox_unavailable",
  "storage_error",
  "tool_unavailable",
  "tool_output_invalid",
  "infrastructure_timeout",
  "rpc_unavailable",
  "committed_data_unavailable",
  "bad_bundle",
] as const;
export type RunErrorCode = (typeof RUN_ERROR_CODES)[number] | "unknown_code";

/**
 * A verifier run as reported over HTTP. `stage` stays a plain string: a stage this page does not
 * know is a fact to surface (state `unknown`), not a parse failure. A run is never an authority:
 * its certificate counts only after its bytes hash to the gate's stored `certificate_hash`.
 * A `failed` run has no certificate, signature, verdict or submission; the parser enforces it.
 */
export interface VerifierRun {
  gate_account: string;
  proposal_id: number;
  boot_id: string;
  stage: string;
  started_at: number;
  stages: RunStage[];
  outcome: string | null;
  verdict: RunVerdict | null;
  certificate_b64: string | null;
  signature_b64: string | null;
  near_tx_hash: string | null;
  evidence_root: string | null;
  elapsed_ms: number | null;
  /** Present on `failed` runs when the verifier names a cause. */
  error_code: RunErrorCode | null;
}

export interface RuleEvidence {
  status: StageStatus;
  artifact_sha256: string;
  detail_code: number;
}

/** The parts of the evidence manifest the page reads. Hash anchoring needs a Borsh encoder (not yet built). */
export interface EvidenceManifest {
  certificate_fields_digest: string;
  compile: { solc: string; settings_hash: string; runtime_hash: string };
  current: { address: string; runtime_hash: string; reproduced: boolean };
  r1: RuleEvidence;
  r2: RuleEvidence;
  r3: RuleEvidence;
  r4: RuleEvidence;
  r5: RuleEvidence;
  ai: {
    model: string;
    completion_id: string;
    request_sha256: string;
    response_sha256: string;
    signature_kind: string;
    signature: string;
    preflight_digest: string;
    candidates: { sha256: string; reproduced: boolean }[];
  };
  evidence_block: { number: number; hash: string };
  archive_provider: string;
}

export type RunRefusalCode =
  | "bad_bundle"
  | "wrong_gate"
  | "unsupported_proposal"
  | "proposal_not_found"
  | "rpc_unavailable"
  | "rate_limited"
  | "queue_full"
  | "cooling_down"
  | "busy"
  | "proposal_not_pending"
  | "not_ready"
  | "run_in_progress";

/** `POST /v1/runs` was refused with one of the documented statuses. */
export class RunRefusedError extends Error {
  readonly code: RunRefusalCode;
  readonly status: number;
  /** The verifier's plain-text reason, bounded. */
  readonly detail: string;
  /** Whole seconds from a `Retry-After` header; `null` when the header is absent or not seconds. */
  readonly retryAfterSeconds: number | null;
  constructor(
    code: RunRefusalCode,
    status: number,
    detail: string,
    retryAfterSeconds: number | null = null,
  ) {
    super(
      `verifier refused the run: ${code}${detail ? ` (${detail})` : ""}${
        retryAfterSeconds === null ? "" : `; retry after ${retryAfterSeconds}s`
      }`,
    );
    this.name = "RunRefusedError";
    this.code = code;
    this.status = status;
    this.detail = detail;
    this.retryAfterSeconds = retryAfterSeconds;
  }
}

/**
 * Codes the verifier emits per status, as the plain-text body. The first entry is the default when
 * the body names none of them (for example a free-text unseal error on 400).
 */
const REFUSALS: Readonly<Record<number, readonly RunRefusalCode[]>> = {
  400: ["bad_bundle", "wrong_gate", "unsupported_proposal"],
  404: ["proposal_not_found"],
  409: ["run_in_progress", "proposal_not_pending", "not_ready"],
  429: ["queue_full", "rate_limited", "cooling_down", "busy"],
  502: ["rpc_unavailable"],
};

function refusalCode(status: number, body: string): RunRefusalCode | undefined {
  const candidates = REFUSALS[status];
  return candidates?.find((code) => code === body.trim()) ?? candidates?.[0];
}

/** `Retry-After` in delta-seconds form; the HTTP-date form is not used by the verifier. */
function parseRetryAfter(header: string | null): number | null {
  if (header === null || !/^\d{1,9}$/.test(header.trim())) return null;
  return Number(header.trim());
}

export interface VerifierClientOptions extends HttpOptions {
  /** Verifier origin, for example `https://verifier.example`. */
  baseUrl: string;
}

export interface StartRunArgs {
  gate: string;
  proposalId: number;
  /** Output of `sealBundle`. */
  sealedBundle: Uint8Array;
}

export interface VerifierClient {
  identity(): Promise<VerifierIdentity>;
  startRun(args: StartRunArgs): Promise<{ runId: number; gateAccount: string; proposalId: number; bootId: string; label: string | null }>;
  /** `null` when the verifier does not know this run id. */
  getRun(runId: number): Promise<VerifierRun | null>;
  /** `null` when the run is unknown or has no evidence yet. */
  getEvidence(runId: number): Promise<EvidenceManifest | null>;
}

function nullableHash(o: Obj, key: string): string | null {
  return o.maybe(key, (v, p) => asHexFixed(v, 32, p)) ?? null;
}

function parseQuoteHex(value: unknown, path: string): string {
  const text = asString(value, path);
  if (!/^(?:0x)?(?:[0-9a-fA-F]{2})*$/.test(text)) throw new WireError(path, "expected hex");
  return text;
}

export function parseIdentity(value: unknown): VerifierIdentity {
  const o = new Obj(value, "identity");
  return {
    verifier_pubkey: o.hex("verifier_pubkey", 32),
    transport_pubkey: o.hex("transport_pubkey", 32),
    release_hash: nullableHash(o, "release_hash"),
    report_data_preimage: o.hexBytes("report_data_preimage"),
    quote_hex: parseQuoteHex(o.raw("quote_hex"), o.at("quote_hex")),
    event_log: asArray(o.raw("event_log"), o.at("event_log")).map((entry, i) =>
      asString(entry, `${o.at("event_log")}[${i}]`),
    ),
    compose_hash: nullableHash(o, "compose_hash"),
    supported_rule_set_hashes: asArray(
      o.raw("supported_rule_set_hashes"),
      o.at("supported_rule_set_hashes"),
    ).map((hash, i) => asHexFixed(hash, 32, `${o.at("supported_rule_set_hashes")}[${i}]`)),
    label: o.maybe("label", asString) ?? null,
  };
}

function parseVerdict(value: unknown, path: string): RunVerdict {
  if (value === "Pass") return { kind: "Pass" };
  const o = new Obj(value, path);
  // serde external tagging is what the verifier emits; the tagged-object form is accepted too.
  if (o.has("Reject")) {
    const r = o.obj("Reject");
    return {
      kind: "Reject",
      rule_id: r.safe("rule_id", 255),
      reason_code: r.safe("reason_code", 65535),
    };
  }
  const kind = o.oneOf("kind", ["Pass", "Reject"] as const);
  if (kind === "Pass") return { kind: "Pass" };
  return {
    kind: "Reject",
    rule_id: o.safe("rule_id", 255),
    reason_code: o.safe("reason_code", 65535),
  };
}

export function parseRun(value: unknown): VerifierRun {
  const o = new Obj(value, "run");
  const run: VerifierRun = {
    gate_account: o.str("gate_account"),
    proposal_id: o.safe("proposal_id"),
    boot_id: o.str("boot_id"),
    stage: o.str("stage"),
    started_at: o.safe("started_at"),
    stages: o.arr("stages").map((entry, i) => {
      const s = new Obj(entry, `run.stages[${i}]`);
      return {
        name: s.str("name"),
        status: s.oneOf("status", STAGE_STATUSES),
        detail: s.str("detail"),
        elapsed_ms: s.maybe("elapsed_ms", (v, p) => asSafeUint(v, p)) ?? null,
      };
    }),
    outcome: o.maybe("outcome", asString) ?? null,
    verdict: o.maybe("verdict", parseVerdict) ?? null,
    certificate_b64:
      o.maybe("certificate_b64", (v, p) => {
        const text = asString(v, p);
        base64ToBytes(text, p);
        return text;
      }) ?? null,
    signature_b64:
      o.maybe("signature_b64", (v, p) => {
        const text = asString(v, p);
        base64ToBytes(text, p, 64);
        return text;
      }) ?? null,
    near_tx_hash: o.maybe("near_tx_hash", asString) ?? null,
    evidence_root: nullableHash(o, "evidence_root"),
    elapsed_ms: o.maybe("elapsed_ms", (v, p) => asSafeUint(v, p)) ?? null,
    error_code:
      o.maybe("error_code", (v, p) => {
        const code = asString(v, p);
        return RUN_ERROR_CODES.find((known) => known === code) ?? "unknown_code";
      }) ?? null,
  };
  if (run.stage === "failed") {
    // `failed` has no certificate, signature, verdict or submission: a run claiming both is not trusted.
    for (const key of ["certificate_b64", "signature_b64", "verdict", "near_tx_hash"] as const) {
      if (run[key] !== null)
        throw new WireError(`run.${key}`, "a failed run carries no such field");
    }
  }
  return run;
}

function parseRule(value: unknown, path: string): RuleEvidence {
  const o = new Obj(value, path);
  return {
    status: o.oneOf("status", STAGE_STATUSES),
    artifact_sha256: o.hex("artifact_sha256", 32),
    detail_code: o.safe("detail_code", 65535),
  };
}

export function parseEvidence(value: unknown): EvidenceManifest {
  const o = new Obj(value, "evidence");
  const compile = o.obj("compile");
  const current = o.obj("current");
  const ai = o.obj("ai");
  const block = o.obj("evidence_block");
  return {
    certificate_fields_digest: o.hex("certificate_fields_digest", 32),
    compile: {
      solc: compile.str("solc"),
      settings_hash: compile.hex("settings_hash", 32),
      runtime_hash: compile.hex("runtime_hash", 32),
    },
    current: {
      address: current.hex("address", 20),
      runtime_hash: current.hex("runtime_hash", 32),
      reproduced: asBoolean(current.raw("reproduced"), current.at("reproduced")),
    },
    r1: parseRule(o.raw("r1"), o.at("r1")),
    r2: parseRule(o.raw("r2"), o.at("r2")),
    r3: parseRule(o.raw("r3"), o.at("r3")),
    r4: parseRule(o.raw("r4"), o.at("r4")),
    r5: parseRule(o.raw("r5"), o.at("r5")),
    ai: {
      model: ai.str("model"),
      completion_id: ai.str("completion_id"),
      request_sha256: ai.hex("request_sha256", 32),
      response_sha256: ai.hex("response_sha256", 32),
      signature_kind: ai.str("signature_kind"),
      signature: ai.hexBytes("signature"),
      preflight_digest: ai.hex("preflight_digest", 32),
      candidates: ai.arr("candidates").map((entry, i) => {
        const c = new Obj(entry, `${ai.at("candidates")}[${i}]`);
        return { sha256: c.hex("sha256", 32), reproduced: c.bool("reproduced") };
      }),
    },
    evidence_block: { number: block.safe("number"), hash: block.hex("hash", 32) },
    archive_provider: o.str("archive_provider"),
  };
}

/** Typed client for the verifier HTTP API. The browser sends only the sealed bundle and public ids. */
export function createVerifierClient(options: VerifierClientOptions): VerifierClient {
  const base = parseHttpUrl(options.baseUrl, "baseUrl");
  const endpoint = (path: string) => new URL(`${base.href.replace(/\/+$/, "")}${path}`);

  function requireRunId(runId: number): number {
    if (!Number.isSafeInteger(runId) || runId < 0)
      throw new InputError("runId must be an unsigned safe integer");
    return runId;
  }

  async function getOptional<T>(
    path: string,
    label: string,
    parse: (value: unknown) => T,
  ): Promise<T | null> {
    try {
      const { text } = await requestText(endpoint(path), { method: "GET" }, options, label);
      return parse(parseJson(text));
    } catch (error) {
      if (error instanceof HttpStatusError && error.status === 404) return null;
      throw error;
    }
  }

  return {
    async identity() {
      const { text } = await requestText(
        endpoint("/v1/identity"),
        { method: "GET" },
        options,
        "verifier identity",
      );
      return parseIdentity(parseJson(text));
    },

    async startRun({ gate, proposalId, sealedBundle }) {
      if (!isAccountId(gate)) throw new InputError("gate must be a NEAR account id");
      if (!Number.isSafeInteger(proposalId) || proposalId < 0) {
        throw new InputError("proposalId must be an unsigned safe integer");
      }
      if (
        sealedBundle.length <= SEAL_OVERHEAD_BYTES ||
        sealedBundle.length > MAX_BUNDLE_BYTES + SEAL_OVERHEAD_BYTES
      ) {
        throw new InputError("sealedBundle is not a sealed bundle of a valid size");
      }
      const body = JSON.stringify({
        gate,
        proposal_id: proposalId,
        sealed_bundle_b64: bytesToBase64(sealedBundle),
      });
      let retryAfter: string | null = null;
      const impl = options.fetch ?? globalThis.fetch;
      const observing: typeof fetch = async (input, init) => {
        const response = await impl(input, init);
        retryAfter = response.headers.get("retry-after");
        return response;
      };
      try {
        const { text } = await requestText(
          endpoint("/v1/runs"),
          { method: "POST", headers: { "content-type": "application/json" }, body },
          { ...options, fetch: observing },
          "verifier run",
        );
        const o = new Obj(parseJson(text), "run_started");
        const gateAccount = o.str("gate_account");
        const returnedProposal = o.safe("proposal_id");
        const bootId = o.str("boot_id");
        if (gateAccount !== gate || returnedProposal !== proposalId || !/^[0-9a-f]{64}$/.test(bootId))
          throw new InputError("verifier run binding differs from the proposal");
        return { runId: o.safe("run_id"), gateAccount, proposalId: returnedProposal, bootId, label: o.maybe("label", asString) ?? null };
      } catch (error) {
        const code =
          error instanceof HttpStatusError ? refusalCode(error.status, error.body) : undefined;
        if (error instanceof HttpStatusError && code !== undefined) {
          throw new RunRefusedError(code, error.status, error.body, parseRetryAfter(retryAfter));
        }
        throw error;
      }
    },

    getRun: async (runId) =>
      getOptional(`/v1/runs/${requireRunId(runId)}`, "verifier run status", parseRun),

    getEvidence: async (runId) =>
      getOptional(`/v1/runs/${requireRunId(runId)}/evidence`, "verifier evidence", parseEvidence),
  };
}
