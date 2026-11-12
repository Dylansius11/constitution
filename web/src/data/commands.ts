import type { CodeLine, CodeTab } from "../components/ui/code-types.ts";
import { finalRuns, finalVerifier } from "./final-evidence.ts";

/**
 * The read-only CLI checks shown in "Check it yourself", one tab each. Every flag was read from the
 * clap definitions in crates/cli/src/main.rs and the documented invocations in README.md ("Verify
 * it yourself", "Read public state without a signing key"). Run from the repository root with the
 * CLI built as in the README's local quickstart. Nothing here signs, spends or writes to a chain.
 */
const CLI = "./target/debug/constitution";
const GATE = "constitution-gate.testnet";
// README.md, line 32: the single verifier instance of the final run. It may not stay online.
const VERIFIER_URL =
  "https://d72d204232fe1d3a56c6b4a1b1f83d33cc867168-8787.dstack-pha1-prod9.phala.network";

const { keylessAudit } = finalRuns;

export const cliTabs: CodeTab[] = [
  {
    id: "audit",
    label: "audit",
    lines: [
      {
        note: "Reads the final gate and compares it with the committed deployment record and rules.",
      },
      // crates/cli/src/main.rs:95-122 (--deployment and --manifest; no --network flag).
      { cmd: `${CLI} audit --deployment deployments/testnet.json --manifest rules/manifest.json` },
      {
        note: `Recorded 2026-11-10: exit ${keylessAudit.exitCode}, ${keylessAudit.lines} lines, ${keylessAudit.check} ${keylessAudit.result}. State can change: read every check.`,
      },
    ],
  },
  {
    id: "status",
    label: "status",
    lines: [
      {
        note: "Reads a proposal from the gate. Id 0 is the refused drain, id 1 the executed upgrade.",
      },
      // crates/cli/src/main.rs:60-69,211-235 (id and --gate required; --network defaults to testnet).
      { cmd: `${CLI} status 0 --gate ${GATE}` },
      { cmd: `${CLI} status 1 --gate ${GATE}` },
    ],
  },
  {
    id: "receipt",
    label: "receipt",
    lines: [
      { note: "Assembles the receipt for a proposal from public state and prints it as JSON." },
      // crates/cli/src/main.rs:70-79,211-235 (read-only; --json prints constitution.receipt/1).
      { cmd: `${CLI} receipt 0 --gate ${GATE} --json` },
      { cmd: `${CLI} receipt 1 --gate ${GATE} --json` },
    ],
  },
  {
    id: "appraise",
    label: "appraise",
    lines: [
      {
        note: "Checks the verifier's Intel TDX attestation against its recorded compose hash. The endpoint may not stay online.",
      },
      // crates/cli/src/main.rs:85-89,175-186 (url and --expect-compose-hash; README.md:218).
      {
        cmd: `${CLI} verifier appraise ${VERIFIER_URL} --expect-compose-hash ${finalVerifier.composeHash}`,
      },
    ],
  },
];

/** Local test and manifest-hash commands. They are not on the page; the agent guide lists them. */
export const localChecks: CodeLine[] = [
  { note: "Local checks from a checkout (Rust/Foundry: Linux or WSL2; web: native pnpm):" },
  // README.md:150 (the documented gate test invocation; sandbox prerequisites at README.md:44-53).
  { cmd: "cargo test -p constitution-gate" },
  // README.md:151 (--root selects the Foundry project).
  { cmd: "forge test --root contracts" },
  // web/package.json (test = vitest run).
  { cmd: "pnpm --dir web test" },
  // README.md:134; crates/cli/src/main.rs:80-84,169-173.
  { cmd: `${CLI} rules hash rules/manifest.json` },
];
