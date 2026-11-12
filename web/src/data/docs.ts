import type { CodeTab } from "../components/ui/code-types.ts";
import { type DemoAttack, parseDemoGallery } from "../lib/demo-gallery.ts";
import { reasonText, ruleName } from "../lib/reasons.ts";
import { asString, Obj } from "../lib/wire.ts";
import { demoGate as recordedDemoGate, demoVerifier as recordedDemoVerifier } from "./demo-evidence.ts";
import { finalGate } from "./final-evidence.ts";
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
    id: "write-an-upgrade",
    title: "Write an upgrade that passes",
    summary: "The five rules your vault upgrade must satisfy, with the numbers the demo gate committed.",
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

export const CLI = "./target/debug/constitution";
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

function tab(label: string, lines: CodeTab["lines"]): CodeTab {
  return { id: "run", label, lines };
}

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
        tab: tab("audit", [
          { note: "Compares the demo gate with its deployment record and committed rules." },
          { cmd: `${CLI} audit --deployment deployments/demo.json --manifest rules/demo/manifest.json --near-rpc ${NEAR_RPC}` },
        ]),
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
        note: "The defaults point at the final gate, so the demo gate needs both --deployment and --manifest. Audit compares public state with your local files; it does not appraise the verifier.",
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
      "These write: they spend gate credit, sign with your NEAR key, send source to the verifier or broadcast a transaction. Appraise the verifier before you seal private source to it.",
    commands: [
      {
        id: "propose",
        // crates/cli/src/main.rs:37-59 (Propose) with SignerArgs, 252-279. The doc comment says the
        // verifier identity is not appraised, which is why `verifier appraise` comes first.
        usage: "propose",
        purpose: "Register your source commitment on the gate, seal the bundle to the verifier and start a run.",
        access: "Signs with your NEAR key",
        tab: tab("propose", [
          { note: "Prints the verifier run id on success. Run it from the repository root." },
          {
            cmd: `${CLI} propose --account <you>.testnet --gate ${demoGate.account} --near-rpc ${NEAR_RPC} --bundle contracts --contract src/<File>.sol:<Contract> --impl <address of your deployed implementation> --init 0x --verifier ${VERIFIER}`,
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
        note: "Proposal storage is paid from prepaid credit on the gate, so fund it first; the first step of the live demo does that. Your implementation's runtime code hash must equal what the verifier compiles from your source.",
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
        note: "The bundle must hash to the commitment already recorded on the gate; the verifier refuses any other.",
      },
      {
        id: "relay",
        // crates/cli/src/main.rs:123-159 (Relay).
        usage: "relay <id>",
        purpose: "Broadcast the raw transaction the gate signed, after recovering its sender off-chain.",
        access: "Broadcasts a signed transaction",
        tab: tab("relay", [
          { note: "Only a proposal the gate has Signed has a transaction to broadcast. Run it promptly." },
          { cmd: `${CLI} relay <proposal id> --gate ${demoGate.account} --near-rpc ${NEAR_RPC} --watch 300` },
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
        note: "Anyone can relay. The executor on Base Sepolia accepts only a transaction signed for the gate's derived address.",
      },
      {
        id: "demo",
        // crates/cli/src/demo.rs:19-61 (Scenario, DemoArgs): all flags are required except --init,
        // --send-rpc and --watch. The example is the first gallery attack, read from demo-gallery.json.
        usage: "demo <attack|upgrade>",
        purpose: "Propose, watch, check the outcome and keep evidence, in one run.",
        access: "Signs with your NEAR key",
        tab: tab("demo", [
          { note: "attack expects a rejection with the proxy unchanged. upgrade relays an honest upgrade once Signed." },
          {
            cmd: `${CLI} demo attack --bundle contracts --contract ${drain.contract} --impl ${drain.impl} --init ${drain.initCalldata} --account <you>.testnet --gate ${demoGate.account} --key-file .secrets/near-credentials/testnet/<you>.testnet.json --network testnet --near-rpc ${NEAR_RPC} --evm-rpc ${EVM_RPC_DEFAULT} --verifier ${VERIFIER} --output ./demo-drain`,
          },
        ]),
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
        note: "Demo writes never inherit endpoints or wallet defaults, and never reuse an output directory. If a run is interrupted, inspect the evidence directory and the chain before you run again.",
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
    summary: "Timelocked changes to a gate. Scheduling and cancelling need a role you do not hold on the demo gate.",
    commands: [
      {
        id: "govern",
        // crates/cli/src/main.rs:160-166 (Govern), 305-316 (GovernCommand), SignerArgs 252-279.
        usage: "govern <schedule|apply|cancel>",
        purpose: "Schedule, apply or cancel a timelocked change.",
        access: "Signs with your NEAR key",
        tab: tab("govern", [
          { note: `Anyone can apply a matured change. The demo gate's delay is ${days(demoGate.governanceDelayDays)}.` },
          { cmd: `${CLI} govern apply <change id> --account <you>.testnet --gate ${demoGate.account} --near-rpc ${NEAR_RPC}` },
        ]),
        flags: [
          { flag: "schedule --kind <json>", note: `Governor only. For example --kind '"Unpause"'.` },
          { flag: "apply <change id>", note: "Anyone. Apply a change whose delay has elapsed." },
          { flag: "cancel <change id>", note: "Guardian only. Cancel a pending change." },
          ...SIGNER_FLAGS,
        ],
        note: "An early apply is refused with E53_TIMELOCK_NOT_ELAPSED, as recorded on the final gate.",
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
    demo: "Retained while the founder tests it, so its audit still fails gate_has_no_access_keys (README, 2026-11-11).",
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
