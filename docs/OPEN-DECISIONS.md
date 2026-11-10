# CONSTITUTION: open decisions and blockers

## TL;DR

- [V] Founder chose stack **Option C (Phala TDX CVM verifier + Rust-first operations + Astro static page)** on 2026-10-26 in this planning session (D1). ARCHITECTURE is written against it.
- [V] Founder resolved on 2026-10-26: attestation appraisal off-chain first, on-chain DCAP as P1 (D5); spam control by refundable deposit plus one active proposal per account (D3); verifier host Phala with the USD 20 verified-account credit, OutLayer only as a separately decided fallback (D2); archive RPC Validation Cloud free tier (D10); public repo on GitHub `Dylansius11`, X post from the founder's own account (D9).
- [V] Founder answers on 2026-11-02 (UTC): the founder creates the GitHub repo personally and the lead commits locally only until told to push (D9); the lead creates NEAR testnet accounts via the faucet with keys in the repo-local gitignored `.secrets/` (B1, done); D7 waits for the founder's logo and references; D8 deferred between Cloudflare Pages and Vercel.
- Open founder rows: D8 hosting, D14 funding amounts, D18 verifier uptime budget (after spike P1.6). D7 visual direction resolved 2026-11-07. Open organizer questions: D16. D4 repair scope is approved (2026-11-05); implementation remains gated on core completion.
- [V, founder decision 2026-11-10] D20 replaces the fixed seven-day design with an initialization-selected, immutable governance delay of 1–30 whole days, default three. D21 requires user-facing E2E and audit before final lockdown, with fresh explicit approval before gate key deletion.
- Build opened 2026-11-01T00:00:00Z; build work started 2026-11-02. Remaining BLOCKERS below are founder-owned manual prerequisites.

Founder-owned rows are never silently decided by agents. Status: open, blocked or resolved. Lead-owned rows record the reasoning so the founder can overrule. Each row has its explanation below.

## Decisions

| Id | Decision | Owner | Status | Needed by | Resolution |
| --- | --- | --- | --- | --- | --- |
| D1 | Technology stack | founder | resolved 2026-10-26 | before ARCHITECTURE | Option C |
| D2 | Verifier host | founder | resolved 2026-10-26, confirm in spike P1.6 | P1.6 | Phala Cloud CVM on USD 20 credit; OutLayer fallback needs a new decision |
| D3 | Proposal spam control | founder | resolved 2026-10-26 | P2.2 | Refundable credit reservation + one active proposal per account |
| D4 | Ship Act 2 self-repair | founder | resolved scope 2026-11-05; not implemented | after core gate | Opt-in, semi-autonomous repair of rejected upgrade proposals; developer approval before deployment/resubmission; S1.1 to S1.4 |
| D5 | Verifier key registration appraisal | founder | resolved 2026-10-26 | P2.4 | Off-chain appraisal + 7-day timelock; on-chain DCAP is P1 |
| D6 | Gate code mutability | lead | resolved 2026-10-26 | P2.1 | Immutable gate; authority migration only via timelocked change |
| D7 | Visual direction for the public page | founder | resolved 2026-11-07 | P3.4 | Founder brand (`asset/` logos) and reference video, elevated; recorded in `docs/DESIGN.md` sections 2 to 4 |
| D8 | Hosting for the public page | founder | resolved 2026-11-10 | P3.6 | Cloudflare Pages Free; static page only, no paid Workers or domain purchase |
| D9 | Public repo and X accounts | founder | resolved 2026-10-26, refined 2026-11-02 | P4 | GitHub `Dylansius11`, founder creates the repo; lead commits locally until asked to push; founder's X |
| D10 | Archive / debug and proof RPC for replay | founder | resolved 2026-10-26, refined 2026-11-05 | P1.7 | Validation Cloud free tier for archive/trace; Base public RPC for recent-block proofs; no QuickNode purchase |
| D11 | NEAR AI Cloud inference access | founder | in progress | P2.6 | Founder applied for hackathon credits |
| D12 | EVM authority shape | lead | resolved 2026-10-26 | P1.3 | `ConstitutionExecutor` owns ProxyAdmin |
| D13 | Effect of rule changes on already-signed transactions | lead | resolved 2026-10-26 | P2.3 | Valid only until their short deadline; disclosed |
| D14 | Gas, fee caps and funding amounts | founder (funding), lead (caps) | open | P1.4 | Set from observed Base Sepolia fees in spike |
| D15 | Rust toolchain pin | lead | resolved 2026-10-26, confirm in spike | P1.1 | Host 1.99.1; gate via cargo-near reproducible image |
| D16 | Organizer questions | founder | open | before submission | Draft below, not sent |
| D17 | Demo key exposure model | lead | resolved 2026-10-26 | P3.3 | QR key = governor (schedule-only) function-call key |
| D18 | Verifier key lifetime vs CVM budget | founder (budget), lead (design) | open | P1.6 | Ephemeral key; CVM runs continuously from activation to 2026-11-18T23:59:00Z |
| D19 | EVM target for contracts and the rule-set manifest | lead | resolved 2026-11-02 | P1.3, P2.5 | `prague` everywhere; Halmos 0.3.3 cannot execute Osaka CLZ |
| D20 | Deployment-selected governance delay | founder | resolved 2026-11-10; local cutover implemented | P2.1, P2.4, P2.7, P2.8, P3.5, F11 | Option A: 1–30 whole days selected at initialization, default 3, immutable afterwards; no old-state migration |
| D21 | Pre-lockdown acceptance and final ceremony permission | founder | resolved process 2026-11-10; not exercised | P1.6, P2.8, P3.2, P3.5, P4.1 | Rehearse E2E as a user, review/audit and fix first; explicit fresh authorization before irreversible gate key deletion |
| D22 | KMS encryption-key freshness during disposable validation | founder (risk acceptance), lead (verification) | resolved for bounded validation 2026-11-10; final deployment policy open | P1.6, P2.8 | Official legacy signature with independently pinned root and app binding; local receipt time is not signed KMS freshness |

