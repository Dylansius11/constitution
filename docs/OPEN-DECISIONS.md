# CONSTITUTION: open decisions and blockers

## TL;DR

- [V] Founder chose stack **Option C (Phala TDX CVM verifier + Rust-first operations + Astro static page)** on 2026-10-26 in this planning session (D1). ARCHITECTURE is written against it.
- [V] Founder resolved on 2026-10-26: attestation appraisal off-chain first, on-chain DCAP as P1 (D5); spam control by refundable deposit plus one active proposal per account (D3); verifier host Phala with the USD 20 verified-account credit, OutLayer only as a separately decided fallback (D2); archive RPC Validation Cloud free tier (D10); public repo on GitHub `Dylansius11`, X post from the founder's own account (D9).
- [V] Founder answers on 2026-11-02 (UTC): the founder creates the GitHub repo personally and the lead commits locally only until told to push (D9); the lead creates NEAR testnet accounts via the faucet with keys in the repo-local gitignored `.secrets/` (B1, done); D7 waits for the founder's logo and references; D8 deferred between Cloudflare Pages and Vercel.
- Open founder rows: D4 Act 2 (decide at core gate 2026-11-10), D7 visual direction, D8 hosting, D14 funding amounts, D18 verifier uptime budget (after spike P1.6). Open organizer questions: D16.
- Build opened 2026-11-01T00:00:00Z; build work started 2026-11-02. Remaining BLOCKERS below are founder-owned manual prerequisites.

Founder-owned rows are never silently decided by agents. Status: open, blocked or resolved. Lead-owned rows record the reasoning so the founder can overrule. Each row has its explanation below.

## Decisions

| Id | Decision | Owner | Status | Needed by | Resolution |
| --- | --- | --- | --- | --- | --- |
| D1 | Technology stack | founder | resolved 2026-10-26 | before ARCHITECTURE | Option C |
| D2 | Verifier host | founder | resolved 2026-10-26, confirm in spike P1.6 | P1.6 | Phala Cloud CVM on USD 20 credit; OutLayer fallback needs a new decision |
| D3 | Proposal spam control | founder | resolved 2026-10-26 | P2.2 | Refundable credit reservation + one active proposal per account |
| D4 | Ship Act 2 self-repair | founder | open | core gate 2026-11-10 | Decide after core gate |
| D5 | Verifier key registration appraisal | founder | resolved 2026-10-26 | P2.4 | Off-chain appraisal + 7-day timelock; on-chain DCAP is P1 |
| D6 | Gate code mutability | lead | resolved 2026-10-26 | P2.1 | Immutable gate; authority migration only via timelocked change |
| D7 | Visual direction for the public page | founder | open, founder collecting logo and references (2026-11-02) | P3.4 | Candidates in `docs/DESIGN.md`; page work waits |
| D8 | Hosting for the public page | founder | open, Cloudflare Pages or Vercel (2026-11-02) | P3.6 | Deferred by founder |
| D9 | Public repo and X accounts | founder | resolved 2026-10-26, refined 2026-11-02 | P4 | GitHub `Dylansius11`, founder creates the repo; lead commits locally until asked to push; founder's X |
| D10 | Archive / debug RPC for replay | founder | resolved 2026-10-26, confirm in spike | P1.7 | Validation Cloud free tier |
| D11 | NEAR AI Cloud inference access | founder | in progress | P2.6 | Founder applied for hackathon credits |
| D12 | EVM authority shape | lead | resolved 2026-10-26 | P1.3 | `ConstitutionExecutor` owns ProxyAdmin |
| D13 | Effect of rule changes on already-signed transactions | lead | resolved 2026-10-26 | P2.3 | Valid only until their short deadline; disclosed |
| D14 | Gas, fee caps and funding amounts | founder (funding), lead (caps) | open | P1.4 | Set from observed Base Sepolia fees in spike |
| D15 | Rust toolchain pin | lead | resolved 2026-10-26, confirm in spike | P1.1 | Host 1.99.1; gate via cargo-near reproducible image |
| D16 | Organizer questions | founder | open | before submission | Draft below, not sent |
| D17 | Demo key exposure model | lead | resolved 2026-10-26 | P3.3 | QR key = governor (schedule-only) function-call key |
| D18 | Verifier key lifetime vs CVM budget | founder (budget), lead (design) | open | P1.6 | Ephemeral key; CVM runs continuously from activation to 2026-11-18T23:59:00Z |
| D19 | EVM target for contracts and the rule-set manifest | lead | resolved 2026-11-02 | P1.3, P2.5 | `prague` everywhere; Halmos 0.3.3 cannot execute Osaka CLZ |

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

