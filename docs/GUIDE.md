# CONSTITUTION user guide

[I] Use CONSTITUTION to inspect one protected proxy, propose a Solidity upgrade,
and follow its real verifier verdict through NEAR signing and Base Sepolia
execution. Everything here is **testnet only**. A deterministic pass is not a
security audit, and a gate signature is not an executed upgrade.

Evidence convention: **[V]** source-verified or observed with the stated date;
**[I]** workflow/design, including commands below that are instructions rather
than new execution evidence; **[U]** unverified. Source/flag review: 2026-11-12,
[`main.rs`](../crates/cli/src/main.rs), [`demo.rs`](../crates/cli/src/demo.rs).
[V, implemented and smoke-tested read-only 2026-11-12] The TUI is part of the same
`constitution` binary; recorded deployment evidence remains separate from a fresh terminal run.

## What you need

- Linux or WSL2 Ubuntu on Windows. Run Rust, Foundry and NEAR tools inside WSL,
  not a Windows shell. Keep the machine awake during long verifier work.
- Rust/rustup, using repository `rust-toolchain.toml` (**1.99.1**).
- Foundry **1.8.6** (`forge`, `cast`, and `anvil` for local integration).
  `contracts/foundry.toml` pins solc **0.8.38**, Prague and optimizer 200 runs.
- The repository checkout with its existing Solidity dependencies under
  `contracts/lib/`. If your checkout omits them, obtain the repository's pinned
  dependencies before building; do not substitute newer library versions.
- For proposal/governance writes: a funded **NEAR testnet account** and its
  near-cli legacy JSON key-file path. A read-only audit/status/receipt needs no
  wallet. Never paste key contents into commands, chat, docs or screenshots.
- A deployed matching candidate implementation on **Base Sepolia**, plus an
  available, appraised demo verifier. The CLI does not deploy implementations.
- For full local verifier integration only: Docker/WSL integration, NEAR sandbox,
  existing gate wasm and pinned verifier tool image; see
  [verifier local setup](../crates/verifier/src/LOCAL.md).

[I] Account creation, funding, signing, deployment and paid inference require the
operator's authorization. These instructions do not grant it. A propose-only
NEAR function-call key cannot deposit credit: fund prepaid credit separately with
an authorized wallet before using it. A new account's first funding includes an
entry fee; proposals consume a non-refundable storage fee plus reserve refundable
credit. Read current required credit rather than assuming a fixed cost.

## Build the CLI

Run from the repository root in Linux/WSL:

```sh
source "$HOME/.cargo/env"
export PATH="$HOME/.foundry/bin:$PATH"
cargo build --locked --release -p constitution-cli
export PATH="$PWD/target/release:$PATH"
forge build --root contracts
```

The binary is `constitution`. The Foundry build supplies the local executor
artifact required by audit. `constitution --help` lists headless commands;
`constitution demo --help` and `constitution verifier --help` describe their
respective inputs. These are build/use instructions [I], not a claimed new build.

## TUI tour

Run in an interactive terminal from the repository root:

```sh
constitution
```

[I] With no arguments, CONSTITUTION opens the TUI. Headless subcommands remain
available for automation. The terminal is a view of observed chain/verifier state,
not a simulated dashboard. Unreadable or pending data must remain unknown/pending,
not display a fabricated success, transaction hash or progress percentage.

### Choose a profile first

| Profile | Intended use | Gate / trusted local inputs |
| --- | --- | --- |
| **Demo gate** | Live inspection; explicitly authorized attack or honest-upgrade runs | `constitution-demo.testnet`, `deployments/demo.json`, `rules/demo/manifest.json` |
| **Final gate — read-only** | Audit and inspect recorded final evidence | `constitution-gate.testnet`, `deployments/testnet.json`, `rules/manifest.json` |

