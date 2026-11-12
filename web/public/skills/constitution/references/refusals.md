# Refusals: meaning and recovery

[V, source inspection 2026-11-12] Sources: [gate specification](../../../docs/technical/SMART-CONTRACT.md#panic-code-table),
[verifier pipeline](../../../crates/verifier/src/pipeline.rs),
[filter/schema implementation](../../../crates/verifier/src/rules.rs),
[operational taxonomy](../../../crates/verifier/src/LOCAL.md#outcome-classification),
[CLI recovery text](../../../crates/cli/src/failure.rs). Recovery recommendations are [I].
Do not confuse numeric verifier `rule_id/reason_code` with gate `E..` panic prefixes.
Only report a refusal as on-chain when the gate actually records it.

## Numeric verifier rules

| Rule/code | Meaning | Correct recovery |
| --- | --- | --- |
| 0/1 | Specified source hash mismatch code. Current pipeline instead returns `bad_bundle` without a certificate on source commitment mismatch. | Recover the original canonical bundle; do not claim a recorded Reject 0/1 unless observed. |
| 0/2 | Candidate compilation failure, unavailable runtime, or compiled runtime differs from deployed `new_impl_codehash`; proposal-attributed compile/tool failure also maps here. | Fix compiler diagnostics and match solc/settings/dependencies to deployment. No unsupported constructor arguments, immutables or library links. Deploy corrected code and create a fresh proposal. |
| 0/3 | Observed current implementation differs from proposal's expected current implementation. | Inspect competing upgrade receipts/current state; reassess against that version before a new proposal. |
| 0/4 | Reserved harness/reference source modified/missing or current runtime not reproduced by committed references. | Restore pinned bytes; use a new path/name for custom code. Missing trusted copies may be supplied by verifier. An unsupported current implementation needs an authorized rule-set/verifier change, not spoofed references. |
| R1/10 | Incompatible storage layout (also attribution target for layout-stage tool failure). | Preserve old namespace/fields/order/types; append only. Inspect stage detail if attribution came from a tool fault. |
| R1/11 | Reference layout missing (specified code). | Restore the committed current artifact/reference/tooling; do not certify without layout evidence. Ordinary current tool-unavailable failures withhold. |
| R2/20 | Bounded symbolic solvency or slot-preservation counterexample. | Reproduce trace; repair transfers/accounting or implementation-slot mutation. The four-call gallery drain reaches this code. |
| R2/21 | Seeded Foundry fuzz counterexample. | Reproduce with committed seed and failing sequence; repair the invariant. |
| R2/22 | AI-suggested counterexample deterministically reproduced. | Inspect reproduced trace and repair the demonstrated violation; AI prose alone is not this verdict. |
| R2/23 | Explicit solver incomplete, bounded loops, malformed/path-incomplete solver result; also attribution target for solvency tool faults. | Inspect tool details and candidate complexity. A terminal rejection requires healthy same-stage canaries before/after; without them the run fails without a certificate. Never increase bounds or call unknown Pass. |
| R3/30 | Forbidden runtime opcode: SELFDESTRUCT, CALLCODE or DELEGATECALL. | Remove the unsafe operation. Access control does not exempt it; R1 unsafe_allow does not override R3. |
| R3/31 | Literal ERC-1967 implementation/admin slot, ±1 neighbor, Foundry/Halmos cheatcode address, or ERC-1967 ASCII label. | Remove forbidden proxy/cheatcode machinery from implementation. Do not obfuscate constants to evade the policy. |
| R4/40 | Missing/unknown initializer selector, including nonempty data shorter than four bytes. | Use empty calldata or exactly the allowlisted initializeV2 selector; a new migration schema needs governance. |
| R4/41 | Wrong ABI length or argument exceeds `2^128-1`. | Encode exactly one uint256 word in range, with no trailing bytes. Also test cap compatibility through R5. |
| R5/50 | Old/candidate replay differs, or candidate's upgrade/initializer fails. | Preserve declared corpus behavior, logs, return bytes and state; keep initializer changes within declared exceptions. A low in-schema cap can cause this refusal. |
| R5/51 | Repeated matching authenticated candidate execution/witness-bound incompleteness; also replay-stage proposal-attribution code. | Inspect the evidence; simplify unsupported candidate behavior or prove required state within committed bounds. Ordinary RPC/precondition faults withhold rather than sign this rejection. |
| Rule 6 (AI) | No on-chain reason code for an incomplete required review. | Restore provider/attestation availability; retry the same Pending proposal if still valid. Never report R6/0 or treat Clean as authorization by itself. |

First failure wins: later stages can be NotRun/Skipped. A filter example that
also breaks storage can stop at R1, not necessarily its intended later rule.
Stage detail, evidence hash and the accepted gate certificate identify the actual refusal.

## Gate panic prefixes

These refuse the attempted gate action; they are not automatically proposal Rejects.

| Prefix | Meaning | Correct recovery |
| --- | --- | --- |
| `E10_ZERO_DEPOSIT` | Zero credit funding deposit. | Attach a positive authorized deposit to deposit_credit. |
| `E11_INSUFFICIENT_CREDIT` | Unreserved credit insufficient or checked arithmetic overflow. | Read required credit/entry fee; fund the actual shortfall. Fix overflowing amounts, not just top up. |
| `E12_STORAGE_INSOLVENT` | Gate liquid balance cannot cover user liabilities and contingent reserves after write. | Ask operator to restore gate operating surplus; do not bypass liability accounting. |
| `E20_SIGNING_PAUSED` | Guardian paused signature creation. | Inspect pause/governance; only a valid timelocked Unpause can restore signing. |
| `E21_ACTIVE_PROPOSAL_EXISTS` | Proposer has an active reservation. | Resume existing proposal; expire only after its actual expiry. |
| `E22_INIT_CALLDATA_TOO_LONG` | More than 1,024 initializer bytes. | Use permitted bounded initializer; a larger migration is not supported. |
| `E23_ZERO_ADDRESS` | Implementation is zero. | Supply the observed deployed nonzero implementation address. |
| `E24_DUPLICATE_ACTIVE` | Same proposer already has active codehash/calldata pair. | Inspect and resume that proposal rather than duplicate it. |
| `E30_NOT_FOUND` | Unknown proposal. | Check retained submission id and correct gate. |
| `E31_WRONG_STATE` | Action invalid for target state or retry too early. | Read current state. Signing-request timeout is 20 minutes; do not retry a terminal rejection. |
| `E32_NOT_EXPIRED` | Expiry not passed. | Wait until past stored proposal expiry. |
| `E40_BAD_ENCODING` | Malformed Borsh certificate/trailing bytes. | Use canonical certificate bytes from verifier, not hand-edited JSON. |
| `E41_WRONG_DOMAIN` | Wrong certificate domain/gate. | Use this gate's exact proposal/run. |
| `E42_UNKNOWN_VERIFIER` | Key/release not registered. | Check allowlist and appraised identity; new verifier registration is timelocked. |
| `E43_BAD_SIGNATURE` | Invalid Ed25519 certificate signature. | Preserve evidence; inspect issuer/bytes. Never override validation. |
| `E44_PROPOSAL_EXPIRED` | Proposal expired before verdict submission. | Reassess and create a new authorized proposal; old certificates cannot revive it. |
| `E45_BINDING_MISMATCH` | Certificate/transaction/rule-set field differs. | Inspect named field and original inputs/current rules. Do not splice a certificate from another proposal. |
| `E46_CERT_TIME` | Invalid issue/expiry time or maximum TTL. | Check clock and certificate lifetime; request fresh verification while proposal is valid. |
| `E47_AI_REVIEW_REQUIRED` | Required Clean review absent or Pass carries Vetoed. | Complete required provider review; never replace AI fields manually. |
| `E48_FEE_CAP` | Fee bump overflow or exceeds committed cap. | Inspect timelocked fee parameters and funding; no different-calldata replacement. |
| `E49_TOO_MANY_BUMPS` | Eight bumps exhausted or attempt overflow. | Inspect existing signed transactions; no unbounded bump loop. |
| `E50_NOT_GOVERNOR` | Scheduler lacks governor role. | Use authorized governor, not a proposer/session key. |
| `E51_INVALID_CHANGE` | Invalid initialization/change/parameter/URI/address/release, duplicate cancellation id, arithmetic overflow or stale Unpause. | Validate exact ChangeKind against spec; an Unpause must be scheduled strictly after latest pause. |
| `E52_NOT_FOUND` | Unknown/removed governance change. | Check live queue and recorded change events; terminal bodies may be deleted. |
| `E53_TIMELOCK_NOT_ELAPSED` | Current time before immutable ETA. | Wait for observed ETA; demo delay is 1 day, final 3 days. No bypass flag. |
| `E54_NOT_PENDING` | Change not Pending or retryable applied migration. | Read actual queue/migration state; don't reapply a terminal change blindly. |
| `E55_SIGNATURES_OUTSTANDING` | Rule-set change conflicts with outstanding signature deadline/request. | Inspect outstanding proposals; wait through inclusive deadline/request window and coordinate guardian if needed. |
| `E56_NOT_GUARDIAN` | Restrictive action caller lacks guardian role. | Use authorized guardian; governor cannot cancel. |
| `E57_LAST_VERIFIER` | Would remove final verifier. | Preserve a valid allowlist; arrange timelocked replacement before removal. |
| `E58_AI_SIGNATURE_KIND` | Clean review not provider_tee kind 1. | Restore required provider-TEE review; gateway-only is insufficient. |
| `E59_PENDING_CHANGE_LIMIT` | Eight Pending changes or cancellation batch exceeds eight. | Inspect queue and authorized cancellations; split valid bounded batches. |
| `E60_DUPLICATE_AUTHORITY_CHANGE` | Per-kind cap reached: 1 authority change, 2 other changes. | Inspect existing pending same-kind change rather than enqueue duplicates. |
| `E61_STORAGE_FEE` | First deposit does not exceed credit-entry fee, or propose attached nonzero deposit. | Fund sufficient prepaid credit separately; propose attaches zero. |
| `E62_BUMP_TOO_EARLY` | Less than 600 s since target's latest signing request. | Wait for actual minimum age before a permitted bump. |

Signing retries preserve bytes/nonce; fee bumps preserve calldata/deadline and use
higher fees within cap. Neither fixes an expired executor deadline. At equality
with deadline the EVM call is still executable; expiry is strictly later.
The CLI does not expose dedicated expire/retry_sign/bump_fee subcommands; these
are gate interfaces. Do not invent CLI invocations for them.

## Operational and admission refusals

| Observed code/state | Meaning | Correct recovery |
| --- | --- | --- |
| `bad_bundle` | Unsealing, canonical validation or source commitment failed. | Reuse original canonical bytes; fetch appraised current identity and re-seal using verifier run. Editing source changes commitment. |
| `worker_panic` | Worker stopped unexpectedly. | Operator inspects worker; proposal remains Pending unless a separately observed verdict landed. |
| `sandbox_unavailable` | Tool sandbox unavailable. | Restore Docker/sandbox; no signing fallback. |
| `storage_error` | Working storage failure. | Operator restores writable storage/free space. |
| `tool_unavailable` | Missing required pinned tool. | Restore committed toolchain, not another compiler/version. |
| `tool_output_invalid` | Malformed/inconsistent tool artifacts. | Inspect artifacts/harness; no interpretation as a successful test. |
| `infrastructure_timeout` | Resource or enclosing run deadline exceeded. | Inspect sandbox limits/load; no silent bound changes. |
| `rpc_unavailable` | Transport/protocol/provider fault. | Restore chain RPC access; retain pinned-state authentication. |
| `replay_unavailable` | Replay environment/preconditions unavailable. | Inspect detail such as precondition_unmet, expected_outcome_mismatch or proof_window_exceeded; operator restores valid witnessed state/provider. Do not patch source merely to hide RPC failure. |
| `committed_data_unavailable` | Missing/inconsistent verifier-owned policy data. | Operator restores validated committed data/image. |
| `429 cooling_down` / Retry-After | Admission cool-down after withheld/failed work. | Honor indicated wait; retry original Pending proposal, no automatic loop. |
| Verifier busy/rate limited | Admission capacity, not a rule verdict. | Honor server recovery/wait; do not duplicate proposal. |
| Required AI Incomplete | Provider/response/attestation/candidate replay incomplete. | No certificate; restore service then explicitly retry before expiry. |
| `submit: Incomplete` | Certificate retained; transaction outcome may be unknown. | Read gate and relay account transactions before another submission/run. |
| `SignFailed` | MPC signing failed, not a rule rejection. | Diagnose callback/signing failure; authorized recovery must use identical bytes. |
| `Signed` but EVM pending | Signature exists, no successful inclusion observed. | Use verified relay, inspect nonce/funding/deadline; never label pending executed. |
| Audit FAIL | A keyless/ownership/binding/read prerequisite failed. | Fix actual precondition or report limitation. A recorded final audit is not a live demo audit. |

Unexpected/missing error codes are not success: retain output and ask the verifier
operator to inspect. Verifier runs can consume inference credits even though
`verifier run` makes no NEAR proposal write.
