# CONSTITUTION: three-minute demo script

**Recording instructions for the founder:** Record a fresh screen capture and narration; this document is a script, not an existing video. Never show a secret key or a key-file's contents. Keep the final-gate scenes explicitly labeled as historical. The final gate's current V2 proxy cannot repeat its original R2 run under its V1-referencing manifest, and its verifier CVM was retired on 2026-11-12; the demo gate uses a different proxy, lineage manifest and CVM. Never interchange their results.

At recording time, re-read live values (gate access-key count, audit output, proxy implementation, change 0 status) instead of reading them from this script. If a live value or service is unavailable, say so and show the labeled recorded receipts; never fabricate a live state. After change 0's ETA, show its actual status; elapsed time is not proof of application. A new transaction, key exposure or publication requires the applicable founder authorization.

## Live demo gate recording flow

[I, planned founder recording] Show [/app](https://constitution-gate.pages.dev/app) with one live attack, then the **TUI** for honest V1→V2→V3 upgrades and the demo audit. No web honest-upgrade button exists. [V, testnet 2026-11-11 to 11-12] Observed so far: the lead's four CLI attacks (proposals 0–3), the founder's four wallet attacks (proposals 5–8), demo key deletion and a keyless demo audit (exit 0). [I] Both honest demo upgrades are not yet exercised on testnet; local lineage R1–R5 checks passed with AI withheld and are not testnet proof.

1. **Introduce the boundary.** In the TUI Overview (`1`), show `constitution-demo.testnet`, 0 access keys, the rule-set binding, the 1-day delay and Base Sepolia proxy `0xc24a9b7b139af4e458df40a2644229a4ca5fec91`. Say "This separate testnet vault is for live attacks; the permanent final-gate evidence belongs to a different deployment."
2. **One live attack in the browser.** Connect a NEAR testnet wallet on `/app` and run one attack; keep the real pending states visible. Only after observation say "R2 refused this proposal; no signature was requested and the proxy stayed unchanged." Prior runs took 35–117 s on one warm CVM; that is history, not a promise. AI does not run on these failures because R1–R5 fail first.
3. **Honest V1→V2 in the TUI.** In Overview, confirm the proxy implementation is V1 `0x48879e182efbf2db3633088073017e8239eb1457`; anyone can consume this version first. Open Run (`3`), select the honest V2 preset with `←/→`, press `e` to enter the authorized account and key-file **path**, press `Enter` for the confirmation screen, read the costs aloud, then press `y`. Narrate only stages the stepper shows as observed: proposal, verifier stages, AI review, certificate, MPC signature, Base inclusion and slot readback. The final gate's honest run took 515.8 s (AI 419 s); cut waits only with an explicit elapsed-time label.
4. **Honest V2→V3 in the TUI.** Proceed only after the V2 receipt shows inclusion and the V2 slot `0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005`. Select the V3 preset (`0xaeed3e98706f4a4a6c1c5062c1f6541a595db4be`, empty init) and repeat. V3 inherits V2's storage and adds `remainingCapacity()`.
5. **Audit honestly.** Open Audit (`4`) and press `Enter`. Read the actual result; the 2026-11-12 keyless run exited 0 with `gate_has_no_access_keys` PASS ([output](../deployments/demo-receipts/audit-keyless-2026-11-12.txt)). Never borrow the final gate's audit for the demo gate.

Each TUI run keeps its evidence in a new `scratch/tui-runs/<attack|upgrade>-<UTC>` directory. Afterwards, record each honest upgrade's observed Base transaction in `deployments/demo-gallery.json` `upgrades[].observed`.

### Headless fallback

[I, planned commands] If the TUI cannot be used, run the same flow headless from the repository root in Linux/WSL2 ([build steps](../README.md#build-and-test-locally), [user guide](GUIDE.md#headless-commands)). Supply the authorized credential only as a local path in `DEMO_KEY_FILE`; commands do not source `.env`.

```sh
export NEAR_RPC_URL=https://near-testnet.drpc.org
export BASE_SEPOLIA_RPC_URL=https://sepolia.base.org
export DEMO_VERIFIER_URL=https://db06dd366642c074483472e3afcb330ff72e4954-8787.dstack-pha1-prod5.phala.network
export EXPECT_COMPOSE_HASH=0xf7571e2688238dd87217f2210789e395c505e3e8eecb3ef026c62051feebe79e
mkdir -p receipts   # each --output below must not exist yet

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
  --verifier "$DEMO_VERIFIER_URL" --output receipts/demo-upgrade-v1-v2 --watch 1800

# Only after the V1->V2 receipt confirms successful inclusion and the V2 slot.
./target/debug/constitution demo upgrade \
  --bundle contracts --contract src/demo/VaultV3Honest.sol:VaultV3Honest \
  --impl 0xaeed3e98706f4a4a6c1c5062c1f6541a595db4be --init 0x \
  --account constitution-governor.testnet --key-file "$DEMO_KEY_FILE" \
  --gate constitution-demo.testnet --network testnet \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL" \
  --verifier "$DEMO_VERIFIER_URL" --output receipts/demo-upgrade-v2-v3 --watch 1800

./target/debug/constitution audit \
  --deployment deployments/demo.json --manifest rules/demo/manifest.json \
  --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL"
```

[I, planned recovery] Run sequentially and stop on any failure. On an incomplete run inspect the retained `submission.json`, `run.json`, `failure.json` and the existing proposal before using documented `status`, `receipt`, `verifier run` or `relay` recovery. Never rerun an uncertain submission or overwrite output. Upgrade success requires inclusion and the matching proxy slot; Signed state or AI completion alone is not execution.

[I, planned close] Free credit and available versions can be burned by anyone; the current product is the Vault pack; other rule packs and a builder are planned, not built. If showing F11, switch the TUI profile (`p`) to the **final gate**: change 0 ETA is 2026-11-13T16:56:01Z; show its actual status at recording time.

## Final gate, permanent evidence recording

[I, planned presentation of historical evidence] The table below is the final gate's recorded-results fallback, not the demo gate's receipts. Label the attack, honest upgrade and early-apply scenes **recorded testnet results, 2026-11-10**, not live reruns. The recorded drain failed deterministic R2; do not say AI discovered it.

| Time | Screen | Narration | Source receipt |
| --- | --- | --- | --- |
| 0:00–0:20 | Fresh capture of the [public page](https://constitution-gate.pages.dev); identify the final-gate evidence explicitly, not the `/app` demo account. | "An upgrade key can put a vault at risk. CONSTITUTION connects a NEAR rule gate to a Base Sepolia vault. These are the permanent recorded testnet results." | [Final deployment record](../deployments/testnet.json); [README evidence](../README.md#proof-not-promises). Inspect what the page actually displays before narrating a live value. |
| 0:20–0:35 | Show the live "Gate access keys" row reading 0 and the read-only keyless audit exiting 0, only if freshly observed. Otherwise show the dated recorded result. | "Recorded 2026-11-10, re-read live when recording: the gate had zero access keys and the keyless audit exited zero." | [V, exercised on testnet 2026-11-10] `gate_has_no_access_keys` PASS; [README try it](../README.md#try-it) for current output. |
| 0:35–1:00 | Show proposal 0 and its recorded NEAR verdict; label this a prior run, not a live attack. | "First, the hidden drain was proposed. The deterministic solvency check rejected it in 62.6 seconds. The gate requested no signature, and the vault stayed on version one." | [Propose](https://testnet.nearblocks.io/txns/23jnKgx7pQK3TvW3EKTCXSR8oRcLYtebs3NMYnNmAZec); [verdict](https://testnet.nearblocks.io/txns/7CmP3rRa3WYNuBa7Me3qJ5BrCBpKTX893GTVueCRJzWv); recorded proposal 0: R2 Fail, reason 20. |
| 1:00–1:30 | Show proposal 1 and open its Base Sepolia explorer transaction; distinguish the recorded run from current explorer state. | "Next, the honest upgrade passed the five rules. A private AI review produced four candidates; none reproduced. The gate requested a Chain Signature, and the upgrade succeeded on Base Sepolia." | [NEAR proposal](https://testnet.nearblocks.io/txns/6XqKZ2ptbcJhBooJMdLfstmKMP1e61DxzU3UKsGRyDwo); [NEAR verdict](https://testnet.nearblocks.io/txns/cMJQU3T4r2jHmvGc3mkYytT2Lah2hV316CNQEaPd9X1); [Base transaction](https://sepolia.basescan.org/tx/0xe323d9b9c0169174e2b3895b8bdc41dc81e8e83c1e8cb62fcd51e8f708601da2), success in block 57042237. Recorded total 515.8 s; AI review 419 s. |
| 1:30–1:55 | Show the recorded early-apply refusal, then the live timelock state if available. | "Governance cannot apply this change early. This attempt was refused. Change zero was scheduled for November thirteenth at 16:56:01 UTC." Then state its observed current status. | [Schedule](https://testnet.nearblocks.io/txns/2mn9jrjBzAuBHBgc5ExWJMeYLfTdDpvMEUhVfPC5vA6P); [early apply](https://testnet.nearblocks.io/txns/AvHkJNNzU7zQvQmncDSUhsXTWM8F5bciHVhZSRDBuHP), `E53_TIMELOCK_NOT_ELAPSED`. |
| 1:55–2:25 | Fresh capture of the README evidence table and limits. | "You can inspect every proposal and the audit without a signing key. AI can only veto; its search and the deterministic rules are bounded." | [README evidence](../README.md#proof-not-promises); recorded proposal 0 and 1 receipts above. |
| 2:25–3:00 | Show the architecture and trust limits in the README. | "This is for teams protecting upgradeable contracts. Next: rule packs for lending and DEXs, independent review and testnet pilots. This is not production-ready." | [PRD business hypotheses](PRD.md#success-and-business-essentials); [README limits](../README.md#guarantees-and-limits). |

## X post draft

Founder to publish with the recorded video; not yet posted. The draft is within the X character limit:

> Built for NEAR HACKATHON: CONSTITUTION puts a vault's upgrade key in a keyless NEAR gate. Upgrades ship only if an attested verifier proves they keep the rules; a private AI can veto. Watch live attacks get refused and an honest upgrade go through. Testnet only.
