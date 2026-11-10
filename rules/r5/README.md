# R5: bounded differential replay

## Contract and integration [I]

`constitution_verifier::replay` is a library. The caller parses the committed file with `Corpus::from_json(bytes, manifest.r5.max_txs, manifest.r5.confirmations)`, compares `corpus.sha256` with the manifest commitment, supplies an independently pinned `EvidenceBlock { number, hash }`, calls `fetch_witness(rpc_url, block, &corpus, proxy, new_impl, new_impl_codehash, init_calldata)` and then pure `replay`. Fetch errors and `(Incomplete, 51)` must never issue a pass certificate. `(Fail, 50)` is rule 5 behavior divergence.

The corpus is an ordered **stateful** sequence of calls, not independent eth_calls. It is committed in the object schema below with the project-owned Base Sepolia actor `0x289541ff1cd4882653a02b049ebb5d6a06374826` (see the rotation record). This bounded regression is not coverage of every behavior or an exploit search.

The required JSON object has exactly `preconditions` and `calls`. Preconditions has exactly `actor` (20-byte address), `min_token_balance`, `min_allowance`, `min_vault_balance`, and `max_vault_balance`; amounts are unsigned decimal strings in raw token units, and the vault bounds are inclusive. Each call has exactly `from`, `to` (`"proxy"`), `value`, `data`, `label`, and `expect` (`"success"` or `"revert"`). Unknown or missing fields are invalid. The planned seven calls declare success for deposit 10, the three intermediate views, withdrawal 4 and the final view; the impossible withdrawal declares revert.

Before candidate installation or corpus replay, the verifier executes token `balanceOf(actor)`, token `allowance(actor, proxy)` and proxy `balanceOf(actor)` against independently proof-authenticated original state. An unmet bound is Incomplete (`precondition_unmet`), never a proposal rejection. Every old-side observation must match its declared `expect` before it is compared with the candidate; a mismatch is Incomplete (`expected_outcome_mismatch`), even if both versions revert identically. A failed view or missing proof is also Incomplete. No expected nonce is declared: replay uses the proof-authenticated actor nonce; its code and balance must permit execution.

## Project-owned actor rotation [V, executed 2026-11-10 on Base Sepolia; staging proxy only]

Executed 2026-11-10 with founder authorization on the staging proxy `0x8cb83a61cfb51db342be472cbbdb820ac6207f14` and `MockToken` `0x3e6654a09a0e1e5f0c90ee48f789d5695252b5b5`. The actor's keystore and password live only in the gitignored `.secrets/evm/`. Observed transactions: funding 0.002 ETH from the deployer `0xc0ff2a43e56c08aeec3faf559bc7c70b6f5cf7e78b6d97d93d80af2b827e36fb`; mint 1000 `0x36bea92c5d8ba68361e46039d01e0110373ca28a8687d2856c25918c222d8929`; approve 1000 `0x0864f01c8cfe11b1dfc80bd3b7fc6cd45e4c50d26a9b0e5ef622423f8c9218bf`; deposit 100 `0x1e0f874ddc8110749fb9007a00f8e295d4be122c5548916cfddad37b9cd566d7`. Read back afterward: token balance 900, allowance to the proxy 900, vault balance 100, actor nonce 3, empty code. The procedure below is the reproducible form of what was done; a new vault needs a new rotation.

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
3. After the receipts, create the required object schema, replace every `from`
   and both ABI-encoded `balanceOf` address arguments, and declare the six
   successes and final revert. From the repository root with `ACTOR` set:
   ```
   jq --arg actor "$(printf '%s' "$ACTOR" | tr '[:upper:]' '[:lower:]')" \
     '{preconditions: {actor: $actor, min_token_balance: "900", min_allowance: "900", min_vault_balance: "100", max_vault_balance: "100"}, calls: [to_entries[] | .key as $i | .value | .from = $actor | .data |= gsub("f39fd6e51aad88f6f4ce6ab8827279cfffb92266"; $actor[2:]) | .expect = (if $i == 6 then "revert" else "success" end)]}' \
     rules/r5/corpus.json > rules/r5/corpus.new.json
   mv rules/r5/corpus.new.json rules/r5/corpus.json
   sha256sum rules/r5/corpus.json
   ```
   Verify the precondition actor, all seven `from` values and both `balanceOf`
   arguments equal ACTOR, each expectation is correct, all bounds match the
   observed seed, and no old actor remains. Put `0x` plus the printed digest in
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

- The authenticated candidate account is the proposal's actual `new_impl`, with its proved code, balance and nonce (the supported deployment has nonce 1). The executor installs that address through the witnessed ProxyAdmin and proxy; the resulting implementation slot is checked against `new_impl` explicitly.
- The appended vault `depositCap` slot, `0x199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416803`, may be read during corpus calls; its initial value must have an authenticated witness. Nonempty initialization requires this slot to be zero and may set it. Corpus calls may not change it.
- Initialization runs as part of the signer-to-executor upgrade call, so proxy delegatecall sees the ProxyAdmin as `msg.sender` and the derived signer as `tx.origin`. Gate configuration is checked against the proof-backed authority accounts. During nonempty initialization the OpenZeppelin Initializable namespace must transition from version 1 to version 2, and initialization logs/return bytes are not compared to V1. Successful initialization may not change other account state or storage. The simulated signer's transaction nonce and the executor's expected implementation and certificate-use writes are normalized for comparison.