### D3. Proposal spam control

Options presented: refundable deposit; per-account rate limit; deposit plus one active proposal per account. Founder response, 2026-10-26: deposit plus one active proposal per account.

Implementation consequence [I, lead]: NEAR function-call access keys cannot attach deposits, and the public demo key (D17) is a function-call key. The deposit is therefore an internal **credit reservation**: anyone tops up an account's credit with `deposit_credit` (attached deposit), `propose` reserves `params.proposal_deposit` from that credit, and the reservation returns to credit when the proposal reaches a terminal state. One active proposal per account is enforced in `active_by_account`. The amount is a timelocked parameter set at activation (D14). Residual risk: many funded accounts can still queue verifier work; the verifier service rate-limits its off-chain queue per account and this is disclosed.

### D4. Act 2 self-repair

Founder response, 2026-10-26: "Putuskan di core gate". No Act 2 work is planned before the core gate (BUILD-PLAN phase 3 acceptance). Decision point: 2026-11-10. If yes, the patch must use exactly the same gate, rules and verifier; no special path.

### D5. Verifier key registration appraisal

Options presented: off-chain appraisal plus timelock; on-chain DCAP via the `tee-verifier` contract in near/mpc; off-chain first with on-chain as P1. Founder response, 2026-10-26: off-chain first, on-chain as P1.

Meaning [I]: the verifier's Ed25519 key, its release hash and the full TDX quote, collateral and event log are published; `constitution verifier appraise` checks them off-chain; the initial key enters the gate at initialization, later keys only through `AddVerifier` with the 7-day delay. The gate does not cryptographically verify Intel evidence in P0, and every doc must say so. P1: call the near/mpc `tee-verifier` `verify_quote_with_collateral_dates` at registration if a deployed immutable verifier account and measured gas exist (research note: source exists, deployment and gas [U]).

### D6. Gate code mutability (lead)

[V] MPC derivation depends on the gate **account id** and path, not on its code (near/mpc `kdf.rs`, `common.rs`, read 2026-10-26). Anyone who can redeploy code or add a key on the gate account inherits the EVM authority. Resolution [I]: the gate exposes no code-upgrade, add-key, delete-account or arbitrary self-call method, and after activation the account holds zero access keys. Escaping a buggy gate uses `MigrateAuthority`, which after the 7-day delay signs `ConstitutionExecutor.rotateSigner(newSigner, ...)` toward a new gate's derived address. Rejected alternative: timelocked self-upgrade of gate code (research option "C" in the chain note), because a delayed arbitrary code swap does not support the headline claim. Founder may overrule; the claim must then name the code-upgrade path.

### D7. Visual direction

Founder response, 2026-10-26 (paraphrased from Indonesian): explore Awwwards and save.design for inspiration, relay browser allowed; the frontend must be as premium and semantic as possible and should amaze; references are fine if the direction is still unclear; frontend comes later. `docs/DESIGN.md` holds candidate directions and references. Open until the founder picks; blocks P3.4 only.

### D8. Hosting

Options presented: GitHub Pages, Cloudflare Pages, Vercel Hobby (non-commercial terms). Founder response, 2026-10-26: decide later. Needed by P3.6. [I] The page is static and reads public endpoints only, so any of the three works without server secrets.

### D9. Public repo and X

Founder response, 2026-10-26: public repo `constitution` under GitHub account `Dylansius11`, created after 2026-11-01T00:00:00Z; the founder publishes the X demo and the submission personally.

### D10. Archive / debug RPC

