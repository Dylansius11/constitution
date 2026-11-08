import type { CodeLine } from "../components/ui/code-types.ts";

/** Run from the repository root; activation-dependent reads never use spike values. */
export const humanCommands: CodeLine[] = [
  { note: "Today, from a checkout (Rust/Foundry: Linux or WSL2; web: native pnpm):" },
  // docs/BUILD-PLAN.md:143; README.md:44-53 (sandbox/toolchain prerequisites).
  { cmd: "cargo test -p constitution-gate" },
  // docs/BUILD-PLAN.md:36-37; README.md:85 (--root selects the Foundry project).
  { cmd: "forge test --root contracts" },
  // web/package.json:12 (test = vitest run).
  { cmd: "pnpm --dir web test" },
  // crates/cli/src/main.rs:80-84,169-173; docs/technical/ARCHITECTURE.md:235.
  { cmd: "cargo run -p constitution-cli -- rules hash rules/manifest.json" },
  { note: "After activation: build contracts; set GATE_ACCOUNT and PROPOSAL_ID to observed values. See README.md." },
  // crates/cli/src/main.rs:96-122 (record/artifact and both public RPCs have defaults).
  { cmd: "cargo run -p constitution-cli -- audit --deployment deployments/testnet.json" },
  // crates/cli/src/main.rs:60-69,211-235; README.md:101-106 (actual id and gate required).
  { cmd: 'cargo run -p constitution-cli -- status "$PROPOSAL_ID" --gate "$GATE_ACCOUNT"' },
  // crates/cli/src/main.rs:70-79,211-235; README.md:108-110 (read-only JSON receipt).
  { cmd: 'cargo run -p constitution-cli -- receipt "$PROPOSAL_ID" --gate "$GATE_ACCOUNT" --json' },
  // Makefile:12-40 requires twelve explicit inputs; short make commands would be misleading.
  { note: "For make attack / make upgrade, use docs/technical/ARCHITECTURE.md, Demo runner operation. These write and may spend inference credits." },
];

export const agentCommands: CodeLine[] = [
  { note: "Give your agent this page's /llms.txt, then ask it to verify each claim." },
  { note: "From a checkout: start with docs/PRD.md and docs/BUILD-PLAN.md. Local tests are not activation evidence." },
  // crates/cli/src/main.rs:80-84,169-173; docs/technical/ARCHITECTURE.md:235.
  { cmd: "cargo run -p constitution-cli -- rules hash rules/manifest.json" },
  // docs/BUILD-PLAN.md:143; README.md:44-53 (run Rust checks on Linux or WSL2).
  { cmd: "cargo test -p constitution-gate" },
  // web/package.json:12 (run web checks using the native pnpm installation).
  { cmd: "pnpm --dir web test" },
  { note: "Do not deploy, sign, spend, or treat a spike or local result as an activated deployment." },
];
