import type { CodeTab } from "../components/ui/code-types.ts";
import { type DemoAttack, parseDemoGallery } from "../lib/demo-gallery.ts";
import { reasonText, ruleLabel, ruleName } from "../lib/reasons.ts";
import { asString, Obj } from "../lib/wire.ts";
import { demoGate as recordedDemoGate, demoKeyless, demoVerifier as recordedDemoVerifier } from "./demo-evidence.ts";
import { finalEvm, finalGate, VERIFIER_RETIRED_ON } from "./final-evidence.ts";
import { type ManifestFacts, parseManifest } from "./manifest.ts";
import { site } from "./site.ts";

/**
 * Content of `/docs`. `sections` is the single ordered list that drives the table of contents and the
 * page: a section is one entry here plus one file in `components/docs/sections/` named after its id.
 *
 * Every number about the rules comes from the committed demo manifest at build time. Every CLI flag
 * was read from the clap definitions in `crates/cli/src/main.rs` (and `crates/cli/src/demo.rs` for
 * `demo`); the line ranges are noted beside each command below. Nothing here is a live read.
 */

export interface DocsSection {
  id: string;
  title: string;
  summary: string;
}

export const sections: readonly DocsSection[] = [
  {
    id: "quickstart",
    title: "Quickstart",
    summary: "Build the CLI and audit the demo gate from public state.",
  },
  {
    id: "tui",
    title: "TUI tour",
    summary: "Run constitution with no arguments to inspect a gate, run an attack or an honest upgrade, audit it and read receipts.",
  },
  {
    id: "write-an-upgrade",
    title: "Write an upgrade that passes",
    summary: "The five rules your vault upgrade must satisfy, with the numbers the demo gate committed.",
  },
  {
    id: "skill",
    title: "AI skill",
    summary: "Give your coding agent the same workflow: author, check and propose an upgrade, then read the verdict.",
  },
  {
    id: "commands",
    title: "Headless commands",
    summary: "Every subcommand of the constitution CLI, with the flags it defines.",
  },
  {
    id: "limits",
    title: "Limits",
    summary: "What this is, and what the rules do not prove.",
  },
];

// ---------------------------------------------------------------------------------------------------
// Committed inputs. A missing or malformed file fails the build; nothing is defaulted.
// ---------------------------------------------------------------------------------------------------

const demoManifestFiles = import.meta.glob("../../../rules/demo/manifest.json", {
  eager: true,
  import: "default",
});
const finalManifestFiles = import.meta.glob("../../../rules/manifest.json", {
  eager: true,
  import: "default",
});
const galleryFiles = import.meta.glob("../../../deployments/demo-gallery.json", {
  eager: true,
  import: "default",
});
const deploymentFiles = import.meta.glob("../../../deployments/demo.json", {
  eager: true,
  import: "default",
});

function committed(files: Record<string, unknown>, path: string): unknown {
  const values = Object.values(files);
  const [first] = values;
  if (values.length !== 1 || first === undefined) {
    throw new Error(`docs: ${path} is not in the repository; the docs page reads it at build time`);
  }
  return first;
}

const OPCODE_NAMES: Readonly<Record<number, string>> = {
  255: "SELFDESTRUCT",
  242: "CALLCODE",
  244: "DELEGATECALL",
};

/** SMART-CONTRACT.md "initializeV2(uint256) selector 0x5b4e128c". An unknown selector shows as itself. */
const SELECTOR_SIGNATURES: Readonly<Record<string, string>> = {
  "0x5b4e128c": "initializeV2(uint256)",
};

export interface SelectorFact {
  selector: string;
  signature: string | null;
  args: readonly { type: string; max: string | null }[];
}

export interface ManifestSummary {
  /** Path of the file these numbers were read from, relative to the repository root. */
  path: string;
  name: string;
  version: number;
  facts: ManifestFacts;
  opcodes: readonly { hex: string; name: string }[];
  forbiddenConstants: number;
  selectors: readonly SelectorFact[];
  /** `VaultV1`, `VaultV2Honest`, ...: implementations the pre-check recognises as current. */
  lineage: readonly string[];
}

function summarize(path: string, value: unknown): ManifestSummary {
  const root = new Obj(value, path);
  const r3 = root.obj("r3");
  const r4 = root.obj("r4");
  return {
    path,
    name: root.str("name"),
    version: root.safe("version"),
    facts: parseManifest(value),
    opcodes: r3.arr("forbidden_opcodes").map((entry, index) => {
      if (typeof entry !== "number") throw new Error(`${path}.r3.forbidden_opcodes[${index}]: expected a number`);
      const name = OPCODE_NAMES[entry];
      if (name === undefined) throw new Error(`${path}.r3.forbidden_opcodes[${index}]: unrecognized opcode`);
      return { hex: `0x${entry.toString(16).padStart(2, "0")}`, name };
    }),
    forbiddenConstants: r3.arr("forbidden_constants").length,
    selectors: r4.arr("allowlist").map((entry, index) => {
      const item = new Obj(entry, `${r4.at("allowlist")}[${index}]`);
      const selector = item.hex("selector", 4);
      return {
        selector,
        signature: SELECTOR_SIGNATURES[selector] ?? null,
        args: item.arr("args").map((arg, argIndex) => {
          const a = new Obj(arg, `${item.at("args")}[${argIndex}]`);
          return { type: a.str("type"), max: a.maybe("max", asString) ?? null };
        }),
      };
    }),
    lineage: root.arr("reference_lineage").map((entry, index) => {
      const contract = new Obj(entry, `${root.at("reference_lineage")}[${index}]`).str("contract_name");
      return contract.slice(contract.lastIndexOf(":") + 1);
    }),
  };
}

