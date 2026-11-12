import type { CodeLine, CodeTab } from "../components/ui/code-types.ts";
import { demoGate, demoKeyless, demoVerifier } from "./demo-evidence.ts";
import { finalRuns, RECORDED_ON } from "./final-evidence.ts";

/**
 * The CLI checks shown in "Check it yourself", one tab each: the terminal UI first, then the
 * read-only commands with the live demo gate as the primary example. Every flag was read from the
 * clap definitions in crates/cli/src/main.rs (`audit` takes --deployment and --manifest and has no
 * --network flag). Run from the repository root with the CLI built as in the README's local
 * quickstart. Nothing here signs, spends or writes to a chain. The public NEAR RPC default in the
 * CLI answers 429 under load, so reads pass the dRPC endpoint the page itself uses.
 */
const CLI = "./target/debug/constitution";
const FINAL_GATE = "constitution-gate.testnet";
const NEAR_RPC = "https://near-testnet.drpc.org";

const { keylessAudit } = finalRuns;

export const cliTabs: CodeTab[] = [
  {
    id: "tui",
    label: "terminal UI",
    lines: [
      {
        note: "One command opens the terminal UI: Overview, Proposals, Run, Audit and Receipt, with a Demo and a Final profile.",
      },
      { cmd: CLI },
      {
        note: "Every screen has a script-friendly subcommand: the tabs beside this one are those subcommands.",
      },
    ],
  },
  {
    id: "audit",
    label: "audit",
    lines: [
      {
        note: "Reads the live demo gate and compares it with the committed deployment record and the demo rules. Needs the executor artifact from the quickstart build.",
      },
      {
        note: `Demo gate, recorded ${demoKeyless.date}: exit ${demoKeyless.auditExitCode}, ${demoKeyless.auditChecks} checks, gate_has_no_access_keys PASS after its key was deleted.`,
      },
      // crates/cli/src/main.rs:95-122 (--deployment, --manifest, --near-rpc; no --network flag).
      {
        cmd: `${CLI} audit --deployment deployments/demo.json --manifest rules/demo/manifest.json --near-rpc ${NEAR_RPC}`,
      },
      {
        note: `Final gate, recorded ${RECORDED_ON}: exit ${keylessAudit.exitCode}, ${keylessAudit.lines} lines, ${keylessAudit.check} ${keylessAudit.result}. Re-run it; state can change.`,
      },
      { cmd: `${CLI} audit --deployment deployments/testnet.json --manifest rules/manifest.json --near-rpc ${NEAR_RPC}` },
    ],
  },
  {
    id: "status",
    label: "status",
    lines: [
      {
        note: "Reads a proposal from a gate. On the demo gate, proposal 0 is the refused drain; on the final gate, id 1 is the executed upgrade.",
      },
      // crates/cli/src/main.rs:60-69,211-235 (id and --gate required; --network defaults to testnet).
      { cmd: `${CLI} status 0 --gate ${demoGate.account} --near-rpc ${NEAR_RPC}` },
      { cmd: `${CLI} status 1 --gate ${FINAL_GATE} --near-rpc ${NEAR_RPC}` },
    ],
  },
  {
    id: "receipt",
    label: "receipt",
    lines: [
      { note: "Assembles the receipt for a proposal from public state and prints it as JSON." },
      // crates/cli/src/main.rs:70-79,211-235 (read-only; --json prints constitution.receipt/1).
      { cmd: `${CLI} receipt 0 --gate ${demoGate.account} --near-rpc ${NEAR_RPC} --json` },
      { cmd: `${CLI} receipt 1 --gate ${FINAL_GATE} --near-rpc ${NEAR_RPC} --json` },
    ],
  },
  {
    id: "appraise",
    label: "appraise",
    lines: [
      {
        note: "Checks the demo verifier's Intel TDX attestation against its recorded compose hash. The endpoint may not stay online; the final verifier was retired on 2026-11-12.",
      },
      // crates/cli/src/main.rs:85-89,175-186 (url and --expect-compose-hash; README.md demo table).
      {
        cmd: `${CLI} verifier appraise ${demoVerifier.url} --expect-compose-hash ${demoVerifier.composeHash}`,
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
