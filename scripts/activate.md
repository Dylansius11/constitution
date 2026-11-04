# Activation ceremony (BUILD-PLAN P2.8)

Status: **planned, not run.** Every command below is the intended command; none has been executed against testnet or Base Sepolia. Target start 2026-11-07T12:00Z so the F11 change can mature before the 2026-11-14T23:59Z cutoff.

Deleting the gate's last access key is irreversible: after step 7 nobody can redeploy, re-initialize or fix the gate. Every check before step 7 must pass; if one fails, fix it and start again on a fresh gate account.

## Preconditions (each needs its proof before step 1)

| # | Precondition | Owner | Proof to record |
| --- | --- | --- | --- |
| A | B2: deployer `0x67689e905bAA12cC0Fd045b2717694245F298cAb` holds Base Sepolia ETH | founder | `cast balance <deployer> --rpc-url $BASE_SEPOLIA_RPC_URL` |
| B | B3: verifier CVM running the pinned image; `constitution verifier appraise <url>` passes (D5) | founder approves deploy, lead appraises | appraisal output: verifier pubkey, release hash, compose hash |
| C | P1.4 stage B passed on live Base Sepolia; D14 params chosen from observed fees | lead | SMART-CONTRACT "Deployment" rows |
| D | `rules/manifest.json` committed and pushed; `constitution rules hash rules/manifest.json` printed the hash | lead | hash + commit SHA; `manifest_uri` = raw GitHub URL at that SHA |
| E | Reproducible gate wasm from a clean clone of the activation commit | lead | `cargo near build reproducible-wasm` sha256 and NEAR code hash |
| F | Derived EOA for `(constitution-gate.testnet, constitution/base-sepolia/v1)` re-read live | lead | MPC `derived_public_key` view maps to `0x3b20f46860d27d81917a45221c594fc051af7e94` (recorded 2026-11-02, `vectors/mpc-derivation.json`) |

## Steps

1. **Executor, vault and proxy on Base Sepolia** (deployer key from the encrypted keystore):
   ```sh
   cd contracts
   SIGNER=0x3b20f46860d27d81917a45221c594fc051af7e94 ~/.foundry/bin/forge script script/Deploy.s.sol \
     --rpc-url "$BASE_SEPOLIA_RPC_URL" --keystore ../.secrets/evm/deployer \
     --password-file ../.secrets/evm/deployer.password --broadcast
   ```
   Record `executor`, `proxy`, `proxyAdmin`, `vaultV1Implementation`, `token` and the tx hashes. Check `cast call <proxyAdmin> 'owner()(address)'` equals `executor`.