export const demoManifest = summarize(
  "rules/demo/manifest.json",
  committed(demoManifestFiles, "rules/demo/manifest.json"),
);
const finalManifestRecord = new Obj(committed(finalManifestFiles, "rules/manifest.json"), "rules/manifest.json");
/** The final gate's rule set is named only to compare it with the demo's; its rules are not documented here. */
export const finalManifest = {
  path: "rules/manifest.json",
  version: finalManifestRecord.safe("version"),
};

const gallery = parseDemoGallery(committed(galleryFiles, "deployments/demo-gallery.json"));
const deployment = new Obj(committed(deploymentFiles, "deployments/demo.json"), "deployments/demo.json");

// The account is the recorded constant that /app, the check-it-yourself tabs and this page share; the
// committed gallery must agree with it, so a gate that moved fails the build instead of splitting the page.
if (gallery.gate !== recordedDemoGate.account) {
  throw new Error(
    `docs: deployments/demo-gallery.json names ${gallery.gate}, but data/demo-evidence.ts records ${recordedDemoGate.account}`,
  );
}
export const demoGate = {
  account: recordedDemoGate.account,
  proxy: gallery.proxy,
  governanceDelayDays: deployment.safe("governance_delay_days"),
};
export const attacks: readonly DemoAttack[] = gallery.attacks;

/**
 * The honest upgrade presets, from `deployments/demo-gallery.json` `upgrades[]`. The gallery's
 * `observed` fields are null in this snapshot, so a preset is a reference, not proof that the demo
 * gate already ran it. The contract path follows the repository's `src/demo/<Name>.sol:<Name>` layout.
 */
const upgradeRecords = new Obj(committed(galleryFiles, "deployments/demo-gallery.json"), "deployments/demo-gallery.json")
  .arr("upgrades")
  .map((entry, index) => new Obj(entry, `deployments/demo-gallery.json.upgrades[${index}]`));

function honestUpgrade(from: string, to: string): { contract: string; impl: string } {
  const record = upgradeRecords.find((entry) => entry.str("from") === from && entry.str("to") === to);
  if (record === undefined) {
    throw new Error(`docs: deployments/demo-gallery.json has no upgrade ${from} to ${to}`);
  }
  return { contract: `src/demo/${to}.sol:${to}`, impl: record.hex("impl", 20) };
}

export const honestV2 = honestUpgrade("VaultV1", "VaultV2Honest");
export const honestV3 = honestUpgrade("VaultV2Honest", "VaultV3Honest");
/** `false` while every `upgrades[].observed` in the gallery is null: the presets are then not proof of an executed demo upgrade. */
export const honestUpgradesObserved: boolean = upgradeRecords.every((entry) => entry.raw("observed") !== null);

// The demo verifier recorded in data/demo-evidence.ts (README "Live demo gate" table, 2026-11-11). The
// page build may override the endpoint with PUBLIC_DEMO_VERIFIER_URL, the same input /app uses, so the
// two pages never name different verifiers. The final verifier was retired on 2026-11-12: only the
// demo verifier is ever shown here.
function configuredVerifier(): string | null {
  const raw = import.meta.env.PUBLIC_DEMO_VERIFIER_URL?.trim();
  if (!raw) return null;
  const url = new URL(raw);
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("PUBLIC_DEMO_VERIFIER_URL must be an http(s) URL");
  }
  return url.toString().replace(/\/$/, "");
}

const fromBuild = configuredVerifier();
export const demoVerifier = {
  url: fromBuild ?? recordedDemoVerifier.url,
  /** `true` when the URL came from the build, `false` when it is the README's recorded endpoint. */
  fromBuild: fromBuild !== null,
  composeHash: recordedDemoVerifier.composeHash,
};

// ---------------------------------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------------------------------

export interface RuleDoc {
  id: string;
  name: string;
  checks: string;
  yourCode: string;
  numbers: readonly { term: string; value: string }[];
  limit: string;
  /** The gallery attack this rule refuses on /app, or `null` when no gallery attack targets it. */
  refuses: { id: string; title: string; reason: number; reasonText: string | null } | null;
}

const count = new Intl.NumberFormat("en-US");

function refusal(rule: number): RuleDoc["refuses"] {
  const attack = attacks.find((entry) => entry.expectedRule === rule);
  if (attack === undefined) return null;
  return {
    id: attack.id,
    title: attack.title,
    reason: attack.expectedReason,
    reasonText: reasonText(attack.expectedReason),
  };
}

