# Gate integration spike

Throwaway P1.1, P1.2 and P1.4 experiment. This is not the production gate and
MUST NOT become the activated gate. `spike_sign` accepts arbitrary hashes and is
intentionally unsafe for production. NEAR writes target `constitution-spike.testnet`
only. P1.4 Stage A targets a local Base Sepolia fork; live Base writes require
the lead's separate Stage B authorization and testnet funding.

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
- `sign_execute` builds and hashes the executor transaction inside the contract,
  then its private `on_execute_signed` callback validates the observed MPC
  response and reconstructs the raw signed type-2 transaction with
  omni-transaction. ECDSA r/s are encoded as minimal RLP integers, y-parity is
  restricted to 0 or 1 and s must be low. Callback failures return a recorded
  failure result, not a signed transaction.
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

## P1.4 driver

The Python standard-library driver uses pinned Foundry binaries on PATH and the
repo-local NEAR CLI. It keeps Foundry compiler cache, artifacts and broadcasts
under this spike's ignored `target/driver/`, never under the read-only
`contracts/` paths. No private key or password is loaded or printed by Python.

Stage A uses an Anvil unlocked development account and synthetic fork-only
funding of the spike-derived EOA:

```sh
export PATH="$HOME/.foundry/bin:$PATH"
anvil --fork-url https://sepolia.base.org --chain-id 84532 --port 18545 --host 0.0.0.0 --silent
# In another terminal, from the repository root:
CARGO_BUILD_JOBS=4 cargo near build non-reproducible-wasm --manifest-path spikes/gate-min/Cargo.toml
.tools/bin/near contract deploy constitution-spike.testnet use-file spikes/gate-min/target/near/gate_min.wasm without-init-call network-config testnet sign-with-legacy-keychain send
python3 spikes/gate-min/driver.py --mode fork --rpc-url http://127.0.0.1:18545
```

Each run deploys a fresh executor/proxy via `contracts/script/Deploy.s.sol` and
a fresh VaultV2Honest. It requests real NEAR MPC signatures for the honest
upgrade and expired-deadline cases, verifies each signing hash against an
independent Python ABI/RLP computation, verifies the signer with cast, and
relays the callback's exact raw transaction. The honest upgrade includes
`initializeV2(1000e18)`. The driver requires success, exact ERC-1967
implementation slot, exact UpgradeExecuted topics/data, initialized cap,
duplicate-send rejection and nonce consumption. The expired transaction must
be included with status zero, consume its nonce, leave the implementation
unchanged and reproduce the DeadlinePassed selector at its inclusion block.

Evidence is written to `../../vectors/evm-tx-spike.json`, labeled
`local fork of Base Sepolia, real MPC signature`. Fork results are not live
Base Sepolia evidence.

Only after the lead explicitly authorizes Stage B and confirms testnet ETH:

```sh
python3 spikes/gate-min/driver.py --mode testnet --rpc-url https://sepolia.base.org --relay-url https://sepolia-sequencer.base.org --output spikes/gate-min/base-sepolia-evidence.json
```

Stage B never calls `anvil_setBalance`; Foundry signs deployment transactions
through `.secrets/evm/deployer` and its password file. The RPC chain id must be
84532 in both modes. Keep the fork vector intact when recording live evidence.

[V] The sequencer endpoint rejects read methods: `cast rpc --rpc-url
https://sepolia-sequencer.base.org eth_chainId` returned HTTP 403 with JSON-RPC
`-32601`, `rpc method is not allowed`. The driver therefore separates the read
RPC from the raw-transaction relay, including the duplicate-send check. RPC
error bodies on non-200 HTTP responses remain errors, not success fallbacks.

### Stage A observed result

[V] `python3 spikes/gate-min/driver.py --mode fork --rpc-url
http://127.0.0.1:18545` completed both cases. This is a local fork of Base
Sepolia at block 53606308, with real NEAR MPC signatures. No live Base
Sepolia transaction was sent and no production gate account was touched.

- Spike redeploy: `7p2Ju2f5ksEUJiJXmiXBuzspUc5a8ijizVNgFQu8Dvdx`.
- New local wasm: 164236 bytes, SHA-256
  `0d1adc563840217e829fb5a0d368610b7dec87787cfa162cad80d0652fff33e9`.
- Executor: `0xa44d05793c89be410c93647fd08f3523444681ae`.
- Proxy: `0xb0c3b305fea795e4bd6081c7880f000846544d9d`.
- Honest implementation: `0x7c03601778461ce80bac321297b65756f34418ca`.
- Honest NEAR signing transaction:
  `6f1G8LktpqCLxWJmdtJnjEAZe7JgDjbshZe1Pj1AqLqb`.
