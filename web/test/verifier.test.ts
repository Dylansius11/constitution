import { describe, expect, it } from "vitest";
import { HttpStatusError, InputError, NetworkError, WireError } from "../src/lib/errors.ts";
import {
  createVerifierClient,
  isVerifierStage,
  RunRefusedError,
  VERIFIER_STAGES,
} from "../src/lib/verifier.ts";
import { hex } from "./support/fixtures.ts";

const BASE = "https://verifier.example";

interface Seen {
  url: string;
  method: string;
  body: unknown;
}

function server(routes: Record<string, () => Response>) {
  const seen: Seen[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const url = String(input);
    seen.push({
      url,
      method: init?.method ?? "GET",
      body: init?.body === undefined ? undefined : JSON.parse(String(init.body)),
    });
    const key = `${init?.method ?? "GET"} ${new URL(url).pathname}`;
    const route = routes[key];
    return route === undefined ? new Response("no route", { status: 500 }) : route();
  };
  return { seen, client: createVerifierClient({ baseUrl: BASE, fetch: fetchImpl }) };
}

const json =
  (value: unknown, status = 200) =>
  () =>
    new Response(JSON.stringify(value), { status });

const IDENTITY = {
  verifier_pubkey: hex(0x01, 32),
  transport_pubkey: hex(0x02, 32),
  release_hash: null,
  report_data_preimage: `0x${"63".repeat(8)}`,
  quote_hex: "",
  event_log: [],
  compose_hash: null,
  supported_rule_set_hashes: [hex(0xa1, 32)],
  label: "local, not attested; never register on testnet",
};

const RUN = {
  stage: "r2_solvency",
  started_at: 1_793_700_005,
  stages: [
    { name: "unsealing", status: "Pass", detail: "ok", elapsed_ms: 12 },
    { name: "r1_layout", status: "Pass", detail: "append only", elapsed_ms: 340 },
  ],
  outcome: "pending",
  elapsed_ms: 400,
};