### D1. Technology stack

Context: the product needs a NEAR contract that controls an EVM upgrade key, an attested verifier that can run real Solidity tooling, private inference, a CLI and one public page. All options shared the same security core; they differed in where the verifier runs and which language the operations tooling uses. All versions were observed live on 2026-10-26 (sources in ARCHITECTURE "Approved tech stack").

Shared core in every option: Rust gate (near-sdk 5.29.1, cargo-near 0.22.0, omni-transaction 0.5.0 EVM-only), immutable gate account, Chain Signatures `v1.signer-prod.testnet`, Foundry 1.8.6, solc 0.8.38, OpenZeppelin Contracts 5.6.1 Transparent proxy, `ConstitutionExecutor`, NEAR AI Cloud `z-ai/glm-5.3-flash` (Confidential TEE tier).

| Option | Verifier | Operations and page | Trade-off |
| --- | --- | --- | --- |
| A (lead recommendation) | Phala Cloud dstack Intel TDX CVM running pinned solc, Foundry, Halmos 0.3.3, OZ upgrades-core 1.47.0 | TypeScript CLI (near-api-js 7.4.0, viem 2.57.11, Commander 15.0.0) and Vite 8.3.5 + React 19.3.1 page | Full checks; single off-chain language; needs Phala account |
| B | OutLayer WASI program (revm 43.0.3, `wasm32-wasip2`, hash-pinned) | Same as A | NEAR-native, no extra cloud account; no solc/Halmos inside the job, 180 s cap, WASI port unproven, trusts OutLayer worker operator |
| C (selected) | Same as A | Rust CLI (near-api 0.8.6, alloy 2.5.0) sharing types with the gate; Astro 7.4.2 static page with viem for public reads | Full checks; one canonical type crate for gate, verifier and CLI; more CLI ergonomics work; needs an independent viem encoding oracle |

Founder response, 2026-10-26: "C: Phala CVM + Rust-first". Rationale recorded [I]: Rust-first removes a second canonical encoder for certificates and transactions (gate, verifier and CLI share `constitution-types`), which is the most security-sensitive byte format in the system. Affected docs: ARCHITECTURE, INTEGRATIONS, SMART-CONTRACT, BUILD-PLAN, CLAUDE.md.

### D2. Verifier host

Context: Halmos and solc need ordinary processes and a filesystem; OutLayer's current WASI P2 sandbox documents HTTP and memory, no processes (research note 2026-10-26). Founder asked for a fully free first option.

Evidence [V, 2026-10-26]: https://phala.com/pricing states "Verified accounts get $20 in CVM credits"; https://docs.phala.com/phala-cloud/getting-started/sign-up-for-cloud-account states a card is checked with a USD 1 charge refunded immediately and some new workspaces get a welcome credit "while the offer lasts"; https://cloud.phala.com/about/pricing gives `tdx.large` 4 vCPU / 8 GB at USD 0.232 per hour compute plus storage USD 0.000139 per GB-hour with a 20 GB minimum, and phala.com/pricing advertises CVMs "from $0.06/hour". [I] USD 20 covers about 86 hours of `tdx.large` compute; which instance size Halmos needs is [U] until spike P1.6.

Options presented: (1) Phala credit with a hard USD 20 cap; (2) Phala credit, OutLayer as fallback; (3) OutLayer fully free with R2 downgraded from bounded symbolic checks to concrete property fuzzing and OutLayer operators added to the trust base.