- Honest fork transaction:
  `0x62a8b7392f00453e67ba383bd52d978c45704e620721ddea96539e5ddecfcdf0`,
  status 1, gas used 105159, nonce 0 to 1. The implementation slot and exact
  UpgradeExecuted event match the new implementation, and depositCap is
  1000e18. Repeating the raw transaction returned `-32003`, `nonce too low`.
- Expired NEAR signing transaction:
  `2PVrxh9hcE5rcRThqfSii9ixUAF4HvdfqyVsFSjBXmKE`.
- Expired fork transaction:
  `0xc5c4c45608e6ae6a1ee83e26dc008d6c07598ededd45a7305cc00ad17ef589a3`,
  status 0, gas used 28900, nonce 1 to 2. Inclusion-block replay returns
  DeadlinePassed selector `0x3dee80f2`; the implementation is unchanged.

The vector contains complete inputs, preimages, signing hashes, MPC responses,
raw signed transaction bytes, receipts, events, duplicate error and revert
data. It is sufficient to independently recover both senders and recompute
the unsigned and signed transaction hashes.

### Stage B observed result (live Base Sepolia)

[V] 2026-11-03 UTC, founder-authorized. The deployer (funded through the
official Base bridge, OPEN-DECISIONS B2) sent 0.002 ETH to the spike-derived
EOA (`0x2b52e1407a2da2d82ee7b93321cfd5e2f19542183ed78b3b624edf86409685c2`),
then `driver.py --mode testnet` ran both cases with real NEAR MPC signatures
and relayed through `sepolia-sequencer.base.org`. Complete inputs, preimages,
MPC responses, raw transactions, receipts and RPC responses:
`base-sepolia-evidence.json`.

- Deploy (forge, keystore signer): MockToken `0xe92ae79ea060f027804e11687b1ac2546cf570a2`
  (`0x16d577ab…54de1`), VaultV1 `0xfb842dfa1fca052fecf02ed8b31822a403c44e2f`
  (`0x8b90cf0a…7409f`), ConstitutionExecutor
  `0xf26e7a09f0b4b22ec964f7cf23fd1b8a543851e8` (`0xccaddd37…fcbc3`) with signer
  `0x8221c9477886cc456d7e1cccc0866e0401b81070`; proxy
  `0xef855da4d6f2e1c7814f1898fc5771e3bba8b569`, ProxyAdmin
  `0xe1531f314cf21ca10ec0a93fbd167bf17b791316`; VaultV2Honest
  `0x92aba426dc77a914b07dfefbed3f57e020436e96` (`0xf87b4fc0…351c5`).
- Honest upgrade: NEAR signing `55bekU9mqsWr8VP5TQp5xzViHxGLTUH5b55mduo3azSy`;
  Base `0xc232188629763bec4eb9a72be7fd08f66ab5690a2ad997ac6ed20697ff0729d8`,
  block 53766464, status 1, 105,159 gas, 3 logs, nonce 0 to 1. Resending the
  same raw transaction returned `-32000 nonce too low: next nonce 1, tx nonce 0`.
- Expired deadline: NEAR signing `5m3wpnxMQQTenexs5yCmPF1WEUMC43oiBMj4WWVwX36i`;
  Base `0x4e095a6be399e32cd5e72dc5140a433c128716e6dfe6f2c92191f4ca2c0afee3`,
  block 53766644, status 0, 28,900 gas, nonce 1 to 2; replay at the inclusion
  block returns `DeadlinePassed` (`0x3dee80f2`); implementation unchanged.
- Independent re-read with cast after the run: ERC-1967 implementation slot =
  VaultV2Honest, `ProxyAdmin.owner()` = executor, spike EOA nonce 2, honest
  receipt sender `0x8221…1070`, `depositCap()` = 1000e18.
- D14 observation (one run): base fee 5,000,000 wei, suggested priority fee
  1,000,000 wei, max fee 11,000,000 wei, effective gas price 6,000,000 wei,
  L1 data fee 501 wei (honest) and 455 wei (expired). Not a worst-case bound.

### D14 fee observations, not activation caps

[V] The public source RPC at Base Sepolia block 53612404 reported base fee
5000000 wei and suggested priority fee 1000000 wei. The Anvil fork separately
suggested priority fee 1000000000 wei. Do not confuse Anvil's default suggestion
with the source network's suggestion.

The honest fork transaction used max fee 1009731712 wei and effective gas price
1004846875 wei. The expired fork transaction used max fee 1009693750 wei and
effective gas price 1004827590 wei. Both used gas limit 300000. These are
observations of one run, not a production worst-case bound or funding policy.

Initial driver attempts failed before MPC signing due to the public RPC
rejecting Python's default User-Agent, then due to forge create using `--from`
where forge script uses `--sender`. The final driver uses a project User-Agent
and the correct command-specific flags.
