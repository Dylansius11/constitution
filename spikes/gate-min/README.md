# Gate integration spike

Throwaway P1.1 and P1.2 experiment. This is not the production gate and
MUST NOT become the activated gate. `spike_sign` accepts arbitrary hashes and is
intentionally unsafe for production. The authorized online target is
`constitution-spike.testnet` only. No Base Sepolia transactions are authorized.

## Scope

- near-sdk 5.29.1, omni-transaction 0.5.0 with EVM only, alloy-sol-types 1.7.3.
- `build_execute` produces the unsigned type-2 preimage and its Keccak signing
  hash for Base Sepolia, chain 84532. Destination is the supplied spike executor,
  value is zero, access list is empty. The call is
  `execute(bytes32,address,bytes32,address,uint64,bytes)`, selector `90c70cd8`.
- `spike_sign` constructs the v2 ECDSA request to `v1.signer-prod.testnet` with
  domain 0, fixed path `constitution/base-sepolia/v1`, 15 Tgas and 1 yoctoNEAR.
  The private callback preserves the exact MPC response bytes in the result and
  a log. Local receipt tests are not evidence of a real MPC response.
- Host-only test code derives the child key with SHA3-256 (not Keccak), then
  `root + tweak * G`, rejecting a noncanonical scalar as upstream does. The EVM
  address is the last 20 bytes of Keccak-256 of the uncompressed x/y bytes.
  k256 and SHA3 are dev dependencies and do not enter the wasm.
- `constitution-types` is a shared-crate dependency. No production
  type interface or production gate is implemented by this spike.

## Re-run locally

From this directory inside WSL2, after `source ~/.cargo/env`:

```sh
CARGO_BUILD_JOBS=4 cargo test -- --nocapture
CARGO_BUILD_JOBS=4 cargo near build non-reproducible-wasm
cargo near build reproducible-wasm
```

The standalone `[workspace]` keeps these dependencies out of the root workspace.
The local toolchain is pinned to Rust 1.99.1. A reproducible build requires the
spike and its inputs committed in the enclosing repository. Never commit build
output or the local `target/` directory.

## Read-only derivation evidence

[V] Captured on 2026-11-03 from `https://test.rpc.fastnear.com`, account
`v1.signer-prod.testnet`, query type `call_function`, finality `final`, domain 0.
Full JSON-RPC requests, raw responses, finalized block heights, root key and
independent offline outputs are in `../../vectors/mpc-derivation.json`.

The view arguments for `derived_public_key` are:

```json
{"predecessor":"constitution-gate.testnet","path":"constitution/base-sepolia/v1","domain_id":0}
```

[V] At finalized block 275927720 the view returned
`secp256k1:2LxkyX7Nuw1MbtVBAHWubggYkZ91pGE2yxuEo7bNuVakGYsBrmnHvAuDQtiVExSFF2RPLeepXsYszq8ynFToa8KZ`.
The corresponding offline-derived EVM address is
`0x3b20f46860d27d81917a45221c594fc051af7e94`. This is derivation evidence only,
not proof that the named gate account exists or controls a deployed executor.

Additional pairs change the path to `constitution/base-sepolia/v2` (block
275927721) and the predecessor to `constitution-spike.testnet` (block 275927722).
All key comparisons are made by offline Rust tests against the captured view.

[V] Source read at near/mpc main commit
`b78f4f5b79faed8cb32d869b726230661fa02255`:

- https://github.com/near/mpc/blob/b78f4f5b79faed8cb32d869b726230661fa02255/crates/near-mpc-crypto-types/src/kdf.rs
- https://github.com/near/mpc/blob/b78f4f5b79faed8cb32d869b726230661fa02255/crates/contract/src/api/keys.rs
- https://github.com/near/mpc/blob/b78f4f5b79faed8cb32d869b726230661fa02255/crates/contract/src/crypto_shared/kdf.rs

The source commit is not proven identical to deployed MPC wasm; the live view
comparison is the observed compatibility check.

## Independent encoding oracle

The fixed test uses dummy repeated-byte addresses and hashes, nonce 7,
300000 gas, max fee 2000000000 wei, priority fee 1000000 wei, deadline 1800000000
seconds, and init calldata `deadbeef`. A hand ABI and hand RLP implementation
independent of alloy and omni checks every preimage byte. A separate Python
standard-library ABI/RLP computation with PyCryptodome Keccak produced the
pinned signing hash:
`f4cabf1bbeafae5b1b847737b03d31ac62d5805cb3204b04faefe5e22bb750c3`.
These values are local fixtures, not observed testnet fee policy or transactions.

## Build evidence

