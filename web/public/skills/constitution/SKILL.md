---
name: constitution
description: >-
  Use when asked to write an upgrade for a CONSTITUTION-protected proxy, check
  a Solidity implementation against R1 to R5, propose to constitution-demo.testnet,
  read a CONSTITUTION verdict or receipt, or explain why a proposal got refused
  R2/20, R1/10, R3/30, R4/41, or another rule/reason code. Covers local checks,
  honest VaultV2/V3 examples, demo submission, and fail-closed recovery.
---

# Build an upgrade the constitution can accept

[I] This is an agent workflow, not authorization to spend, deploy, sign, create
accounts, change governance, or publish secrets. Ask the operator for the applicable
authorization before a write. Testnet only. Never read or print key-file contents.
Never weaken committed rules, trusted harnesses or reference sources to pass a check.

[V, source inspection 2026-11-12] Policy comes from
[`rules/demo/manifest.json`](../../rules/demo/manifest.json) (demo, v2),
[`rules/manifest.json`](../../rules/manifest.json) (final evidence, v1),
[`rules/r2`](../../rules/r2/), [`rules/r5`](../../rules/r5/), and
[`crates/verifier/src`](../../crates/verifier/src/). Gate interfaces and codes:
[SMART-CONTRACT](../../docs/technical/SMART-CONTRACT.md). Coverage limits:
[PRD](../../docs/PRD.md#risks-and-guarantees). Human setup and all CLI flags:
[GUIDE](../../docs/GUIDE.md).

## 1. Establish the actual target

- Demo: `constitution-demo.testnet`, record `deployments/demo.json`, manifest
  `rules/demo/manifest.json`; Base Sepolia chain id `84532`.
- Final: `constitution-gate.testnet`, `deployments/testnet.json`,
  `rules/manifest.json`. **Read-only evidence profile**; its verifier CVM was
  retired on 2026-11-12. Do not propose, relay, run its verifier or govern it.
- Read the current proxy implementation and gate constitution, not a presumed
  version. Demo v2 selects the first reproduced current runtime in its committed
  VaultV1 / VaultV2Honest / VaultV3Honest lineage. It is not a forward-only policy:
  same-version and V3-to-V2 empty-init candidates can satisfy deterministic rules.
- The final v1 policy knows only V1 as current reference; a later implementation
  cannot be made acceptable merely by changing the bundle's reference name.
- Run the read-only audit from GUIDE. The keyless claim requires no gate access
  keys and the complete executor → ProxyAdmin → proxy authority chain. A retained
  gate key or failed ownership check is not a keyless success.

## 2. Write against the committed vault policy

### R1 — append-only storage

Preserve existing field order, types, namespace, inherited storage and offsets.
`VaultV1`'s ERC-7201 `constitution.storage.Vault` is `token`, `balances`,
`totalDeposits`. Honest V2 appends `depositCap`; do not move `totalDeposits` before
`balances` as `VaultV2StorageShuffle` does (R1/10).

Use a new source path/name for a custom candidate. Demo lineage source pins include
honest V2/V3: editing those reference files is pre-check 0/4, not a valid custom
upgrade. Honest V2 is deliberately standalone with the namespace defined once;
inheriting V1 and redefining its namespace duplicates the definition for
upgrades-core. V3 instead inherits unchanged V2 and adds only `remainingCapacity()`.

The manifest's R1 `unsafe_allow` entries (`constructor`, `missing-initializer`,
`delegatecall`, `selfdestruct`) are **layout-tool exceptions only**. They do not
waive R3's opcode bans. Keep implementation initializers disabled in the
constructor, as the honest examples do.

### R2 — bounded aggregate solvency and implementation-slot preservation

Maintain `totalDeposits() <= token.balanceOf(vault)` and preserve the implementation
slot after every explored call. Debit only the withdrawing user's balance; reduce
aggregate liabilities consistently before transferring their tokens. Do not add
rebates that spend other depositors' assets.

The committed harness deploys the candidate behind an ERC1967Proxy, initializes
its token and `initializeV2(1000 ether)`, deposits `100 ether` for a fixed victim,
and funds two fixed actors with `10 ether` each. Only their `deposit(uint96)` and
`withdraw(uint96)` handlers are explored. Halmos depth is **4**, loop bound **2**,
assertion solver timeout **10 s**; Foundry uses **2,000** runs, depth **128**, seed
`0x0000000000000000000000000000000000000000000000000000000000000001`.

`VaultV2Drain`'s minimum four-call sequence (deposit 1, withdraw 1, deposit 1,
then withdraw more than the actor's balance) is within that symbolic bound and
is the gallery's R2/20 example. Fix accounting, not the bound or test actors.
R2 is **not** proof of per-depositor withdrawability or arbitrary migrations;
it initializes the candidate directly, not from the proposed live initializer.