[V, deployment records inspected 2026-11-12] Demo proxy:
`0xc24a9b7b139af4e458df40a2644229a4ca5fec91`, immutable governance delay **1 day**.
Final proxy: `0x0b54b582dc5486677622c6fde431716767165e74`, delay **3 days**.
These are separate deployments, not interchangeable profiles of the same vault.
[V, founder decision 2026-11-12] The final verifier CVM is retired; the TUI shows its
recorded identity and never probes it. Final-gate receipt/audit reads need no verifier
and the profile has no write path. [V, 2026-11-12] The demo gate is keyless: its only
key was deleted and the demo audit exited 0.

### Overview

[I] Start here to identify the selected gate and protected proxy, rule commitment,
current implementation, authority/audit context and service/read errors. Compare
observed state with the chosen deployment record. A failing keyless prerequisite
is a real failure; final-gate key deletion does not establish demo-gate keylessness.

### Proposals

[I] Browse proposals for the selected gate, inspect a proposal's inputs and lifecycle,
and follow its actual verifier stages when a run is known. `Pending` means no final
accepted verdict yet. `Rejected` carries the numeric rule/reason; `SignRequested`
means an MPC request exists; `SignFailed` needs signing diagnosis. `Signed` means
raw transaction bytes exist, not that the proxy changed. Read EVM inclusion and
implementation observations separately.

### Run

[I] Use the **Demo gate** profile for runs. Choose a rule-violating attack preset to
show a refusal, or honest **V2 then V3** to exercise the upgrade path. Inspect the
selected source contract, deployed implementation and exact initializer before
confirming a write. Supply your authorized NEAR account/key-file path and a fresh
evidence output directory. Existing directories are never overwritten.

An attack succeeds as a demonstration only when the gate records rejection,
there is no signing record, and the proxy implementation is unchanged. It never
relays. An honest upgrade can relay only after Signed; success requires a
successful Base receipt and candidate implementation-slot readback. No page button
or AI answer overrides these conditions. The Final profile has no write path.

[V, gallery inspected 2026-11-12] The recorded gallery contains four attack
refusals: drain **R2/20**, storage shuffle **R1/10**, self-destruct **R3/30**, forbidden
initializer **R4/41**. Its honest `upgrades[].observed` fields are null in this
snapshot: presets are not proof that those demo upgrades already executed.

### Audit

[I] Run the public-state authority audit for the selected profile. It compares the
observed gate, rule hash, registered verifiers and executor/ProxyAdmin/proxy binding
with the deployment record and local committed executor artifact, including the
expected governance delay. FAIL exits nonzero in headless mode; WARN is not a clean
bill of health even though it alone exits zero. Read the named failed precondition
and recovery rather than dismissing it as a UI error.

### Receipt

[I] Inspect a proposal's canonical evidence: inputs, accepted certificate or
rejection, rule/AI outcomes when available, observed NEAR transaction and EVM
inclusion/implementation data. Public gate-only reads work without verifier access;
`--run` adds verifier evidence and requires its endpoint. Missing or unavailable
accepted-certificate evidence is incomplete, not a successful verified receipt.
Recorded receipts under `deployments/demo-receipts/` are historical evidence, not
current network status.

### Keyboard reference

[V, read from `crates/cli/src/tui/mod.rs` 2026-11-12]

| Where | Key | Action |
| --- | --- | --- |
| Anywhere | `1`–`5` | Overview, Proposals, Run, Audit, Receipt |
| Anywhere | `Tab` | Next screen |
| Anywhere | `p` | Switch profile (Demo gate / Final gate, read-only) |
| Anywhere | `r` | Refresh observations |
| Anywhere | `?` | Key help |
| Anywhere | `q` / `Esc` | Close overlay or editor, back to Overview, then quit |
| Anywhere | `Ctrl+C` | Cancel a running write (asks once; on-chain actions are not undone) |
| Overview | `a` | Appraise the demo verifier against the pinned compose hash |
| Overview, Proposals | `v` | Show or hide raw JSON |
| Proposals | `↑`/`↓` or `j`/`k`, `Enter` | Select a proposal, fetch its assembled detail |
| Run | `←`/`→` | Choose a preset (rule-violating attacks, honest V2, honest V3) |
| Run | `e`, then `Tab`/`Shift+Tab`, `Enter` | Edit account, key-file path, bundle, contract, implementation, init |
| Run | `u` | Treat custom inputs as attack or upgrade |
| Run | `Enter`, then `y` | Prepare the confirmation, then authorize the write |
| Audit | `Enter` | Run the audit for the selected profile |
| Receipt | `i`, `Enter`, `s` | Enter a proposal id, fetch `constitution.receipt/1`, save JSON (create-only) |

