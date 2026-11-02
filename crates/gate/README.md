# Gate credit, proposals and governance

[I] This crate implements BUILD-PLAN P2.2 and the unsigned P2.4 changes. Signing and verdict delivery belong to P2.3. A matured `MigrateAuthority` is refused with `E51_INVALID_CHANGE: MigrateAuthority apply lands with signing`; scheduling and cancellation work normally. This is sequencing, not a deployable replacement for the complete gate.

## Storage and lifecycle

The shared types in `constitution-types` define all protocol bytes. The gate uses Borsh state and unique Borsh enum collection prefixes. Two internal indexes approved by the lead supplement SMART-CONTRACT's storage sketch:

- `active_by_key: LookupMap<[u8;32], u64>` hashes `new_impl_codehash || init_calldata` with sha256. It enforces duplicate rejection without scanning historical proposals. Pending, SignRequested and SignFailed are active; terminal release removes both active indexes.
- `outstanding_signatures: IterableSet<u64>` indexes SignRequested and Signed proposals. Rule-set application checks only these ids and prunes Signed records with `deadline_s <= now_s`. P2.3 must maintain this index on signing transitions.

A proposal reserves unreserved account credit at creation. Expiration is permissionless strictly after `expires_at_ns`, zeroes the reservation, refunds account credit and clears the active indexes. Withdrawals debit before transferring to the predecessor. Proposal admission and credit operations do not depend on signing pause.

## Interpretation of underspecified interfaces

[I] Initialization and SetParams share the specified `sign_gas_tgas >= 15` validation. Proposal TTL must convert to nanoseconds without overflow. The MPC path and domain are fixed to `constitution/base-sepolia/v1` and 0, matching the immutable protocol identity. Initialization failures use E51 because no separate initialization error is specified.

[I] Manifest and attestation URIs must be nonempty and at most 256 UTF-8 bytes. Adding an existing verifier and removing an unknown verifier fail E51; removing the last verifier fails E57 through either governance path. Apply revalidates against current verifier membership, since queued changes may overlap with guardian removals. Verifier addition records the actual apply time.

[I] Ids start at zero. Getters return null for absent proposal, change or active id; absent credit is the string `"0"`. Lists are ascending ids with `from_id` inclusive and a clamped maximum of 50 results. `list_verifiers(from_index, limit)` paginates iterable storage order. Guardian events without an associated change use `change_id: null`; no synthetic change id is emitted.

## Local checks

Run in WSL2 from the workspace, with the pinned Rust toolchain available:

```sh
(cd crates/gate && cargo near build non-reproducible-wasm)
cargo test -p constitution-gate
cargo clippy -p constitution-gate --all-targets -- -D warnings
```

The build emits `target/near/constitution_gate/constitution_gate.wasm` and the sibling `constitution_gate_abi.json`. The integration tests require these artifacts and fail rather than silently skip if missing. Sandbox tests are local evidence only. The ABI allowlist includes the SDK-generated read-only `contract_source_metadata` method and no upgrade/key/account/signing escape hatch.

Tests map credit and active proposal admission to T-GATE-1, timelock boundaries to T-GATE-6, role separation to T-GATE-7, and constructed signing states to T-GATE-8. P2.3 owns T-GATE-2 to T-GATE-5 and T-GATE-9 to T-GATE-11; this slice only preserves the initial EVM nonce.

## Dependency provenance

[V] Sparse registry entries read on 2026-11-03: near-sdk 5.29.1 (`https://index.crates.io/ne/ar/near-sdk`), near-workspaces 0.23.0 (`https://index.crates.io/ne/ar/near-workspaces`), tokio 1.54.1 (`https://index.crates.io/to/ki/tokio`). Existing workspace serialization pins are reused. Unit tests enable near-sdk's `unit-testing` feature only as a dev-dependency. Deployed wasm still requires the lead's reproducible build and activation ceremony.

[V] The first local cargo-near ABI attempt on 2026-11-03 rejected `Config`, `Constitution` and `Roles` with E0277 because `near(contract_state)` also requires BorshSchema under ABI generation. Always-on schemars derives are necessary for JSON interfaces but not sufficient for the state schema. The shared-types owner added BorshSchema support; the gate does not redefine protocol types or suppress state schema generation.

[V, locally built 2026-11-03] After the shared BorshSchema change, `cargo near build non-reproducible-wasm` generated both JSON and embedded ABI and built a 304,902-byte optimized wasm. Observed SHA-256: `8938f8a17c227bacaa6e0ec5959a735dc50a43558a0879a24cd93d485da3d4af`. This host build is not the deployment artifact or reproducible-build proof.

[V, locally tested 2026-11-03] `cargo test -p constitution-gate` passed 13 unit tests and 2 integration tests, with no ignored tests. The local near-workspaces sandbox deployed this wasm and exercised initialization, credit deposit, proposal admission, governance scheduling, early-apply E53 refusal and public views. The generated ABI matched the exact 21-method allowlist, including SDK metadata. `cargo clippy -p constitution-gate --all-targets -- -D warnings` completed cleanly. No testnet behavior is claimed.