### R3 — runtime filter

Do not introduce `SELFDESTRUCT` (`0xff`), `CALLCODE` (`0xf2`), or `DELEGATECALL`
(`0xf4`). `VaultV2SelfDestruct` fails R3/30 even with an access check. Under Prague,
SELFDESTRUCT is not a general deletion of an already-deployed proxy's code.

Do not embed the ERC-1967 implementation/admin slot constants, their ±1 neighbors,
`eip1967.proxy.implementation` / `eip1967.proxy.admin` ASCII labels, or literal
Foundry/Halmos cheatcode addresses. Exact eight ordered constants are in the
manifest; numeric PUSH immediates are checked, including PUSH20 addresses.
R3/31 is a filter refusal. Computed slots/addresses can evade the filter; do not
interpret its acceptance as permission to write arbitrary proxy storage.

### R4 — initializer schema, not arbitrary migration

Both manifests allow empty `0x` or selector `0x5b4e128c`
(`initializeV2(uint256)`) with **exactly one 32-byte ABI word**, maximum
`2^128-1`. No other selector, trailing bytes or larger integer is accepted.
The gate separately caps all initializer calldata at **1,024 bytes**.

For V1 → honest V2 use `cast calldata 'initializeV2(uint256)' 1000`.
For V2 → honest V3 use `0x`; V3 introduces no initializer. Re-running V2's
reinitializer on an already-initialized V2 is not a valid migration. An in-schema
cap that makes the replay's deposit revert still fails R5: R4 acceptance is not
behavioral compatibility. The gallery's forbidden initializer uses `2^128`
and fails R4/41 despite selecting the honest implementation.

### R5 — exact bounded regression compatibility

Keep the committed seven-call corpus behavior: deposit 10 raw units, inspect user
balance and aggregate, withdraw 4, inspect balance and aggregate again, then revert
on maximum-uint overdraw. Before replay, the committed actor needs at least
900 token units, allowance 900, and vault balance exactly 100. Do not mutate live
actor state to repair a candidate; unmet preconditions are an operator/environment
issue, not evidence of malicious code.