Founder response, 2026-10-26: "Phala credit, OutLayer cadangan". Resolution: Phala Cloud CVM; run it only during spikes, rehearsals, recording and the judging window 2026-11-16 to 2026-11-18; delete it after judging. Spending beyond the credit needs a new founder approval. If the credit is not granted, the fallback to OutLayer is a **new decision**, not automatic, because it narrows R2 and changes the trust statement.

[V, founder screenshot and primary-source recheck 2026-11-05] The founder's card-onboarding offer displays **USD 5**, not USD 20; receipt of either amount is not yet confirmed. https://phala.com/pricing still advertises USD 20 for verified accounts, but https://cloud.phala.com/about/billing says welcome-credit eligibility and amount are workspace-specific and shown before claim. Do not assume a further USD 15 will arrive. The screenshot also shows auto-topup enabled for USD 20; switching it off is not a hard spending cap because card-backed post-paid usage can continue after the balance is exhausted. No card, top-up, deploy or new spending authorization is implied.

[V, dashboard observation 2026-11-10] The authenticated workspace shows Balance USD 20, Gifted credits USD 0, no payment method and auto-topup disabled; Compute shows no deployment. The draft Small (`tdx.small`, 1 vCPU / 2 GB) plus 20 GB disk estimate is USD 0.06074/hour. Published storage arithmetic gives USD 0.06078/hour, used conservatively for planning; neither is an observed bill. The founder restricts deployment to Small, with no automatic resize, top-up or spending beyond approved bounds.

### D3. Proposal spam control

Options presented: refundable deposit; per-account rate limit; deposit plus one active proposal per account. Founder response, 2026-10-26: deposit plus one active proposal per account.

Implementation consequence [I, lead]: NEAR function-call access keys cannot attach deposits, and the public demo key (D17) is a function-call key. The deposit is therefore an internal **credit reservation**: anyone tops up an account's credit with `deposit_credit` (attached deposit), `propose` reserves `params.proposal_deposit` from that credit, and the reservation returns to credit when the proposal reaches a terminal state. The non-refundable proposal storage fee is also prepaid in credit and permanently debited at proposal creation; `propose` attaches zero. Credit must cover their checked sum, not just the refundable reservation. One active proposal per account is enforced in `active_by_account`. The reservation amount is a timelocked parameter set at activation (D14). Residual risk: many funded accounts can still queue verifier work; the verifier service rate-limits its off-chain queue per account and this is disclosed.

### D4. Act 2 self-repair

Historical decision (2026-10-26): decide at the core gate. [V, founder approval 2026-11-05] The founder selected **semi-autonomous repair of a rejected upgrade proposal**, not repair of the currently active vault. The UI must offer opt-in repair. This resolves the feature choice early; it does not waive the core-gate dependency or the stretch cut trigger in BUILD-PLAN.

[I, approved scope] NEAR AI Cloud generates a proposed patch using the private inference requirements already applied to AI review. Our backend validates untrusted output and runs compilation/checks; inference is not hosted on the VPS/CVM. A developer reviews the diff and explicitly approves before deploying the candidate or submitting a new proposal. The rejected proposal is immutable; the patch gets a new source commitment, runtime code hash and proposal id and passes the same gate, rules, independent veto stage and certificate checks.

[I, bounded implementation] One candidate per explicit repair request, one heavy verifier job at a time, no autonomous retry loop. RPC failures, timeouts and incomplete reviews do not trigger source repair. Repair cannot edit the committed rules, trusted harness or verifier configuration. No new gate bypass, server-held user deployment key, automatic TEE-to-VPS downgrade, payment integration or claim of general bug freedom is authorized by D4. Premium pricing is an unvalidated product hypothesis, not an implemented billing feature.

Tracking: S1.1 to S1.4 in BUILD-PLAN; PRD S1 defines the product contract. Production repair is not implemented and no testnet repair receipt exists. The founder-authorized synthetic Solidity review/patch spike on 2026-11-06 used two verified GLM completions, cost USD 0.00166912 in reported billing, and passed five local behavioral checks plus a before/after Anvil exploit smoke; it is not an integrated proposal repair or general audit (`crates/verifier/evidence/ai/review-patch-probe-2026-11-06.json`). Paid calls, deployments and transactions still require the applicable founder authorization.

[V, founder policy approval 2026-11-06] Review has a 300 s maximum and 45,000 total completion tokens; repair generation has a 100,000 total-completion-token ceiling and the complete repair job has a 1,800 s deadline including validation and review again. Use `thinking_token_budget: null` to remove only the separate reasoning cap; reasoning remains part of completion usage. No automatic retries, new paid rehearsal, model switch or gate bypass is authorized. These are initial operational ceilings, not measured requirements, a safety guarantee or a provider USD spending cap. Repair settings remain planned until S1 is implemented.

