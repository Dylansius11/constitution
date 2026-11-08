import { humanCommands } from "./commands.ts";

/** Only public repository configuration affects this static, deterministic guide. */
export interface LlmsSite {
  repoUrl: string | null;
}

/** Facts are dated, not inferred from endpoint availability or build time. */
export function buildLlmsText(site: LlmsSite): string {
  const source = (path: string): string =>
    site.repoUrl ? `[${path}](${site.repoUrl}/blob/dev/${path})` : path;
  const commands = humanCommands
    .map((line) => (line.cmd ? line.cmd : `# ${line.note ?? ""}`))
    .join("\n");

  // Mechanism: PRD F1-F7; SMART-CONTRACT "TL;DR and necessity" and "Identities and constants".
  // Status: DESIGN:12-13; BUILD-PLAN P1.4, P1.6, P2.5, P2.8, P3.1, P4.1:143.
  // Guarantees, limits, non-claims and rules below follow PRD:104-122,160-164,203-221.
  // Stages: ARCHITECTURE:206-217; panic: SMART-CONTRACT:289; labels: DESIGN:329-336.
  // Map: ARCHITECTURE:157-202; README:155-166. Commands cite their own source lines.
  return `# CONSTITUTION

> CONSTITUTION is a testnet upgrade-control mechanism for one Base Sepolia vault proxy: a keyless NEAR gate requests a Chain Signature only for an exact executor transaction backed by an allowlisted verifier's certificate over owner-committed rules. An attested verifier is the intended private checking boundary; a private red-team AI can only veto through a reproduced counterexample. This guide records status as of 2026-11-07, not a claim of production activation or an independent audit.

## Mechanism (intended activated deployment)

- Anyone can register a proposal binding the candidate implementation, runtime code hash, expected current implementation, initializer calldata and private-source commitment; prepaid credit and one-active-proposal limits apply.
- The verifier receives sealed source, compiles with the pinned compiler, matches its runtime hash to the proposal and checks committed rules R1-R5.
- NEAR AI Cloud searches privately for counterexamples. Only a deterministically reproduced violation can veto; an incomplete required AI review cannot certify.
- The gate validates the allowlisted verifier's signed certificate and all bindings, constructs the exact EIP-1559 executor call itself and only then requests NEAR Chain Signatures. No caller supplies a hash to sign.
- Anyone can relay the signed transaction. ConstitutionExecutor checks the derived caller, single-use certificate id, deadline, code hash and expected current implementation before ProxyAdmin.upgradeAndCall.
- Activation requires deleting every gate access key; the gate has no code-upgrade method. Rules, verifier allowlist and authority migration require a public seven-day timelock. The guardian may only reduce authority immediately. Ordinary rule-compliant upgrades do not wait seven days.
- Sources: ${source("docs/PRD.md")}, ${source("docs/technical/SMART-CONTRACT.md")}.

## Status as of 2026-11-07

- Production gate: not deployed. The final protected deployment has not been activated; deployments/testnet.json intentionally does not exist yet.
- CVM verifier: not running; final CVM attestation and appraisal are not proven. Local service images and offline checks are not CVM evidence.
- Spike, 2026-11-03: a real NEAR MPC signature executed an honest upgrade on Base Sepolia, transaction 0xc232188629763bec4eb9a72be7fd08f66ab5690a2ad997ac6ed20697ff0729d8, receipt status 1. This used constitution-spike.testnet with no verifier in the loop, not the production gate.
- Local test, recorded 2026-11-05: gate checks passed 83 tests (58 unit, 18 property, seven sandbox/integration). This is a dated result, not a claim that tests were rerun for this guide.
- Local test: R1-R5 are exercised locally. The committed depth-four R2 check rejects VaultV2Drain; bounded local checks are not a testnet rejection receipt.
- AI rehearsal, 2026-11-06: real completions verified as provider_tee; one saved candidate reproduced the drain in a local replay with explicitly funded/approved prestate, not on the honest control. No general find rate, final gate certificate or real-testnet prestate is claimed.
- Testnet only. Not independently audited. No activated production receipts, in-browser proposal submission or public demo key are available yet.
- Status sources: ${source("docs/DESIGN.md")}, ${source("docs/BUILD-PLAN.md")}, ${source("deployments/README.md")}.
- Spike evidence: ${source("spikes/gate-min/base-sepolia-evidence.json")}.
- Local evidence: ${source("crates/verifier/evidence/depth/README.md")}, ${source("crates/verifier/evidence/ai/candidate-replay-2026-11-06.json")}.

## Guarantees (conditional on the stated trust assumptions)

- G1. Against implementations that cannot write arbitrary proxy storage, the proxy changes implementation only through ConstitutionExecutor, which only accepts transactions signed for the gate's derived EOA. This is conditional: an implementation capable of computed-slot SSTORE can bypass that upgrade path; the executor's tracker is not an on-chain storage-write guard.
- G2. The gate requests a signature only for the exact executor call bound to a certificate from an allowlisted verifier over the active rule-set hash.
- G3. Rules, verifier allowlist and authority migration change only after a public seven-day delay; guardian actions can only reduce authority. Liveness limit: EVM nonces are assigned at certification, so a certified transaction that nobody ever mines delays later ones until someone relays it (permissionless; a reverted transaction still consumes the nonce). Delay only; no signature is ever created without a certificate.
- G4. AI output can only cause rejection, never certification.
- Source: ${source("docs/PRD.md")}, Risks and guarantees. These are mechanism guarantees under assumptions, not evidence that activation has occurred.

## Trust assumptions and limits

- MPC network honesty and availability are required; threshold signing is not post-quantum.
- The verifier and inference provider depend on TEE vendors and attestation chains. Attestation proves which code ran on which input and output, not that the conclusion is correct or that RPC data was honest.
- Rules protect only what they declare. A dormant backdoor outside the declared properties and bounds can pass.
- Opcode and selector filters are filters, not proofs. R3 catches committed literals and labels, not every computed-slot SSTORE. R2 checks slot preservation only along bounded deposit/withdraw sequences; other entry points and dormant triggers are outside that search. Aggregate solvency is not per-depositor withdrawability; bounded symbolic checks are bounded.
- R2 uses one fixed block environment (only chain id is pinned). R3 does not forbid block-environment opcodes; R5 replays only the committed corpus at its real header. A drain dependent on a later time or block value can pass.
- AI review is nondeterministic; no find rate is promised. Clean means the provider answered with a well-formed receipt and nothing was reproduced locally. Rust shape-checks the receipt, rather than cryptographically verifying it. AI replay uses the SHA-256 precompile address as victim, so it finds attacker-side counterexamples only. The shared sidecar/verifier network namespace is a deployment assumption the tree does not enforce; a simulator-backed attestation key must never be registered.
- MPC, verifier or inference outages delay upgrades; they never cause a signature without certification.
- MockToken is a permissionlessly mintable testnet token with ordinary OpenZeppelin ERC20 allowances. Static checks do not remove the nonzero-to-nonzero approve race. The vault is not qualified for arbitrary fee-on-transfer, rebasing or callback-capable tokens.
- The gate checks certificate signatures and bindings, not Intel DCAP evidence on-chain. Appraisal and initial registration are operator responsibilities; a stolen verifier signing key can forge check results. Pausing or removing a verifier does not revoke an already signed EVM transaction; its executor deadline still matters.
- Sources: ${source("docs/PRD.md")}, Risks and guarantees; ${source("README.md")}, Verifier deployment and trust limits.

## Not claimed

- Full formal verification; prevention of Bybit-style incidents (a Safe-authorized delegatecall writing slot 0, not an upgrade through this mechanism); protection of contracts other than the configured proxy; protection against bugs the declared rules do not cover.
- Mainnet deployment, UUPS support, a multi-proxy platform or implemented autonomous proposal repair are not current product capabilities. Repair is approved stretch scope, not implemented.
- Source: ${source("docs/PRD.md")}, Non-goals and scope cuts, and S1.

## Committed rules R1-R5

- R1 storage layout: the new compiler-reported layout must be an append-only extension of the deployed current implementation's layout.
- R2 solvency: bounded symbolic checks and seeded fuzzing of two fixed actors' deposit/withdraw handlers enforce aggregate solvency and implementation-slot preservation, not per-depositor withdrawability; current symbolic depth is four.
- R3 opcode and slot filter: reject SELFDESTRUCT, CALLCODE, DELEGATECALL, literal ERC-1967 implementation/admin slots and their +/-1 neighbors, embedded ERC-1967 ASCII labels and literal Foundry/Halmos cheatcode addresses; computed values can evade literal filters.
- R4 initializer allowlist: initializer calldata must be empty or match a committed selector and argument schema.
- R5 bounded differential replay: replay the committed demo corpus on current and new implementations from a pinned prestate; outputs and state must match except declared changes.
- Sources: ${source("docs/PRD.md")}, F5; ${source("docs/technical/ARCHITECTURE.md")}, Rule-set manifest; ${source("rules/manifest.json")}.

## Verifier stages and timelock refusal

- Stage order: queued, unsealing, compiling, r1_layout, r2_solvency, r3_filter, r4_initializer, r5_replay, ai_review, reproducing, signing, submitting, done.
- The first deterministic Fail stops later stages (Skipped); AI review runs only after all deterministic checks pass, otherwise NotRun. Incomplete results fail closed.
- Here signing means signing the verifier verdict; it is not the later Chain Signature requested by the gate.
- E53_TIMELOCK_NOT_ELAPSED means apply_change was attempted before the immutable seven-day eta. No role can apply early; the governor schedules, the guardian can cancel, and anyone may apply after eta. It is not a seven-day wait on ordinary certified upgrades.
- Sources: ${source("docs/technical/ARCHITECTURE.md")}, Verifier HTTP API; ${source("docs/technical/SMART-CONTRACT.md")}, Governance and Panic-code table; ${source("docs/DESIGN.md")}, State matrix.

## How to verify

- Run from the repository root with the pinned prerequisites. Rust, Foundry and sandbox checks use Linux/WSL2; use native pnpm for web checks (Windows pnpm on Windows). These local checks do not deploy, sign or call paid inference.
- The commands below are shared with the public page. After-activation reads require the real trusted deployment record and locally built executor artifact. Set GATE_ACCOUNT and PROPOSAL_ID from actual observed deployment/proposal values in the Linux/WSL2 shell, not spike values; select the actual RPCs via NEAR_RPC_URL and BASE_SEPOLIA_RPC_URL when necessary. The public testnet RPC defaults need no signing key.

\`\`\`sh
${commands}
\`\`\`

- Inspect audit warnings and ETAs as well as the exit status. Audit uses trusted local artifacts and the committed deployment record; it is not independent CVM appraisal or proof of those inputs' provenance.
- Appraise the verifier before sharing private source. make attack and make upgrade are write paths requiring explicit inputs and an already-appraised verifier; a funded verifier may spend inference credits. Signing, spending and deployment require separate founder authorization.
- Sources: ${source("README.md")}, Local quickstart and Read public state; ${source("deployments/README.md")}; ${source("crates/cli/src/main.rs")}; ${source("Makefile")}; ${source("web/package.json")}.

## Repository map

- ${source("crates/gate")}: NEAR gate, governance, proposal state and exact EVM transaction encoder.
- ${source("crates/constitution-types")}: canonical certificate, proposal, manifest and source-bundle bytes shared by Rust components.
- ${source("crates/verifier")}: deterministic checks, private AI integration, evidence, identity and verdict submission.
- ${source("crates/cli")}: constitution CLI, read-only audit, receipts, verified relay and demo runner.
- ${source("contracts/")}: ConstitutionExecutor, proxy/vault demo contracts, Foundry tests and deployment script.
- ${source("rules/manifest.json")}: committed compiler pins, rule parameters and evidence corpus commitments.
- ${source("web/")}: static public page and independent public-data decoding.
- ${source("docs/PRD.md")}: scope, R1-R5, G1-G4 and limits.
- ${source("docs/technical/ARCHITECTURE.md")}: architecture, verifier stages and demo-runner operation.
- ${source("docs/technical/SMART-CONTRACT.md")}: protocol interfaces, panic codes and recorded deployment evidence.
- ${source("docs/BUILD-PLAN.md")}: dated acceptance evidence and unfinished activation/testnet work.
- Map source: ${source("docs/technical/ARCHITECTURE.md")}, Codebase structure and interfaces.

## Evidence labels

- Observed: data the page itself has read from the chain; only then may it show an explorer link. A saved hash alone is not a fresh observation.
- Recorded: a real receipt saved from an earlier run, labeled with its date; never replayed as live.
- Spike: real chain evidence from the minimal spike gate, with no verifier in the loop; not a production run.
- Local test: local unit, property, sandbox, Anvil or Docker evidence; not testnet or CVM evidence. Local time travel is not seven real days on testnet.
- Awaiting activation: the real gate/CVM configuration or activation evidence is unavailable; no placeholder deployment, fake receipts, timestamps or progress are substituted.
- Sources: ${source("docs/DESIGN.md")}, Today’s data reality, State matrix and Receipt anatomy.
`;
}
