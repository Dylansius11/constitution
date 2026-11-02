# Shared types dependency evidence

[V] Read from the crates.io sparse index on 2026-11-03. These are the direct
versions pinned in the workspace and consumed by this crate, not testnet evidence.

| Dependency | Version | Index evidence | Features used |
| --- | --- | --- | --- |
| borsh | 1.8.1 | https://index.crates.io/bo/rs/borsh | derive, unstable__schema |
| serde | 1.0.229 | https://index.crates.io/se/rd/serde | derive |
| schemars | 0.8.22 | https://index.crates.io/sc/he/schemars | default derive |
| near-account-id | 2.6.0 | https://index.crates.io/ne/ar/near-account-id | serde, borsh, schemars-v0_8, abi |
| sha2 | 0.11.1 | https://index.crates.io/sh/a2/sha2 | no default features |
| base64 | 0.23.1 | https://index.crates.io/ba/se/base64 | alloc, no default features |
| serde_json (dev) | 1.0.154 | https://index.crates.io/se/rd/serde_json | default std |
| proptest (dev) | 1.12.0 | https://index.crates.io/pr/op/proptest | std, no default features |

[V] near-account-id 2.6.0 was the latest non-yanked stable 2.x entry. proptest
1.12.0 was the latest non-yanked stable entry. borsh and schemars use the versions
specified by the assignment; sha2 uses the architecture pin. The remaining entries
use the latest non-yanked stable versions observed in the index.

[V] near-sdk 5.29.1's index entry requires near-account-id `^2.3.0` with `serde`
and `borsh`, and schemars `^0.8.22` for non-wasm ABI generation. Its
`__abi-generate` feature enables `abi` and `near-sdk-macros/__abi-generate`.
Its core dependency is `near-sdk-core ~4.2.3`.

[V] The re-export chain is visible at:

- https://docs.rs/near-sdk/5.29.1/src/near_sdk/types/mod.rs.html, line 5:
  `pub use near_sdk_core::types::*;`
- https://docs.rs/near-sdk-core/4.2.3/src/near_sdk_core/types/mod.rs.html, line 7:
  `pub use near_account_id::{self as account_id, AccountId, AccountIdRef};`

[I] The exact 2.6.0 pin satisfies the SDK's range, making a unified Cargo resolution
use the identical AccountId definition. This source/range evidence establishes
plausible interchangeability; it is not a gate integration test.

[I] schemars is always-on. A dependency-local `abi` feature would not automatically
be enabled by cargo-near enabling the gate's `near-sdk/__abi-generate`. Argument
and view types therefore always implement the SDK-compatible schemars 0.8 trait.
The library contains no networking or filesystem operations; proptest is dev-only.

[I] Storage and certificate fields retain the raw array, Vec and u128 types in the
specification. JSON adapters use hex for addresses, hashes and raw transactions,
base64 for Ed25519 keys, and decimal strings for amounts. ProposeArgs uses the
explicit Hex newtypes. Base64Bytes transports canonical Borsh bytes without
parsing them; Certificate::from_canonical_bytes is the exact-consumption boundary.
GateError contains the 28 named panic prefixes found in SMART-CONTRACT, including
the distinct E30 and E52 not-found prefixes; unnamed numeric gaps are not invented.

[I] BorshSchema is also always-on: near-sdk ABI generation requires a schema for
contract state, including Config, Constitution and Roles. Enabling borsh's
unstable__schema and near-account-id's abi features provides the matching trait
for all shared Borsh-serialized models and AccountId. A dependency-local ABI
feature would have the same activation problem as feature-gating JsonSchema.
Byte wrappers forward their schema declaration and definitions to the raw
array, Vec or u128 type, so their Borsh schemas are exactly wire-transparent.
JSON string schemas remain independent of Borsh's raw-byte schemas.

[V] near-account-id 2.6.0 gates its AccountId BorshSchema derive on abi:
https://docs.rs/crate/near-account-id/2.6.0/source/src/account_id.rs.