function buildRules(m: ManifestSummary): RuleDoc[] {
  const f = m.facts;
  const selectors = m.selectors.map((entry) => {
    const bound = entry.args
      .map((arg) => (arg.max === null ? arg.type : `${arg.type}, at most ${arg.max}`))
      .join("; ");
    return `${entry.selector} ${entry.signature ?? ""}`.trim() + (bound ? ` (${bound})` : "");
  });
  const calldata = f.r4.allow_empty
    ? "empty or a call to a committed selector whose arguments fit the committed schema"
    : "a call to a committed selector whose arguments fit the committed schema";
  return [
    {
      id: "R1",
      name: ruleName(1) ?? "Storage layout",
      checks:
        "The compiler's storage layout for your contract must extend the layout of the implementation already live on the proxy, append-only, so every depositor's recorded balance stays readable after the upgrade.",
      yourCode: "Add new state variables after the last existing one. Never reorder, retype or remove a field.",
      numbers: [{ term: "Layout check", value: `OpenZeppelin upgrades-core ${f.tools.upgrades_core}` }],
      limit: "Compares declared layouts only. A slot computed at runtime is outside it.",
      refuses: refusal(1),
    },
    {
      id: "R2",
      name: ruleName(2) ?? "Solvency",
      checks:
        "Bounded symbolic execution and seeded fuzzing drive two fixed depositors through deposit and withdraw. The vault must never owe more than it holds, and the proxy's implementation slot must survive every call.",
      yourCode:
        "Keep accounting in step with token transfers in deposit and withdraw. An extra payout path is exactly what the search looks for.",
      numbers: [
        { term: "Symbolic depth", value: `${f.r2.invariant_depth} calls, loop bound ${f.r2.loop_bound} (Halmos ${f.tools.halmos})` },
        {
          term: "Fuzzing",
          value: `${count.format(f.r2.fuzz_runs)} runs, depth ${f.r2.fuzz_depth}, fixed seed (Foundry ${f.tools.foundry})`,
        },
        { term: "Solver timeout", value: `${f.r2.solver_timeout_assertion_s} s per assertion` },
      ],
      limit: "Aggregate solvency inside those bounds, not each depositor's ability to withdraw. If the solver cannot finish inside its bound, the proposal fails closed (reason 23).",
      refuses: refusal(2),
    },
    {
      id: "R3",
      name: ruleName(3) ?? "Opcode and slot filter",
      checks: `Your compiled runtime bytecode is scanned for the forbidden opcodes and for ${m.forbiddenConstants} committed constants: the proxy's implementation and admin slots, their immediate neighbours, and the Foundry and Halmos cheatcode addresses.`,
      yourCode:
        "Do not use selfdestruct, callcode or delegatecall, and do not hard-code the proxy's implementation or admin slot.",
      numbers: [
        {
          term: "Forbidden opcodes",
          value: m.opcodes.map((op) => `${op.hex} ${op.name}`).join(", "),
        },
        { term: "Forbidden constants", value: String(m.forbiddenConstants) },
      ],
      limit:
        "A filter, not a proof. A slot computed at runtime evades it, and block-environment opcodes are not forbidden.",
      refuses: refusal(3),
    },
    {
      id: "R4",
      name: ruleName(4) ?? "Initializer allowlist",
      checks: `The init calldata sent with your upgrade must be ${calldata}.`,
      yourCode:
        "Propose with --init 0x, or with calldata for the one committed initializer and a value inside its bound.",
      numbers: [
        { term: "Allowed selectors", value: selectors.join("; ") || "none" },
        { term: "Empty calldata", value: f.r4.allow_empty ? "allowed" : "refused" },
      ],
      limit: "Any other function is refused (reason 40), and so are arguments outside the schema (reason 41).",
      refuses: refusal(4),
    },
    {
      id: "R5",
      name: ruleName(5) ?? "Differential replay",
      checks:
        "A committed corpus of transactions replays against the old and the new code from a pinned state through the real upgrade path. Outputs and state must match, except for declared changes.",
      yourCode: "Behave like the previous implementation on everything the corpus exercises.",
      numbers: [{ term: "Corpus size", value: `up to ${f.r5.max_txs} transactions` }],
      limit: `Only the committed corpus is replayed: up to ${f.r5.max_txs} transactions. Behaviour it does not exercise is unobserved.`,
      refuses: refusal(5),
    },
  ];
}

export const rules: readonly RuleDoc[] = buildRules(demoManifest);

export const precheck: readonly string[] = [1, 2, 3, 4].map((code) => {
  const text = reasonText(code);
  if (text === null) throw new Error(`docs: no reason text for code ${code}`);
  return text;
});

// ---------------------------------------------------------------------------------------------------
// Headless commands
// ---------------------------------------------------------------------------------------------------

/** The binary name as the guide invokes it: built with `cargo build --locked --release -p constitution-cli`, then on PATH. */
export const CLI = "constitution";
const VERIFIER = '"$VERIFIER_URL"';
/** The CLI's compiled-in default. It answers HTTP 429 under load, so examples pass `NEAR_RPC` instead. */
export const NEAR_RPC_DEFAULT = "https://test.rpc.fastnear.com";
/** The endpoint the page itself reads through: one source, `site.nearRpc`. */
const NEAR_RPC = site.nearRpc;
const EVM_RPC_DEFAULT = "https://sepolia.base.org";
const READ_FLAGS = [
  { flag: "--gate <account>", note: "Required. The gate account. Also read from GATE_ACCOUNT_ID." },
  { flag: "--near-rpc <url>", note: `NEAR RPC. Also read from NEAR_RPC_URL. Default ${NEAR_RPC_DEFAULT}.` },
  { flag: "--evm-rpc <url>", note: `Base Sepolia RPC. Also read from BASE_SEPOLIA_RPC_URL. Default ${EVM_RPC_DEFAULT}.` },
  { flag: "--verifier <url>", note: "Verifier address. Also read from VERIFIER_URL. Needed only with --run." },
  { flag: "--network <testnet|local>", note: "Labels the evidence. Default testnet. It does not restrict endpoints." },
] as const satisfies readonly FlagDoc[];
const SIGNER_FLAGS = [
  { flag: "--account <account>", note: "The NEAR account that signs. Also read from NEAR_ACCOUNT_ID." },
  {
    flag: "--key-file <path>",
    note: "near-cli legacy key file. Default .secrets/near-credentials/<network>/<account>.json. The key never leaves your machine.",
  },
  { flag: "--gate <account>", note: "The gate account. Also read from GATE_ACCOUNT_ID." },
  { flag: "--network <name>", note: "Also read from NEAR_NETWORK_ID. Default testnet." },
  { flag: "--near-rpc <url>", note: `NEAR RPC. Also read from NEAR_RPC_URL. Default ${NEAR_RPC_DEFAULT}.` },
] as const satisfies readonly FlagDoc[];

export type Access =
  | "Read-only"
  | "Sends sealed source to the verifier"
  | "Signs with your NEAR key"
  | "Broadcasts a signed transaction";

export interface FlagDoc {
  flag: string;
  note: string;
}

