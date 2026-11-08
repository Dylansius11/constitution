import { shortValue, utcStamp } from "./display.ts";
import type { SurfaceState } from "./state.ts";
import type { RunStage } from "./verifier.ts";

/** Structural StageLadder contract; kept TS-only so tsc never needs to load an Astro component. */
export interface Stage {
  label: string;
  state: "pending" | "active" | "done" | "refused" | "skipped";
  status?: string;
  detail?: string;
}

export const VERIFIER_LABELS = {
  queued: { label: "Queued", detail: "Waiting for the verifier to start." },
  unsealing: {
    label: "Opening the sealed bundle",
    detail: "Unsealed inside the attested verifier. The hash must equal the commitment.",
  },
  compiling: {
    label: "Compiling",
    detail: "Pinned compiler. The runtime code hash must equal the proposal.",
  },
  r1_layout: {
    label: "R1 Storage layout",
    detail: "The new layout must extend the current one, append only.",
  },
  r2_solvency: {
    label: "R2 Solvency",
    detail: "Within the bound, deposits and withdrawals never leave the vault owing more than it holds.",
  },
  r3_filter: {
    label: "R3 Opcode filter",
    detail: "Rejects SELFDESTRUCT, CALLCODE, DELEGATECALL and literal proxy slots in the bytecode.",
  },
  r4_initializer: {
    label: "R4 Initializer allowlist",
    detail: "Init calldata must be empty or match a committed selector and schema.",
  },
  r5_replay: {
    label: "R5 Differential replay",
    detail:
      "A committed corpus replays against old and new code. Outputs must match except declared changes.",
  },
  ai_review: {
    label: "Private red-team review",
    detail: "The AI can only refuse. It cannot approve.",
  },
  reproducing: {
    label: "Reproducing candidate",
    detail: "Anything the AI proposes must be reproduced deterministically.",
  },
  signing: {
    label: "Signing the verdict",
    detail: "The verifier signs its verdict inside the enclave.",
  },
  submitting: {
    label: "Recording the verdict on NEAR",
    detail: "The signed verdict is sent to the gate.",
  },
} as const;

type StageKey = keyof typeof VERIFIER_LABELS;
const keys = Object.keys(VERIFIER_LABELS) as StageKey[];
const ruleKeys = ["r1_layout", "r2_solvency", "r3_filter", "r4_initializer", "r5_replay"] as const;
const ruleNames = [
  "Storage layout",
  "Solvency",
  "Opcode filter",
  "Initializer allowlist",
  "Differential replay",
];

export interface SurfaceContext {
  fixture?: string;
  proposalId?: number;
  ruleSetHash?: string;
  ruleCount?: number;
  /** Only pass a plain reason that is actually present in observed evidence. */
  ruleReason?: string;
  runStages?: readonly RunStage[];
  /** False when the page has no run id, even if a certificate exists on the gate. */
  verifierObserved?: boolean;
}

export interface SurfaceView {
  title: string;
  body: string;
  tone: "neutral" | "pass" | "refuse" | "pending";
  stages: Stage[];
  live: "polite" | "assertive" | "off";
}