## Write an upgrade that passes

[I] Read the [CONSTITUTION agent skill](../skills/constitution/SKILL.md) for the full
workflow, worked honest/violating examples and local checks. In short:

1. Observe the current implementation and committed rule set. Use a new source
   path for custom code; do not edit pinned V1/MockToken/honest lineage references.
2. Preserve existing storage; append only (**R1**). Preserve aggregate solvency
   and implementation slot along bounded deposit/withdraw sequences (**R2**).
3. Remove forbidden opcodes and proxy/cheatcode literals (**R3**).
4. Use empty init or `initializeV2(uint256)` with one ABI word ≤ `2^128-1` (**R4**).
   Honest V1→V2 uses cap **1000**; V2→V3 uses empty `0x`.
5. Preserve exact committed corpus behavior, state and logs (**R5**), then require
   completed AI review. AI can only veto; it cannot turn any failure into a Pass.
6. Run candidate-specific local tests before authorized deployment/proposal.
   Existing preset tests do not automatically exercise a custom implementation.

For a refusal, use the complete [rule/code and gate error reference](../skills/constitution/references/refusals.md).
A code fix requires a new deployment, source commitment and proposal. A recoverable
service failure normally retries the same still-Pending proposal instead.

### Know the boundary

[I] CONSTITUTION does not protect keys/allowances outside the gate, the frontend
supply chain, unrelated contracts, Safe arbitrary signing, or a proxy whose
upgrade authority was never migrated. R1–R5 are bounded vault-specific checks;
computed-slot writes, untested entry points and delayed/environment-dependent
behavior can escape them. R2 is aggregate solvency, not per-user withdrawability.
MPC/TEE trust, RPC evidence policy and verifier/inference availability remain.
AI is veto-only, and incomplete required review withholds certification. See
[PRD guarantees and limits](PRD.md#risks-and-guarantees).

## Headless commands

[I] Run examples from repository root in bash/WSL. The following variables contain
public endpoints and an approved measurement, not secrets. They do not load `.env`
automatically. Set `DEMO_ACCOUNT` and `DEMO_KEY_FILE` to your authorized **testnet**
account and credential **path** before a write; never put a private key in a variable.
Set `PROPOSAL_ID` / `RUN_ID` only from actual returned or retained identifiers.

```sh
export NEAR_RPC_URL=https://near-testnet.drpc.org
export BASE_SEPOLIA_RPC_URL=https://sepolia.base.org
export VERIFIER_URL=https://db06dd366642c074483472e3afcb330ff72e4954-8787.dstack-pha1-prod5.phala.network
export EXPECT_COMPOSE_HASH=0xf7571e2688238dd87217f2210789e395c505e3e8eecb3ef026c62051feebe79e
```

Use explicit `--near-rpc` in these examples so older binary defaults do not select
a rate-limited public endpoint. `--network testnet` is an evidence/key-location
label, not endpoint validation or a sandbox. Never point these writes at mainnet.

### Audit (no signer, no verifier connection)

```sh
constitution audit --deployment deployments/demo.json \
  --manifest rules/demo/manifest.json \
  --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL"
```

For final evidence substitute `deployments/testnet.json` and `rules/manifest.json`.
There is **no `--network` or `--gate` flag on audit**; the gate is in the record.
The local artifact/record are trusted inputs, not independently authenticated
source provenance. Offline executor helper:

```sh
constitution executor-runtime-hash \
  --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json
```

### Status and receipt (no signer)

```sh
constitution status "$PROPOSAL_ID" --gate constitution-demo.testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --run "$RUN_ID" --network testnet --watch 900
constitution receipt "$PROPOSAL_ID" --gate constitution-demo.testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --run "$RUN_ID" --network testnet --json
```

Both take positional proposal id and the common flags `--gate`, `--near-rpc`,
`--evm-rpc`, `--verifier`, `--network` (`testnet` or `local`), `--run`.
Only status has `--watch SECONDS`; only receipt has `--json`. Omit both `--run` and
`--verifier` for gate-only reads. To inspect final recorded proposal 0 (drain) or
1 (honest), use that positional id, `--gate constitution-gate.testnet`, the RPC
flags, and no verifier/run flags. Status polls every two seconds when watched.

### Demo attack: recorded drain preset

[V, inputs from gallery; execution not repeated here] The following address is a
recorded deployed candidate, not an address produced by this guide. Confirm its
current runtime before any write. Run only with explicit authorization:

```sh
mkdir -p receipts
constitution demo attack --bundle contracts \
  --contract src/demo/VaultV2Drain.sol:VaultV2Drain \
  --impl 0xbdd4826a0124a2443e9e3561d17e38f749c084e4 --init 0x \
  --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --output receipts/drain-001 --watch 900
```

Other attack tuples (`contract`, `impl`, `init_calldata`) are in
[`deployments/demo-gallery.json`](../deployments/demo-gallery.json). Copy the
entire tuple, not an initializer from a different preset. The headless flag for
`init_calldata` is **`--init`**, and implementation is **`--impl`**.

### Demo upgrade: honest V2, then V3

[I] Only if observed current state is V1, submit the V2 preset:

```sh
constitution demo upgrade --bundle contracts \
  --contract src/demo/VaultV2Honest.sol:VaultV2Honest \
  --impl 0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005 \
  --init "$(cast calldata 'initializeV2(uint256)' 1000)" \
  --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --output receipts/honest-v2-001 --watch 900
```

After observed successful V2 inclusion/slot readback, submit V3:

```sh
constitution demo upgrade --bundle contracts \
  --contract src/demo/VaultV3Honest.sol:VaultV3Honest \
  --impl 0xaeed3e98706f4a4a6c1c5062c1f6541a595db4be --init 0x \
  --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --output receipts/honest-v3-001 --watch 900
```

[I] Demo's required flags are `--bundle`, `--contract`, `--impl`, `--account`,
`--key-file`, `--gate`, `--network`, `--near-rpc`, `--evm-rpc`, `--verifier`,
`--output`. Optional `--init` defaults to `0x`; `--send-rpc` selects a separate
broadcaster (otherwise explicit EVM RPC); `--watch` defaults to **2100 s** per
polling stage in the current clap definition, not an HTTP or transaction deadline.
The examples intentionally choose 900. Each output's parent must exist, and output
itself must not. Use a fresh name for a genuinely new run, not to retry an uncertain
write. Inspect `submission.json`, `run.json`, `failure.json` and any readable
`receipt.json` before recovery; no automatic reproposal.

### Propose without orchestration

[I] `propose` submits once and starts verification; it does not watch or relay:

```sh
constitution propose --bundle contracts \
  --contract src/demo/VaultV2Honest.sol:VaultV2Honest \
  --impl 0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005 \
  --init "$(cast calldata 'initializeV2(uint256)' 1000)" \
  --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL"
```

This is an **alternative** to demo upgrade, not the next step after it. It shares
signer flags with govern: `--account`, `--key-file`, `--network`, `--gate`,
`--near-rpc`. `--init` defaults to `0x`. Record returned proposal/run ids. The exact
identity used for sealing is appraised against `EXPECT_COMPOSE_HASH` before NEAR
writes. Plaintext bundles are not uploaded as public chain data.

### Relay an already Signed honest proposal

[I] Broadcasts only gate-signed transaction bytes after recovering/checking sender
and transaction binding; no wallet key argument:

```sh
constitution relay "$PROPOSAL_ID" --gate constitution-demo.testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --send-rpc https://sepolia-sequencer.base.org --watch 900
```

Flags: `--gate`, `--near-rpc`, `--evm-rpc`, optional `--send-rpc`, `--watch SECONDS`,
`--change`. `--change` interprets the positional id as an applied MigrateAuthority
change, not a proposal; it is not an upgrade retry. Relay has **no `--network`**.
Never relay a rule-violating attack. Demo upgrade already performs relay when safe;
use this command only after inspecting existing evidence and state.

### Rules and verifier

```sh
constitution rules hash rules/demo/manifest.json
constitution verifier appraise "$VERIFIER_URL" \
  --expect-compose-hash "$EXPECT_COMPOSE_HASH" --json
```

[I] Rules has only `hash <manifest>`; it prints SHA-256 of validated canonical
Borsh manifest bytes, not SHA-256 of JSON text. Appraise takes positional URL,
optional `--expect-compose-hash`, `--pccs-url` and `--json`. Always supply the
owner-approved expected hash rather than trusting the endpoint's own measurement.
`PCCS_URL` may select collateral service; Intel production signatures remain the
trust anchor. No signer required.

Retry a recoverable still-Pending proposal with the **original** canonical source:

```sh
constitution verifier run --proposal "$PROPOSAL_ID" --bundle contracts \
  --contract src/demo/VaultV2Honest.sol:VaultV2Honest \
  --gate constitution-demo.testnet --near-rpc "$NEAR_RPC_URL" \
  --verifier-url "$VERIFIER_URL"
```

Run flags are exactly `--proposal`, `--bundle`, optional `--contract`, `--gate`,
`--near-rpc`, **`--verifier-url`** (not `--verifier`), and no signer/network/EVM flags.
Directory bundles require `--contract`; canonical Borsh SourceBundle files do not.
This re-seals and starts verification, not a new proposal/NEAR write, but can consume
verifier/inference credit. Honor Retry-After; do not automatically retry. If
submission is Incomplete/uncertain, inspect relay transactions and the gate first.

### Govern (authorized demo operators only)

[I] Public queue changes are not immediate upgrades. Use observed `CHANGE_ID` from
schedule/queue; no final-gate writes. The quoted JSON below schedules Unpause,
not a harmless read: do not run it merely to test syntax.

```sh
constitution govern --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet --near-rpc "$NEAR_RPC_URL" \
  schedule --kind '"Unpause"'
constitution govern --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet --near-rpc "$NEAR_RPC_URL" \
  apply "$CHANGE_ID"
constitution govern --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet --near-rpc "$NEAR_RPC_URL" \
  cancel "$CHANGE_ID"
```

Subcommands: `schedule --kind <ChangeKind JSON>`, `apply <change_id>`,
`cancel <change_id>`. Schedule requires governor; cancel requires guardian; anyone
with an appropriate signing account may apply a matured change. A proposer/session
key is not necessarily authorized for any of these. Early apply refuses
`E53_TIMELOCK_NOT_ELAPSED`. The delay is immutable after initialization; normal
certified upgrades do not wait for it. Unpause must be scheduled strictly after
the latest guardian pause/renewal. Full ChangeKind schema and queue restrictions:
[SMART-CONTRACT governance](technical/SMART-CONTRACT.md#governance).

## When a run stops

[I] Keep the evidence directory. Read existing proposal state before another write.
A signed verifier Reject needs a code fix/new proposal; a service failure can leave
the existing proposal Pending. Required AI timeout/incomplete means no certificate,
not rejection. `submit: Incomplete` means outcome uncertain, not safe to resubmit.
`SignFailed` is an MPC issue; Signed with no inclusion is a relay/nonce/funding/
deadline issue. Audit read failures are not passes.

Use [the refusal reference](../skills/constitution/references/refusals.md) for every
numeric rule/reason, stable gate prefix and operational class. Preserve observed
hashes and timestamps; label retained receipts **recorded**, local tests **local**,
and current network observations **live** only after actually reading them.
