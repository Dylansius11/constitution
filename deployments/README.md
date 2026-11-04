# Audit deployment records

[I] Activation is planned, not run. There is deliberately **no `testnet.json`**: no production gate/executor deployment has been activated. Never copy spike addresses or the CLI's local sample into an activation record.

`testnet.schema.json` describes the JSON consumed by `Deployment`, `DeployedVerifier` and `PlannedChange` in `crates/cli/src/audit.rs`. The CLI does not fetch or execute the schema; it deserializes with the canonical Rust types. Unknown fields are rejected at these three record envelopes. The schema uses JSON Schema draft 2020-12; integer bounds are unsigned 64-bit. Preserve integer precision (not JavaScript floating-point numbers) when editing nanosecond timestamps.

## Fields

| Field | Wire value | Meaning / how to obtain it |
| --- | --- | --- |
| `chain_id` | required u64 JSON integer | EVM chain id; activation targets Base Sepolia `84532`. Audit compares the record with the gate and EVM RPC. |
| `gate` | required NEAR account id | Fresh real gate account, not the spike gate. Lowercase NEAR account syntax, 2–64 characters. |
| `gate_code_hash` | required string | NEAR base58 `view_account.code_hash`, committing to the reproducible gate wasm (SHA-256 bytes encoded as base58). Do not use a hex SHA-256 or an unverified RPC value as the build commitment; compare independently with the reproducible build. |
| `mpc_account` | required NEAR account id | MPC account bound by the gate config; planned activation uses `v1.signer-prod.testnet`. |
| `executor` | required lowercase `0x` + 40 hex digits | Actual `ConstitutionExecutor` address observed from deployment. |
| `executor_runtime_hash` | lowercase `0x` + 64 hex digits | Keccak256 of the local Foundry deployed runtime after zeroing only compiler-declared immutable regions. Required for a passing audit. Missing/null is accepted at decode solely to produce named `FAIL executor_code_is_committed_build`, never to skip this check. See the exact command below. |
| `proxy` | required lowercase `0x` + 40 hex digits | Actual protected proxy address from the executor deployment. ProxyAdmin is not a record field: audit reads and cross-checks it through the gate, executor and ERC-1967 slot. |
| `verifiers` | required array | Exact committed verifier allowlist, including no extras. Each entry has only `pubkey` and `release_hash` below. An empty list represents no verifiers, not a successful attestation. |
| `verifiers[].pubkey` | required standard padded base64 of 32 bytes | Verifier Ed25519 public key; obtain from the running CVM and independently appraise it before registration. Not a secret key. |
| `verifiers[].release_hash` | required lowercase `0x` + 64 hex digits | Release measurement bound to that verifier key; obtain from appraised evidence. Audit compares the allowlist; it does not itself appraise the CVM. |
| `roles` | optional object `{governor, guardian}` of NEAR account ids | Governor and guardian committed at activation. When present, `gate_roles` FAILs if the gate reports different accounts; when absent the audit only prints them. Commit it in the ceremony (review R3-7). |
| `planned_changes` | optional array, default `[]` | Only exact committed governance changes expected at audit time. Omitting it never hides pending changes. Each entry has only `id`, `kind`, `eta_ns`. |
| `planned_changes[].id` | required u64 JSON integer | Observed gate change id. |
| `planned_changes[].kind` | required canonical `ChangeKind` JSON | Exact enum and payload scheduled on the gate (see below), not a free-form label. |
| `planned_changes[].eta_ns` | required u64 JSON integer | Exact observed gate ETA in **nanoseconds**, not seconds or a quoted decimal. Only matching all three of id/kind/ETA marks a change planned. |

### Produce `executor_runtime_hash`

[I] Planned offline commands, from the repo root, using the pinned Foundry/solc and intended committed source:

```sh
forge build --root contracts --offline
cargo run -p constitution-cli -- executor-runtime-hash --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json
```

Copy the helper's final lowercase `0x` hash into the observed deployment record before committing it. This is **not** the unmasked `cast codehash` of an instance: the constructor embeds proxy and ProxyAdmin immutables. Audit independently requires both the committed hash and the observed masked runtime to equal the local artifact's masked hash, with identical byte length. Compiler metadata and every non-immutable byte stay hashed. The artifact and record are trusted audit inputs, not independent source-provenance proof.

### Governance payloads

`kind` is `"Unpause"`, or exactly one externally tagged object:

| Variant | Payload fields |
| --- | --- |
| `SetRuleSet` | `rule_set_hash` (32-byte lowercase hex), `manifest_uri` (string), `ai_required` (boolean) |
| `AddVerifier` | `pubkey` (32-byte base64), `release_hash` (32-byte lowercase hex), `attestation_uri` (string) |
| `RemoveVerifier` | `pubkey` (32-byte base64) |
| `SetParams` | `params` (object described below) |
| `SetGovernor`, `SetGuardian` | `account` (NEAR account id) |
| `MigrateAuthority` | `new_signer` (20-byte lowercase hex) |

`SetParams.params` requires all eleven fields: `proposal_deposit`, `max_fee_per_gas`, `max_priority_fee_per_gas`, `fee_cap` are decimal **strings** representing u128 amounts (yoctoNEAR for the deposit, wei for fees); `proposal_ttl_s`, `max_cert_ttl_s`, `sign_validity_s` are u64 seconds; `gas_limit` is u64 gas; `sign_gas_tgas`, `callback_gas_tgas` are u64 Tgas. The schema describes wire values, not gate safety-policy acceptance of a scheduled change. Shared `ChangeKind`/`Params` deserialization tolerates unknown payload fields; the three audit record envelopes remain strict.

## Audit ceremony and local coverage

[I] See [`scripts/activate.md`](../scripts/activate.md) for the exact planned audit command and key-deletion gates. Before deletion, only `gate_has_no_access_keys` may FAIL (exit 1). After deletion, expect 12 PASS preconditions (the new one is `gate_operating_surplus`, at least 1 NEAR) plus 3 PASS information lines and `audit passed: 15 named checks/information lines` (exit 0).

An unplanned `MigrateAuthority`, `AddVerifier`, `RemoveVerifier`, `SetRuleSet`, `SetGovernor` or `SetGuardian` prints `WARN pending_authority_change`, its exact `eta_ns`, and the loud warning summary. Pause prints `WARN signing_paused`; WARN alone exits 0, while any FAIL still exits 1. A pause never revokes already signed EVM transactions.

`crates/cli/tests/audit_anvil.rs` exercises the real hardened gate wasm in near-workspaces and real `Deploy.s.sol` contracts in anvil. The **only** offline double is the MPC `derived_public_key` view, with strict predecessor/path/domain checks and an archived vector key; it is not fresh MPC/testnet evidence. Tests delete the sandbox gate's real key, schedule real governance changes and call the guardian's real pause method. Sandbox account creation/signatures and unlocked anvil transactions are disposable local test actions only. The CLI's sample record and actual local integration record are checked against this schema by a small dependency-free schema-keyword checker in `crates/cli/tests/fixtures/deployment_schema.rs`; negative cases cover missing/unknown fields and malformed values. No real deployment record is generated by these tests.
