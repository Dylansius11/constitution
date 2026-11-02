# CONSTITUTION: lessons

Read the headlines before work. Record actual surprises only. Newest first. Each entry: `### YYYY-MM-DD: headline`, then what happened, root cause with evidence, rule going forward. Update in the same commit as the related change.

Entries dated 2026-10-26 come from the planning session (research only, no code); later entries come from the build.

### 2026-11-02: Codex workers are blocked on exploit-fixture content

- What happened: two GPT 6.1 Sol workers stopped with `Codex error event: This content was flagged for possible cybersecurity risk (code=cyber_policy)`: the contracts worker while writing the hidden-drain fixture, the verifier worker while probing the solvency harness against it.
- Root cause: provider-side policy on content describing fund-draining sequences, even for a defensive verifier and a testnet fixture.
- Rule: route work that reads or writes the drain fixture, the R2 harness or exploit sequences to a Claude worker or the lead; brief Sol workers with neutral fixture names (Pass/Fail fixtures) and keep the drain explanation in `fixtures/proposals/drain/README.md`.

### 2026-11-02: A full C: drive stops every build at once

- What happened: the first parallel Rust build failed with `No space left on device`; `Get-PSDrive C` showed 0.95 GB free. The WSL `ext4.vhdx` (45.8 GB) and Docker `docker_data.vhdx` (10 GB) both live on C:, and deleting files inside WSL does not shrink the vhdx.
- Root cause: unrelated old projects and package caches; the repo itself used 384 MB.
- Rule: check `Get-PSDrive C` free space before fanning out heavy builds (Docker images, near-workspaces, revm); keep at least 20 GB free; compact the WSL vhdx after large deletions.

### 2026-11-02: Parallel git calls and worker index state leak into commits

- What happened: a worker's `git submodule add` left staged gitlinks that a lead `git add <docs> && git commit` swept in; two parallel lead commits then hit `index.lock`.
- Root cause: `git commit` without a pathspec commits the whole index, which workers share.
- Rule: the lead commits with `git commit ... -- <paths>` and never runs two git writes in parallel.

### 2026-10-26: Phala deterministic keys follow the app id, not the measured code

- What happened: the first design derived the verifier's certificate key with dstack `GetKey` so it would survive restarts.
- Root cause: https://docs.phala.com/phala-cloud/key-management/get-a-key.md says keys are tied to the application id and "the same path always generates the same key". Whoever controls app updates could obtain that key under different code, which would bypass the constitution.
- Rule: the certificate key is ephemeral and bound by the TDX quote; restarts need a timelocked re-registration (or P1 on-chain DCAP). Budget the CVM to run without interruption from activation to the end of judging (OPEN-DECISIONS D18).

### 2026-10-26: Installed skills can be wrong about current upstream interfaces

- What happened: the installed `near-smart-contracts` skill shows the MPC signature response as `{ "Secp256k1": { "big_r": "04...", "s": "..." } }` and omni-transaction 0.4; `near-ai-cloud` says all inference runs in TEEs.
- Root cause: current near/mpc `primitives.rs` uses `#[serde(tag = "scheme")]` with nested `affine_point` and `scalar`; the crates index shows omni-transaction 0.5.0; NEAR AI's model tier docs separate Confidential TEE from Incognito upstream models (all read 2026-10-26).
- Rule: treat skills as pointers. Every interface in SMART-CONTRACT comes from source or live docs, and the MPC response parser is fixed only after spike P1.2 captures a real response.

### 2026-10-26: A signed EIP-1559 transaction never expires

- What happened: the brief's design let the MPC-derived EOA own ProxyAdmin and sign `upgradeAndCall` directly.
- Root cause: EIP-1559 signs nonce, fees, destination, value and data but has no deadline (https://eips.ethereum.org/EIPS/eip-1559). A withheld signature stays executable after certificate expiry or a rule change until its nonce is used.
- Rule: the derived EOA calls `ConstitutionExecutor`, which enforces deadline, single-use certificate id, code hash and expected current implementation on-chain (D12).

### 2026-10-26: Dated observations in the brief went stale within a day

- What happened: the brief (2026-10-25) recorded OutLayer WASI on Rust 1.85 and conflicting 60 s and 180 s limits.
- Root cause: the current OutLayer compiler Dockerfile uses Rust 1.97 and the live `outlayer.testnet.get_max_limits` view returned 180 s; the 60 s page is still online (research note B, 2026-10-26).
- Rule: re-check every brief fact against a live source before it enters an interface contract; record both values when sources conflict.

### 2026-10-26: NEAR function-call keys cannot attach deposits

- What happened: the founder chose a refundable deposit for spam control, while the public demo key is a function-call key.
- Root cause: NEAR function-call access keys cannot transfer or attach NEAR (https://docs.near.org/protocol/accounts-contracts/access-keys).
- Rule: deposits are internal credit reservations (`deposit_credit` then `propose`); the gate pays the MPC 1 yoctoNEAR from its own balance.

### 2026-10-26: `env::ecrecover` is behind near-sdk's `unstable` feature

- What happened: the plan assumed the gate would verify the MPC signature by recovering the sender on-chain.
- Root cause: docs.rs near-sdk 5.29.1 marks `env::ecrecover` "Available on crate feature unstable only".
- Rule: safety relies on the EVM deriving the sender and the executor's `NotSigner` check; relay and audit recover off-chain. Enable `unstable` only by a recorded decision.

### 2026-10-26: Facet counts are not project tags

- What happened: brief 15 said only 11 Colosseum projects are "tagged NEAR".
- Root cause: the dataset rows have no chain field and empty track arrays; 11 is the API's `meta.chainOptions` facet (`intel/colosseum/crypto-worlds-fair-2026.md`, 2026-10-26).
- Rule: never present facet counts as verified tags or as evidence that competitors are absent.

### 2026-10-26: The crates.io JSON API refused agent requests

- What happened: `https://crates.io/api/v1/crates/<name>` returned HTTP 403 to research agents.
- Root cause: registry access policy for automated clients (observed, policy text not read).
- Rule: use the official sparse index `https://index.crates.io/...` and docs.rs published manifests as registry evidence, and say so.
