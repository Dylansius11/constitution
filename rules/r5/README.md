# R5: bounded differential replay

## Contract and integration [I]

`constitution_verifier::replay` is a library. The caller parses the committed file with `Corpus::from_json(bytes, manifest.r5.max_txs, manifest.r5.confirmations)`, compares `corpus.sha256` with the manifest commitment, supplies an independently pinned `EvidenceBlock { number, hash }`, calls `fetch_witness(rpc_url, block, &corpus, proxy, new_impl, new_impl_codehash, init_calldata)` and then pure `replay`. Fetch errors and `(Incomplete, 51)` must never issue a pass certificate. `(Fail, 50)` is rule 5 behavior divergence.

The corpus is an ordered **stateful** sequence of calls, not independent eth_calls. All target the protected proxy. Fixed actor `0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266` is a public local-test actor, not a wallet credential. Before selecting the evidence block, mint MockToken to this actor, approve the proxy, and seed a deposit. Amounts are raw token units. Deposit 10, observe balances, withdraw 4, observe balances, and attempt an impossible withdrawal. This is a bounded regression check, not coverage of every behavior or an exploit search.

## Project-owned actor rotation [I; planned, not executed]

Founder authorization and Base Sepolia ETH funding are required before the
transactions below. Use a fresh, dedicated account; do not use the public Anvil
key. Never publish its private key, print it in logs, or put it in an environment
variable. The commands assume the deployed demo's `TOKEN` and `PROXY` addresses
and a Base Sepolia RPC in `BASE_SEPOLIA_RPC_URL`.

1. On the founder's trusted machine, create a password-encrypted Foundry
   keystore interactively: `cast wallet new r5-actor`. Record the **public**
   address shown as `ACTOR=0x...`. Back up the encrypted keystore and password
   separately. Verify chain 84532 with `cast chain-id --rpc-url "$BASE_SEPOLIA_RPC_URL"`.
   Check `cast code "$ACTOR" --rpc-url "$BASE_SEPOLIA_RPC_URL"` returns `0x`;
   do not use an account with delegation or contract code.
2. Founder funds `ACTOR` with enough Base Sepolia ETH for approve and deposit
   transactions and the desired operational reserve; funding amount is founder
   decision D14, not a hard-coded estimate. Anyone can mint the demo MockToken:
   ```
   cast send "$TOKEN" 'mint(address,uint256)' "$ACTOR" 1000 --account r5-actor --rpc-url "$BASE_SEPOLIA_RPC_URL"
   cast send "$TOKEN" 'approve(address,uint256)' "$PROXY" 1000 --account r5-actor --rpc-url "$BASE_SEPOLIA_RPC_URL"
   cast send "$PROXY" 'deposit(uint256)' 100 --account r5-actor --rpc-url "$BASE_SEPOLIA_RPC_URL"
   ```
   Amounts are raw units. Record actual transaction hashes and successful
   receipts. Confirm `balanceOf(ACTOR)` on the proxy is 100, token balance
   is 900 and token allowance to the proxy is 900, using `cast call` and the
   respective `balanceOf(address)(uint256)` / `allowance(address,address)(uint256)`
   signatures. Stop if any check fails; these balances assume a fresh actor.
3. After the receipts, replace every `from` and both ABI-encoded `balanceOf`
   address arguments in the corpus. From the repository root, with `ACTOR`
   set to the recorded address:
   ```
   jq --arg actor "$(printf '%s' "$ACTOR" | tr '[:upper:]' '[:lower:]')" \
     'map(.from = $actor | .data |= gsub("f39fd6e51aad88f6f4ce6ab8827279cfffb92266"; $actor[2:]))' \
     rules/r5/corpus.json > rules/r5/corpus.new.json
   mv rules/r5/corpus.new.json rules/r5/corpus.json
   sha256sum rules/r5/corpus.json
   ```
   Verify all seven `from` values and both `balanceOf` arguments equal ACTOR
   and no old actor remains. Put `0x` plus the printed digest in
   `rules/manifest.json` at `r5.corpus_sha256`; preserve the final newline
   because the digest commits to exact bytes.