export interface CommandDoc {
  id: string;
  /** The invocation as `--help` names it, without the binary. */
  usage: string;
  purpose: string;
  access: Access;
  tab: CodeTab;
  /** Further examples for the same command, shown as extra tabs beside `tab`. */
  moreTabs?: readonly CodeTab[];
  flags: readonly FlagDoc[];
  note?: string;
}

export interface CommandGroup {
  id: string;
  title: string;
  summary: string;
  commands: readonly CommandDoc[];
}

const days = (n: number): string => `${n} ${n === 1 ? "day" : "days"}`;

function tab(label: string, lines: CodeTab["lines"], id = "run"): CodeTab {
  return { id, label, lines };
}

/** The flags every `demo` write passes, after the preset's own `--bundle --contract --impl --init`. */
function demoWriteFlags(output: string): string {
  return `--account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" --gate ${demoGate.account} --network testnet --near-rpc ${NEAR_RPC} --evm-rpc ${EVM_RPC_DEFAULT} --verifier ${VERIFIER} --output ${output} --watch 900`;
}

/** `initializeV2(uint256)` with the honest cap, the way the guide builds it. Needs Foundry's `cast`. */
const HONEST_V2_INIT = `"$(cast calldata 'initializeV2(uint256)' 1000)"`;

const drain = attacks[0];
if (drain === undefined) throw new Error("docs: the demo gallery has no attack to show in `demo`");