There is no wildcard for new state, addresses, or slots. R1 separately checks append-only layout; R4 separately restricts initializer calldata. Every corpus call must have identical success/revert status, return/revert bytes, ordered logs, accessed storage-key set (except cap), storage values, account balances and nonces; the implementation slot must equal the real candidate address. Gas consumption is not compared; the fixed local call budget is 3,000,000 gas, and out-of-gas/halt is incomplete. Gas charging is disabled. Corpus executions use the pinned block environment, chain id 84532 and Prague EVM semantics. Unknown BLOCKHASH requests are incomplete.

An unknown account or slot is **never** assumed empty or zero. Added state still needs a state-root proof even when its value is zero. Empty initialization does not waive behavioral equality.

[I] The certificate does not yet exist during replay, so the simulated executor call uses the fixed all-zero certificate ID and a maximum deadline. An initializer that reads executor certificate-use state, including `usedCertificate(0)`, can distinguish this simulation from the eventual transaction. Executor writes described above are normalized; this is not full transaction-context equivalence.

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

The integration test uses disposable anvil, a pinned block and proof-backed runtimes. Discovery traces the executor upgrade and corpus calls; tracer output is only an access hint, while replay uses proof-validated original state. Historical local evidence below predates this replay-context change and is not evidence that the revised code has run.

### Observed local evidence, 2026-11-03

- `cargo test -p constitution-verifier --test replay -- --nocapture`: **2 passed**, 0 failed; 26.79 s including local deployment, compilation, RPC discovery/proofs and all negative cases. The seven-call honest differential replay itself, including proof/header revalidation, measured **36.188 ms** in the debug build (one run, not a benchmark).
- Anvil chain 84532, evidence block 6: `debug_traceCall` with `prestateTracer` and `eth_getProof` both succeeded. Header hash, account proofs, storage inclusion/exclusion proofs and code hashes were checked; no unsupported-method fallback was needed.
- V1 to honest V2 with `initializeV2(1000000)`: Pass 0. V1 to the neutral R2 Fail fixture through production `fetch_witness`: Fail 50 (follow-up run, 2 passed in 34.06 s). This observation is not a claim that R5 replaces R2.
- Testnet note [V]: the old committed actor was anvil's account 0 (public key). It is replaced by the project-owned actor above. Local tests rewrite the committed calls to Anvil's account 0 (or impersonate the committed actor) because the committed actor exists only on Base Sepolia.
- Changed withdrawal logs and an initializer changing pre-existing state: Fail 50. Missing account/slot, changed unproved storage value, invalid account proof, altered code/header/environment and an eighth corpus entry: Incomplete 51. The corpus was independently sent to local anvil: six successful calls, observed balances/totals 110 then 106, and the final reverting withdrawal.
- `cargo clippy -p constitution-verifier --all-targets -- -D warnings` and `cargo fmt -p constitution-verifier -- --check`: exit 0. No workspace-wide test suite or testnet transaction was run by this worker.

## Committed manifest values [I]

[V, local CLI, 2026-11-10] The 600 s manifest with the project-owned corpus has rule-set hash
`0x1efe635ac3eb0eb59b0df0d56c7243b2728922898b55487bc5cf8063db94a410`
(`cargo run -p constitution-cli -- rules hash rules/manifest.json`). Earlier hashes
(`0xa8a4bfb1...cbf93` for the 300 s manifest, `0x45db4a27...fe80` for 600 s with the legacy corpus) are historical.

```json
{
  "r5": {
    "corpus_sha256": "0x9534a4b731515547e2e4321b6c13f937e76159cdfde51c2ca5e531ebe13a8aee",
    "max_txs": 7,
    "confirmations": 2
  }
}
```

The hash commits to the exact 2,018-byte file, including its final newline. Seven entries preserve state observations before and after the successful withdrawal and include an explicit revert without pretending to cover arbitrary sequences. Two successor blocks are a minimal demo evidence-lag policy, not a Base finality claim. The fetcher checks this lag and rechecks the pinned block hash after proof collection. The pipeline maps Fail to Reject{5,50}; repeated matching authenticated contradictions may yield Reject{5,51}. Provider, committed-data and other environmental failures withhold the certificate rather than becoming a rejection. The stage detail includes a non-secret reason and never the endpoint URL or provider response body.

Dependency: `revm = 43.0.3`, non-yanked, MSRV 1.91.0, verified from the official [sparse index](https://index.crates.io/re/vm/revm) on 2026-11-03. Existing workspace pins provide reqwest 0.13.5, hex 0.4.3 and rlp 0.6.1; no additional versions are introduced.