4. Recompute `cargo run -p constitution-cli -- rules hash rules/manifest.json`,
   update every affected committed vector/document, rebuild and attest the
   verifier release, and bind the new rule-set and release hashes in the gate
   setup or its timelocked governance process. Corpus bytes change corpus SHA-256
   and rule-set hash; changed embedded rules/verifier image or configuration
   changes release/measurement commitments as applicable. Do not reuse an old
   attestation or claim a hash without computing it. Select each evidence block
   only after seeding and the required confirmations.

All seven `from` fields use live account nonce, balance and code at the pinned
block; the two `balanceOf` calldata arguments also depend on the actor's vault
balance, while deposit depends on token balance and allowance. Aggregate
observations depend on all vault activity. The owned actor reduces unsolicited
actor-state changes; it does not freeze shared vault state or eliminate RPC
provider disagreement.

## Declared changes [I]

Only these exact changes are exempted:

- The proxy ERC-1967 implementation slot is replaced with a virtual candidate implementation address, and the candidate runtime exists at that address. The witnessed current runtime must match `current_runtime` exactly. Both executions still run the witnessed transparent proxy bytecode.
- The appended vault `depositCap` slot, `0x199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416803`, may be read during corpus calls; its initial value must have an authenticated witness. Nonempty initialization requires this slot to be zero and may set it. Corpus calls may not change it.
- During nonempty initialization only, the OpenZeppelin Initializable namespace must transition from version 1 to version 2, and initialization logs/return bytes are not compared to V1. Successful initialization may not change other account state or storage. Its sender nonce increment is excluded by restoring the sender's nonce after this simulated installation transaction. Corpus calls may not alter this metadata or the installed implementation slot.

There is no wildcard for new state, addresses, or slots. R1 still separately proves append-only layout; R4 still separately restricts initializer calldata. Every corpus call must have identical success/revert status, return/revert bytes, ordered logs (including emitter, topics and data), accessed storage-key set (except cap/implementation), storage values, account balances and nonces. Gas consumption is deliberately not compared; the fixed local call budget is 3,000,000 gas. Out-of-gas/halt is incomplete, not evidence of equivalence. Gas charging is disabled to avoid rewarding an upgrade merely for changing gas cost. Corpus executions use the pinned block environment, chain id 84532 and Prague EVM semantics. Unknown BLOCKHASH requests are incomplete.

An unknown account or slot is **never** assumed empty or zero. Added state still needs a state-root proof even when its value is zero. Candidate initialization executes through the actual proxy after changing its implementation slot, before the corpus. Empty initialization is allowed but does not waive behavioral equality.

## Witness provenance and limits [I]

Fetch gathers `debug_traceCall` / `prestateTracer` observations for each corpus entry at the same pinned block, then obtains `eth_getProof` for the union of observed accounts/slots. It additionally requests proofs for the exact declared initialization slots. Tracer values are discovery hints only; account balance/nonce/code and storage values are validated against Merkle Patricia proofs rooted in the evidence block's stateRoot. Code bytes are checked against the proved code hash. Inclusion and exclusion proofs are both checked. Traces are independent calls at the evidence block; replay is stateful, so a later state-dependent access missed by discovery is incomplete, not a zero-filled fallback. A caller may supply supplemental **proved** prestate; pure replay validates it identically. There is no production candidate-discovery fallback or fixture-specific storage exemption.

The block number/hash is supplied by the integration, not trusted from a proposal's private bundle. The execution header is RLP-encoded in [Ethereum header order](https://github.com/ethereum/go-ethereum/blob/master/core/types/block.go) and its keccak hash must equal that independently pinned hash; both fetch and pure replay revalidate it, and replay checks that its environment/stateRoot match the authenticated header. Confirmations and a final canonical-hash recheck guard provider races; proof verification authenticates state relative to that header, **not the header's chain-consensus provenance**. Integration must pin/check the block using its trusted evidence policy. Validation Cloud archive-provider execution remains [U] until exercised against its authenticated endpoint. Unknown header formats fail closed. No endpoint or credential is committed here.

## Local proof

Run in WSL with pinned Rust and Foundry on PATH:

```sh
CARGO_BUILD_JOBS=6 cargo test -p constitution-verifier --test replay -- --nocapture
cargo clippy -p constitution-verifier --all-targets -- -D warnings
cargo fmt -p constitution-verifier -- --check
```

The integration test starts disposable anvil with chain id 84532, deploys via `contracts/script/Deploy.s.sol` using an unlocked local account (no key files), seeds deposits, pins a block, obtains witness proofs and replays real compiler-produced runtimes. The changed-withdraw fixture only adds an observable log to a successful withdrawal. Production `fetch_witness` discovers accesses for the current runtime, for the candidate runtime (fetched at `new_impl`, keccak checked against the proposal codehash, placed by code override at the implementation address) and for the initializer; tracer values only select which slots to prove, and replay uses only values proved at the original block. The process is killed on test exit. No testnet writes occur.

### Observed local evidence, 2026-11-03

- `cargo test -p constitution-verifier --test replay -- --nocapture`: **2 passed**, 0 failed; 26.79 s including local deployment, compilation, RPC discovery/proofs and all negative cases. The seven-call honest differential replay itself, including proof/header revalidation, measured **36.188 ms** in the debug build (one run, not a benchmark).
- Anvil chain 84532, evidence block 6: `debug_traceCall` with `prestateTracer` and `eth_getProof` both succeeded. Header hash, account proofs, storage inclusion/exclusion proofs and code hashes were checked; no unsupported-method fallback was needed.
- V1 to honest V2 with `initializeV2(1000000)`: Pass 0. V1 to the neutral R2 Fail fixture through production `fetch_witness`: Fail 50 (follow-up run, 2 passed in 34.06 s). This observation is not a claim that R5 replaces R2.
- Testnet note [I]: the committed corpus actor is anvil's account 0, whose private key is public, so its Base Sepolia state is not under project control. Activation needs a corpus whose actor is an account the project seeds on Base Sepolia, and a fresh `corpus_sha256`.
- Changed withdrawal logs and an initializer changing pre-existing state: Fail 50. Missing account/slot, changed unproved storage value, invalid account proof, altered code/header/environment and an eighth corpus entry: Incomplete 51. The corpus was independently sent to local anvil: six successful calls, observed balances/totals 110 then 106, and the final reverting withdrawal.
- `cargo clippy -p constitution-verifier --all-targets -- -D warnings` and `cargo fmt -p constitution-verifier -- --check`: exit 0. No workspace-wide test suite or testnet transaction was run by this worker.

## Committed manifest values [I]

[V, local CLI, 2026-11-06] The previous 300 s manifest's rule-set hash was
`0xa8a4bfb14f17a60b8dac20aaba63dfda2f89152fe7b7d4829f9d33edbb2cbf93`.
[I] Recompute the changed 600 s manifest with
`cargo run -p constitution-cli -- rules hash rules/manifest.json`.
The R5 corpus bytes and bounds below are unchanged.

```json
{
  "r5": {
    "corpus_sha256": "0x8ee8b040243abe7eddb3f6af99f67cdfa2a9af445b17f87c60f5295869cfacc4",
    "max_txs": 7,
    "confirmations": 2
  }
}
```

The hash commits to the exact 1,276-byte file, including its final newline. Seven entries preserve state observations before and after the successful withdrawal and include an explicit revert without pretending to cover arbitrary sequences. Two successor blocks are a minimal demo evidence-lag policy, not a Base finality claim. The fetcher checks this lag and rechecks the pinned block hash after proof collection. The pipeline maps Fail to Reject{5,50}; repeated matching authenticated contradictions may yield Reject{5,51}. Provider, committed-data and other environmental failures withhold the certificate rather than becoming a rejection. The stage detail includes a non-secret reason and never the endpoint URL or provider response body.

Dependency: `revm = 43.0.3`, non-yanked, MSRV 1.91.0, verified from the official [sparse index](https://index.crates.io/re/vm/revm) on 2026-11-03. Existing workspace pins provide reqwest 0.13.5, hex 0.4.3 and rlp 0.6.1; no additional versions are introduced.