export const commandGroups: readonly CommandGroup[] = [
  {
    id: "read",
    title: "Read public state",
    summary: "No key, no spend. These read the gate and the chain and print what they observed.",
    commands: [
      {
        id: "audit",
        // crates/cli/src/main.rs:95-122 (Audit): every flag and default below is from that block.
        usage: "audit",
        purpose: "Prove the claim's preconditions from public state. Exits 1 on any failed check.",
        access: "Read-only",
        tab: tab("demo gate", [
          { note: "Compares the demo gate with its deployment record and committed rules." },
          { cmd: `${CLI} audit --deployment deployments/demo.json --manifest rules/demo/manifest.json --near-rpc ${NEAR_RPC}` },
        ]),
        moreTabs: [
          tab(
            "final gate",
            [
              { note: "Recorded evidence only: the final gate is read-only and its verifier is retired." },
              { cmd: `${CLI} audit --deployment deployments/testnet.json --manifest rules/manifest.json --near-rpc ${NEAR_RPC}` },
            ],
            "final",
          ),
        ],
        flags: [
          { flag: "--deployment <path>", note: "Deployment record written at activation. Default deployments/testnet.json." },
          { flag: "--manifest <path>", note: "Committed rule-set manifest the gate must enforce. Default rules/manifest.json." },
          {
            flag: "--executor-artifact <path>",
            note: "Local artifact built from the committed executor source. Default contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json, which forge build writes.",
          },
          { flag: "--near-rpc <url>", note: `NEAR RPC. Also read from NEAR_RPC_URL. Default ${NEAR_RPC_DEFAULT}.` },
          { flag: "--evm-rpc <url>", note: `Base Sepolia RPC. Also read from BASE_SEPOLIA_RPC_URL. Default ${EVM_RPC_DEFAULT}.` },
        ],
        note: "The defaults point at the final gate, so the demo gate needs both --deployment and --manifest. There is no --network or --gate flag: the gate comes from the deployment record. Audit compares public state with your local files, which are trusted inputs, and does not appraise the verifier. FAIL exits 1; WARN alone exits 0 but is not a clean bill of health, so read every line.",
      },
      {
        id: "status",
        // crates/cli/src/main.rs:60-69 (Status), 211-235 (ReadArgs); watch loop in crates/cli/src/status.rs:48-131.
        usage: "status <id>",
        purpose: "Read a proposal's gate state, any rejection, and the EVM transaction state.",
        access: "Read-only",
        tab: tab("status", [
          { note: "Demo proposal 0 was the hidden-drain attack." },
          { cmd: `${CLI} status 0 --gate ${demoGate.account} --near-rpc ${NEAR_RPC}` },
          { note: "Follow a proposal of your own until it settles. 900 is the number of seconds to wait." },
          { cmd: `${CLI} status <proposal id> --gate ${demoGate.account} --near-rpc ${NEAR_RPC} --watch 900` },
        ]),
        flags: [
          { flag: "<id>", note: "Required. The proposal id." },
          ...READ_FLAGS,
          { flag: "--run <run id>", note: "Also print the verifier's stages for this run. Needs --verifier or VERIFIER_URL." },
          {
            flag: "--watch <seconds>",
            note: "Poll every two seconds until the proposal is Rejected, Expired or Signed. Fails if that takes longer than this many seconds.",
          },
        ],
      },
      {
        id: "receipt",
        // crates/cli/src/main.rs:70-79 (Receipt), 211-235 (ReadArgs).
        usage: "receipt <id>",
        purpose: "Assemble constitution.receipt/1 for a proposal from observed public state.",
        access: "Read-only",
        tab: tab("receipt", [
          { note: "Print the receipt as JSON." },
          { cmd: `${CLI} receipt 0 --gate ${demoGate.account} --near-rpc ${NEAR_RPC} --json` },
        ]),
        flags: [
          { flag: "<id>", note: "Required. The proposal id." },
          ...READ_FLAGS,
          { flag: "--run <run id>", note: "Include the verifier run's stages and evidence. Needs --verifier or VERIFIER_URL." },
          { flag: "--json", note: "Print the receipt as JSON." },
        ],
        note: "A receipt is an observation. A requested run that is unavailable or mismatched makes the command fail rather than drop evidence.",
      },
    ],
  },
  {
    id: "send",
    title: "Send an upgrade",
    summary:
      "These write: they spend gate credit, sign with your NEAR key, send source to the verifier or broadcast a transaction. Each needs your explicit authorization, so none of them is safe to paste blindly. Testnet only.",
    commands: [
      {
        id: "propose",
        // crates/cli/src/main.rs:37-59 (Propose) with SignerArgs, 252-279. The clap doc comment still says
        // the identity is not appraised; crates/cli/src/appraise.rs `sealing_key` appraises any remote
        // verifier against EXPECT_COMPOSE_HASH before sealing, which is what the guide says. The code wins.
        usage: "propose",
        purpose: "Register your source commitment on the gate, seal the bundle to the verifier and start a run.",
        access: "Signs with your NEAR key",
        tab: tab("propose", [
          { note: "Submits once and starts verification. It does not watch or relay. Prints the proposal and run ids; keep them." },
          {
            cmd: `${CLI} propose --bundle contracts --contract src/<File>.sol:<Contract> --impl <address of your deployed implementation> --init 0x --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" --gate ${demoGate.account} --network testnet --near-rpc ${NEAR_RPC} --evm-rpc ${EVM_RPC_DEFAULT} --verifier ${VERIFIER}`,
          },
        ]),
        flags: [
          { flag: "--bundle <path>", note: "Required. Source directory; the CLI builds the canonical bundle from it." },
          { flag: "--contract <path:Name>", note: "Required. The contract to deploy, for example src/<File>.sol:<Contract>." },
          { flag: "--impl <address>", note: "Required. Your implementation, already deployed to Base Sepolia." },
          { flag: "--init <hex>", note: "Initializer calldata. Default 0x." },
          { flag: "--verifier <url>", note: "Required. Verifier address. Also read from VERIFIER_URL." },
          { flag: "--evm-rpc <url>", note: `Base Sepolia RPC. Also read from BASE_SEPOLIA_RPC_URL. Default ${EVM_RPC_DEFAULT}.` },
          ...SIGNER_FLAGS,
        ],
        note: "An alternative to demo upgrade, not the step after it. Before it seals anything the CLI appraises the verifier against EXPECT_COMPOSE_HASH and refuses if that is unset or the appraisal fails. Plaintext source is not uploaded as public chain data. Proposal storage is paid from prepaid credit on the gate: a propose-only key cannot deposit it, so fund it separately with an authorized wallet. Your implementation's runtime code hash must equal what the verifier compiles from your source.",
      },
      {
        id: "verifier-run",
        // crates/cli/src/main.rs:187-208 (VerifierCommand::Run).
        usage: "verifier run",
        purpose: "Re-seal the committed source for a proposal that is still Pending. No on-chain write.",
        access: "Sends sealed source to the verifier",
        tab: tab("verifier run", [
          { note: "Use it when a run never started, for example after the verifier restarted." },
          {
            cmd: `${CLI} verifier run --proposal <proposal id> --bundle contracts --contract src/<File>.sol:<Contract> --gate ${demoGate.account} --near-rpc ${NEAR_RPC} --verifier-url ${VERIFIER}`,
          },
        ]),
        flags: [
          { flag: "--proposal <id>", note: "Required. A Pending proposal." },
          { flag: "--bundle <path>", note: "Required. Source directory (needs --contract) or a canonical Borsh SourceBundle file." },
          { flag: "--contract <path:Name>", note: "The contract inside the source directory." },
          { flag: "--gate <account>", note: "Required. Also read from GATE_ACCOUNT_ID." },
          { flag: "--near-rpc <url>", note: `NEAR RPC. Also read from NEAR_RPC_URL. Default ${NEAR_RPC_DEFAULT}.` },
          { flag: "--verifier-url <url>", note: "Required. Verifier address. Also read from VERIFIER_URL." },
        ],
        note: "The bundle must hash to the commitment already recorded on the gate; the verifier refuses any other. This is a re-seal, not a new proposal, but it can still consume verifier and inference credit: honor Retry-After and do not retry in a loop. If a submission is uncertain, inspect the gate and the relay transactions first.",
      },
      {
        id: "relay",
        // crates/cli/src/main.rs:123-159 (Relay).
        usage: "relay <id>",
        purpose: "Broadcast the raw transaction the gate signed, after recovering its sender off-chain.",
        access: "Broadcasts a signed transaction",
        tab: tab("relay", [
          { note: "Only a proposal the gate has Signed has a transaction to broadcast. Never relay a rule-violating attack." },
          {
            cmd: `${CLI} relay <proposal id> --gate ${demoGate.account} --near-rpc ${NEAR_RPC} --evm-rpc ${EVM_RPC_DEFAULT} --send-rpc https://sepolia-sequencer.base.org --watch 900`,
          },
        ]),
        flags: [
          { flag: "<id>", note: "Required. A proposal id, or a change id with --change." },
          { flag: "--change", note: "Relay an applied MigrateAuthority change instead of a proposal." },
          { flag: "--watch <seconds>", note: "Wait up to this many seconds for inclusion and report the receipt." },
          { flag: "--gate <account>", note: "Required. Also read from GATE_ACCOUNT_ID." },
          { flag: "--near-rpc <url>", note: `NEAR RPC. Also read from NEAR_RPC_URL. Default ${NEAR_RPC_DEFAULT}.` },
          { flag: "--evm-rpc <url>", note: `Base Sepolia RPC. Also read from BASE_SEPOLIA_RPC_URL. Default ${EVM_RPC_DEFAULT}.` },
          {
            flag: "--send-rpc <url>",
            note: "Broadcast endpoint. Also read from BASE_SEPOLIA_SEND_URL. Default https://sepolia-sequencer.base.org.",
          },
        ],
        note: "Anyone can relay. The executor on Base Sepolia accepts only a transaction signed for the gate's derived address. There is no --network flag. demo upgrade already relays when it is safe, so use this only after you have read the existing evidence and state. A change id with --change is an applied MigrateAuthority change, not an upgrade retry.",
      },
      {
        id: "demo",
        // crates/cli/src/demo.rs:19-61 (Scenario, DemoArgs): all flags are required except --init,
        // --send-rpc and --watch. The example is the first gallery attack, read from demo-gallery.json.
        usage: "demo <attack|upgrade>",
        purpose: "Propose, watch, check the outcome and keep evidence, in one run.",
        access: "Signs with your NEAR key",
        tab: tab("attack", [
          { note: `A refusal demo. It is a success only if the gate records a rejection, nothing is signed and the proxy is unchanged. The implementation address is the recorded ${drain.title.toLowerCase()} candidate, not one this page produced.` },
          { cmd: "mkdir -p receipts" },
          {
            cmd: `${CLI} demo attack --bundle contracts --contract ${drain.contract} --impl ${drain.impl} --init ${drain.initCalldata} ${demoWriteFlags("receipts/drain-001")}`,
          },
        ]),
        moreTabs: [
          tab(
            "honest V2",
            [
              { note: "Run only when the observed current version is V1. Needs Foundry's cast for the initializer." },
              { cmd: "mkdir -p receipts" },
              {
                cmd: `${CLI} demo upgrade --bundle contracts --contract ${honestV2.contract} --impl ${honestV2.impl} --init ${HONEST_V2_INIT} ${demoWriteFlags("receipts/honest-v2-001")}`,
              },
            ],
            "v2",
          ),
          tab(
            "honest V3",
            [
              { note: "Only after V2's inclusion and implementation-slot readback were observed. V3 has no initializer." },
              {
                cmd: `${CLI} demo upgrade --bundle contracts --contract ${honestV3.contract} --impl ${honestV3.impl} --init 0x ${demoWriteFlags("receipts/honest-v3-001")}`,
              },
            ],
            "v3",
          ),
        ],
        flags: [
          { flag: "<attack|upgrade>", note: "Required. attack: accept only an observed rejection. upgrade: relay after Signed." },
          { flag: "--bundle <path>", note: "Required. Source directory." },
          { flag: "--contract <path:Name>", note: "Required. The contract to propose." },
          { flag: "--impl <address>", note: "Required. The deployed implementation." },
          { flag: "--init <hex>", note: "Initializer calldata. Default 0x." },
          { flag: "--account <account>", note: "Required. The NEAR account that signs." },
          { flag: "--gate <account>", note: "Required." },
          { flag: "--key-file <path>", note: "Required. Unlike propose, there is no default key file." },
          { flag: "--network <local|testnet>", note: "Required." },
          { flag: "--near-rpc <url>", note: "Required. No default." },
          { flag: "--evm-rpc <url>", note: "Required. No default." },
          { flag: "--send-rpc <url>", note: "Separate broadcaster. Defaults to --evm-rpc." },
          { flag: "--verifier <url>", note: "Required." },
          { flag: "--output <dir>", note: "Required. A new directory whose parent exists. An existing path is never reused." },
          { flag: "--watch <seconds>", note: "Maximum polling time per gate or relay stage. Default 2100." },
        ],
        note: "Demo writes never inherit endpoints or wallet defaults. The output path's parent must exist and the path itself must not, so use a fresh name for a genuinely new run, never to retry an uncertain write. An honest upgrade succeeds only on a successful Base receipt plus a matching implementation-slot readback; Signed is not enough. After an interruption read submission.json, run.json, failure.json and any readable receipt.json, then the chain, before you run again. The examples set --watch 900 on purpose.",
      },
    ],
  },
  {
    id: "verify",
    title: "Check the machinery",
    summary: "Local and read-only helpers for the verifier, the rules and the executor build.",
    commands: [
      {
        id: "verifier-appraise",
        // crates/cli/src/main.rs:85-89,174-186 (VerifierCommand::Appraise). The compose hash is the
        // expected demo hash in deployments/README.md lines 88-90.
        usage: "verifier appraise <url>",
        purpose: "Appraise a verifier's Intel TDX evidence before you register or seal anything to it.",
        access: "Read-only",
        tab: tab("verifier appraise", [
          { note: `The expected compose hash is the demo CVM's, from compose.phala.demo.yaml.` },
          { cmd: `${CLI} verifier appraise ${VERIFIER} --expect-compose-hash ${demoVerifier.composeHash}` },
        ]),
        flags: [
          { flag: "<url>", note: "Required. The verifier's address." },
          { flag: "--expect-compose-hash <0x…>", note: "A 32-byte hash the attested compose file must match." },
          {
            flag: "--pccs-url <url>",
            note: "Collateral service. Also read from PCCS_URL. Default https://pccs.phala.network. Signatures stay anchored to Intel's production root.",
          },
          { flag: "--json", note: "Print the appraisal as JSON." },
        ],
        note: "It verifies the DCAP collateral, the TCB level, the runtime measurements and the key bindings. A passing appraisal describes that moment, not future uptime.",
      },
      {
        id: "rules-hash",
        // crates/cli/src/main.rs:80-84,169-173 (Rules, RulesCommand::Hash); README table for the demo hash.
        usage: "rules hash <manifest>",
        purpose: "Print the rule-set hash, sha256 of the Borsh-encoded manifest, of a validated manifest.",
        access: "Read-only",
        tab: tab("rules hash", [
          { note: "README.md records 0xa988551688a3c7a7c61faa25591b933a419d1233779ce6e5bac38456474d8cc8 for this manifest." },
          { cmd: `${CLI} rules hash rules/demo/manifest.json` },
        ]),
        flags: [{ flag: "<manifest>", note: "Required. Path to a manifest file." }],
      },
      {
        id: "executor-runtime-hash",
        // crates/cli/src/main.rs:90-94 (ExecutorRuntimeHash).
        usage: "executor-runtime-hash",
        purpose: "Print the masked runtime keccak256 of the executor from a local Foundry artifact.",
        access: "Read-only",
        tab: tab("executor-runtime-hash", [{ cmd: `${CLI} executor-runtime-hash` }]),
        flags: [
          {
            flag: "--executor-artifact <path>",
            note: "Default contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json.",
          },
        ],
        note: "A masked local hash. It excludes the compiler-declared immutable regions. It is not a transaction hash and does not prove a live deployment is safe.",
      },
    ],
  },
  {
    id: "governance",
    title: "Governance",
    summary:
      "Timelocked changes to a gate. For authorized demo operators only: scheduling needs the governor, cancelling the guardian, and the final gate has no write path here.",
    commands: [
      {
        id: "govern",
        // crates/cli/src/main.rs:160-166 (Govern), 305-316 (GovernCommand), SignerArgs 252-279.
        usage: "govern <schedule|apply|cancel>",
        purpose: "Schedule, apply or cancel a timelocked change.",
        access: "Signs with your NEAR key",
        tab: tab("govern", [
          { note: `Anyone with a signing account can apply a matured change. The demo gate's delay is ${days(demoGate.governanceDelayDays)}.` },
          {
            cmd: `${CLI} govern --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" --gate ${demoGate.account} --network testnet --near-rpc ${NEAR_RPC} apply <change id>`,
          },
        ]),
        flags: [
          { flag: "schedule --kind <json>", note: `Governor only. For example --kind '"Unpause"'.` },
          { flag: "apply <change id>", note: "Anyone. Apply a change whose delay has elapsed." },
          { flag: "cancel <change id>", note: "Guardian only. Cancel a pending change." },
          ...SIGNER_FLAGS,
        ],
        note: "Use a change id you observed in the queue. An early apply is refused with E53_TIMELOCK_NOT_ELAPSED, as recorded on the final gate. A proposer key is not necessarily authorized for any of these. The Unpause example schedules a real change, so do not run it only to test syntax.",
      },
    ],
  },
];