[V] WSL2 Ubuntu, `rustc --version`: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`.
The official pinned release installer installed cargo-near 0.22.0;
`cargo near --version` printed `cargo-near-near 0.22.0`.
`CARGO_BUILD_JOBS=4 cargo near build non-reproducible-wasm` completed with
ABI generation and wasm-opt enabled. Rust 1.99.1 was accepted for near-sdk
protocol 84; the fallback toolchain was not needed.

[V] Final callback implementation wasm: 137861 bytes, SHA-256
`7cb060a6dca9e251f500f4a272d6a834fadd87435ff423b1b4a04459368bfa32`.
This is a non-reproducible local artifact, not a reproducibly verified release.
[I] Retain alloy-sol-types 1.7.3: it builds for wasm and the complete spike,
including embedded ABI and signing callback, is 137.9 KB. There is no observed
compatibility or size blocker requiring a second hand-written production
encoder. The hand encoder exists only as an independent test oracle; no claim
is made that alloy is smaller than that oracle.

[V] Docker Desktop was started and `docker info` inside WSL succeeded.
`docker buildx imagetools inspect sourcescan/cargo-near:0.22.0-rust-1.99.1`
returned digest
`sha256:c9655eaf7436b90c87ea8b25739f91063032c5c30ba24cfeafe72c142943b5bb`.
This exact image and digest are configured in the spike manifest.
`CARGO_BUILD_JOBS=4 cargo near build reproducible-wasm` was attempted but timed
out after 300 seconds at `Checking if git is dirty...`, before pulling an image.
The command's help requires all changes committed. No commit was authorized or
made. After review, commit the spike source, manifest, lock, toolchain, tests,
fixtures, vectors and shared-crate inputs in the enclosing repository, then
retry. Docker integration is not the blocker.

The first host test build failed while archiving aws-lc-sys with
`No space left on device`; `df -h /mnt/c` showed zero free space. Only this
worker's failed target output was deleted, then the lead freed host space.
Dev debug symbols and incremental output are disabled to limit spike storage.

## Online P1.2 evidence

[V] The lead extended authorization to deploy and request one test signature on
`constitution-spike.testnet` only. The production gate account was not touched.

Commands from the repository root inside WSL:

```sh
.tools/bin/near contract deploy constitution-spike.testnet use-file spikes/gate-min/target/near/gate_min.wasm without-init-call network-config testnet sign-with-legacy-keychain send
.tools/bin/near contract call-function as-transaction constitution-spike.testnet spike_sign file-args spikes/gate-min/sign-request.json prepaid-gas "100 Tgas" attached-deposit "0 NEAR" sign-as constitution-spike.testnet network-config testnet sign-with-legacy-keychain send
```

[V] Deployment transaction:
https://explorer.testnet.near.org/transactions/FsFz4zHeeMhEyxDfvdNM1VSgG1VQxGM75zKQgiqsNasr

[V] Signing transaction:
https://explorer.testnet.near.org/transactions/FNd6BUDznLA3wwmdtiCE2F8auBaJmfQMTCDS4g4ro6Rz

The signed payload was the fixed encoding fixture's signing hash
`f4cabf1bbeafae5b1b847737b03d31ac62d5805cb3204b04faefe5e22bb750c3`.
The final callback returned this exact JSON (also `../../vectors/mpc-response.json`):

```json
{"scheme":"Secp256k1","big_r":{"affine_point":"0303a3a381219d87a2d948ca31dd1223f5c8963e62239cf76e2c325fd9ba6bf32d"},"s":{"scalar":"2617462cafebb333c8e8aafe74968f734517916c9bec7da562e9bfc48a66f810"},"recovery_id":1}
```

[V] Offline recovery of that signature yielded
`0x8221c9477886cc456d7e1cccc0866e0401b81070`, equal to the offline derivation
and live `derived_public_key` for `constitution-spike.testnet` and the fixed
path. The different-path and different-predecessor negative tests are offline;
no second on-chain signature was requested.

Read-only `tx` RPC used `wait_until: "FINAL"` and sender
`constitution-spike.testnet`. Complete requests and responses, config view,
recovery output and per-receipt gas are saved in `online-evidence.json`.

| Receipt | Block | Gas burnt | Tgas |
| --- | --- | --- | --- |
| MPC initial `sign` | 275931932 | 4281002230265 | 4.281002230265 |
| MPC completion | 275931938 | 3466865745162 | 3.466865745162 |
| Spike `on_spike_signed` | 275931939 | 1356670645990 | 1.356670645990 |

[V] Attached gas was 15 Tgas for `sign`, 20 Tgas for the callback, and attached
MPC deposit was 1 yoctoNEAR. The live config still required at least 15 Tgas
(config view block 275928365). The CLI's `0.309 Tgas` summary was the transaction
outcome only, not the sum of asynchronous receipts. This single observation
does not establish production worst-case gas limits.

No Base Sepolia transaction was sent. P1.4 execution, nonce consumption,
deadline rejection and a reproducible release build remain unproven.

## Local test result

[V] `CARGO_BUILD_JOBS=4 cargo test -- --nocapture` completed: 9 passed,
0 failed. No root workspace or other worker's tests were run.

- `eip1559_preimage_matches_independent_abi_and_rlp`
- `abi_dynamic_bytes_boundaries_match_hand_encoder`
- `rejects_oversize_init_calldata`
- `rejects_priority_fee_above_max_fee`
- `offline_derivation_matches_all_live_finalized_views`
- `different_path_and_predecessor_change_derived_address`
- `spike_sign_emits_exact_v2_request_deposit_and_gas`
- `callback_preserves_success_bytes_and_records_failure_without_panic`
- `observed_mpc_signature_recovers_to_live_derived_spike_key`