describe("verifier client", () => {
  it("identity: local mode parses with null release and compose hashes and keeps its label", async () => {
    const { client, seen } = server({ "GET /v1/identity": json(IDENTITY) });
    const id = await client.identity();
    expect(seen[0]?.url).toBe(`${BASE}/v1/identity`);
    expect(id.release_hash).toBeNull();
    expect(id.compose_hash).toBeNull();
    expect(id.quote_hex).toBe("");
    expect(id.label).toBe("local, not attested; never register on testnet");
    expect(id.transport_pubkey).toBe(hex(0x02, 32));
    expect(id.supported_rule_set_hashes).toEqual([hex(0xa1, 32)]);
  });

  it("identity: attested shape with hashes, quote and event log", async () => {
    const attested = {
      ...IDENTITY,
      release_hash: hex(0x03, 32),
      compose_hash: hex(0x04, 32),
      quote_hex: "04000200",
      event_log: ["e1", "e2"],
      label: undefined,
    };
    const id = await server({ "GET /v1/identity": json(attested) }).client.identity();
    expect(id.release_hash).toBe(hex(0x03, 32));
    expect(id.event_log).toEqual(["e1", "e2"]);
    expect(id.label).toBeNull();
  });

  it("identity: a short or uppercase transport key is a wire error", async () => {
    for (const transport_pubkey of [
      hex(0x02, 31),
      hex(0xab, 32).toUpperCase().replace("0X", "0x"),
    ]) {
      const { client } = server({ "GET /v1/identity": json({ ...IDENTITY, transport_pubkey }) });
      await expect(client.identity()).rejects.toThrow(WireError);
    }
    const { client } = server({ "GET /v1/identity": json({ ...IDENTITY, quote_hex: "xyz" }) });
    await expect(client.identity()).rejects.toThrow(/quote_hex/);
  });

  it("startRun: posts the documented body and returns the run id", async () => {
    const { client, seen } = server({
      "POST /v1/runs": json({ run_id: 4, label: "local, not attested" }, 202),
    });
    const sealed = new Uint8Array(100).fill(1);
    const started = await client.startRun({
      gate: "gate.constitution.testnet",
      proposalId: 7,
      sealedBundle: sealed,
    });
    expect(started).toEqual({ runId: 4, label: "local, not attested" });
    expect(seen[0]?.method).toBe("POST");
    expect(seen[0]?.body).toEqual({
      gate: "gate.constitution.testnet",
      proposal_id: 7,
      sealed_bundle_b64: btoa(String.fromCharCode(...sealed)),
    });
  });

  it("startRun: maps each documented refusal to a typed error", async () => {
    const cases: [number, string, string][] = [
      [400, "sealed box authentication failed", "bad_bundle"],
      [404, "proposal_not_found", "proposal_not_found"],
      [409, "run_in_progress", "run_in_progress"],
      [429, "queue_full", "queue_full"],
    ];
    for (const [status, body, code] of cases) {
      const { client } = server({ "POST /v1/runs": () => new Response(body, { status }) });
      const error = await client
        .startRun({ gate: "gate.testnet", proposalId: 1, sealedBundle: new Uint8Array(100) })
        .catch((e: unknown) => e);
      expect(error, String(status)).toBeInstanceOf(RunRefusedError);
      expect((error as RunRefusedError).code).toBe(code);
      expect((error as RunRefusedError).detail).toBe(body);
    }
  });

  it("startRun: parses every refusal the verifier now emits from the plain-text body", async () => {
    const cases: [number, string, string][] = [
      [400, "bad_bundle", "bad_bundle"],
      [400, "wrong_gate", "wrong_gate"],
      [400, "unsupported_proposal", "unsupported_proposal"],
      [502, "rpc_unavailable", "rpc_unavailable"],
      [429, "rate_limited", "rate_limited"],
      [429, "queue_full", "queue_full"],
      [409, "run_in_progress", "run_in_progress"],
      [409, "proposal_not_pending", "proposal_not_pending"],
      [409, "not_ready", "not_ready"],
      [429, "cooling_down", "cooling_down"],
      [429, "busy", "busy"],
      // A body naming no known code falls back to the status default.
      [400, "", "bad_bundle"],
      [502, "upstream unhappy", "rpc_unavailable"],
    ];
    for (const [status, body, code] of cases) {
      const { client } = server({ "POST /v1/runs": () => new Response(body, { status }) });
      const error = await client
        .startRun({ gate: "gate.testnet", proposalId: 1, sealedBundle: new Uint8Array(100) })
        .catch((e: unknown) => e);
      expect(error, `${status} ${body}`).toBeInstanceOf(RunRefusedError);
      expect(error, `${status} ${body}`).toMatchObject({ code, status, retryAfterSeconds: null });
    }
  });

  it("startRun: carries Retry-After seconds when present and ignores values that are not seconds", async () => {
    const refuse = async (body: string, retryAfter: string | null) => {
      const headers: Record<string, string> =
        retryAfter === null ? {} : { "retry-after": retryAfter };
      const { client } = server({
        "POST /v1/runs": () => new Response(body, { status: 429, headers }),
      });
      return client
        .startRun({ gate: "gate.testnet", proposalId: 1, sealedBundle: new Uint8Array(100) })
        .catch((e: unknown) => e);
    };
    const limited = await refuse("rate_limited", "7");
    expect(limited).toBeInstanceOf(RunRefusedError);
    expect(limited).toMatchObject({
      code: "rate_limited",
      retryAfterSeconds: 7,
      message: expect.stringContaining("retry after 7s"),
    });
    expect(await refuse("queue_full", "1")).toMatchObject({
      code: "queue_full",
      retryAfterSeconds: 1,
    });
    expect(await refuse("queue_full", null)).toMatchObject({ retryAfterSeconds: null });
    for (const bad of ["soon", "-3", "1.5", "Wed, 21 Oct 2026 07:28:00 GMT", ""]) {
      expect(await refuse("rate_limited", bad), bad).toMatchObject({ retryAfterSeconds: null });
    }
  });

  it("startRun: a Retry-After from one refusal never leaks into the next call", async () => {
    let first = true;
    const { client } = server({
      "POST /v1/runs": () => {
        const headers: Record<string, string> = first ? { "retry-after": "9" } : {};
        first = false;
        return new Response("rate_limited", { status: 429, headers });
      },
    });
    const args = { gate: "gate.testnet", proposalId: 1, sealedBundle: new Uint8Array(100) };
    expect(await client.startRun(args).catch((e: unknown) => e)).toMatchObject({
      retryAfterSeconds: 9,
    });
    expect(await client.startRun(args).catch((e: unknown) => e)).toMatchObject({
      retryAfterSeconds: null,
    });
  });

  it("startRun: other failures stay HTTP or network errors, and bad input never leaves the page", async () => {
    const { client, seen } = server({
      "POST /v1/runs": () => new Response("boom", { status: 500 }),
    });
    const args = { gate: "gate.testnet", proposalId: 1, sealedBundle: new Uint8Array(100) };
    await expect(client.startRun(args)).rejects.toThrow(HttpStatusError);
    seen.length = 0;
    await expect(client.startRun({ ...args, gate: "Bad Gate" })).rejects.toThrow(InputError);
    await expect(client.startRun({ ...args, proposalId: -1 })).rejects.toThrow(InputError);
    await expect(client.startRun({ ...args, sealedBundle: new Uint8Array(48) })).rejects.toThrow(
      InputError,
    );
    await expect(
      client.startRun({ ...args, sealedBundle: new Uint8Array(2 * 1024 * 1024 + 49) }),
    ).rejects.toThrow(InputError);
    expect(seen).toHaveLength(0);
    const offline = createVerifierClient({
      baseUrl: BASE,
      fetch: async () => {
        throw new TypeError("down");
      },
    });
    await expect(offline.startRun(args)).rejects.toThrow(NetworkError);
  });

  it("getRun: parses an in-flight run, serde verdicts and the unknown-stage case", async () => {
    const { client, seen } = server({ "GET /v1/runs/4": json(RUN) });
    const run = await client.getRun(4);
    expect(seen[0]?.url).toBe(`${BASE}/v1/runs/4`);
    expect(run?.stage).toBe("r2_solvency");
    expect(run?.stages.map((s) => s.status)).toEqual(["Pass", "Pass"]);
    expect(run?.verdict).toBeNull();
    expect(run?.certificate_b64).toBeNull();

    const reject = await server({
      "GET /v1/runs/5": json({
        ...RUN,
        stage: "done",
        verdict: { Reject: { rule_id: 2, reason_code: 22 } },
        evidence_root: hex(0xe7, 32),
        near_tx_hash: "AbC123",
      }),
    }).client.getRun(5);
    expect(reject?.verdict).toEqual({ kind: "Reject", rule_id: 2, reason_code: 22 });
    expect(reject?.evidence_root).toBe(hex(0xe7, 32));
    expect(reject?.near_tx_hash).toBe("AbC123");

    const pass = await server({
      "GET /v1/runs/6": json({
        ...RUN,
        verdict: "Pass",
        certificate_b64: "AAEC",
        signature_b64: btoa("s".repeat(64)),
      }),
    }).client.getRun(6);
    expect(pass?.verdict).toEqual({ kind: "Pass" });
    expect(pass?.certificate_b64).toBe("AAEC");

    const future = await server({
      "GET /v1/runs/7": json({ ...RUN, stage: "from_the_future" }),
    }).client.getRun(7);
    expect(future?.stage).toBe("from_the_future");
    expect(isVerifierStage("from_the_future")).toBe(false);
    expect(VERIFIER_STAGES.every(isVerifierStage)).toBe(true);
    expect(isVerifierStage("failed")).toBe(true);
  });

  it("getRun: 404 is null; malformed runs are wire errors", async () => {
    const missing = server({ "GET /v1/runs/9": () => new Response("", { status: 404 }) });
    expect(await missing.client.getRun(9)).toBeNull();
    for (const bad of [
      { ...RUN, stage: 3 },
      { ...RUN, stages: [{ name: "x", status: "Maybe", detail: "" }] },
      { ...RUN, verdict: { Reject: { rule_id: 300, reason_code: 1 } } },
      { ...RUN, certificate_b64: "not base64!" },
      { ...RUN, signature_b64: "AAEC" },
      { ...RUN, started_at: -1 },
    ]) {
      const { client } = server({ "GET /v1/runs/1": json(bad) });
      await expect(client.getRun(1), JSON.stringify(bad).slice(0, 60)).rejects.toThrow(WireError);
    }
    await expect(missing.client.getRun(-1)).rejects.toThrow(InputError);
    await expect(missing.client.getRun(1.5)).rejects.toThrow(InputError);
  });

  it("getRun: a failed run keeps its error code; no code is null; an unknown code is accepted as unknown_code", async () => {
    const failed = { ...RUN, stage: "failed", outcome: "failed" };
    const get = (body: unknown) => server({ "GET /v1/runs/1": json(body) }).client.getRun(1);
    expect(await get({ ...failed, error_code: "worker_panic" })).toMatchObject({
      stage: "failed",
      error_code: "worker_panic",
      verdict: null,
      certificate_b64: null,
    });
    for (const code of [
      "sandbox_unavailable",
      "storage_error",
      "tool_unavailable",
      "tool_output_invalid",
      "infrastructure_timeout",
      "rpc_unavailable",
      "committed_data_unavailable",
      "bad_bundle",
    ]) {
      expect((await get({ ...failed, error_code: code }))?.error_code, code).toBe(code);
    }
    expect((await get(failed))?.error_code).toBeNull();
    expect((await get({ ...failed, error_code: null }))?.error_code).toBeNull();
    expect((await get({ ...failed, error_code: "from_the_future" }))?.error_code).toBe(
      "unknown_code",
    );
    // Not a failed run: the code, when present, is still parsed and the run is otherwise normal.
    expect((await get({ ...RUN, error_code: "storage_error" }))?.error_code).toBe("storage_error");
  });

  it("getRun: malformed failed runs are wire errors", async () => {
    const failed = { ...RUN, stage: "failed", error_code: "worker_panic" };
    for (const bad of [
      { ...failed, error_code: 7 },
      { ...failed, error_code: ["worker_panic"] },
      { ...failed, started_at: "now" },
      { ...failed, stages: "none" },
      // A failed run has no certificate, signature, verdict or submission.
      { ...failed, certificate_b64: "AAEC" },
      { ...failed, signature_b64: btoa("s".repeat(64)) },
      { ...failed, verdict: "Pass" },
      { ...failed, verdict: { Reject: { rule_id: 2, reason_code: 22 } } },
      { ...failed, near_tx_hash: "AbC123" },
    ]) {
      const { client } = server({ "GET /v1/runs/1": json(bad) });
      await expect(client.getRun(1), JSON.stringify(bad).slice(0, 80)).rejects.toThrow(WireError);
    }
  });

  it("getEvidence: parses the manifest the page reads; 404 is null", async () => {
    const rule = { status: "Pass", artifact_sha256: hex(0x10, 32), detail_code: 0 };
    const manifest = {
      certificate_fields_digest: hex(0x20, 32),
      compile: { solc: "0.8.38", settings_hash: hex(0x21, 32), runtime_hash: hex(0x22, 32) },
      current: { address: hex(0x91, 20), runtime_hash: hex(0x23, 32), reproduced: true },
      r1: rule,
      r2: { ...rule, status: "Fail", detail_code: 22 },
      r3: rule,
      r4: rule,
      r5: { ...rule, status: "Skipped" },
      ai: {
        model: "z-ai/glm-5.3-flash",
        completion_id: "c1",
        request_sha256: hex(0x30, 32),
        response_sha256: hex(0x31, 32),
        signature_kind: "provider_tee",
        signature: "0x00ff",
        preflight_digest: hex(0x32, 32),
        candidates: [
          { sha256: hex(0x33, 32), reproduced: true },
          { sha256: hex(0x34, 32), reproduced: false },
        ],
      },
      evidence_block: { number: 41_000_000, hash: hex(0x35, 32) },
      archive_provider: "validationcloud",
    };
    const { client, seen } = server({ "GET /v1/runs/4/evidence": json(manifest) });
    const evidence = await client.getEvidence(4);
    expect(seen[0]?.url).toBe(`${BASE}/v1/runs/4/evidence`);
    expect(evidence?.r2).toEqual({
      status: "Fail",
      artifact_sha256: hex(0x10, 32),
      detail_code: 22,
    });
    expect(evidence?.ai.candidates.filter((c) => c.reproduced)).toHaveLength(1);
    expect(evidence?.current.address).toBe(hex(0x91, 20));
    const absent = server({ "GET /v1/runs/8/evidence": () => new Response("", { status: 404 }) });
    expect(await absent.client.getEvidence(8)).toBeNull();
    const broken = server({
      "GET /v1/runs/4/evidence": json({
        ...manifest,
        current: { ...manifest.current, address: "0x1" },
      }),
    });
    await expect(broken.client.getEvidence(4)).rejects.toThrow(/current\.address/);
  });

  it("tolerates a base URL with a trailing slash and refuses a non-URL", async () => {
    const seen: string[] = [];
    const client = createVerifierClient({
      baseUrl: `${BASE}/`,
      fetch: async (input) => {
        seen.push(String(input));
        return json(IDENTITY)();
      },
    });
    await client.identity();
    expect(seen).toEqual([`${BASE}/v1/identity`]);
    expect(() => createVerifierClient({ baseUrl: "verifier.example" })).toThrow(InputError);
  });
});