At a pinned header with two successor blocks, old/new replay must agree on
success/revert, return/revert bytes, ordered logs, accessed storage keys and state,
account balances and nonces, except explicitly declared changes. Those exceptions
are the real implementation switch, the appended `depositCap` read/initialization,
and normalized executor/initializer bookkeeping—not arbitrary new fields or logs.
See [R5 declared changes](../../rules/r5/README.md#declared-changes-i).

## 3. Check locally before deployment or proposal

Use Linux/WSL, pinned Rust and Foundry, and the repository's existing libraries.
Compiler settings: solc `0.8.38`, Prague, optimizer 200 runs, bytecode hash `none`,
CBOR metadata off; no FFI. Verifier tools are Foundry `1.8.6`, Halmos `0.3.3`,
upgrades-core `1.47.0`, Z3 `4.12.6.0`, Yices `2.6.4`.

```sh
forge build --root contracts
constitution rules hash rules/demo/manifest.json
forge test --root contracts --match-contract VaultTest
forge test --root contracts --match-contract VaultV3Test
```

[I] These are local commands to run, not newly observed passes. Existing tests
exercise the supplied implementations; they do not automatically select a newly
written contract. Add candidate-specific tests using the existing executor/proxy
pattern in `contracts/test/VaultV3.t.sol`: seed balances, execute the exact proposed
initializer, compare storage/returns/logs, test withdrawals, overdraw and repeated
initialization. Run those tests by their actual contract name. Never deploy a
candidate solely because the unchanged preset tests pass.

For the supplied lineage, the production deterministic local integration is:

```sh
VERIFIER_MANIFEST=rules/demo/manifest.json CARGO_BUILD_JOBS=1 \
  flock --close /tmp/constitution-e2e.lock cargo test -p constitution-verifier \
  --test lineage_e2e -- --nocapture --test-threads=1
```

It requires the existing gate wasm, NEAR sandbox, Anvil, Docker and pinned tool
image; setup is in [LOCAL.md](../../crates/verifier/src/LOCAL.md#demo-reference-lineage-check).
It runs R1–R5 for the presets with AI deliberately unavailable, so no Pass
certificate is issued. It is neither a generic `check <candidate>` command nor a
live approval. A custom candidate needs equivalent candidate-specific pipeline
coverage; do not alter committed policy to manufacture that coverage.

## 4. Propose once to the demo gate

[I] Only after authorization and local checks: deploy matching implementation
bytecode on Base Sepolia using the operator's approved deployment procedure.
The CLI does **not** deploy it. Use `--bundle contracts` and a `src/...sol:Name`
identifier; the canonical bundle is at most 2 MiB, at most 128 unique `.sol` files
under `src/`, profile `default`, no symlinks/traversal. Trusted dependencies and
compiler settings come from the verifier, not bundle configuration.

Use GUIDE's environment setup (`DEMO_ACCOUNT`, `DEMO_KEY_FILE`, `VERIFIER_URL`,
`NEAR_RPC_URL`, `BASE_SEPOLIA_RPC_URL`, `EXPECT_COMPOSE_HASH`). Source/private tests
stay in the sealed verifier boundary; deployed bytecode is public.

For a human at a terminal, the same flow is interactive: run `constitution` with no
arguments, switch to **Run** (`3`), pick a preset or enter custom inputs, review the
confirmation, and authorize with `y`. The TUI launches the same `demo` subcommand,
so every check below still applies. Agents should use the headless form.

```sh
# Existing honest V2 preset; run only when the observed current version is V1.
mkdir -p receipts
constitution demo upgrade --bundle contracts \
  --contract src/demo/VaultV2Honest.sol:VaultV2Honest \
  --impl 0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005 \
  --init "$(cast calldata 'initializeV2(uint256)' 1000)" \
  --account "$DEMO_ACCOUNT" --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --output receipts/honest-v2-001 --watch 900
```

A new output directory is mandatory. It retains `submission.json` (proposal id,
observed NEAR transaction), `run.json`, observed `receipt.json` when readable, and
`failure.json` on failure. Proposal fees are real testnet credit charges; required
credit includes refundable reservation plus non-refundable storage and a new-account
entry fee. The CLI funds a shortfall when the signing key permits it.

Use `demo attack` only to demonstrate refusal, never to relay a rule-violating
candidate. A Signed attack is an error and is not broadcast. Honest success
requires inclusion **and** matching implementation-slot readback, not just Signed.

## 5. Read the verdict; choose the correct recovery

```sh
constitution status "$PROPOSAL_ID" --gate constitution-demo.testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --run "$RUN_ID" --watch 900
constitution receipt "$PROPOSAL_ID" --gate constitution-demo.testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$VERIFIER_URL" --run "$RUN_ID" --network testnet --json
```

`Pending → Rejected` records a signed verifier refusal, releases reservation,
and never requests MPC signing. `Pending → SignRequested → Signed` means gate
signature creation, not EVM execution. `SignFailed` requires MPC diagnosis.
Expired proposals cannot be certified. Certificate TTL is at most **300 s**;
relay promptly through GUIDE's `relay` command when appropriate.

All numeric rule/reason pairs, gate `E..` panic prefixes, operational refusals and
fixes are in [references/refusals.md](references/refusals.md). Important distinctions:

- Rule 0/1 exists in the specification, but the current source-commitment mismatch
  path is `bad_bundle`, no certificate. Never invent an on-chain rejection.
- Incomplete required AI review is **not** R6 rejection or approval: no certificate,
  proposal stays Pending. AI runs only after R1–R5 pass; a reproduced AI veto is R2/22.
- Retry a recoverable verifier run with `constitution verifier run` on the same
  Pending proposal and identical canonical bundle, after the indicated wait.
  A code fix needs a new bundle, deployment and proposal; never overwrite history.
- If submission is uncertain or `submit: Incomplete`, inspect gate/relay transactions
  first. Do not automatically repropose, resubmit, relay an attack or retry spending.

## Honest limits — include these in every security explanation

[I] These are declared design limits, not newly proven protections:

- Only the configured proxy with fully migrated authority is covered. Keys and
  allowances outside the gate, other contracts, Safe arbitrary transactions,
  frontend supply-chain attacks and signing-device malware remain outside it.
- R1–R5 are bounded, vault-specific checks, not full formal verification or an
  audit substitute. R2 tests aggregate solvency, not every depositor's ability to
  withdraw. Untested entry points, delayed/environment-dependent drains and
  computed-slot storage writes can escape coverage.
- R5 depends on authenticated witnessed state and a pinned header policy, and
  replays an all-zero certificate id with maximum deadline. Initializers can
  distinguish that executor context from a real certificate. Gas is not compared.
- MPC honesty/availability and TEE vendor/attestation trust remain. Attestation
  identifies measured execution; it does not prove the rules complete or RPC data
  honest. The AI receipt is shape-checked in Rust; do not claim end-to-end Rust
  cryptographic verification of that receipt.
- AI is **veto-only**: a Clean review cannot override a deterministic failure.
  Timeout/error withholds certification. An honest deterministic pass alone cannot
  sign while required AI is incomplete.
- Governance is delayed, not impossible: demo delay 1 day, final delay 3 days;
  governor may schedule public changes, guardian can pause/cancel. A hostile
  guardian can halt indefinitely. No CLI switch removes these trust assumptions.
- Keyless claims are per gate: read the current audit. The demo gate's key was
  deleted on 2026-11-12 (audit exit 0); that does not prove any other deployment.
