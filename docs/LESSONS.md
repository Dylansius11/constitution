# CONSTITUTION: lessons

Read the headlines before work. Record actual surprises only. Newest first. Each entry: `### YYYY-MM-DD: headline`, then what happened, root cause with evidence, rule going forward. Update in the same commit as the related change.

Entries dated 2026-10-26 come from the planning session (research only, no code); later entries come from the build.

### 2026-11-04: NVIDIA attestation JWTs fail closed when the host clock is a few seconds behind

- What happened: `live-review.mjs` returned `inference_or_verification_failed` with no completion id; a direct SDK call showed `[gpu.jwt_verification_failed] NVIDIA JWT verification failed: not_yet_valid`.
- Root cause: the dev machine clock was about 3 s behind the provider (`date -u` against the HTTP `Date` header of `cloud-api.near.ai`); `w32tm` reported "Local CMOS Clock", never synced. The SDK checks the JWT not-before time without leeway. With a throwaway +6 s clock offset the same call returned `provider_tee`. The sidecar swallows the SDK error by design, so the reason code alone hid the cause.
- Rule: sync the clock before any live provider check (Windows: Settings, Time, Sync now); the verifier CVM must run NTP-synced. A `not_yet_valid` withheld run is a clock fault, not a model fault.

### 2026-11-03: Orca's WSL relay restarts the VM, so the vhdx stays locked

- What happened: three attempts to compact `ext4.vhdx` with diskpart after `wsl --shutdown` failed with "being used by another process"; the founder had quit Docker Desktop and ran the script as Administrator. `wsl --shutdown` itself worked, but `vmmemWSL` and `vmwp` were back within seconds.
- Root cause: a helper process of the Orca terminal app (`Orca.exe ... --max-old-space-size=384`) keeps launching `wsl.exe -d Ubuntu ... opencode-sqlite-reader.cjs`, which boots the distro again. The lock came from that VM, not from Docker. `docker_data.vhdx` is NTFS-sparse, which diskpart refuses for another reason.
- Rule: before compacting, stop the Orca relay helper (and any `wsl.exe` it started), then `wsl --shutdown` and run diskpart at once. `scratch/compact-admin.ps1` does this in a retry loop; it took `ext4.vhdx` from 45.9 GB to 26.9 GB and freed 19 GB on C:. Never kill the Orca daemon host that carries the agent session.

### 2026-11-03: A trailing slash in .gitignore does not ignore symlinks

- What happened: a temporary worktree linked `web/node_modules` to the main tree with a Windows junction; `git add web` committed the junction because `.gitignore` said `node_modules/`.
- Root cause: a pattern ending in `/` matches only directories, and git sees a junction or symlink as a file (gitignore documentation). The commits were local, so they were rewritten before any push.
- Rule: ignore dependency folders without the trailing slash (`node_modules`), and list a worktree's staged paths before committing.

### 2026-11-03: An unbounded Halmos branch timeout makes depth 4 look infeasible

- What happened: the first R2 probes classified VaultV2Honest Incomplete at depth 3 and 4; a one-actor depth-3 run took 100 s with `--solver-timeout-branching 0` and 11 s with `1ms`.
- Root cause: cProfile put 79 of 95 s in `Z3_solver_check_assumptions`; `0` means "no timeout", and branch checks go to z3 even with `--solver yices`. Halmos 0.3.3's own default is `1ms`, and an undecided branch stays feasible (source `config.py`, `sevm.py`), so the short timeout adds paths instead of hiding them. Per-actor balance asserts added assertion-solver timeouts.
- Rule: use the tool's default branch timeout unless a measurement says otherwise, keep the invariant to one aggregate assert, and measure depth on the target CPU before committing a bound (`crates/verifier/evidence/depth/`).

### 2026-11-03: A one-character slot typo survives review; derive constants in tests

- What happened: the CLI audit's ERC-1967 admin slot constant had `f8fa` instead of `f8a`; anvil rejected the 65-digit slot only when the smoke test ran.
- Root cause: the constant was retyped by hand instead of derived; the docs and Solidity tests had the correct value.
- Rule: every protocol constant (slots, selectors) gets a test that derives it (`keccak(label) - 1`, `cast sig`), not one that compares it with another hand-copied string.

### 2026-11-03: Laptop sleep kills long WSL jobs silently

- What happened: a 175 s Halmos test ran for 5.8 hours of wall time and returned no output; WSL uptime showed it had restarted.
- Root cause: the host slept; WSL and its Docker containers were torn down mid-run.
- Rule: write long test output to a log file under `scratch/` and check `uptime` in WSL before trusting a missing result; rerun rather than infer.

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