function ladder(
  state: Exclude<SurfaceState, { kind: "network_unavailable" }>,
  ctx: SurfaceContext,
): Stage[] {
  const idle = state.kind === "idle";
  const submitting = state.kind === "submitting";
  const unobserved =
    ctx.verifierObserved === false || (state.kind === "verifying" && !state.runObserved);
  const stages: Stage[] = [
    {
      label: "Registering the proposal",
      state: idle
        ? "pending"
        : submitting
          ? "active"
          : state.kind === "error" || state.kind === "unknown"
            ? "pending"
            : "done",
    },
  ];
  const current =
    state.kind === "verifying" || state.kind === "ai_review" || state.kind === "review_incomplete"
      ? state.stage
      : null;
  const failedRule = state.kind === "rejected" ? ruleKeys[state.ruleId - 1] : undefined;
  const beforeAI =
    !unobserved &&
    state.kind === "rejected" &&
    failedRule !== undefined &&
    !ctx.runStages?.some(
      (s) => s.name === "ai_review" && s.status !== "NotRun" && s.status !== "Skipped",
    );
  const terminal = [
    "rejected",
    "expired",
    "verifier_failed",
    "review_incomplete",
    "error",
  ].includes(state.kind);
  for (const key of keys) {
    const spec = VERIFIER_LABELS[key];
    // The active pipeline stage is "submitting"; the recorded chain result is "submit".
    const observed = ctx.runStages?.find((s) => s.name === (key === "submitting" ? "submit" : key));
    let stage: Stage = { ...spec, state: "pending" };
    if (unobserved && !idle && !submitting)
      stage = { ...spec, state: "skipped", status: "Not observed by this page" };
    if (observed !== undefined) {
      stage = {
        ...spec,
        state:
          observed.status === "Pass" ? "done" : observed.status === "Fail" ? "refused" : "skipped",
        status:
          observed.status === "Pass"
            ? "Done"
            : observed.status === "Fail"
              ? "Refused"
              : observed.status === "Incomplete"
                ? "Incomplete"
                : "Not run",
        detail: observed.detail || spec.detail,
      };
    } else if (!unobserved && key === current) stage.state = "active";
    if (terminal && observed === undefined && !unobserved)
      stage = { ...spec, state: "skipped", status: "Not reached" };
    if (beforeAI && (key === "ai_review" || key === "reproducing"))
      stage = {
        ...spec,
        state: "skipped",
        status: "Not run",
        detail: "A deterministic rule refused first.",
      };
    if (state.kind === "rejected" && key === failedRule && !unobserved)
      stage = {
        ...spec,
        state: "refused",
        status: "Refused",
        detail: ctx.ruleReason ?? `Reason code ${state.reasonCode}.`,
      };
    if (state.kind === "review_incomplete" && key === current)
      stage = { ...spec, state: "skipped", status: "Incomplete" };
    stages.push(stage);
  }
  const signed = ["relaying", "executed", "reverted"].includes(state.kind);
  stages.push({
    label: "Chain Signature",
    state: signed
      ? "done"
      : state.kind === "signing"
        ? "active"
        : state.kind === "signing_retry"
          ? "refused"
          : terminal
            ? "skipped"
            : "pending",
    ...(state.kind === "signing_retry"
      ? { status: "Failed" }
      : terminal
        ? { status: "Not reached" }
        : {}),
  });
  stages.push({
    label: "Base Sepolia execution",
    state:
      state.kind === "executed"
        ? "done"
        : state.kind === "reverted"
          ? "refused"
          : state.kind === "relaying"
            ? "active"
            : terminal
              ? "skipped"
              : "pending",
    ...(state.kind === "reverted"
      ? { status: "Reverted" }
      : terminal
        ? { status: "Not reached" }
        : {}),
  });
  return stages;
}