1a. **R5 corpus on the new vault**: rewrite `rules/r5/corpus.json` with a project-controlled actor (not anvil's public account 0), mint, approve and seed that actor's deposit on the new proxy, recompute `corpus_sha256` into `rules/manifest.json`, then commit and push the manifest and run `constitution rules hash rules/manifest.json`. This closes precondition D; record the seeding tx hashes.
2. **Initial EVM nonce**: `cast nonce 0x3b20f46860d27d81917a45221c594fc051af7e94 --rpc-url "$BASE_SEPOLIA_RPC_URL"` (expected 0) becomes `initial_evm_nonce`.
3. **Gate deploy and `new` in one transaction** (`constitution-gate.testnet`, key in `.secrets/near-credentials/testnet/`):
   ```sh
   ./.tools/bin/near contract deploy constitution-gate.testnet \
     use-file target/near/constitution_gate/constitution_gate.wasm \
     with-init-call new json-args "$(cat scratch/activation/init-args.json)" \
     prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \
     network-config testnet sign-with-legacy-keychain send
   ```
   `init-args.json` (fill from steps 1 and 2 and preconditions B, C, D):
   `{"config":{"chain_id":84532,"executor":"0x..","proxy":"0x..","proxy_admin":"0x..","derived_eoa":"0x3b20f46860d27d81917a45221c594fc051af7e94","mpc_account":"v1.signer-prod.testnet","mpc_path":"constitution/base-sepolia/v1","mpc_domain_id":0},"constitution":{"rule_set_hash":"0x..","manifest_uri":"https://raw.githubusercontent.com/Dylansius11/constitution/<sha>/rules/manifest.json","ai_required":true,"params":{...D14...}},"roles":{"governor":"constitution-governor.testnet","guardian":"constitution-guardian.testnet"},"verifiers":[["<base64 pubkey>",{"release_hash":"0x..","attestation_uri":"<quote URL>","added_at_ns":0}]],"initial_evm_nonce":0}`
   Record the NEAR tx hash and `view_account` code hash; the code hash must equal precondition E.
3a. **Fund the gate's operating surplus** (founder approves the amount; testnet NEAR): transfer the operating surplus X documented in SMART-CONTRACT ("Operating surplus") to `constitution-gate.testnet`, then read `get_operating_surplus` and record it. The gate pays the storage of every proposal and credit entry out of fees it collects, but the guard (E12) stalls every write if the uncredited surplus is exhausted, and nobody can top up the code after step 7, so only plain transfers remain possible. Audit step 6 checks the surplus.
4. **Fund the derived EOA** with the D14 amount from the deployer: `cast send 0x3b20f46860d27d81917a45221c594fc051af7e94 --value <D14> ...`. Record the tx hash.
5. **Deployment record** (planned, not run): build the executor artifact from the intended committed source with the pinned tools, then compute its immutable-masked runtime commitment from the repo root:
   ```sh
   forge build --root contracts --offline
   cargo run -p constitution-cli -- executor-runtime-hash --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json
   ```
   Copy the final lowercase `0x` hash into `executor_runtime_hash`. Write `deployments/testnet.json` only once real deployment values have been observed, following [`deployments/testnet.schema.json`](../deployments/testnet.schema.json) and the [field guide](../deployments/README.md). Required audit inputs: `chain_id`, `gate`, `gate_code_hash`, `mpc_account`, `executor`, `executor_runtime_hash`, `proxy`, `verifiers[{pubkey, release_hash}]`; `planned_changes` is optional and defaults to `[]`; also commit `roles` `{governor, guardian}` so the audit compares them instead of only printing them. `gate_code_hash` is the NEAR base58 hash, not hex; EVM addresses and hashes are lowercase `0x` hex; verifier public keys are 32-byte base64. Do not substitute spike addresses or fixture values. Commit the observed record before key deletion.
6. **F11 change, before the governor key or QR is published** (so nobody can fill the pending-change queue first, security review N4): the governor schedules the harmless `SetParams` change (same params, documented in PRD F11):
   `constitution govern --account constitution-governor.testnet --gate constitution-gate.testnet schedule --kind '{"SetParams":{...}}'`. Record change id and `eta_ns`, and add it to `planned_changes` in `deployments/testnet.json`. The demo governor key and its QR (BUILD-PLAN P3.3) are created only after step 8.
6b. **Audit before key deletion** (planned, not run), from the repo root with the public RPC endpoints configured in the environment:
   ```sh
   cargo run -p constitution-cli -- audit --deployment deployments/testnet.json --manifest rules/manifest.json --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL"
   ```
   Expected exit **1**, with exactly one named failure: `FAIL gate_has_no_access_keys` (this is the only expected exception to the pre-step-7 checks). The other preconditions PASS, plus `PASS gate_roles`, `PASS signing_paused: signing_paused=false`, the operating-surplus check, and `pending_governance_changes` showing exactly the one planned F11 change. The summary names only `audit failed: gate_has_no_access_keys`. Any additional FAIL, any WARN other than the planned change, missing view or unexpected role stops the ceremony.
7. **Founder deletes every gate access key** (explicit founder authorization at that moment):
   `./.tools/bin/near account list-keys constitution-gate.testnet network-config testnet now`, then
   `./.tools/bin/near account delete-keys constitution-gate.testnet public-keys <every key> network-config testnet sign-with-legacy-keychain send`.
8. **Audit after deletion** (planned, not run): repeat the exact step-6 command. Expected exit **0**, all 12 named preconditions PASS (including `gate_operating_surplus`), the governance information lines PASS with exactly the planned F11 change pending, no unplanned WARN/FAIL. Save the full observed output as the activation receipt; the expected text here is not evidence.
9. **Matured apply**: at or after `eta_ns` (about 2026-11-14T12:00Z) anyone runs `apply_change` for the F11 change (BUILD-PLAN P4.5).
10. **Record**: every address, tx hash, code hash and the audit output in SMART-CONTRACT "Deployment"; note the CVM uptime start (D18).