[V, founder clarification 2026-11-10] Review/repair operational settings may be reconsidered during testing; this does not approve a particular new value, additional paid inference or weaker verification. Distinguish sidecar token/reasoning settings from manifest-committed AI model, required policy, signature kind, candidate count and timeout: manifest changes alter `rule_set_hash`. Before final initialization, tune and freeze together; after activation, a new rule-set commitment requires governance, and an image/restart can independently require verifier re-registration. Repair remains unimplemented and subject to the existing core dependency.

### D5. Verifier key registration appraisal

Options presented: off-chain appraisal plus timelock; on-chain DCAP via the `tee-verifier` contract in near/mpc; off-chain first with on-chain as P1. Founder response, 2026-10-26: off-chain first, on-chain as P1.

Meaning [I]: the verifier's Ed25519 key, its release hash and the full TDX quote, collateral and event log are published; `constitution verifier appraise` checks them off-chain; the initial key enters the gate at initialization, later keys only through `AddVerifier` with the 7-day delay. The gate does not cryptographically verify Intel evidence in P0, and every doc must say so. P1: call the near/mpc `tee-verifier` `verify_quote_with_collateral_dates` at registration if a deployed immutable verifier account and measured gas exist (research note: source exists, deployment and gas [U]).

### D6. Gate code mutability (lead)

[V] MPC derivation depends on the gate **account id** and path, not on its code (near/mpc `kdf.rs`, `common.rs`, read 2026-10-26). Anyone who can redeploy code or add a key on the gate account inherits the EVM authority. Resolution [I]: the gate exposes no code-upgrade, add-key, delete-account or arbitrary self-call method, and after activation the account holds zero access keys. Escaping a buggy gate uses `MigrateAuthority`, which after the 7-day delay signs `ConstitutionExecutor.rotateSigner(newSigner, ...)` toward a new gate's derived address. Rejected alternative: timelocked self-upgrade of gate code (research option "C" in the chain note), because a delayed arbitrary code swap does not support the headline claim. Founder may overrule; the claim must then name the code-upgrade path.

### D7. Visual direction

Founder response, 2026-10-26 (paraphrased from Indonesian): explore Awwwards and save.design for inspiration, relay browser allowed; the frontend must be as premium and semantic as possible and should amaze.

Resolved 2026-11-07: the founder supplied the brand (mark and wordmark, lavender) and a reference video (Alia site scroll-through) in `asset/` (gitignored) and asked to follow the reference, then elevate it. `docs/DESIGN.md` sections 2 to 4 record the result: paper ground, ink, flat lavender, Geist and Geist Mono, GSAP ScrollTrigger scroll scenes. Retires the October directions A to C.

### D8. Hosting

Options presented: GitHub Pages, Cloudflare Pages, Vercel Hobby (non-commercial terms). Founder response, 2026-10-26: decide later. Needed by P3.6. [I] The page is static and reads public endpoints only, so any of the three works without server secrets.

[V, founder constraint 2026-11-10] Cloudflare usage must remain on the free tier. Static Astro hosting on Pages is the proposed purpose, not running verification or AI in Workers. The founder requests an explanation before execution; no Cloudflare project, Worker, paid feature, domain purchase or deployment is authorized by this clarification alone. The founder supplied a screenshot of the signed-in Cloudflare dashboard; this is not a new agent-verified session check.

[V, founder execution authorization 2026-11-10] Proceed with Cloudflare Pages Free static hosting. The initial build is publicly served at `https://constitution-gate.pages.dev`; activation-dependent write cards remain locked. No Worker, paid resource, custom domain or backend credential is part of this deployment. Later page changes require a new static build/upload and verification.

### D9. Public repo and X

Founder response, 2026-10-26: public repo `constitution` under GitHub account `Dylansius11`, created after 2026-11-01T00:00:00Z; the founder publishes the X demo and the submission personally.

### D10. Archive / debug RPC