// ---------------------------------------------------------------------------------------------------
// Demo gate against final gate
// ---------------------------------------------------------------------------------------------------

export interface GateComparisonRow {
  term: string;
  final: string;
  demo: string;
}

export const gateComparison: readonly GateComparisonRow[] = [
  { term: "Account", final: finalGate.account, demo: demoGate.account },
  {
    term: "Rule set",
    final: `${finalManifest.path}, version ${finalManifest.version}`,
    demo: `${demoManifest.path}, version ${demoManifest.version}`,
  },
  {
    term: "Timelock on rule and authority changes",
    final: days(finalGate.governanceDelayDays),
    demo: days(demoGate.governanceDelayDays),
  },
  {
    term: "Access key",
    final: "Deleted on 2026-11-10; the keyless audit passed.",
    demo: `Deleted on ${demoKeyless.date} (NEAR transaction ${demoKeyless.keyDeletionTx}). The same audit then exited ${demoKeyless.auditExitCode} with ${demoKeyless.auditChecks} checks.`,
  },
  {
    term: "Verifier",
    final: `Retired on ${VERIFIER_RETIRED_ON}. Recorded evidence only; there is nothing to send to.`,
    demo: "A separate demo verifier. It was recorded online on 2026-11-11, and its availability is not promised.",
  },
  {
    term: "Purpose",
    final: "Permanent recorded evidence. Read it on the home page.",
    demo: "Repeatable attacks for anyone to run.",
  },
  {
    term: "Run an attack again",
    final: "No. Its proxy now runs the honest V2, so a new proposal stops at the pre-check before R2.",
    demo: `Yes: the ${attacks.length} gallery attacks on /app.`,
  },
];

