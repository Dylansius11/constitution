# CONSTITUTION: three-minute demo script

**Recording instructions for the founder:** Record a fresh screen capture and narration; this document is a script, not an existing video. Never show a secret key. Choose the separate live-demo flow below for a new recording; keep the final-gate permanent-evidence scenes explicitly labeled as historical. The final gate's current V2 proxy cannot repeat its original R2 run under its V1-referencing manifest. The demo gate uses a different proxy and lineage manifest; never interchange their results.

At recording time, re-read the page's deployment and connectivity, the gate access-key count, audit output, proxy implementation, change 0 status and ETA, and countdown. If a live value or service is unavailable, say so and use the labeled recorded receipts; never fabricate a live state. Show a countdown only if the page actually displays the live change and its ETA is still in the future. After the ETA, show the actual change status instead; elapsed time is not proof of application. A new transaction, key exposure or publication requires the applicable founder authorization.

## Live demo gate recording flow

[I, planned founder recording; not yet exercised end to end on testnet] Show [/app](https://constitution-gate.pages.dev/app), one live attack, then **CLI-only** honest V1→V2→V3 upgrades and the demo audit. No web honest-upgrade button exists. The four lead-run CLI attacks are observed testnet evidence dated 2026-11-11, but the founder wallet flow and both honest demo upgrades remain unexercised [U]. Local lineage R1–R5 checks passed with AI withheld; they are not testnet upgrade proof.

1. **Introduce the boundary.** Show `constitution-demo.testnet`, “Vault pack”, 1-day governance delay and Base Sepolia proxy `0xc24a9b7b139af4e458df40a2644229a4ca5fec91`. Say “This separate testnet vault is for live attacks. The permanent final-gate evidence belongs to a different deployment.” Do not call the retained-key demo gate keyless.
2. **One live attack — planned wallet rehearsal.** Connect a NEAR testnet wallet and show the single confirmation depositing credit and adding a `propose`-only session key. Run the hidden drain; keep the real pending/recovery states visible. Only after observation say “R2 refused this proposal; no signature was requested and the proxy stayed unchanged.” The other cards target storage R1/10, self-destruct R3/30 and initializer R4/41. The lead's prior CLI attack latencies were 35–117 s, not a countdown guarantee. AI does not run on these failures because R1–R5 fail first.
3. **Honest V1→V2 — planned CLI recording.** Before signing, confirm that the demo proxy is still V1 `0x48879e182efbf2db3633088073017e8239eb1457`. Anyone can submit an honest CLI proposal and consume this version; do not assume it is reserved for recording. Use the commands below, narrate observed stages only, and wait for real AI review, certificate, MPC signature and successful Base inclusion plus slot readback. Preserve the receipt.
4. **Honest V2→V3 — planned CLI recording.** Proceed only after the prior receipt confirms V2 `0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005`. Use V3 `0xaeed3e98706f4a4a6c1c5062c1f6541a595db4be` with empty init: V3 inherits V2's existing initialized storage and adds `remainingCapacity()`, not a new initializer. Preserve a separate observed receipt. No honest demo-testnet upgrade transaction is yet recorded.
5. **Audit honestly.** Run the demo audit below. [V, exercised on testnet 2026-11-11] Its pre-deletion result FAILs only `gate_has_no_access_keys` because the gate key remains during founder testing. Say that, not “audit passed”. [I, planned] After acceptance and fresh founder deletion authorization, perform the [separate ceremony](../deployments/README.md#3-finalize-the-record-rehearse-then-remove-the-gate-key), then record a zero-key readback and a passing repeat audit. The final gate's existing keyless audit cannot substitute.

### Planned CLI honest upgrades and audit

[I, planned commands, not executed by this script] From the repository root in Linux/WSL2 with the [README prerequisites](../README.md#local-quickstart). Follow [Demo runner operation](technical/ARCHITECTURE.md#demo-runner-operation). Use only the founder-authorized testnet governor credential, supplied through `DEMO_KEY_FILE` as a local path; never paste its contents or authenticated RPC URLs into the recording. Commands do not source `.env`. Live signing/spending still need authorization.

[V, observed and appraised 2026-11-11] Demo URL below is on **prod5** (`cvm_VKZRaXKr`), not the final CVM's prod9 endpoint. Appraisal pinned the demo compose hash, verifier public key `0x2411cb1631e76179c75595679127843dd6f74877ac14f00acfc924bcad698af1` and release hash `0xfe5ca64ccd11b9ecfd29d0a2ba5553bafd6d642d7f7fc8db4528117daaacea5b`. Recheck identity/availability before sending private source; these observations do not promise continuing availability.

```sh
export NEAR_RPC_URL=https://test.rpc.fastnear.com
export BASE_SEPOLIA_RPC_URL=https://sepolia.base.org
export DEMO_VERIFIER_URL=https://db06dd366642c074483472e3afcb330ff72e4954-8787.dstack-pha1-prod5.phala.network
export EXPECT_COMPOSE_HASH=0xf7571e2688238dd87217f2210789e395c505e3e8eecb3ef026c62051feebe79e
# Set DEMO_KEY_FILE privately to the authorized governor credential's local path.
# The parent may exist; each --output directory below MUST NOT exist.
mkdir -p receipts

./target/debug/constitution verifier appraise "$DEMO_VERIFIER_URL" \
  --expect-compose-hash "$EXPECT_COMPOSE_HASH" --json

# Only while the demo proxy is observed at V1. Cap 1000 preserves R5 preconditions.
./target/debug/constitution demo upgrade \
  --bundle contracts --contract src/demo/VaultV2Honest.sol:VaultV2Honest \
  --impl 0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005 \
  --init "$(cast calldata 'initializeV2(uint256)' 1000)" \
  --account constitution-governor.testnet --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$DEMO_VERIFIER_URL" --output receipts/demo-upgrade-v1-v2 \
  --watch 1800

# Only after the V1->V2 receipt confirms successful inclusion and the V2 slot.
./target/debug/constitution demo upgrade \
  --bundle contracts --contract src/demo/VaultV3Honest.sol:VaultV3Honest \
  --impl 0xaeed3e98706f4a4a6c1c5062c1f6541a595db4be --init 0x \
  --account constitution-governor.testnet --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$DEMO_VERIFIER_URL" --output receipts/demo-upgrade-v2-v3 \
  --watch 1800

./target/debug/constitution audit \
  --deployment deployments/demo.json --manifest rules/demo/manifest.json \
  --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL"
```

[I, planned recovery] Execute these sequentially, not as an unattended batch: stop on any failure. On an incomplete run inspect retained `submission.json`, `run.json`, `failure.json` and the existing proposal before using documented `status`, `receipt`, `verifier run` or `relay` recovery. Do not rerun an uncertain submission or overwrite output. The runner accepts upgrade success only after inclusion and matching proxy slot; a timer, Signed state alone or AI completion is not proof of execution. Cut waits in the video only with an explicit elapsed-time label.

[I, planned close] Explain that free credit and available versions can be burned, the current product is Vault pack, and other presets plus a rule-pack builder are planned, not built; no TUI. If showing F11, switch visibly to **final gate** `constitution-gate.testnet`: change 0 ETA is 2026-11-13T16:56:01Z and apply is still pending. Do not delete or restart the final CVM for this recording; deletion needs founder re-confirmation.

## Final gate, permanent evidence recording

[I, planned presentation of historical evidence] The following table remains the final gate's recorded-results fallback, not the separate demo gate's receipts. Label the attack, honest upgrade and early-apply scenes **recorded testnet results, 2026-11-10**, not live reruns. The recorded drain failed deterministic R2; do not say AI discovered it.

| Time | Screen | Narration | Source receipt |
| --- | --- | --- | --- |
| 0:00–0:20 | Fresh capture of the [public page](https://constitution-gate.pages.dev); identify the final-gate evidence explicitly, not the `/app` demo account. | “An upgrade key can put a vault at risk. CONSTITUTION connects a NEAR rule gate to a Base Sepolia vault. These are the permanent recorded testnet results.” | [Final deployment record](../deployments/testnet.json); [README](../README.md#final-gate-permanent-evidence). Inspect what the page actually displays before narrating a live value. |
| 0:20–0:35 | Show the page's live “Gate access keys” row reading 0 and the read-only keyless audit exiting 0, only if freshly observed. Otherwise say unavailable or different and show the dated recorded result instead. | “Recorded 2026-11-10, re-read live when recording: the gate had zero access keys and the keyless audit exited zero.” | [V, exercised on testnet 2026-11-10] `gate_has_no_access_keys` PASS; [Verify it yourself](../README.md#verify-it-yourself) for current output. |
| 0:35–1:00 | Show proposal 0 and its recorded NEAR verdict; label this a prior run, not a live attack. | “First, the hidden drain was proposed. The deterministic solvency check rejected it in 62.6 seconds. The gate requested no signature, and the vault stayed on version one.” | [Propose](https://testnet.nearblocks.io/txns/23jnKgx7pQK3TvW3EKTCXSR8oRcLYtebs3NMYnNmAZec); [verdict](https://testnet.nearblocks.io/txns/7CmP3rRa3WYNuBa7Me3qJ5BrCBpKTX893GTVueCRJzWv); recorded proposal 0: R2 Fail, reason 20. |
| 1:00–1:30 | Show proposal 1 and open its Base Sepolia explorer transaction; distinguish the recorded run from the current explorer state. | “Next, the honest upgrade passed the five rules. A private AI review produced four candidates; none reproduced. The gate requested a Chain Signature, and the upgrade succeeded on Base Sepolia.” | [NEAR proposal](https://testnet.nearblocks.io/txns/6XqKZ2ptbcJhBooJMdLfstmKMP1e61DxzU3UKsGRyDwo); [NEAR verdict](https://testnet.nearblocks.io/txns/cMJQU3T4r2jHmvGc3mkYytT2Lah2hV316CNQEaPd9X1); [Base transaction](https://sepolia.basescan.org/tx/0xe323d9b9c0169174e2b3895b8bdc41dc81e8e83c1e8cb62fcd51e8f708601da2), success in block 57042237. Recorded total 515.8 s; AI review 419 s. |
| 1:30–1:55 | Show the recorded early-apply refusal, then the page's live timelock only if its current value is available. | “Governance cannot apply this change early. This attempt was refused. Change zero was scheduled for November thirteenth at 16:56:01 UTC. Check the live state before treating it as applied.” | [Schedule](https://testnet.nearblocks.io/txns/2mn9jrjBzAuBHBgc5ExWJMeYLfTdDpvMEUhVfPC5vA6P); [early apply](https://testnet.nearblocks.io/txns/AvHkJNNzU7zQvQmncDSUhsXTWM8F5bciHVhZSRDBuHP), `E53_TIMELOCK_NOT_ELAPSED`. |
| 1:55–2:25 | Fresh capture of the page and README's recorded results and limits. | “You can inspect both proposals and the audit without a signing key. The proxy has already upgraded, so this rule set cannot repeat the drain demonstration. AI can only veto; its search and the deterministic rules are bounded.” | [README commands and limits](../README.md#verify-it-yourself); recorded proposal 0 and 1 receipts above. |
| 2:25–3:00 | Show the architecture and trust limits in the README. | “This is for teams protecting upgradeable contracts. Pricing per protected proxy and verification is a hypothesis, not a product price. Next we would develop rule templates, seek independent review and run testnet pilots. This is not production-ready.” | [PRD business hypotheses](PRD.md#success-and-business-essentials); [README trust limits](../README.md#verifier-deployment-and-trust-limits). |

## X post draft

Founder to publish with the recorded video; not yet posted. The draft is within the X character limit:

> Built for NEAR HACKATHON: CONSTITUTION connects a NEAR rule gate to a Base Sepolia vault. Watch a recorded drain rejection, an honest upgrade and a timelock refusal. Testnet only; the gate had zero access keys on 2026-11-10.