Founder response, 2026-10-26: Validation Cloud free tier. [V, 2026-10-26] docs list Base Sepolia archive, `debug_traceCall` with `prestateTracer`, `eth_getProof`, 50M CU/month, 120 requests/s, no card (https://docs.validationcloud.io/nodeapi/base/overview, https://docs.validationcloud.io/nodeapi/about/billing). Spike P1.7 must exercise the methods at a fixed block before R5 depends on them.

### D11. NEAR AI Cloud access

Founder response, 2026-10-26: hackathon credits already requested. Grant amount and timing are [U]. No inference runs until a key exists in the founder's local `.env`. If credits do not arrive by 2026-11-04, the founder decides on a small top-up (new row).

### D12. EVM authority shape (lead)

[V] A signed EIP-1559 transaction has no deadline field (EIP-1559 spec, read 2026-10-26); a direct `ProxyAdmin.upgradeAndCall` signature withheld by a relayer stays valid until its nonce is consumed, even after the certificate expired or the rules changed. Resolution [I]: `ConstitutionExecutor` constructs the proxy itself, owns the proxy's ProxyAdmin, and accepts calls only from the derived EOA; it enforces certificate single use, deadline, new implementation code hash and expected current implementation before calling `upgradeAndCall`. Full ABI in SMART-CONTRACT.

### D13. Already-signed transactions when rules change (lead)

Resolution [I]: the executor does not know NEAR rule epochs. A signature issued under rule set N stays executable until its `deadline`, which is at most `params.sign_validity_s` after signing. The gate never signs anything under a superseded rule set, and `apply_change(SetRuleSet)` is refused while any proposal is `SignRequested` or `Signed` with an unexpired deadline. Disclosed in PRD risks.

### D14. Gas, fee caps and funding

Open. Lead sets `gas_limit`, `max_fee_per_gas`, `max_priority_fee_per_gas` and `fee_cap` at activation from fees observed on Base Sepolia during spike P1.4, recorded with the observation. Founder funds: NEAR testnet accounts (faucet), the derived EOA with Base Sepolia ETH (CDP faucet advertises up to 0.1 ETH per 24 h, docs.base.org/get-started/get-funds, 2026-10-26), demo-key credit. No real money involved except D2.

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

Options for the founder at P1.6: (a) entry tier fits: no new spend; (b) approve a top-up for a larger tier; (c) activate later and drop F11 (real 7-day maturation proof); (d) after P1 on-chain DCAP lands, permit restarts because a new key for an approved `release_hash` can be registered without the delay. Lead recommendation will follow the P1.6 measurements.

### D19. EVM target (lead)

[V, 2026-11-02] Base Sepolia runs Osaka: Azul (Osaka CLZ, EIP-7939) activated 2026-04-20 and Denim 2026-10-21 (https://docs.base.org/upgrades/overview). [V, local 2026-11-02] Halmos 0.3.3 in the verifier image fails on solc 0.8.38 Osaka output that contains CLZ: `Unsupported opcode 0x1e`, exit code 3, which the runner maps to Incomplete (fails closed); ordinary vault bytecode passes on both targets (`spikes/verifier-min` osaka probe). Resolution [I]: compile the demo contracts and pin the rule-set manifest to `evm_version = "prague"`. Prague bytecode runs unchanged on Osaka, every opcode it can contain is supported by Halmos, and the pre-check that recompiles a proposal must use the same target as its deployment. Cost: no CLZ gas savings. A proposal compiled for Osaka fails the runtime-hash pre-check (`rule_id 0`, reason 2), never passes.

## BLOCKERS

| Id | Missing prerequisite | Owner | Status | Blocks plan ids | Next action / proof |
| --- | --- | --- | --- | --- | --- |
| B1 | NEAR testnet accounts (spike, gate, guardian, demo governor, verifier relay) funded from faucet | lead (founder-authorized 2026-11-02) | resolved 2026-11-02 | P1.2, P1.5, P2.x | Accounts and creation txs below; keys only in gitignored `.secrets/` |
| B2 | Base Sepolia ETH for the derived EOA and a deployer EOA | founder | open; founder can claim (2026-11-02) | P1.4, P1.5 | Faucet claim after the derived address is computed in P1.2 |
| B3 | Phala Cloud account, card verification, USD 20 credit | founder | open; steps sent to founder 2026-11-02 | P1.6 | INTEGRATIONS "Phala Cloud"; confirm credit visible in workspace |
| B4 | NEAR AI Cloud credits and API key in local `.env` | founder | founder reports key available (2026-11-02); not yet in `.env` | P2.6 | Founder writes `NEARAI_API_KEY` into local `.env`; key never shared in chat |
| B5 | Validation Cloud account and Base Sepolia endpoint | founder | open; steps sent to founder 2026-11-02 | P1.7, P2.5 | Endpoint URL in local `.env` |
| B6 | Public GitHub repo `Dylansius11/constitution` | founder | open; founder creates it (2026-11-02) | P4 | Local repo has commits since 2026-11-02; lead pushes only when asked |
| B7 | Hosting choice | founder | open | P3.6 | Resolve D8 |
| B8 | Visual direction | founder | open | P3.4 | Resolve D7 |

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

### B5. Validation Cloud

Unblock evidence: `eth_chainId` returns `0x14a34` from the keyed endpoint, then the P1.7 method probes.

### B6. Public repo

Code may only start after 2026-11-01T00:00:00Z. The first authorized commit is this documentation set plus the workspace scaffold (BUILD-PLAN P0).

### B7, B8

See D8 and D7.