Founder response, 2026-10-26: Validation Cloud free tier. [V, 2026-10-26] docs list Base Sepolia archive, `debug_traceCall` with `prestateTracer`, `eth_getProof`, 50M CU/month, 120 requests/s, no card (https://docs.validationcloud.io/nodeapi/base/overview, https://docs.validationcloud.io/nodeapi/about/billing). Spike P1.7 must exercise the methods at a fixed block before R5 depends on them.

Founder-approved refinement, 2026-11-05: keep Validation Cloud for archive state and debug traces, and use `https://sepolia.base.org` for proofs via `BASE_SEPOLIA_PROOF_RPC_URL`. The endpoints must serve the same pinned Base block; verify both chain ids/headers and every account/storage proof. Public RPC rate limits and bounded proof retention are accepted hackathon liveness risks, not permission to use `latest` or skip authentication. No paid provider or node self-hosting is authorized by this decision.

### D11. NEAR AI Cloud access

Founder response, 2026-10-26: hackathon credits already requested. Grant amount and timing are [U]. No inference runs until a key exists in the founder's local `.env`. If credits do not arrive by 2026-11-04, the founder decides on a small top-up (new row).

### D12. EVM authority shape (lead)

[V] A signed EIP-1559 transaction has no deadline field (EIP-1559 spec, read 2026-10-26); a direct `ProxyAdmin.upgradeAndCall` signature withheld by a relayer stays valid until its nonce is consumed, even after the certificate expired or the rules changed. Resolution [I]: `ConstitutionExecutor` constructs the proxy itself, owns the proxy's ProxyAdmin, and accepts calls only from the derived EOA; it enforces certificate single use, deadline, new implementation code hash and expected current implementation before calling `upgradeAndCall`. Full ABI in SMART-CONTRACT.

### D13. Already-signed transactions when rules change (lead)

Resolution [I]: the executor does not know NEAR rule epochs. A signature issued under rule set N remains executable **through** its `deadline` second, at most `params.sign_validity_s` after signing. The gate refuses `apply_change(SetRuleSet)` while any SignRequested, SignFailed or Signed proposal has `now_s <= deadline_s`. A SignRequested proposal also blocks until at least 20 minutes after its latest request. Superseded-rule retry/bump is allowed only at `now_s > deadline_s`, preserves the original deadline and calldata, and therefore can only fill an expired nonce. Equality is not expiry. Disclosed in PRD risks.

### D14. Gas, fee caps and funding

Open for funding; lead caps set from observation. Founder funds: NEAR testnet accounts (faucet), the derived EOA with Base Sepolia ETH, demo-key credit. No real money involved except D2.

Observation [V, Base Sepolia, 2026-11-03, spike P1.4 stage B]: base fee 5,000,000 wei, suggested priority fee 1,000,000 wei, effective gas price 6,000,000 wei; the `execute` upgrade with `initializeV2` used 105,159 gas, an expired-deadline revert 28,900 gas; L1 data fee about 500 wei.

Lead caps for activation [I]: `gas_limit = 300000` (2.8x the observed upgrade; init calldata is capped at 1,024 bytes), `max_priority_fee_per_gas = 1000000`, `max_fee_per_gas = 50000000` (10x the observed base fee), `fee_cap = 1000000000` (ceiling for bumped fees). Worst case per signed transaction at the cap: 300,000 x 1 gwei = 0.0003 ETH.

Funding proposal for the founder [I]: 0.01 ETH to the derived EOA from the deployer at activation (at least 33 transactions at the cap, thousands at observed fees); deployer balance after the spike is 0.04698 ETH.

### D15. Rust toolchain pin (lead)

[V, 2026-10-26] Latest stable Rust 1.99.1 (2026-10-13, static.rust-lang.org channel manifest); minimum versions: near-sdk 5.29.1 needs 1.93, revm 43.0.3 needs 1.91.0, alloy 2.5.0 needs 1.94.1; cargo-near 0.22.0 removes the old 1.86 ceiling for protocol version 84+. Resolution [I]: `rust-toolchain.toml` pins 1.99.1 for the workspace; the deployed gate wasm is built with `cargo near build reproducible-wasm`, whose container image digest is recorded in SMART-CONTRACT. Fallback if the spike shows a wasm incompatibility: 1.94.1, the lowest version meeting every minimum.

### D16. Organizer questions (draft, not sent)

Official channel per EVENT.md: https://discord.com/invite/nearprotocol. Founder sends, if at all.

1. May one project receive placements in multiple tracks and the grand prize, or are awards exclusive?
2. Is testnet-only evidence (NEAR testnet + Base Sepolia) sufficient for judging a security product that must not hold mainnet funds?
3. What pre-November code, libraries and starter kits may be reused, and how should they be disclosed? (The founder already forbids any code before opening.)
4. Age, geography, KYC and payout conversion rules?
5. Is there a preferred video length for the X demo?

### D17. Demo key exposure model (lead)

Resolution [I]: the QR "take it" key is a NEAR **function-call access key** on a throwaway testnet account that holds the gate's `governor` role, restricted to receiver = gate and methods `propose`, `schedule_change` and `apply_change` (`apply_change` is permissionless anyway and only succeeds after `eta`; it is included so a judge can watch the early attempt fail). Governor can only schedule changes; it cannot cancel, apply early or sign. The guardian (founder-held, never shown) can cancel any pending change. A function-call key cannot transfer NEAR, delete the account or add keys (NEAR access-key docs, read 2026-10-26). The finite allowance bounds how many changes a holder of the public key can schedule; the founder tops it up deliberately. Constraint: one active proposal per account (D3) means simultaneous judges share one in-flight proposal; the page shows it live.

### D18. Verifier key lifetime vs CVM budget

[V, 2026-10-26] Phala deterministic `GetKey` keys are derived per application id, "the same path always generates the same key" (https://docs.phala.com/phala-cloud/key-management/get-a-key.md). They are not bound to the measured compose, so whoever controls app updates could obtain the certificate key under different code. Lead design [I]: the certificate key is ephemeral, generated in memory at boot and bound by the TDX quote (ARCHITECTURE "Verifier identity"). Consequence: a CVM restart yields a key the gate rejects until a 7-day `AddVerifier`, so the activated CVM must run without interruption from activation until judging ends (2026-11-18T23:59:00Z).

Budget arithmetic [I]: activation on 2026-11-07T12:00:00Z gives about 276 hours of runtime. At USD 0.232/h (`tdx.large`) that is about USD 64, above the USD 20 credit; at the advertised USD 0.06/h entry tier about USD 17, inside it, if that tier can run solc, Halmos and revm for the demo (unknown until P1.6).

[I, updated estimate 2026-11-05 using https://cloud.phala.com/about/instance-types and https://cloud.phala.com/about/pricing] With the minimum 20 GB disk, hourly compute plus storage is USD 0.06078 / 0.11878 / 0.23478 for small / medium / large. About 276 hours costs USD 16.77 / 32.78 / 64.80; USD 5 covers about 82 / 42 / 21 running hours respectively if spent only on that CVM. Resource fit remains unproven in TDX. Stopping removes compute charges but retains disk charges of USD 0.00278/hour; deleting removes both. Stop/start still loses the ephemeral signing key, so demo-only uptime would change the activated/judging plan and needs a new founder decision.

Options for the founder at P1.6: (a) entry tier fits: no new spend; (b) approve a top-up for a larger tier; (c) activate later and drop F11 (real 7-day maturation proof); (d) after P1 on-chain DCAP lands, permit restarts because a new key for an approved `release_hash` can be registered without the delay. Lead recommendation will follow the P1.6 measurements.

[V, founder execution authorization 2026-11-10] Proceed with private GHCR publication and disposable Small validation: 1 vCPU / 2 GB, 20 GB disk, maximum two hours and USD 0.25 allocation across this validation, no resize, card or top-up. The allocation is not a provider-enforced spending cap. Delete the disposable CVM after collecting evidence; continuous post-activation uptime remains separately unapproved. D20 replaces fixed seven-day re-registration waits with the initialized duration on new gates.

### D19. EVM target (lead)

[V, 2026-11-02] Base Sepolia runs Osaka: Azul (Osaka CLZ, EIP-7939) activated 2026-04-20 and Denim 2026-10-21 (https://docs.base.org/upgrades/overview). [V, local 2026-11-02] Halmos 0.3.3 in the verifier image fails on solc 0.8.38 Osaka output that contains CLZ: `Unsupported opcode 0x1e`, exit code 3, which the runner maps to Incomplete (fails closed); ordinary vault bytecode passes on both targets (`spikes/verifier-min` osaka probe). Resolution [I]: compile the demo contracts and pin the rule-set manifest to `evm_version = "prague"`. Prague bytecode runs unchanged on Osaka, every opcode it can contain is supported by Halmos, and the pre-check that recompiles a proposal must use the same target as its deployment. Cost: no CLZ gas savings. A proposal compiled for Osaka fails the runtime-hash pre-check (`rule_id 0`, reason 2), never passes.

### D20. Deployment-selected governance delay

[V, founder choice 2026-11-10] Select Option A, not a governable delay: the owner selects the duration when initializing its gate, default three days. The chosen value cannot be changed afterwards by any role or `SetParams`. This is a governance/authority-change delay, not a wait on ordinary certified vault upgrades, and does not add a gate code-upgrade method.

[V, founder range approval and local implementation 2026-11-10] The allowed range is **1–30 whole days**, default 3 only when omitted at initialization; explicit 0 or 31 rejects. Canonical configuration, gate validation/scheduling, callers, audit commitments, deployment schema and page copy are migrated together. The audit requires an explicitly observed field rather than inheriting the initializer default. This changes Borsh state and is for a fresh gate, not migration of an initialized old gate. A future duration change requires a new gate and delayed authority migration. Affected: P2.1, P2.4, P2.7, P2.8, P3.5 and F11; local implementation is not final testnet deployment or maturation evidence.

### D21. E2E and audit before final lockdown

[V, founder requirement 2026-11-10] Exercise the product as a user, audit/review and fix issues before the final ceremony. The founder must explicitly authorize irreversible gate key deletion at that time; validation permission is not lockdown permission. The founder reports Docker engine re-enabled; no new runtime check is implied.

[I, proposed execution order] First run a labeled staging testnet rehearsal with disposable authority and the real CVM path, including rejection, honest upgrade, early governance refusal and recovery. A staging gate retaining an access key is not keyless-security evidence. Then prepare the final deployment, verify hashes/roles/attestation/ownership and obtain founder approval for lockdown. Re-run the final audit and essential live checks after deletion; staging evidence never substitutes for final keyless evidence. Real maturation requires the actual chosen delay; local time travel is labeled local. Registry visibility and publication credentials, paid inference batch limits, hosting deployment and continuous CVM uptime still require their relevant approvals.

[V, founder execution authorization 2026-11-10] Coherent checked local commits are authorized. One additional real NEAR AI review request is authorized, with no retries; it is not repair-generation permission and does not waive the core dependency. The existing provider-TEE, E2EE, no-aliasing and manifest limits remain mandatory.

### D22. KMS freshness for disposable validation

[V, official source and public API inspection 2026-11-10] The advertised dstack KMS revision implements `timestamp` and `signature_v1`, but the reachable Cloud encryption-key API returns only `public_key` and legacy `signature`. Direct advertised KMS HTTPS failed before HTTP from the tested clients. Official Cloud tooling supports the legacy flow. Sources: [advertised KMS protocol](https://github.com/Dstack-TEE/dstack/blob/dad8326f89a76e60e5d4b61c424a3329c92f60e9/dstack/kms/rpc/proto/kms_rpc.proto#L22-L32), [Cloud SDK key schema](https://github.com/Phala-Network/phala-cloud/blob/5db1ae2dde9cf0616a5bef0b008c4e00be39e90e/js/src/actions/kms/get_app_env_encrypt_pubkey.ts), [independently published KMS root](https://cloud.phala.com/verification/kms/phala).

[V, founder go and lead ruling 2026-11-10] Continue the disposable bounded validation with the official legacy signature, verifying the expected application ID and independently pinned KMS root before local encryption. A backend-generated `received_at` records when we received the response; it does not authenticate when KMS signed it and cannot prevent replay of an old signed key. Do not relabel it as `timestamp`, fabricate `signature_v1`, disable TLS verification or weaken DCAP/TCB appraisal. This exception is not a final-activation freshness guarantee.

[V, initial runtime preparation 2026-11-10] Auto-selection unexpectedly chose a development OS, so that short-lived instance was deleted. Subsequent preparation explicitly pins production `dstack-0.5.12` and its published non-dev hash. The scheduled-delete PATCH is accepted, but the current info/list response does not expose its timestamp for independent readback; a separately armed local deletion watchdog is an additional guard, not proof of a provider spending cap. Final runtime results and confirmed deletion belong in P1.6 evidence.

## BLOCKERS

| Id | Missing prerequisite | Owner | Status | Blocks plan ids | Next action / proof |
| --- | --- | --- | --- | --- | --- |
| B1 | NEAR testnet accounts (spike, gate, guardian, demo governor, verifier relay) funded from faucet | lead (founder-authorized 2026-11-02) | resolved 2026-11-02 | P1.2, P1.5, P2.x | Accounts and creation txs below; keys only in gitignored `.secrets/` |
| B2 | Base Sepolia ETH for the derived EOA and a deployer EOA | founder | resolved 2026-11-03 | P1.3, P1.4, P2.8 | A friend of the founder sent 0.05 ETH to the deployer on Ethereum Sepolia; with founder approval the lead bridged 0.049 ETH through the Base L1StandardBridge `0xfd0Bf71F60660E2f608ed56e1659C450eB113120` (address from docs.base.org "Contract Addresses", `otherBridge()` = `0x4200…0010` checked on chain): Sepolia tx `0xa3b4c26bc922789c6abec82b446605ffe275983fbedf8f1b6edd7d5bebc4f2cd`; deployer balance on Base Sepolia read as 0.049 ETH at block 53763980 (2026-11-03T02:57Z). The derived EOAs are funded from the deployer when needed |
| B3 | Phala Cloud access, runtime fit and approved uptime budget | founder (budget), lead (runtime) | access/balance and bounded execution authorized 2026-11-10; Small runtime validation in progress; continuous uptime open | P1.6, P2.8 | Small only, two-hour / USD 0.25 validation allocation, no resize/top-up; D22 legacy limitation disclosed; independently appraise actual CVM and delete after validation |
| B4 | NEAR AI Cloud credits and API key in local `.env` | founder | credential prerequisite resolved; one additional review request authorized 2026-11-10, not yet consumed | P2.6 prerequisite resolved; P3.1 evidence incomplete | Preserve provider-TEE/E2EE/no-aliasing, 45,000 total-completion cap and 300 s review limit; no retries or automatic repair request. Earlier measured request billed USD 0.00274184; timeout costs remain unknown |
| B5 | Base Sepolia state/trace and fixed-block proof endpoints | founder (provider access), lead (probe) | resolved 2026-11-05 under refined D10: production witness fetch from Validation Cloud + public Base RPC verified 8 accounts/7 slots at fixed block 54,676,922 | P1.7, P2.5 live R5 integration | No new account or payment required. Keep both endpoints configured in the CVM; public RPC rate limits/window expiry remain liveness risks. Final deployment/corpus proof remains P2.8 |
| B6 | GitHub repo `Dylansius11/constitution` | founder | resolved 2026-11-03: repo exists (private, empty at start); `main` and `dev` pushed; default branch `dev`; work via branch then PR into `dev`, `dev` into `main` by PR. Open: the repo is PRIVATE, so make it public before submission (founder) | P4 | `git ls-remote origin` lists `main` and `dev` |
| B7 | Hosting choice | founder | resolved 2026-11-10: Cloudflare Pages Free | P3.6 | Public static URL `https://constitution-gate.pages.dev`; no paid Workers or backend secrets |
| B8 | Visual direction | founder | resolved 2026-11-07 (D7) | P3.4 | Brand and reference supplied in `asset/` |
| B9 | Local Docker Desktop Linux engine and Ubuntu WSL integration for the compiler regression | founder (Desktop/WSL settings), lead (focused rerun) | resolved 2026-11-05: Docker server 28.5.1 available from Ubuntu; focused `proposal_compilation_error_remains_a_signed_reject` rerun passed (1 passed, 0 failed) after the founder enabled integration | Split-RPC PR merge verification, P4.1 | Merge blocker cleared; no code/test changes, R2/solver, image build or full-suite rerun |

### B1. NEAR testnet accounts

[V, exercised on testnet 2026-11-02] The lead created five accounts with `near account create-account sponsor-by-faucet-service <id> autogenerate-new-keypair save-to-legacy-keychain network-config testnet create` (near-cli-rs 0.30.1, binary sha256 `996e9f390d1cc446d17ea24e39c48f6ff906507f2d1dba96fc41f596a7e5de14`, installed to the gitignored `.tools/bin/`). Each `view_account` at finalized blocks 275928118 to 275928122 shows 10 NEAR.

| Account | Role | Creation tx |
| --- | --- | --- |
| `constitution-spike.testnet` | spikes only; never the activated gate | `EBDxdUCQTVmX7WCrzhLjmBo3gLCk5WDnHKDJQUcTpQW1` |
| `constitution-gate.testnet` | production gate; untouched until activation, keys deleted at activation | `Ehjsjb6HGLcWmg5v2ehGHUgMcSK27LTfN5FFR3G4eErf` |
| `constitution-guardian.testnet` | guardian; key moves to the founder before activation | `GEz3zughGNZEzk1hLeaLfrgQqowyS5TkmzDBKyQV7ZsD` |
| `constitution-governor.testnet` | demo governor (D17) | `7WbiBThCadD5A1zm7fDRdxYpwGukZ4uehpBXWZtpEqww` |
| `constitution-relay.testnet` | verifier relay for `submit_verdict` | `7dZJwbTi5afhmRTcRVS68n6ZeoJr4kwPjseQg3Sqgbhc` |

### B2. Base Sepolia ETH

The derived EOA address is unknown until P1.2 computes it from the gate account id and path. Unblock evidence: `eth_getBalance` of that address non-zero on Base Sepolia.

### B3. Phala Cloud

Unblock evidence: workspace shows the credit; CVM creation is a later agent step with founder approval for each deploy.

### B4. NEAR AI Cloud

Unblock evidence: an authenticated `GET /v1/attestation/report?model=z-ai/glm-5.3-flash` succeeds from the founder's machine (planned check, non-billable per docs, not run).

### B5. Split Base Sepolia archive / proof RPC

[V, exercised on Base Sepolia 2026-11-05] Resolved with the founder-approved endpoint split: Validation Cloud serves state/trace; `BASE_SEPOLIA_PROOF_RPC_URL=https://sepolia.base.org` serves proofs. Production `fetch_witness` authenticated the two headers and all account/storage proofs at fixed block 54,676,922 (BUILD-PLAN P1.7). The original Validation Cloud fixed-block proof restriction remains unconfirmed provider behavior; it is not bypassed with `latest`. This closes the RPC prerequisite, not CVM deployment or a final upgrade rehearsal.

### B6. Public repo

Code may only start after 2026-11-01T00:00:00Z. The first authorized commit is this documentation set plus the workspace scaffold (BUILD-PLAN P0).

### B7, B8

See D8 and D7.
