# Audit deployment records

[V, 2026-11-10] `testnet.json` is the record of the final gate `constitution-gate.testnet`: every value was observed on testnet (see SMART-CONTRACT "Deployment and measured costs"). The gate's last access key was deleted at block 277098815 and `constitution audit` against this record then exited 0. Never copy spike or staging (`constitution-validation-1110`) addresses into it.

`testnet.schema.json` describes the JSON consumed by `Deployment`, `DeployedVerifier` and `PlannedChange` in `crates/cli/src/audit.rs`. The CLI does not fetch or execute the schema; it deserializes with the canonical Rust types. Unknown fields are rejected at these three record envelopes. The schema uses JSON Schema draft 2020-12; preserve integer precision (not JavaScript floating-point numbers) when editing nanosecond timestamps.

## Fields

| Field | Wire value | Meaning / how to obtain it |
| --- | --- | --- |
| `chain_id` | required u64 JSON integer | EVM chain id; activation targets Base Sepolia `84532`. Audit compares the record with the gate and EVM RPC. |
| `gate` | required NEAR account id | Fresh real gate account, not the spike gate. Lowercase NEAR account syntax, 2–64 characters. |
| `gate_code_hash` | required string | NEAR base58 `view_account.code_hash`, committing to the reproducible gate wasm (SHA-256 bytes encoded as base58). Do not use a hex SHA-256 or an unverified RPC value as the build commitment; compare independently with the reproducible build. |
| `mpc_account` | required NEAR account id | MPC account bound by the gate config; planned activation uses `v1.signer-prod.testnet`. |
| `governance_delay_days` | required u32 JSON integer, 1–30 | Explicit expected immutable initialization duration; audit compares it to the observed `get_config` value. Omission from gate initialization defaults to 3, but omission from the deployment record or observed `get_config` is an audit failure. |
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

Illustrative record fragment only, **not an observed deployment**:

```json
{"governance_delay_days": 3}
```

Replace 3 with the actual selected and independently observed value in a real record. `SetParams` cannot change this field; changing duration requires a new gate and the existing timelocked authority migration. Ordinary certified vault upgrades do not wait for this delay.


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

[I] See [`scripts/activate.md`](../scripts/activate.md) for the audit command and the D21 key-deletion gates. The record is audited before deletion (only `gate_has_no_access_keys` may FAIL) and after deletion (the final audit, including the committed delay comparison and operating surplus, must pass; it did on 2026-11-10). Retained-key staging is not keyless proof.

An unplanned `MigrateAuthority`, `AddVerifier`, `RemoveVerifier`, `SetRuleSet`, `SetGovernor` or `SetGuardian` prints `WARN pending_authority_change`, its exact `eta_ns`, and the loud warning summary. Pause prints `WARN signing_paused`; WARN alone exits 0, while any FAIL still exits 1. A pause never revokes already signed EVM transactions.

`crates/cli/tests/audit_anvil.rs` exercises the real hardened gate wasm in near-workspaces and real `Deploy.s.sol` contracts in anvil. The **only** offline double is the MPC `derived_public_key` view, with strict predecessor/path/domain checks and an archived vector key; it is not fresh MPC/testnet evidence. Tests delete the sandbox gate's real key, schedule real governance changes and call the guardian's real pause method. Sandbox account creation/signatures and unlocked anvil transactions are disposable local test actions only. The CLI's sample record and actual local integration record are checked against this schema by a small dependency-free schema-keyword checker in `crates/cli/tests/fixtures/deployment_schema.rs`; negative cases cover missing/unknown fields and malformed values. No real deployment record is generated by these tests.

## Separate live demo deployment