/** Pure DESIGN 6 presentation. Does not create observations, run a clock, or promote signed to executed. */
export function surfaceView(state: SurfaceState, ctx: SurfaceContext = {}): SurfaceView {
  if (state.kind === "network_unavailable") {
    const last = state.lastKnown === null ? null : surfaceView(state.lastKnown, ctx);
    const source = { near: "NEAR testnet", base: "Base Sepolia", verifier: "the verifier" }[
      state.source
    ];
    const time =
      state.lastObservedAtMs === null
        ? "No state has been observed yet."
        : `Showing the last observed state from ${utcStamp(state.lastObservedAtMs)}.`;
    return {
      title: `Cannot reach ${source}`,
      body: `${time} Nothing here is live until the connection returns.${last ? ` Last observed: ${last.title}. ${last.body}` : ""}`,
      tone: "neutral",
      live: "assertive",
      stages: (last?.stages ?? ladder({ kind: "idle" }, ctx)).map((s) => ({
        ...s,
        status:
          s.status ??
          {
            pending: "Pending",
            active: "In progress",
            done: "Done",
            refused: "Refused",
            skipped: "Not reached",
          }[s.state],
        state: "skipped",
      })),
    };
  }
  let title: string;
  let body: string;
  let tone: SurfaceView["tone"] = "pending";
  let live: SurfaceView["live"] = "polite";
  const fixture = ctx.fixture ?? "this proposal";
  const rule =
    state.kind === "rejected" ? (ruleNames[state.ruleId - 1] ?? `rule ${state.ruleId}`) : "";
  switch (state.kind) {
    case "idle":
      title = ctx.fixture ? `Submit ${fixture}` : "Watch a proposal";
      body =
        fixture === "VaultV2Drain"
          ? "Looks like a fee update. Contains a hidden path that lets one address withdraw the whole vault. Submit it with the demo key and watch the constitution decide."
          : ctx.fixture
            ? "Adds a feature with append-only storage. It is built to pass every committed rule. Submit it and watch what the constitution decides."
            : "Enter a proposal id to read its observed gate state and Base Sepolia execution.";
      tone = "neutral";
      live = "off";
      break;
    case "submitting":
      title = state.step === "sealing" ? "Sealing the source bundle" : "Registering the proposal";
      body =
        state.step === "sealing"
          ? "Sealing the source bundle for the attested verifier. Nothing has been recorded on NEAR yet."
          : "Signing with the demo key and sending it to NEAR testnet. Waiting for the gate to record a proposal id.";
      break;
    case "verifying":
      title = "Verifying";
      body = state.runObserved
        ? `The attested verifier is checking ${fixture} against the committed rules. Source stays inside the enclave.`
        : "The proposal is recorded on NEAR. Verifier stages are not observed by this page; a run id is needed to read them.";
      break;
    case "verifier_failed":
      title = "Verifier run failed";
      body = `This verifier run did not finish${state.errorCode ? ` (${state.errorCode})` : ""}. The proposal is still Pending on NEAR. No verdict was recorded by this run. A new verifier run may be started with the CLI.`;
      break;
    case "ai_review":
      title = "Private red-team review";
      body =
        "A private AI is searching for a counterexample inside the enclave. Anything it proposes must be reproduced deterministically before it can refuse anything. The AI can only refuse. It cannot approve.";
      break;
    case "certified":
      title = "Certified";
      tone = "pass";
      body =
        ctx.ruleCount !== undefined && ctx.ruleSetHash
          ? `All ${ctx.ruleCount} rules passed under rule set ${shortValue(ctx.ruleSetHash, "hash", "narrow")}. No candidate counterexample reproduced. Next, the gate asks the MPC network to sign this exact transaction.`
          : "The gate recorded a passing certificate. Next, the gate asks the MPC network to sign this exact transaction.";
      break;
    case "signing":
      title = "Signing";
      body =
        "The gate asked the NEAR MPC network to sign the exact upgrade transaction. Waiting for the signature.";
      break;
    case "signing_retry":
      title = "Signature request failed";
      body =
        "The proposal is still certified. Retrying signs the identical transaction with the same nonce. Inspect the failure with the CLI before calling retry_sign on the gate.";
      live = "assertive";
      break;
    case "relaying":
      title =
        state.evm === "included_unverified"
          ? "Included; implementation not yet verified"
          : "Signed, waiting for Base Sepolia";
      body =
        state.evm === "included_unverified"
          ? "The transaction was included successfully. The expected proxy implementation has not yet been observed; this page cannot call it executed."
          : state.evm === "pending"
            ? "The transaction was observed on Base Sepolia and is waiting for inclusion."
            : `The gate holds a signed transaction. Anyone can broadcast it. It is not on Base Sepolia yet.${ctx.proposalId !== undefined ? ` You can also run constitution relay ${ctx.proposalId} yourself.` : ""}`;
      break;
    case "executed":
      title = "Executed on Base Sepolia";
      tone = "pass";
      body = `Block ${state.block}. ${state.implementationBefore === null ? `The proxy implementation is ${shortValue(state.implementationAfter, "address", "wide")}. The previous implementation was not observed by this page.` : `The proxy implementation changed from ${shortValue(state.implementationBefore, "address", "wide")} to ${shortValue(state.implementationAfter, "address", "wide")}.`} This upgrade passed through the gate, was signed by the MPC network, and executed through the constitution executor.`;
      break;
    case "rejected":
      title = `Refused under R${state.ruleId} ${rule}`;
      tone = "refuse";
      body = `${ctx.ruleReason ?? `The verifier recorded a refusal under ${rule} (reason code ${state.reasonCode}).`} No signature was requested.${state.implementationUnchanged ? " The proxy implementation is unchanged." : ""} The reviewer can only refuse, never approve.`;
      break;
    case "expired":
      title = "Expired";
      body = `This proposal was not completed before its deadline and can no longer be certified.${state.implementationUnchanged ? " The proxy implementation is unchanged." : ""} Submit again with the CLI to create a new proposal.`;
      break;
    case "review_incomplete":
      title = "Review incomplete";
      body = `The private AI review did not finish, so nothing was certified. This is not a pass and not a refusal. It can be retried until ${utcStamp(Number(state.deadlineNs / 1_000_000n))}.`;
      break;
    case "reverted":
      title = "Reverted on Base Sepolia";
      tone = "refuse";
      live = "assertive";
      body = `The executor refused this transaction.${ctx.proposalId !== undefined ? ` Run constitution receipt ${ctx.proposalId} for the revert reason.` : " Read the transaction receipt for the revert reason."}`;
      break;
    case "error": {
      tone = "refuse";
      live = "assertive";
      switch (state.cause) {
        case "gate_refused":
          title = "Could not register the proposal";
          body = `The gate refused this proposal: ${state.detail} Nothing was recorded. Read the CLI instructions to submit with your own account.`;
          break;
        case "verifier_unreachable":
          title = "Cannot reach the verifier";
          body = `${state.detail}${ctx.proposalId !== undefined ? ` Inspect proposal ${ctx.proposalId} with the CLI before requesting verification again; do not repropose.` : " Inspect the demo account with the CLI before submitting again."}`;
          break;
        case "broadcast_failed":
          title = "Could not broadcast the transaction";
          body = `The transaction could not be broadcast: ${state.detail} It is still signed and held by the gate.${ctx.proposalId !== undefined ? ` Run constitution relay ${ctx.proposalId} to relay it.` : " Use the CLI to relay it."}`;
          break;
        case "receipt_schema":
          title = "Could not read the receipt";
          body =
            "This receipt does not match the expected format. Treat its contents as unverified. Use the CLI to inspect the raw receipt JSON.";
          break;
        case "implementation_mismatch":
          title = "Implementation does not match";
          body = `${state.detail}. This page cannot call the upgrade executed. Read the chain state and full receipt with the CLI.`;
          break;
        case "verifier_incomplete":
          title = "Verification incomplete";
          body = `${state.detail}. This is not a passing verdict. Inspect the incomplete stage with the CLI before starting a new run.`;
          break;
        case "run_evicted":
          title = "Verifier run no longer available";
          body = `${state.detail}. Start a new verifier run with the CLI to review this proposal.`;
          break;
        case "run_refused":
          title = "Verifier run refused";
          body = `${state.detail}. Inspect the refusal with the CLI before starting another run.`;
          break;
        case "proposal_not_found":
          title = "Proposal not found";
          body = `${state.detail} Check the proposal id and try Watch again.`;
          break;
        case "other":
          title = "Could not complete the read";
          body = `${state.detail} Try Watch again, or use the CLI for the full receipt.`;
          break;
      }
      break;
    }
    case "unknown":
      title = "State not recognized";
      tone = "neutral";
      body = `${state.reason} No outcome is inferred. Read the proposal with the CLI.`;
      break;
  }
  return { title, body, tone, live, stages: ladder(state, ctx) };
}