// ---------------------------------------------------------------------------------------------------
// TUI tour (docs/GUIDE.md "TUI tour"; keymap from crates/cli/src/tui/mod.rs key_event)
// ---------------------------------------------------------------------------------------------------

export interface TuiProfile {
  name: string;
  use: string;
  account: string;
  record: string;
  manifest: string;
  proxy: string;
  delay: string;
  writes: boolean;
}

/** Two separate deployments, not two views of one vault. The final profile has no write path. */
export const tuiProfiles: readonly TuiProfile[] = [
  {
    name: "Demo gate",
    use: "Live inspection, and attack or honest-upgrade runs you explicitly authorize.",
    account: demoGate.account,
    record: "deployments/demo.json",
    manifest: "rules/demo/manifest.json",
    proxy: demoGate.proxy,
    delay: days(demoGate.governanceDelayDays),
    writes: true,
  },
  {
    name: "Final gate, read-only",
    use: "Audit and inspect the recorded final evidence.",
    account: finalGate.account,
    record: "deployments/testnet.json",
    manifest: "rules/manifest.json",
    proxy: finalEvm.proxy,
    delay: days(finalGate.governanceDelayDays),
    writes: false,
  },
];

export interface TuiScreen {
  key: string;
  name: string;
  shows: string;
  honesty: string;
}