[V, exercised on testnet 2026-11-11] The separate gate `constitution-demo.testnet`
is deployed with a fresh executor/proxy, independently appraised Phala
`tdx.small` CVM `cvm_VKZRaXKr`, `rules/demo/manifest.json` v2 lineage and an
immutable **1-day** delay. [`demo.json`](demo.json) is the observed audit record;
[`demo-gallery.json`](demo-gallery.json) contains real candidate addresses,
four rejection observations and evidence roots; [`demo-receipts/`](demo-receipts/)
retains the attack receipts. Both honest upgrade observations remain null.
The [README comparison](../README.md#live-demo-gate) holds addresses, commitments
and result details; do not duplicate or confuse them with final-gate evidence.

[V, identity fetched and appraised 2026-11-11] Demo verifier endpoint:
`https://db06dd366642c074483472e3afcb330ff72e4954-8787.dstack-pha1-prod5.phala.network`
(Phala app `db06dd366642c074483472e3afcb330ff72e4954`, **prod5**, not final prod9).
Verifier public key: `0x2411cb1631e76179c75595679127843dd6f74877ac14f00acfc924bcad698af1`;
release hash: `0xfe5ca64ccd11b9ecfd29d0a2ba5553bafd6d642d7f7fc8db4528117daaacea5b`.
Expected demo compose hash:
`0xf7571e2688238dd87217f2210789e395c505e3e8eecb3ef026c62051feebe79e`
from [`compose.phala.demo.yaml`](../verifier-image/compose.phala.demo.yaml).
Appraisal passed Intel chain, TCB `UpToDate`, RTMR3 replay, compose, report-data
and release binding. Recheck availability and identity before private-source use.

[V, exercised on testnet 2026-11-11] The pre-deletion demo audit FAILs only
`gate_has_no_access_keys`; its key is intentionally retained for founder testing.
[I, planned] Founder wallet rehearsal, recorded honest V1→V2→V3, fresh deletion
authorization, key deletion and keyless demo audit are still pending (P3.7).
The following ceremony documents operator steps, **not new authorization**.
It never changes permanent `constitution-gate.testnet`, `testnet.json`, its
manifest/corpus, gate wasm or final CVM/compose. Final CVM deletion requires
founder re-confirmation (D28); the original `Deploy.s.sol` interface is untouched.

### 1. Derive, deploy and seed EVM state (lead, after authorization)

Run from the repo root in WSL Ubuntu with the pinned Rust/Foundry tools.
Python 3 and `jsonschema` must already be available; no script installs anything.
Do not source the CRLF `.env` as a shell script or enable `set -x`.

```sh
source "$HOME/.cargo/env"
export PATH="$HOME/.foundry/bin:$PATH"
export NEAR_RPC_URL=https://test.rpc.fastnear.com
# Set BASE_SEPOLIA_RPC_URL privately; do not print authenticated provider URLs.
SIGNER="$(python3 -B scripts/demo/derive.py --near-rpc "$NEAR_RPC_URL")"
bash scripts/demo/deploy.sh \
  --rpc "$BASE_SEPOLIA_RPC_URL" --signer "$SIGNER" \
  --deployer-keystore .secrets/evm/deployer \
  --deployer-password-file .secrets/evm/deployer.password \
  --actor-keystore .secrets/evm/r5-actor \
  --actor-password-file .secrets/evm/r5-actor.password \
  --signer-funding 0.0005ether --actor-funding 0.002ether \
  --gallery deployments/demo-gallery.json --output scratch/demo/evm.json
```

The derivation queries finalized `public_key({"domain_id":0})` and
`derived_public_key({"predecessor":"constitution-demo.testnet","path":"constitution/base-sepolia/v1","domain_id":0})`
on `v1.signer-prod.testnet`, independently computes the SHA3-256 secp256k1 tweak
and Keccak EOA, and refuses disagreement. `--offline` uses the archived root
only for local tests. [V, read-only NEAR testnet views, 2026-11-11] The live
comparison printed `0x6e6568bc88e937b84d739c94a1127304d1160132`.

The deploy step checks chain **84532**, requires the encrypted actor keystore
to match `0x289541ff1cd4882653a02b049ebb5d6a06374826`, and refuses actor code.
Before spending, it compiles every unique gallery `contract` plus
`VaultV2Honest` and `VaultV3Honest` and compares attack runtime hashes with the
catalogue. Names come from the catalogue at runtime (no R3 contract-name guess).
It then:

1. Runs `script/DeployDemo.s.sol:DeployDemo` with `SIGNER`, `--broadcast --slow`
   and the deployer keystore. It creates MockToken, VaultV1 and the unchanged
   ConstitutionExecutor. The executor creates and initializes its own proxy,
   with its own ProxyAdmin owned by the executor, **not** the deployer.
2. Deploys each unique candidate/honest implementation with `forge create`.
   `forbidden-init` reuses the same honest V2 deployment; catalogue initializer
   bytes and `observed` entries are never changed.
3. Funds the derived EOA and R5 actor from the deployer with the separately
   configurable amounts above. These are operational starting amounts, not a
   measured live fee guarantee or extra funding authorization.
4. Mints **1000 raw token units** to the actor, approves the new proxy for 1000
   from the actor keystore, and deposits **exactly 100** from that actor.
   Readbacks require actor token balance/allowance at least **900/900**, actor
   vault balance exactly **100**, vault total/token balance 100, initialized
   token slot, V1 ERC-1967 implementation slot, executor signer and admin owner.
   It does **not** rewrite `rules/r5/**`: those existing committed preconditions
   are already exactly the needed 900/900/100.
5. Saves public addresses, candidate mapping, confirmed receipts, initial EOA
   nonce, masked executor runtime hash and summed execution gas to
   `scratch/demo/evm.json`. It verifies each deployed attack's actual codehash
   before filling **only** catalogue `proxy` and `attacks[].impl`.

Keep the broadcast files and scratch record privately as operational evidence.
[V, source inspected 2026-11-11] `deploy --resume` (through
`bash scripts/demo/deploy.sh --resume` with the same arguments and `--output`)
reuses a persisted bootstrap and already-recorded implementation deployments;
without that flag an existing output is refused. It is not a fully idempotent
transaction engine: funding and seeding are not automatically skipped, and an
uncertain transaction may have landed without being recorded. Stop and reconcile
receipts/state before resuming; do not switch output paths to redeploy blindly.
The script's Python JSON-RPC requests include `User-Agent: constitution-demo/1`.
A new token/proxy isolates seeding from the final vault despite the shared R5 actor.

### 2. Appraise the separate CVM and initialize NEAR (lead, after authorization)

Create the fresh account with the existing NEAR CLI recipe:

```sh
./.tools/bin/near account create-account sponsor-by-faucet-service \
  constitution-demo.testnet autogenerate-new-keypair save-to-keychain \
  network-config testnet create
```

Configure the **demo**, not final, CVM with `GATE_ACCOUNT_ID=constitution-demo.testnet`
and `VERIFIER_MANIFEST=rules/demo/manifest.json`, the fresh executor/proxy binding,
split archive/proof RPCs, and a separate relay credential. Follow
[`LOCAL.md`](../crates/verifier/src/LOCAL.md) for deployment and identity
requirements. Do not reuse the final CVM's ephemeral certificate key or restart it.
Before trusting the new key, appraise its independently pinned compose hash:

```sh
cargo run -p constitution-cli -- verifier appraise "$DEMO_VERIFIER_URL" \
  --expect-compose-hash "$DEMO_COMPOSE_HASH" --json
cargo run -p constitution-cli -- rules hash rules/demo/manifest.json
```

Export the **public, appraised** `DEMO_VERIFIER_PUBKEY` (padded base64),
`DEMO_RELEASE_HASH`, `DEMO_ATTESTATION_URI`, and immutable commit-pinned
`DEMO_MANIFEST_URI` ending in `/rules/demo/manifest.json`. Never infer these
from the final deployment. The following builds the exact final ceremony
`new` shape from the fresh EVM observations and the freshly computed demo hash.
The parameter values below copy the final ceremony's observed D14 values;
review funding/fee suitability before authorizing a new deployment.

```sh
python3 - <<'PY'
import json, os, subprocess
from pathlib import Path
e = json.loads(Path("scratch/demo/evm.json").read_text())
h = subprocess.check_output(
    ["cargo", "run", "-p", "constitution-cli", "--", "rules", "hash",
     "rules/demo/manifest.json"], text=True).strip()
config = {"chain_id": 84532, "executor": e["executor"], "proxy": e["proxy"],
          "proxy_admin": e["proxy_admin"], "derived_eoa": e["derived_eoa"],
          "mpc_account": "v1.signer-prod.testnet",
          "mpc_path": "constitution/base-sepolia/v1", "mpc_domain_id": 0,
          "governance_delay_days": 1}
params = {"proposal_deposit": "100000000000000000000000", "proposal_ttl_s": 3600,
          "max_cert_ttl_s": 300, "sign_validity_s": 300, "gas_limit": 300000,
          "max_fee_per_gas": "50000000", "max_priority_fee_per_gas": "1000000",
          "fee_cap": "1000000000", "sign_gas_tgas": 15, "callback_gas_tgas": 20}
args = {"config": config,
        "constitution": {"rule_set_hash": h, "manifest_uri": os.environ["DEMO_MANIFEST_URI"],
                         "ai_required": True, "params": params},
        "roles": {"governor": "constitution-governor.testnet",
                  "guardian": "constitution-guardian.testnet"},
        "verifiers": [[os.environ["DEMO_VERIFIER_PUBKEY"],
                       {"release_hash": os.environ["DEMO_RELEASE_HASH"],
                        "attestation_uri": os.environ["DEMO_ATTESTATION_URI"], "added_at_ns": 0}]],
        "initial_evm_nonce": e["initial_evm_nonce"]}
Path("scratch/demo/init-args.json").write_text(json.dumps(args, indent=2) + "\n")
PY
# Verify SIGNER's confirmed nonce still equals initial_evm_nonce before new.
cast nonce "$SIGNER" --rpc-url "$BASE_SEPOLIA_RPC_URL"
# Reuse the exact final gate wasm; compare SHA-256/code hash to final build evidence.
sha256sum target/near/constitution_gate/constitution_gate.wasm
./.tools/bin/near contract deploy constitution-demo.testnet \
  use-file target/near/constitution_gate/constitution_gate.wasm \
  with-init-call new json-args "$(cat scratch/demo/init-args.json)" \
  prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \
  network-config testnet sign-with-legacy-keychain send
```

The verifier is registered in the initializer's tuple array, just as on the final
gate; there is no invented immediate registration method. Later `AddVerifier`
requires the **one-day** demo governance delay. Fund and read the gate operating
surplus as in `scripts/activate.md` step 3a before user-facing rehearsal.

Add a **separate restricted key on the relay account**, never a key on the
gate. Generate/store its secret locally through the established credential
workflow; only its public key is used below. Confirm its private credential
is supplied to the demo CVM only. With a founder-approved 5 NEAR allowance
(the final run showed that 0.1 NEAR cannot prepay 300 Tgas):

```sh
./.tools/bin/near account add-key constitution-relay.testnet \
  grant-function-call-access --allowance '5 NEAR' \
  --contract-account-id constitution-demo.testnet --function-names submit_verdict \
  use-manually-provided-public-key "$DEMO_RELAY_PUBLIC_KEY" \
  network-config testnet sign-with-legacy-keychain send
```

### 3. Finalize the record, rehearse, then remove the gate key

No nulls, fake verifier, or guessed NEAR code hash are written into
`deployments/demo.json` during the EVM phase. The unchanged `testnet.schema.json`
already describes the demo, so no second schema is needed. After real NEAR
initialization and verifier registration, finalize from observed finality reads:

```sh
# EXPECTED_GATE_CODE_HASH is the independently verified unchanged wasm commitment.
bash scripts/demo/finalize.sh --near-rpc "$NEAR_RPC_URL" \
  --evm scratch/demo/evm.json --expected-gate-code-hash "$EXPECTED_GATE_CODE_HASH" \
  --output deployments/demo.json
python3 - <<'PY'
import json, jsonschema
jsonschema.Draft202012Validator(json.load(open("deployments/testnet.schema.json"))).validate(
    json.load(open("deployments/demo.json")))
print("demo.json schema valid")
PY
cargo run -p constitution-cli -- audit --deployment deployments/demo.json \
  --manifest rules/demo/manifest.json \
  --executor-artifact contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json \
  --near-rpc "$NEAR_RPC_URL" --evm-rpc "$BASE_SEPOLIA_RPC_URL"
```

Finalization checks every gate config field against the EVM record and the
expected demo binding, checks the independent wasm commitment, requires a
nonempty observed verifier allowlist, and records real roles. It is **not**
an attestation appraisal or full audit. [V, exercised on testnet 2026-11-11]
Before deletion, the observed sole FAIL is `gate_has_no_access_keys`; all other
FAILs/unexpected WARNs stop the ceremony. Four CLI rejections have been rehearsed;
both honest testnet upgrades and the founder wallet flow remain planned.
Finish the applicable D21/D28 acceptance without presenting retained-key testing
as keyless evidence; use the [founder recording flow](../docs/DEMO-SCRIPT.md#live-demo-gate-recording-flow).

Only after review and **fresh founder authorization for irreversible deletion**:

```sh
./.tools/bin/near account list-keys constitution-demo.testnet network-config testnet now
./.tools/bin/near account delete-keys constitution-demo.testnet \
  public-keys "$DEMO_GATE_KEYS_TO_DELETE" \
  network-config testnet sign-with-legacy-keychain send
# Repeat the identical audit command above; every check must pass after deletion.
```

`DEMO_GATE_KEYS_TO_DELETE` must enumerate every key actually listed immediately
before deletion, never a remembered key. Record zero remaining keys and real
transaction receipts. The relay key belongs to a different account and remains
restricted to `submit_verdict` on the demo gate.

### Local-only acceptance and cost interpretation

```sh
python3 -B -m unittest discover -s scripts/demo -p 'test_*.py' -v
bash scripts/demo/dry-run.sh
```

The dry run starts its own Anvil at chain id 84532 on a free loopback port.
It derives from the archived MPC root, copies the catalogue under a fresh
`scratch/demo-anvil-*` directory, uses Anvil's unlocked deployer and temporarily
impersonates the **committed** R5 actor locally only, and runs the same deploy
procedure. No project keystore is opened. Its schema-only `demo.json` uses the
unchanged local wasm commitment with an explicitly empty verifier list: it is
not an observed NEAR account, appraised verifier, full audit or testnet evidence.
All local addresses stay in scratch/broadcast. [V, exercised on testnet 2026-11-11]
The committed catalogue is now filled from the separate authorized live deployment;
the local dry run did not produce those live observations.

The script sums confirmed receipt `gasUsed` for the bootstrap, all unique
candidates, two ETH transfers, mint, approve and deposit. That is a local EVM
execution-gas estimate for Base Sepolia, **not** a live fee quote: Base L1 data
fees, live gas price, NEAR/MPC gas, CVM and AI costs are excluded. Multiply by
the chosen live EVM gas price and add L1 data fees and the two configurable
funding transfers (funding itself is not gas spent).


[V, local Anvil only, 2026-11-11] `bash scripts/demo/dry-run.sh` completed with
**13 successful transactions**, **4,873,790 total execution gas**, all four
gallery runtime hashes matching, V1 initialized, executor-owned ProxyAdmin,
R5 readbacks **900/900/100**, and a schema-valid scratch `demo.json`.
The five focused Python tests passed. A first dry run stopped before broadcast
because Forge script uses `--sender` rather than create/send's `--from`;
the corrected procedure then completed. The conditional execution cost is
**0.00002924274 ETH at 6,000,000 wei/gas**, or **0.0002436895 ETH at
50,000,000 wei/gas**, excluding L1 fees and the 0.0025 ETH funding transfers.
Those prices are arithmetic examples, not a new live Base Sepolia quote.
No NEAR write, CVM deployment, live EVM transaction, key deletion, or testnet
demo outcome was exercised by this check.

