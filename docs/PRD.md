# CONSTITUTION: product requirements

## TL;DR

- [I] One-liner: **"Steal every admin key. You still cannot violate the upgrade constitution."** Scoped claim: for the one protected proxy, no key we control (admin, governor, deployer, verifier operator, relayer) can make the proxy run an implementation that has not passed the owner-committed rules, or change those rules faster than a public 7-day timelock. The residual trust (MPC network, TEE vendors, declared rule coverage) is stated in [Risks and guarantees](#risks-and-guarantees).
- [I] Mechanism: an EVM proxy's upgrade authority belongs to an address derived through NEAR Chain Signatures for a NEAR **gate** contract whose account has no full-access keys. The gate asks the MPC network to sign one exact upgrade transaction only after an attested deterministic verifier certifies the proposal against a committed rule set. A private red-team AI can only veto, and only with a counterexample that the deterministic verifier reproduces.
- [V] Status 2026-10-26: scope locked by the founder in brief 15 (source below). Planning docs only. No application code exists before 2026-11-01T00:00:00Z. Nothing is deployed, nothing is audited.
- [I] Primary success condition: on the final submission, a judge can (1) watch or run a hidden-drain V2 being rejected with a receipt while the Base Sepolia proxy stays unchanged, (2) watch or run an honest V2 being certified, MPC-signed and executed through `upgradeAndCall` with real explorer links, and (3) see a governance rule change refused before its 7-day delay ends, with every claim traceable to a receipt.
- [V] Tracks: AI Security & Defense (`security`, primary), Privacy & Private AI (`privacy`, only for the genuinely private analysis path), Open Innovation (`open`, backup). Multiple track ids per entry are allowed; award stacking is UNVERIFIED (EVENT.md).

## Problem, users and buyer

### Evidenced pain

- [V] Proxy upgrade authority is a recurring single point of failure when a key is compromised:
  - Zoth, 2025-03-21: `upgradeToAndCall` after deployer key compromise, about USD 8.4M (https://research.blockscope.co/zoth-vault-breach).
  - Humanity, 2026-06-08: implementation replacement and ProxyAdmin takeover, over USD 21M known ETH proceeds (https://www.humanity.org/hincidentupdate).
  - Solv, 2026-07-13: malicious minting-proxy upgrade, no loss amount stated (https://governance.solv.finance/t/btc-on-bnb-smart-chain-security-update/56).
  Sources recorded in brief 15 on 2026-10-25; amounts are the publishers' statements.
- [V] Hosted OpenZeppelin Defender, a common upgrade-approval workflow, sunset 2026-07-01 (brief 15). Teams lost a familiar managed control point.
- [V] Illia Polosukhin on X, 2026-10-22: security must move from API keys and access tokens to cryptographic models; 2026-10-23: formal verification with private open-weight models avoids uploading code to a third party (quoted in brief 15).
- [I] Existing workaround: multisig plus timelock on the ProxyAdmin. That moves the problem to "enough signers approve", which is exactly what social engineering, malware on signer machines and key theft attack. A multisig checks *who* approved, not *what* the new code does.

### Who

| Role | Job | Pain today |
| --- | --- | --- |
| Protocol security lead (primary buyer) | Ship upgrades fast, including emergency patches, without one stolen key being able to replace the contract | Upgrade safety depends on signer hygiene and manual review under pressure |
| Protocol engineer (primary user) | Propose an implementation and get a fast, explainable verdict | Review is slow, manual and leaks unreleased fixes to more people |
| DAO / governance | Change the rules of what upgrades are allowed, visibly | Governance itself can be captured or rushed |
| Depositors and integrators | Know which code can and cannot become live | Must trust the admin |
| Hackathon judge (human, Claude Code, Codex) | Verify claims in minutes | Most demos show mock success states |

[I] The commercial unit is "a protected proxy": one gate deployment, one committed rule set and one verifier allowlist per protected contract. The demo protects one vault on Base Sepolia.

## Insight and necessity

[I] Move the upgrade key from *people* to a *constitution*: a contract that can sign for the proxy, but only for implementations that satisfy machine-checkable rules the protocol committed to in advance.

Why each unusual primitive is load-bearing:

| Primitive | Job in CONSTITUTION | Remove it and... |
| --- | --- | --- |
| NEAR gate contract | Holds rule-set hash, verifier allowlist, timelock queue, proposal state; decides whether to request a signature | Some server or person decides when to sign; one stolen credential bypasses the rules |
| Chain Signatures (MPC) | The EVM upgrade key exists only as a threshold key that signs on behalf of the gate account | The gate would need a private key somewhere; whoever holds it bypasses the gate |
| Attested verifier (TEE) | Runs deterministic checks on private source and signs a certificate with a key that only exists in measured code | Certificates come from an operator who could lie or be hacked |
| Private red-team AI (NEAR AI Cloud Confidential TEE tier) | Searches for exploit sequences in unreleased code without disclosing it; can only veto | Either no adversarial search, or unreleased patch code leaves the trust boundary |
| EVM `ConstitutionExecutor` | On-chain last line: certificate id single use, deadline, new implementation code hash, expected current implementation | A signed but withheld transaction could land after expiry or against a changed state |
| 7-day public timelock | Rules, verifier allowlist and authority migration change only after public delay | The governor key becomes an instant bypass |

[I] When a simpler solution wins: a protocol that is immutable, or whose upgrades are rare and fully reviewed by an independent auditor plus a well-run multisig, may not need this. CONSTITUTION is for teams that upgrade often, ship emergency patches, or cannot rule out signer compromise. It complements, not replaces, audits.

## Never-cut demo

Editorial target: about 3 minutes [I]. [U] No official video duration limit was found in the rules (EVENT.md section 7). A public X post by a human with media mentioning NEAR HACKATHON is mandatory [V].

1. **"Take it."** A QR code on screen encodes a disposable, testnet-only NEAR key that holds the demo protocol's *governor* role (the closest thing this protocol has to an admin). Never a production key, never a NEAR full-access key of the gate, the MPC-derived EVM key or any founder wallet. (F9, F10)
2. **The attack.** The judge, using that key, proposes `VaultV2Drain`: it looks like a routine fee update but contains a hidden path that lets one address withdraw the whole vault. (F1, F9, F10)
3. **Rejection.** The private red-team AI proposes a counterexample; the deterministic verifier reproduces it ("address X withdrew 100% of the vault"); the gate records a rejection receipt; the MPC signer is never called; the Base Sepolia explorer shows the proxy implementation unchanged. (F5, F6, F8)
4. **Honest upgrade.** `VaultV2Honest` passes every rule; the gate requests the Chain Signature; the relayed transaction executes `ProxyAdmin.upgradeAndCall` via `ConstitutionExecutor` on Base Sepolia; explorer link and receipt shown. (F3, F4, F8)
5. **Even governance waits.** The governor key schedules a rule change and immediately tries to apply it: the gate refuses (`E53_TIMELOCK_NOT_ELAPSED`) with a real testnet receipt. A rule change scheduled at activation and matured after 7 real days is shown only if it actually matured (F11, P1). A local time-travel test is labeled "local test", never presented as testnet evidence.
6. **Business.** Buyer, pricing hypothesis, next 90 days, without invented customers or revenue.

Fallback order if live inference fails during recording: show the real rejection receipt from the rehearsal run with its timestamp and say so on screen. Never replay a fixture as live.

## Features

Priority: P0 = required for the final submission and never-cut demo; P1 = strongly wanted, cut only by recorded decision; Stretch = only after the core gate in BUILD-PLAN. Interfaces are specified in `docs/technical/SMART-CONTRACT.md` and `docs/technical/ARCHITECTURE.md`; this section fixes behavior.

### F1. Proposal lifecycle in the gate (P0)

- Outcome: anyone registers an upgrade proposal for the protected proxy, subject to D3 (resolved 2026-10-26): a refundable credit reservation of `proposal_deposit` and at most one active proposal per account. The gate drives each proposal to exactly one terminal state.
- Inputs: `new_impl` EVM address; `new_impl_codehash` (32 bytes, keccak256 of runtime bytecode); `expected_current_impl`; `init_calldata` (bytes, may be empty, at most 1024); `source_commitment` (sha256 of the private source bundle). Chain id, proxy and executor are fixed gate configuration and are checked against the certificate.
- States: `Pending -> SignRequested -> Signed`, `Pending -> Rejected`, `Pending -> Expired`, `SignRequested -> SignFailed -> (retry_sign) SignRequested`, `Signed -> (bump_fee) SignRequested`. `Signed` is terminal for the gate; EVM inclusion is observed in receipts, not trusted state.
- Rejections at proposal time: account already has an active proposal (`E21`), credit does not cover reservation plus non-refundable storage fee or their sum overflows (`E11`), init calldata over 1024 bytes (`E22`), zero address (`E23`), the same proposer already has the same `(new_impl_codehash, init_calldata)` active (`E24`; another account's identical proposal does not block you), nonzero attached deposit (`E61`; proposal storage is paid from prepaid credit so function-call keys attach zero, and a new credit account's first deposit also pays an entry fee), gate liquid balance would not cover everything it owes users after the write (`E12`). Only the reservation is refundable. The proposal binds the rule-set hash active at creation; a certificate for any other hash is refused.
- Acceptance: every transition and every rejection reachable by a test; no path to `SignRequested` without a valid certificate (see F3); expired proposals cannot be certified.
- Proof surface: gate events, CLI `status`, receipt.

### F2. Constitution governance and timelock (P0, real 7-day maturation proof P1)

- Outcome: the rule set, verifier allowlist, gate parameters and any authority migration change only through a public queue with a fixed 7-day minimum delay that no role can shorten.
- Roles: `governor` may only schedule; `guardian` may only cancel pending changes, pause signing or remove a verifier immediately; nobody may apply before `eta`; anyone may apply after `eta` (D17).
- Change kinds: `SetRuleSet(rule_set_hash, manifest_uri, ai_required)`, `AddVerifier(pubkey, release_hash, attestation_uri)`, `RemoveVerifier(pubkey)`, `SetParams(params)`, `SetGovernor`, `SetGuardian`, `MigrateAuthority(new_signer)`, `Unpause`.
- Exceptions: `RemoveVerifier` and a global `pause_signing` may take effect immediately when invoked by guardian, because they can only reduce what the gate signs (fail closed). Unpause is timelocked.
- Rejections: apply before `eta` (`E53_TIMELOCK_NOT_ELAPSED`), unknown change (`E52`), not pending (`E54`), scheduler is not governor (`E50`), canceller is not guardian (`E56`), rule-set change while signatures are outstanding (`E55`, D13). The 7-day delay is a constant; no change kind alters it.
- Acceptance: unit tests for each rejection; local time-travel test applies after `eta` (labeled local); testnet receipt of an early-apply refusal; P1 testnet receipt of a matured apply.

### F3. Exact upgrade transaction and Chain Signature (P0)

- Outcome: for a certified proposal the gate itself builds the EIP-1559 transaction calling `ConstitutionExecutor.execute(...)`, hashes it, and requests an MPC signature; the caller never supplies a payload hash.
- Bound fields: chain id, `to` = executor, `value` = 0, selector and every argument of `execute`, EOA nonce assigned by the gate, gas limit and fee caps from timelocked parameters.
- States: signature stored on the proposal; signed raw transaction emitted in an event so anyone can relay it.
- Rejections: certificate invalid, expired or signed by a non-allowlisted verifier; any certificate field differing from the proposal or configuration; signing paused; AI review not `Clean` when required. MPC failure moves the proposal to `SignFailed` with its EVM nonce still reserved; `retry_sign` signs the identical bytes.
- Fee bump: re-sign the same nonce and calldata with higher fees up to the timelocked cap, so a stuck transaction cannot be replaced by different calldata.
- Acceptance: test vectors prove the gate's signing hash equals an independent EVM library's hash for the same transaction; mutation tests show any changed field yields a different hash; spike (a) proves an MPC signature recovered to the derived address lands on Base Sepolia.

### F4. EVM side: protected vault, proxy and executor on Base Sepolia (P0)

- Outcome: one `TransparentUpgradeableProxy` in front of `VaultV1`; its ProxyAdmin is owned by `ConstitutionExecutor`; the executor accepts calls only from the MPC-derived EOA.
- Executor checks, in order: caller is the authorized EOA; `certificate_id` not used; `block.timestamp <= deadline`; `new_impl.code.length > 0` and `new_impl.codehash == expected`; tracked current implementation equals `expected_current_impl`. It marks the certificate used and records the new implementation **before** calling `ProxyAdmin.upgradeAndCall`, following checks-effects-interactions; an initializer revert atomically rolls those effects back.
- Demo implementations: `VaultV1`, `VaultV2Honest` (adds a feature with append-only storage), `VaultV2Drain` (hidden drain path, see DEMO notes in BUILD-PLAN phase 3).
- Acceptance: Foundry tests for each executor revert; a fork or local test proves only the executor can upgrade; testnet receipts for deploy and one real upgrade recorded in SMART-CONTRACT.

### F5. Attested deterministic verifier (P0)

- Outcome: a verifier running inside an attested environment receives the private source bundle, compiles it with the pinned compiler, checks that the resulting runtime bytecode hash equals the proposal's `new_impl_codehash`, runs the committed rules and signs a `Certificate` or a `Rejection`.
- Rules in the demo rule set (manifest fields in ARCHITECTURE "Rule-set manifest", reason codes in SMART-CONTRACT "Certificate"):
  - R1 storage layout: new layout is an append-only extension of the current implementation's layout, using compiler storage-layout output for the deployed current artifact.
  - R2 solvency property: bounded symbolic checks and seeded fuzzing of two fixed actors' deposit/withdraw handlers enforce aggregate solvency, not per-depositor withdrawability. The verifier supplies the token/reference definitions and checks the tested candidate runtime hash and implementation-slot preservation. Bounds and measured limits are committed in the manifest and ARCHITECTURE.
  - R3 opcode and slot filter: reject `SELFDESTRUCT`, `CALLCODE`, `DELEGATECALL`, literal ERC-1967 implementation/admin slots and their ±1 neighbors, embedded ERC-1967 ASCII labels, and literal Foundry/Halmos cheatcode addresses. Filters are filters, not proofs: runtime-computed storage slots or cheatcode addresses can evade them.
  - R4 initializer allowlist: `init_calldata` empty or matching a committed selector and argument schema.
  - R5 bounded differential replay: a committed corpus of demo transactions replayed against current and new implementation from a pinned prestate; outputs and state must match except declared changes.
- Outputs: signed certificate or signed rejection with `reason_code`, `rule_id` and an evidence hash pointing to a reproducible trace.
- Acceptance: each rule has a passing and failing fixture; tampered source bundle (hash mismatch) is rejected before rules run; certificate signature verifies in the gate test.

### F6. Private red-team AI veto (P0)

- Outcome: the verifier sends the private source and rule descriptions to a NEAR AI Cloud model in the **Confidential TEE** tier only, asks for exploit candidates as executable test sequences, and runs every candidate deterministically.
- Policy: a candidate that reproduces a rule violation produces a rejection whose evidence includes the AI response hash and attestation verification result. A candidate that does not reproduce is recorded and ignored. AI output can never move a proposal toward certification.
- Liveness: AI review is `Required` in the demo rule set. AI timeout or error produces `ReviewIncomplete` (not certified, retryable before expiry), never a silent pass. The rule set may later commit to a different policy only via F2.
- Privacy limit: the implementation's bytecode becomes public when deployed; the source, comments, tests and AI conversation stay inside the TEE boundary.
- Acceptance: model id checked against the Confidential TEE list at runtime; response verification is executed, not assumed; tests for timeout and non-reproducing candidate.

### F7. Authority lockdown and bypass audit (P0)

- Outcome: a read-only `constitution audit` check that anyone can run proves the claim's preconditions from public state: gate account has zero full-access keys and no self-upgrade path outside F2; derived EOA equals the executor's authorized caller; executor owns ProxyAdmin; ProxyAdmin is the proxy's admin (ERC-1967 admin slot); active rule-set hash and verifier allowlist match the committed manifest.
- Acceptance: audit fails loudly on each violated precondition in local tests; testnet audit output recorded after the activation ceremony.

### F8. Receipts (P0)

- Outcome: one canonical JSON receipt per proposal plus a human view: proposal id, inputs, rule results, AI review outcome, certificate or rejection signature, NEAR transaction hashes, signed EVM transaction hash, Base Sepolia inclusion status and explorer links, proxy implementation before and after.
- Rule: links and hashes appear only after they are observed; pending items say pending.
- Acceptance: receipt for the drain proposal shows no signature request; receipt for the honest proposal shows the matching Base Sepolia transaction and new implementation.

### F9. CLI and judge commands (P0)

- Planned interface (does not exist yet): `constitution propose`, `status <id>`, `relay [<id>]`, `receipt <id>`, `audit`, `govern schedule|apply|cancel`, `rules hash <manifest>`, `verifier appraise <url>` (full table in ARCHITECTURE); `make attack` and `make upgrade` wrap the two demo flows.
- Judge mode: works with the disposable demo governor key and a public read-only path that needs no key (reads receipts and audit).
- Acceptance: README "do X, see Y" steps run in a fresh environment with documented prerequisites; every error prints the cause and the next step.

### F10. Public web page (P0)

- Outcome: one page with three surfaces: Attack (submit `VaultV2Drain`), Honest upgrade (submit `VaultV2Honest`), Receipts (live status and history), plus the audit panel and the QR "take it" key.
- States: idle, submitting, verifying (real verifier stage reported), AI review, certified, signing, relaying, executed, rejected, expired, error with next step. No fake progress.
- Accessibility: keyboard operable, visible focus, readable contrast, works at 390 px and 1280 px, honors `prefers-reduced-motion`.
- Acceptance: browser test walks both flows against testnet; visual check at both widths.

### F11. Real timelock maturation on testnet (P1)

- Outcome: a harmless rule-set change scheduled at the activation ceremony is applied after 7 real days with a receipt, before the submission cutoff. Requires activation by 2026-11-07T12:00:00Z (BUILD-PLAN).

### S1. Act 2 self-repair (stretch, founder decision D4)

- An agent finds a bug in the deployed vault, writes a patch and submits it through exactly the same gate. No special path, no reduced rules. Ships only if D4 says yes and the core gate in BUILD-PLAN is met.

## Non-goals and scope cuts

- Out of scope: NEAR Intents, escrow, staking endowment, Solana or any second chain, a general marketplace, multi-proxy or multi-tenant platform, mainnet deployment, Shade Agents and Agent Market managed hosting (deprecated), UUPS support, token.
- Not claimed: full formal verification, prevention of Bybit-style incidents (that was a Safe-authorized `delegatecall` writing slot 0, not an upgrade through this mechanism), protection of contracts other than the configured proxy, protection against bugs the declared rules do not cover.
- Cut order if time runs short (preserves the never-cut demo): S1, then F11, then R5 differential replay corpus size reduced to the minimum stated in BUILD-PLAN, then web page polish (CLI remains). F1 to F10 are not cut without a founder decision.

## Success and business essentials

- [I] Buyer: security lead at a protocol with upgradeable contracts holding user funds. Users: their engineers and governance.
- [I] Pricing hypothesis, untested: per protected proxy per month, plus per-verification usage covering TEE and inference cost. No price point is claimed as validated.
- [I] Operating costs: MPC signature deposit per signed upgrade on NEAR, verifier host runtime, private inference tokens, EVM gas for the derived EOA. Exact costs recorded only after measurement (SMART-CONTRACT deployment section).
- [I] Distribution hypothesis: teams that relied on hosted Defender upgrade approvals and teams post-incident. No customer conversations have happened; none are claimed.
- [I] Next 90 days: rule-template library for common vault/token patterns; second EVM chain; independent review of gate and executor; design partner pilots on testnet.
- Success measurement for the hackathon: the three proofs in the TL;DR exist as real receipts; README steps reproduce them.

## Evaluation and evidence

[V] Historical NEAR judging guidance names technical implementation, design, impact, originality and sustainability; current weights are UNVERIFIED (EVENT.md section 3). Claude Code and Codex are named judges; their prompts and sandbox are UNVERIFIED.

| Criterion (weight UNVERIFIED) | Artifact that proves it |
| --- | --- |
| Technical implementation | Gate, executor and verifier tests; testnet receipts for rejection and upgrade; audit output |
| Design | Public page with real states, receipts a non-expert can read |
| Impact | Incident evidence above; buyer and job; Defender sunset |
| Originality | Prior-art table below with the exact delta |
| Sustainability | Pricing hypothesis, cost drivers, 90-day plan |
| AI judges | README "do X, see Y", deterministic `make` targets, transparent fixtures, labeled local vs testnet evidence |

## Prior art and honest differentiation

| Alternative | What it already does [V per brief 15, 2026-10-25] | Our delta [I] |
| --- | --- | --- |
| Lit Actions / PKPs | TEE/threshold conditional signing | Upgrade authority owned by a NEAR contract with committed rules and timelock, targeting any EVM chain |
| Venn Safe Guard | Transaction screening on Safe | Gate checks implementation semantics, not transaction risk scores |
| Certora AI Composer | AI-assisted formal verification | AI is veto-only and reproduced; verification gates the actual signing key |
| OpenZeppelin Upgrades Core | Storage-layout validation tooling | We use the same class of check, but its result controls the key |
| Hypernative, Forta, Tenderly | Monitoring, simulation, alerting | Prevent the signature instead of alerting after |
| Doppleganger, Veridise | Security tooling and audits | Complementary; their findings could become committed rules |
| Turnkey | Policy-controlled key infrastructure | Our policy is code analysis of the target implementation, owned by a contract |
| Phala / Turbine | TEE execution and agents | Possible verifier host, not a competitor in function |

[V] A Colosseum Crypto World's Fair 2026 dataset snapshot exists (ideation `intel/colosseum/crypto-worlds-fair-2026.md`, inspected 2026-10-26 by another worker). Its rows have no chain field and empty track arrays; "NEAR 11" is an API facet count, not verified project tags. [I] We make no claim about the absence of competitors and no "first ever" claim.

## Risks and guarantees

What the product guarantees, if the stated trust assumptions hold [I]:

- G1. Against implementations that cannot write arbitrary proxy storage, the proxy changes implementation only through `ConstitutionExecutor`, which only accepts transactions signed for the gate's derived EOA. This is conditional: an implementation capable of computed-slot `SSTORE` can bypass that upgrade path; the executor's tracker is not an on-chain storage-write guard.
- G2. The gate requests a signature only for the exact executor call bound to a certificate from an allowlisted verifier over the active rule-set hash.
- G3. Rules, verifier allowlist and authority migration change only after a public 7-day delay; guardian actions can only reduce authority. Liveness limit: EVM nonces are assigned at certification, so a certified transaction that nobody ever mines delays later ones until someone relays it (permissionless; a reverted transaction still consumes the nonce). Delay only; no signature is ever created without a certificate.
- G4. AI output can only cause rejection, never certification.

Trust assumptions and limits:

- MPC network honesty and availability (threshold signing inside TEEs; not post-quantum per NEAR blog 2026-10-23, brief 15).
- TEE vendor and attestation chain for the verifier and the inference provider. [V] Attestation proves which code ran on which input and output, not that the conclusion is correct or that RPC data was honest.
- Rules protect only what they declare. A dormant backdoor outside the declared properties and bounds can pass.
- Opcode and selector filters are filters, not proofs. R3 catches committed literals and labels, not every computed-slot `SSTORE`; R2 checks slot preservation only along its bounded deposit/withdraw sequences. Other entry points and dormant triggers are outside that search. Aggregate solvency is not per-depositor withdrawability; bounded symbolic checks are bounded.
- Environment-dependent behavior (review N7): R2 runs under one fixed block environment (only the chain id is pinned), R3 does not forbid block-environment opcodes (`TIMESTAMP`, `NUMBER`, `BASEFEE`, `BLOCKHASH`, `ORIGIN`, ...) because a byte-scan false-positives on Solidity's trailing metadata (the honest `VaultV2Honest` contains `0x48` after `INVALID`), and R5 replays only the committed corpus at the real header. A candidate whose drain depends on a later time or block value can therefore pass; this is a stated limit, not covered by the guarantee.
- AI review is nondeterministic; its find rate is reported from rehearsal runs, not promised. A Clean result means the provider answered with a well-formed receipt and nothing was reproduced locally; the receipt is shape-checked in the verifier, not cryptographically verified in Rust (review N8). The AI replay uses the SHA-256 precompile address as the victim account, so it can find attacker-side counterexamples only (N13). The sidecar and verifier share one network namespace by deployment assumption that the tree does not enforce, and a simulator-backed attestation key must never be registered (N12).
- Liveness: MPC, verifier or inference outages delay upgrades; they never cause a signature without certification.

Owner-tagged risks are tracked in `docs/OPEN-DECISIONS.md` (decisions and BLOCKERS).

## Sources and glossary

| Source | Date read | Use |
| --- | --- | --- |
| `C:/Users/HP Victus/Documents/personal-projects/hackathon/ideation/events/near-hackathon-2026/sources/15-constitution-brief.md` | 2026-10-26 | Product direction, incidents, honesty constraints, prior art |
| `C:/Users/HP Victus/Documents/personal-projects/hackathon/ideation/events/near-hackathon-2026/EVENT.md` | 2026-10-26 | Rules, deadlines, submission fields |
| https://nearhackathon.com/HACKATHON.md | refreshed live 2026-10-26, see INTEGRATIONS | Official rules |
| `docs/technical/*` | this foundation | Exact interfaces and live-verified versions |

Glossary:

- **Gate**: the NEAR contract that holds the constitution and requests signatures.
- **Constitution**: the active rule-set hash, verifier allowlist and parameters, changeable only through F2.
- **Derived EOA**: the Base Sepolia address whose key is the MPC threshold key derived for (gate account, path).
- **Executor**: `ConstitutionExecutor`, the EVM contract that owns ProxyAdmin and enforces certificate binding on-chain.
- **Certificate**: a verifier-signed statement that a proposal passed the active rule set, bound to all fields listed in SMART-CONTRACT.
- **Receipt**: the canonical record of a proposal's outcome with observed transaction links.
- **Rule set**: the committed manifest of rule templates, parameters, compiler pins and replay corpus, identified by its hash.