export const tuiScreens: readonly TuiScreen[] = [
  {
    key: "1",
    name: "Overview",
    shows:
      "The selected gate and protected proxy, the rule commitment, the current implementation, the authority and audit context, and any service or read errors. Compare it with the deployment record you chose.",
    honesty: `A failing keyless prerequisite is a real failure. Deleting the final gate's key does not show that the demo gate is keyless: its own audit does, and it exited ${demoKeyless.auditExitCode} on ${demoKeyless.date}.`,
  },
  {
    key: "2",
    name: "Proposals",
    shows:
      "The proposals of the selected gate, each one's inputs and lifecycle, and the verifier's actual stages when a run is known.",
    honesty:
      "Pending means no final accepted verdict yet. Rejected carries the numeric rule and reason. SignRequested means an MPC request exists, and SignFailed needs signing diagnosis. Signed means raw transaction bytes exist, not that the proxy changed: EVM inclusion and the implementation are read separately.",
  },
  {
    key: "3",
    name: "Run",
    shows:
      "Demo gate only. Choose a rule-violating attack preset to show a refusal, or the honest V2 then V3 upgrade. Inspect the source contract, the deployed implementation and the exact initializer, then confirm. You supply your authorized NEAR account, a key-file path and a fresh evidence directory; an existing directory is never overwritten.",
    honesty:
      "An attack succeeds as a demonstration only when the gate records a rejection, there is no signing record and the proxy is unchanged. It never relays. An honest upgrade relays only after Signed and succeeds only on a successful Base receipt plus a matching implementation-slot readback. No button or AI answer overrides this.",
  },
  {
    key: "4",
    name: "Audit",
    shows:
      "The public-state authority audit for the selected profile. It compares the observed gate, rule hash, registered verifiers and the executor, ProxyAdmin and proxy binding with the deployment record and your local executor artifact, including the expected governance delay.",
    honesty:
      "FAIL exits nonzero in headless mode. WARN exits zero but is not a clean bill of health. Read the named failed precondition and its recovery instead of dismissing it as a display problem.",
  },
  {
    key: "5",
    name: "Receipt",
    shows:
      "A proposal's canonical evidence: its inputs, the accepted certificate or the rejection, rule and AI outcomes when available, and the observed NEAR transaction and EVM inclusion and implementation data. Gate-only reads need no verifier.",
    honesty:
      "Missing or unavailable accepted-certificate evidence is incomplete, not a verified receipt. Files under deployments/demo-receipts/ are historical evidence, not current network status.",
  },
];

export interface TuiKey {
  /** Alternatives that do the same thing; the page joins them with "or". */
  keys: readonly string[];
  action: string;
}

export interface TuiKeyGroup {
  scope: string;
  keys: readonly TuiKey[];
}

/**
 * The one keymap, as final on 2026-11-12 (GUIDE keyboard reference; crates/cli/src/tui/mod.rs). The
 * page renders it from this array only, so a new key is one entry in the right group.
 */
export const tuiKeymap: readonly TuiKeyGroup[] = [
  {
    scope: "Everywhere",
    keys: [
      { keys: ["1"], action: "Overview" },
      { keys: ["2"], action: "Proposals" },
      { keys: ["3"], action: "Run" },
      { keys: ["4"], action: "Audit" },
      { keys: ["5"], action: "Receipt" },
      { keys: ["Tab"], action: "Cycle through the screens" },
      { keys: ["p"], action: "Switch profile between Demo gate and Final gate (read-only)" },
      { keys: ["r"], action: "Refresh" },
      { keys: ["?"], action: "Help" },
      { keys: ["q", "Esc"], action: "Back, or quit" },
      { keys: ["Ctrl+C"], action: "Cancel" },
    ],
  },
  {
    scope: "Proposals",
    keys: [
      { keys: ["Up", "k"], action: "Previous proposal" },
      { keys: ["Down", "j"], action: "Next proposal" },
      { keys: ["Enter"], action: "Open the proposal's detail" },
    ],
  },
  {
    scope: "Run",
    keys: [
      { keys: ["Left"], action: "Previous preset" },
      { keys: ["Right"], action: "Next preset" },
      { keys: ["e"], action: "Edit the inputs" },
      { keys: ["u"], action: "Toggle attack and upgrade" },
      { keys: ["Enter"], action: "Open the confirm screen. Nothing is written yet." },
      { keys: ["y"], action: "Authorize the write you reviewed" },
    ],
  },
  {
    scope: "Audit",
    keys: [{ keys: ["Enter"], action: "Run the audit" }],
  },
  {
    scope: "Receipt",
    keys: [
      { keys: ["i"], action: "Set the proposal id" },
      { keys: ["Enter"], action: "Fetch the receipt" },
      { keys: ["s"], action: "Save it to a path" },
    ],
  },
];

// ---------------------------------------------------------------------------------------------------
// Refusal codes
// ---------------------------------------------------------------------------------------------------

export interface RefusalCode {
  /** `R2/20`, or `0/2` for the pre-check, as the gallery and the verifier write it. */
  code: string;
  rule: string;
  text: string;
}

/**
 * Every numeric rule/reason pair this page knows, in order. Rule names and texts come from
 * `lib/reasons.ts`; the rule of a reason is its tens digit, and codes below 10 belong to the pre-check.
 * The complete table, including the gate's E-prefixed panics and recovery steps, is the skill's
 * `references/refusals.md`.
 */
export const refusalCodes: readonly RefusalCode[] = [1, 2, 3, 4, 10, 11, 20, 21, 22, 23, 30, 31, 40, 41, 50, 51].map(
  (reason) => {
    const text = reasonText(reason);
    if (text === null) throw new Error(`docs: no reason text for code ${reason}`);
    const rule = reason < 10 ? 0 : Math.floor(reason / 10);
    return { code: rule === 0 ? `0/${reason}` : `R${rule}/${reason}`, rule: ruleLabel(rule), text };
  },
);

// ---------------------------------------------------------------------------------------------------
// AI skill (web/public/skills/constitution, copied byte for byte from skills/constitution)
// ---------------------------------------------------------------------------------------------------

export interface SkillFile {
  /** Path inside the skill folder. */
  path: string;
  href: string;
  purpose: string;
}

export const skillFiles: readonly SkillFile[] = [
  {
    path: "SKILL.md",
    href: "/skills/constitution/SKILL.md",
    purpose: "The workflow: establish the target, write against R1 to R5, check locally, propose once, read the verdict.",
  },
  {
    path: "references/refusals.md",
    href: "/skills/constitution/references/refusals.md",
    purpose: "Every numeric rule and reason, every gate error prefix and every operational refusal, each with its recovery.",
  },
];
