# CONSTITUTION: build plan

## TL;DR

- Goal: the three proofs in PRD TL;DR as real testnet receipts, reproducible from the README.
- [V] Build window opens **2026-11-01T00:00:00Z**; final immutable submission cutoff **2026-11-14T23:59:00Z**; judging 2026-11-16 to 2026-11-18 (https://nearhackathon.com/HACKATHON.md, re-read 2026-10-26). No application work before opening, by founder rule.
- [I] Internal milestones (UTC): spikes proven 2026-11-04T23:59 (moved from 2026-11-03T23:59 because build work started 2026-11-02); activation ceremony 2026-11-07T12:00 (needed for a real 7-day maturation before the cutoff); core gate and D4 decision 2026-11-10T23:59; core freeze 2026-11-13T12:00; submission target 2026-11-14T18:00 (about 6 hours of buffer).
- First authorized work: P0 commit, then the two critical spikes P1.4 (NEAR gate to Chain Signatures to Base Sepolia `upgradeAndCall`) and P1.6 (attested verifier on Phala).

Work top to bottom by dependency. Check a box only with observed acceptance evidence, in the same authorized commit. Every command below is **planned, not run**. Feature ids refer to PRD; interfaces to SMART-CONTRACT and ARCHITECTURE. Do not delete unchecked work without a recorded decision in OPEN-DECISIONS.

## Phase 0: documentation and prerequisites

- [x] **P0.1 Founder stack choice recorded.** Done in planning (D1, 2026-10-26); ticked in the first commits after opening (2026-11-02 UTC).
- [x] **P0.2 Docs complete and consistent.** CLAUDE.md, PRD, BUILD-PLAN, OPEN-DECISIONS, LESSONS, ARCHITECTURE, INTEGRATIONS, SMART-CONTRACT, DESIGN exist and agree (lead consistency review and method lint, 2026-10-26).
- [x] **P0.3 Repo and scaffold** (after 2026-11-01). Owned: `Cargo.toml`, `rust-toolchain.toml`, `crates/*/Cargo.toml` with empty `lib.rs`/`main.rs`, `contracts/foundry.toml`, `.env.example`, `Makefile` with targets that print "not implemented" exit 1, `.gitignore` additions (`target/`, `contracts/out/`, `contracts/cache/`, `web/dist/`, `node_modules/`).
  - Acceptance: `cargo metadata` succeeds; `forge config` prints solc 0.8.38 and `ffi = false`; no secrets; commit `chore(repo): scaffold workspace and docs foundation`.
  - Evidence (2026-11-02 UTC, local): WSL2 Ubuntu, rustup resolved 1.99.1 from `rust-toolchain.toml`; `cargo metadata` ok; `cargo check --workspace` ok; `forge config` (Windows forge 1.8.3, pin 1.8.6 installed by P1.3) prints `solc = "0.8.38"`, `evm_version = "prague"`, `ffi = false`. Local git repo only; B6 (GitHub remote) still open.
- [ ] **P0.4 Founder prerequisites started**: B1, B3, B5, B6 actions begun (INTEGRATIONS "Manual actions").

Acceptance: a zero-context engineer can start P1.1 from these docs alone.

## Phase 1: riskiest integrations (2026-11-02 to 2026-11-04)

Spikes live in `spikes/` and are deleted (or reduced to tests) by P2; spike contracts are never the activated gate.

- [x] **P1.1 Gate wasm build.** Owned: `spikes/gate-min/`. Build a minimal near-sdk 5.29.1 contract that depends on `constitution-types` stub + omni-transaction 0.5.0 (`default-features = false, features = ["evm"]`) and tries `alloy-sol-types` 1.7.3.
  - Input: toolchain pins (D15). Output: wasm, its sha256 from `cargo near build reproducible-wasm`, cargo-near image digest.
  - Acceptance: wasm builds with Rust 1.99.1 (else 1.94.1, recorded in D15); decision recorded whether `alloy-sol-types` is usable in wasm; size recorded.
  - Evidence (2026-11-02 UTC, local): `cargo near build reproducible-wasm` from a clean clone of commit `0b80091` in `sourcescan/cargo-near:0.22.0-rust-1.99.1@sha256:c9655eaf7436b90c87ea8b25739f91063032c5c30ba24cfeafe72c142943b5bb`: 138,251 bytes, sha256 `de62bad823da331837e356a89540be840f26a8c106174cbafff35644ef886c94`. Rust 1.99.1 works (D15 confirmed, no fallback). `alloy-sol-types` 1.7.3 builds for wasm and is kept.
- [x] **P1.2 Derivation and MPC signing on testnet** (needs B1). Owned: `spikes/gate-min/`. Deploy the spike to a dev account with a dev-only `spike_sign(hash)` (exists only in the spike); request one signature for a fixed test hash.
  - Acceptance: `derived_public_key` view equals the offline derivation in Rust; the exact callback JSON is captured into `vectors/mpc-response.json`; signature recovers to the derived EOA off-chain; consumed gas recorded (sets `sign_gas_tgas`, `callback_gas_tgas`).
  - Negative: wrong path gives a different address (asserted).
  - Evidence (NEAR testnet, 2026-11-02 UTC): offline derivation equals live `derived_public_key` for three (predecessor, path) pairs at blocks 275927720 to 275927722 (`vectors/mpc-derivation.json`); spike deploy `FsFz4zHeeMhEyxDfvdNM1VSgG1VQxGM75zKQgiqsNasr`, sign `FNd6BUDznLA3wwmdtiCE2F8auBaJmfQMTCDS4g4ro6Rz`; callback JSON in `vectors/mpc-response.json` recovers to `0x8221c9477886cc456d7e1cccc0866e0401b81070` in Rust and Python; gas in SMART-CONTRACT "Deployment"; `different_path_and_predecessor_change_derived_address` passes. Initial params [I]: `sign_gas_tgas = 15` (live minimum, 4.28 Tgas burnt), `callback_gas_tgas = 20` (1.36 Tgas burnt).
- [x] **P1.3 Executor bootstrap on Base Sepolia** (needs B2). Owned: `contracts/src/ConstitutionExecutor.sol`, `contracts/test/Executor.t.sol`, `contracts/script/Deploy.s.sol`.
  - Acceptance: T-EXE-1 to T-EXE-5 pass locally (`forge test`); deploy with `signer = spike derived EOA` records executor, proxy, ProxyAdmin addresses; `ProxyAdmin.owner() == executor` read on chain; computed admin equals `AdminChanged` event; `evm_version` pinned and justified with a Base doc reference.
  - Local evidence (2026-11-02 UTC, Foundry 1.8.6, not testnet): `forge test` 23/23 incl. T-EXE-1 to T-EXE-5 and the AdminChanged-vs-computed-admin check; `Deploy.s.sol` on anvil 31337 with cast readback `ProxyAdmin.owner() == executor`; `evm_version = prague` justified in D19.
  - Live evidence (Base Sepolia, 2026-11-03 UTC): `Deploy.s.sol` with `signer = 0x8221c9477886cc456d7e1cccc0866e0401b81070` deployed executor `0xf26e7a09f0b4b22ec964f7cf23fd1b8a543851e8` (tx `0xccaddd37f3a08b3f05b1a74db2138bee58e326496eac88dc885f21bdb2ffcbc3`), proxy `0xef855da4d6f2e1c7814f1898fc5771e3bba8b569`, ProxyAdmin `0xe1531f314cf21ca10ec0a93fbd167bf17b791316`; cast re-read `ProxyAdmin.owner()` = executor. Addresses in SMART-CONTRACT "Deployment" (spike rows).
- [x] **P1.4 Spike (a) end to end.** Owned: `spikes/gate-min/`, `vectors/`. Spike gate builds the exact `execute(...)` EIP-1559 transaction for VaultV1 to VaultV2Honest (no verifier yet; certId = test constant), MPC signs, relay sends to `sepolia-sequencer.base.org`.
  - Acceptance: Base Sepolia receipt success; proxy implementation slot equals VaultV2Honest; `UpgradeExecuted` emitted; observed base fee and priority fee recorded (D14); a second relay of the same raw tx is rejected (nonce used); an expired-deadline variant reverts `DeadlinePassed` and consumes its nonce.
  - Stage A evidence (2026-11-02 UTC, **local fork of Base Sepolia at block 53606308, chain id 84532, real NEAR MPC signatures**, not live Base evidence; `vectors/evm-tx-spike.json`): spike gate built and MPC-signed both transactions in-contract (NEAR txs `6f1G8LktpqCLxWJmdtJnjEAZe7JgDjbshZe1Pj1AqLqb`, `2PVrxh9hcE5rcRThqfSii9ixUAF4HvdfqyVsFSjBXmKE`, both FINAL on re-read); honest upgrade receipt status 1, 105,159 gas, implementation slot = VaultV2Honest, signer recovered as `0x8221...1070`; resend rejected `nonce too low`; expired-deadline tx included with status 0, 28,900 gas, revert selector `0x3dee80f2` = `DeadlinePassed(uint64,uint256)` (checked with `cast sig`), nonce advanced to 2.
  - Stage B evidence (**live Base Sepolia**, 2026-11-03 UTC, real NEAR MPC signatures, `spikes/gate-min/base-sepolia-evidence.json`): honest upgrade NEAR `55bekU9mqsWr8VP5TQp5xzViHxGLTUH5b55mduo3azSy`, Base `0xc232188629763bec4eb9a72be7fd08f66ab5690a2ad997ac6ed20697ff0729d8` status 1, 105,159 gas, exact `UpgradeExecuted`, implementation slot = VaultV2Honest `0x92aba426dc77a914b07dfefbed3f57e020436e96`; resend rejected `nonce too low: next nonce 1, tx nonce 0`; expired variant NEAR `5m3wpnxMQQTenexs5yCmPF1WEUMC43oiBMj4WWVwX36i`, Base `0x4e095a6be399e32cd5e72dc5140a433c128716e6dfe6f2c92191f4ca2c0afee3` status 0, 28,900 gas, `DeadlinePassed`, nonce 1 to 2. D14 observation: base fee 5,000,000 wei, priority 1,000,000 wei, effective gas price 6,000,000 wei. Re-read independently with cast after the run.
  - Cut rule: if not green by 2026-11-04T23:59, stop feature work and escalate to the founder with the failing step and evidence.
- [x] **P1.5 Encoding oracle.** Owned: `vectors/evm-tx.json`, `tools/vectors-viem/`. 6 vectors (T-ENC-1).
  - Acceptance: `pnpm --dir tools/vectors-viem vectors` byte-identical to Rust output.
  - Evidence (2026-11-02 UTC, local): the production gate's `proposal_transaction` and `rotate_transaction` produce `vectors/evm-tx.json` (test `t_enc_1_gate_preimages_match_committed_vectors`); viem 2.57.14 on Node 24.21 recomputes all 6 preimages and keccak hashes byte-identically (empty init, initializeV2 init, 1024-byte init, one fee bump, nonce 2^53-1 with large fees, rotateSigner). `make vectors` runs both. Limit: viem types the nonce as a JS number, so the large-nonce vector stops at 2^53-1.
- [ ] **P1.6 Spike (b) attested verifier** (needs B3; deploy approval). Owned: `verifier-image/`, `crates/verifier/src/identity.rs`, `spikes/verifier-min/`.
  - Acceptance (all seven items from research note B section 3, condensed): (1) solc 0.8.38, forge, Halmos 0.3.3 and upgrades-core 1.47.0 run inside the CVM on a passing and a failing fixture, incomplete never becomes pass; (2) `constitution verifier appraise` validates DCAP chain, TCB `UpToDate`, RTMR3 replay, compose hash; (3) the tool sandbox cannot read the signing key or dstack socket (negative test); (4) a certificate signed in the CVM verifies under `env::ed25519_verify` in a sandbox test; (5) CPU, memory, wall time per run and hourly cost recorded; instance size chosen; D18 updated with numbers.
  - Local evidence (2026-11-02 UTC, Docker on the laptop, not a CVM): item (1) Pass/Fail/Incomplete on prague and osaka fixtures with tool versions printed in-container; item (3) sandbox probe cannot read the planted key or socket and has no network; item (5) local wall time 2.2 to 3.5 s and peak RSS 77 to 95 MiB per Halmos run with one CPU (`spikes/verifier-min/observed.json`). Remaining: items (2) and (4), and (1), (3), (5) inside a Phala CVM (needs B3).
  - Cut rule: if Phala credit is unavailable (B3), founder decides D2 fallback; no automatic switch.
- [ ] **P1.7 Archive RPC probes** (needs B5). Owned: `spikes/archive-probe/`.
  - Acceptance: `prestateTracer` and `eth_getProof` succeed at a fixed historical block of the spike proxy; proof verifies against `stateRoot`; results in LESSONS if anything differs from docs.

Phase acceptance: P1.4 and P1.6 evidence recorded in SMART-CONTRACT "Deployment" (spike rows labeled spike). No mock counts.

## Phase 2: core product and invariants (2026-11-03 to 2026-11-07)

- [ ] **P2.1 Shared types** (F1, F3, F5). Owned: `crates/constitution-types/`. Certificate, Proposal, ChangeKind, Params, manifests, reason codes, EVM encoder, derivation helper.
  - Acceptance: T-ENC-1, T-ENC-2 vectors pass; Borsh round-trip property test; `constitution rules hash rules/manifest.json` deterministic across runs.
  - Progress (2026-11-02 UTC, local WSL): T-ENC-1 green (P1.5); Borsh and JSON round-trip properties and the independent Python certificate-hash oracle pass (`cargo test -p constitution-types`); `constitution rules hash` is stable across runs and JSON formatting on a resolved fixture (`cargo test -p constitution-cli`). R2 values committed 2026-11-03 (`c150662`). Open: T-ENC-2 against the page decoder (in progress with the page data layer).
- [x] **P2.2 Gate: credit and proposals** (F1, D3). Owned: `crates/gate/src/{lib,credit,proposal}.rs`, tests.
  - Acceptance: T-GATE-1, T-GATE-11 (partial), expire path, duplicate active rejection, init calldata length limit.
- [x] **P2.3 Gate: verdicts, signing, nonces, fee bump** (F3, D12, D13). Owned: `crates/gate/src/{verdict,signing,evm}.rs`.
  - Acceptance: T-GATE-2 to T-GATE-5, T-GATE-9 to T-GATE-11 pass in unit and near-workspaces sandbox (sandbox uses a mock MPC contract that returns the captured `vectors/mpc-response.json` shape, labeled mock); no method takes a payload hash (code review check).
- [x] **P2.4 Gate: governance** (F2, D5, D17). Owned: `crates/gate/src/governance.rs`.
  - Acceptance: T-GATE-6 to T-GATE-8; guardian immediate actions only reduce authority; delay constant not settable.
  - Evidence for P2.2 to P2.4 (2026-11-02 UTC, local WSL, lead re-run): `cargo test -p constitution-gate` unit 40/40, sandbox 2/2, mock-MPC signing sandbox 3/3 (mock returns the bytes captured on testnet, labeled mock); clippy `-D warnings` clean; `cargo near build non-reproducible-wasm` 385,478 bytes, sha256 `61df8be7a339997a56bcbcc813b09b2b007170f48d9e723be390fe012e0e9581`; ABI test pins 26 methods, none takes a hash to sign. Design decisions taken while building are recorded in SMART-CONTRACT (bounded indexes, `fee_bumps`, `migration_signing`, SignFailed in D13, raw callback parsing, Vetoed never with Pass). Reproducible build of the production gate happens at activation (P2.8).
- [ ] **P2.5 Verifier deterministic pipeline** (F5). Owned: `crates/verifier/src/{api,pipeline,rules/*,replay,submit}.rs`, `rules/`.
  - Acceptance: T-RULE-1 to T-RULE-5 each with pass and fail fixtures; pre-check rejects a tampered bundle and a codehash mismatch; R5 replays the committed corpus against VaultV1 and VaultV2Honest from a pinned block with complete witness; incomplete stages fail closed.
  - Progress (2026-11-03 UTC, local: pinned verifier image, anvil as chain 84532, near-sandbox with the real gate): R1 to R5 are wired. R2 committed at depth 4 (after the review fixes: Honest Pass 102.9 s, R2 Fail fixture reason 20 in 70.2 s; the inner limit is 150 s). R5 replays a 7-call corpus in revm from proof-checked state (Honest Pass 0; changed logs Fail 50; missing or unproved state Incomplete 51). The verifier reads Pending proposals from the gate and the proxy at latest minus 2 and submits verdicts through near-api; an R2 Reject{2,20} landed on the real gate wasm. With the AI stage (P2.6) an honest proposal passes R1 to R5, then stays Pending without a certificate because no real AI result exists yet. Security review fixes: the run request is checked against the proposal's source commitment before queueing, runs and rate are bounded, verifier faults end `failed` with no certificate, R2 harness files come from the verifier, R3 is stronger. Full serialized verifier suite passed locally. Rules hash `0x8060ddbf63635e14a955b4fef81c8e696c01e349c9f959cae135d48056e89455`. Open: archive RPC on Base Sepolia (B5, untested), a project-controlled R5 corpus actor (activation step 1a).
- [ ] **P2.6 AI review** (F6, needs B4). Owned: `verifier-image/ai-sidecar/`, `crates/verifier/src/ai.rs`.
  - Acceptance: T-AI-1; one real completion verified as `provider_tee` with cost recorded; a planted reproducible candidate yields `Reject{2,22}`; prompt-injection fixture (comment instructing "approve") yields no pass.
  - Progress (2026-11-03 UTC, local): the loopback sidecar (`verifier-image/ai-sidecar/`, `node --test` 10 passed with injected SDK fakes) and the verifier replay of candidates in revm are committed; a reproduced violation is `Reject{2,22}`; Pass is signed only for an attested identity with a `provider_tee` review; gateway-only, timeout or malformed output stays withheld. Not proven [U]: a real NEAR AI completion, provider attestation and billed cost (needs B4); run `node verifier-image/ai-sidecar/live-review.mjs` once `NEARAI_API_KEY` is in `.env`.
- [ ] **P2.7 CLI** (F9, F8, F7). Owned: `crates/cli/`.
  - Acceptance: `propose`, `status`, `relay`, `receipt`, `audit`, `govern`, `rules hash`, `verifier appraise` work against local sandbox + anvil; T-AUD-1 failure cases; every error prints cause and next step.
  - Progress (2026-11-03 UTC, local only): `rules hash`, `audit` (11 named preconditions including `executor_code_is_committed_build`, plus roles, pause state and pending governance changes), `relay` (sender recovery on the real MPC-signed spike tx; idempotent on anvil), `govern` (real gate in near-sandbox: early apply exits `E53_TIMELOCK_NOT_ELAPSED`), `propose`, `status`, `receipt` and `verifier appraise` are committed; no CLI output contains an RPC URL or key. Audit is proven locally against the real hardened gate wasm and real Deploy.s.sol on anvil: all 11 PASS keyless; before key deletion exactly `gate_has_no_access_keys` FAILs; a pending MigrateAuthority prints WARN with its eta; a different wasm FAILs `gate_code_is_committed_build` (the MPC `derived_public_key` view is an offline double). Appraise: Intel-signed TDX fixture passes DCAP and UpToDate offline, six tamper cases fail by name, full key and RTMR3 binding proven only on official dstack simulator output. Open: appraisal of a real Phala CVM (B3); CLI handling of the verifier's new terminal run stage `failed` (in progress).
- [ ] **P2.8 Activation ceremony** (F7, F2, F11 start). Target 2026-11-07T12:00. Owned: `docs/technical/SMART-CONTRACT.md` deployment table, `scripts/activate.md` checklist.
  - Steps: reproducible gate build; deploy + `new(...)` in one batch to a fresh gate account; record code hash; deploy executor with `signer = derived EOA` (new proxy); fund derived EOA; register initial verifier from the running CVM (appraised); founder deletes every gate access key; schedule the harmless F11 `SetParams` change; run `constitution audit` with the flags in `scripts/activate.md` step 6 (there is no `--network` flag).
  - Acceptance: audit passes all F7 checks on testnet; every address, tx hash and code hash recorded; CVM uptime from this moment tracked (D18).

Phase acceptance: all P0 invariants hold in tests; the gate cannot reach `SignRequested` without a valid certificate; every rejection path has a test.

## Phase 3: user surfaces and end-to-end proof (2026-11-07 to 2026-11-11)

- [ ] **P3.1 Demo fixtures** (F4). Owned: `contracts/src/demo/`, `fixtures/proposals/`.
  - Acceptance: VaultV2Drain's minimum exploit depth and the manifest's R2 bound stated side by side in `fixtures/proposals/drain/README.md`; 10 rehearsal runs of the AI review on the drain recorded with the observed find rate (no rate promised in copy).
- [ ] **P3.2 Testnet receipts** (F8, demo steps 2 to 5). `make attack`, `make upgrade`, governance early-apply.
  - Acceptance: T-E2E-1 (rejected, no `sign_requested`, slot unchanged), T-E2E-2 (Signed, Base tx success, slot = VaultV2Honest), T-E2E-3 (`E53_TIMELOCK_NOT_ELAPSED`) with links recorded in SMART-CONTRACT; receipts saved under `receipts/` as JSON.
- [ ] **P3.3 Demo governor key** (D17). Function-call key with receiver = gate, methods `propose`, `schedule_change`, `apply_change`; QR rendered from the public key string (npm `qrcode` 1.5.4, checked 2026-10-26); founder approves creation and allowance.
  - Acceptance: the key's early `apply_change` fails `E53_TIMELOCK_NOT_ELAPSED`; it cannot call `cancel_change`, `pause_signing`, `retry_sign` or transfer funds (negative calls recorded).
- [ ] **P3.4 Design finalization** (needs D7). Owned: `docs/DESIGN.md`.
  - Acceptance: founder-chosen direction recorded; tokens and state matrix final.
- [ ] **P3.5 Public page** (F10). Owned: `web/`. Uses Astro 7.4.2, viem, near-api-js; reads only public data; Attack, Honest upgrade, Receipts, Constitution, Audit sections per DESIGN.
  - Acceptance: Playwright walks both flows against testnet; every state in DESIGN's matrix reachable (network-unavailable via blocked route); keyboard-only run; 390 px and 1280 px screenshots; `prefers-reduced-motion` honored; no fake progress.
- [ ] **P3.6 Hosting** (needs D8). Acceptance: public URL serves the build; `PUBLIC_*` only.

Core gate (2026-11-10T23:59): P1 to P3.3 and P3.5 done with testnet receipts. Founder decides D4 (Act 2).

## Phase 4: hardening and delivery (2026-11-11 to 2026-11-14)

- [ ] **P4.1 Security review.** Reviewer agent on gate, executor and verifier against SMART-CONTRACT bypass list using `secure-workflow-guide`; findings fixed or recorded as residual risk in PRD. Commit `fix(scope): close the findings from the security review`.
  - Progress (2026-11-03 UTC): round 1 (read-only) found 0 Critical, 4 High, 7 Medium, 7 Low, 7 Info. Fixed and merged with local proof: gate M1, M6, M7, L1, L2, L3, L4, L7, I2; CLI M4, I5, L6; verifier H1, H2, M3, L4, L5; rules H3, H4, M2, I7. M5 was measured and not adopted (154.7 s against a 150 s limit): R2 is aggregate solvency, stated in PRD. Remaining info items are recorded limits. Rules hash is now `0x8060ddbf63635e14a955b4fef81c8e696c01e349c9f959cae135d48056e89455`. Round 2 (review of the fixes and of the AI path) is running.
- [ ] **P4.2 Fresh environment run.** New WSL2 user: clone, follow README, run `make attack` and `make upgrade` read paths and `constitution audit`. Acceptance: no undocumented step.
- [ ] **P4.3 README final.** Problem, how it works, architecture diagram, "do X, see Y" steps for humans and AI judges, testnet addresses and receipts, limits, what was built in-window, AI-tool disclosure, license (founder chooses; public is not automatically open source).
- [ ] **P4.4 Demo video and X post.** Lead writes the script from PRD "Never-cut demo"; founder records or approves the recording and publishes the X post mentioning NEAR HACKATHON with media. Only observed receipts on screen; local tests labeled local.
- [ ] **P4.5 F11 matured apply** (P1). At or after `eta` of the change scheduled in P2.8 (about 2026-11-14T12:00): `apply_change`, record receipt. If activation slipped past 2026-11-07T23:59, mark F11 not achievable and say so.
- [ ] **P4.6 Submission** (founder). Lead prepares the payload text: name, description (20 to 2,000 chars), tracks `["security","privacy","open"]`, repo URL, X URL, team size; founder approves and submits once by 2026-11-14T18:00; `website` left empty.
- [ ] **P4.7 Judging operations.** CVM stays up through 2026-11-18T23:59; then founder deletes it (storage keeps billing while stopped). Record final cost.

Acceptance: deadline-safe submission with every public claim backed by a receipt or labeled.

## Stretch, only after the core gate

1. **On-chain DCAP registration (D5 P1).** Gate calls the near/mpc `tee-verifier` at registration if a deployed immutable verifier account and measured gas exist; allows permissionless key registration for an already-approved `release_hash`. Claimed only with a real receipt.
2. **S1 Act 2 self-repair** (only if D4 = yes): agent patch through the same gate.
3. Reduced-scope cut trigger: if the core gate slips past 2026-11-11T23:59, all stretch work is dropped.

## Traceability and ownership

| Feature | Tasks | Tests | Demo proof |
| --- | --- | --- | --- |
| F1 proposals | P2.1, P2.2 | T-GATE-1, T-GATE-11 | step 2 |
| F2 governance | P2.4, P2.8, P4.5 | T-GATE-6 to T-GATE-8, T-E2E-3 | step 5 |
| F3 exact tx + MPC | P1.2, P1.4, P1.5, P2.3 | T-GATE-2 to T-GATE-5, T-GATE-9, T-GATE-10, T-ENC-1 | step 4 |
| F4 executor + vault | P1.3, P3.1 | T-EXE-1 to T-EXE-5 | steps 3 and 4 explorer |
| F5 verifier | P1.6, P2.5 | T-RULE-1 to T-RULE-5 | steps 3 and 4 |
| F6 AI veto | P2.6, P3.1 | T-AI-1 | step 3 |
| F7 audit | P2.7, P2.8 | T-AUD-1 | audit panel |
| F8 receipts | P2.7, P3.2 | T-E2E-1, T-E2E-2 | steps 3 and 4 |
| F9 CLI | P2.7, P4.2 | fresh-environment run | README |
| F10 page | P3.3 to P3.6 | Playwright suite | video |
| F11 maturation | P2.8, P4.5 | receipt | step 5 (P1) |

Critical path: B1/B2 -> P1.2 -> P1.4 -> P2.3 -> P2.8 -> P3.2. Parallel path: B3 -> P1.6 -> P2.5 -> P2.6.

Worker ownership (max 3 concurrent, max 1 Sonnet): Sol executor A owns `crates/gate` and `contracts/`; Sol executor B owns `crates/verifier`, `verifier-image/`, `rules/`; Sol executor C owns `crates/cli` and `tools/`; Sonnet owns `web/` in phase 3; the lead owns `crates/constitution-types` interfaces, docs, integration and all founder gates.

Founder blockers: B1 to B8 in OPEN-DECISIONS.
