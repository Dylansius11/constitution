# Local verifier plumbing

[I] A Pass certificate requires R1–R5 Pass, AI Clean with a verified `provider_tee`
response, complete candidate replay, and a socket-attested signing identity with
a nonzero release hash. Chain-connected startup requires a dstack socket;
without one it stops with `attestation_required`. Only explicit
`--insecure-dev-identity` permits NOT ATTESTED mode (ephemeral boot keys,
zero release hash, no quote claims, local registry and no submitter).
Never register that identity on testnet. Protocol fakes are not attestation.

## Start

Run from the repository root inside WSL as an unprivileged user, with Docker and the
existing `constitution-verifier-spike:local` image available. The listener remains
loopback-only at `127.0.0.1:8787`. Set `PAGE_ORIGIN` to the allowed page origin.

Default mode reads real gate and Base JSON-RPC endpoints and submits issued verdicts:

```text
NEAR_RPC_URL=http://127.0.0.1:<sandbox-port>
BASE_SEPOLIA_RPC_URL=http://127.0.0.1:<anvil-port>
BASE_SEPOLIA_PROOF_RPC_URL=http://127.0.0.1:<anvil-port>
GATE_ACCOUNT_ID=<one-protected-gate-account>
VERIFIER_RELAY_ACCOUNT_ID=<sandbox-relay-account>
VERIFIER_RELAY_KEY_FILE=<near-cli-legacy-credential.json>
cargo run -p constitution-verifier
```

`BASE_SEPOLIA_ARCHIVE_RPC_URL`, when nonempty, takes precedence over
`BASE_SEPOLIA_RPC_URL` for observation, code/storage and trace calls.
`BASE_SEPOLIA_PROOF_RPC_URL` is required in chain mode; use the same Anvil URL
locally, or `https://sepolia.base.org` for recent-block proofs on Base Sepolia.
For the hackathon, keep the keyed Validation Cloud URL for archive/trace and
use the public Base URL only for proofs. Public RPC is rate-limited and has no
production availability guarantee. When `VERIFIER_RELAY_KEY_FILE` is unset,
`NEAR_NETWORK_ID` is required and the file defaults to
`.secrets/near-credentials/<NEAR_NETWORK_ID>/<VERIFIER_RELAY_ACCOUNT_ID>.json`.
The credential contains `account_id`, `public_key`, and `private_key`; never
publish it. Missing required environment variables stop startup with their name.

When `/var/run/dstack.sock` exists, startup obtains `GetQuote` and `Info` for the
boot-generated keys/nonce. RPC or quote errors stop startup instead of downgrading
to an unattested identity. Certificates use `Identity::release_hash()` (zero only
in NOT ATTESTED mode). The socket is never mounted into tool containers.

Explicit offline mode instead requires `LOCAL_PROPOSAL_REGISTRY` pointing to an
operator-controlled JSON array of `LocalProposal` records, and makes no submission:

```text
cargo run -p constitution-verifier -- --insecure-dev-identity
```

POST accepts only gate, proposal id and sealed bundle, never observation hashes.
Gate reads use NEAR `query/call_function` with `finality: final`, require Pending
and the served manifest hash, and floor nanoseconds through the shared time helper.
Base evidence uses `latest - manifest.r5.confirmations`; block number/hash,
ERC-1967 implementation slot (clean high 12 bytes), and nonempty implementation
runtime are observed at that block. A block hash change during reads refuses the
observation. RPC errors and incomplete observations cannot fabricate a proposal.
Blocking reads and signing run outside the HTTP async runtime.

Admission [I]: `GATE_ACCOUNT_ID` pins the only chain-connected gate;
other gates return `400 wrong_gate` before any RPC. The order is fixed so that an
unauthenticated caller can only ever spend its own budget:

1. **Peer limiter**, before reading the POST body: 6 per 10 s per peer, IPv6 /64.
   Chain mode requires `VERIFIER_PEER_SOURCE=socket` or `header:<name>`; local
   insecure mode defaults to `socket`. Behind an ingress use the header only if
   the trusted proxy overwrites it or appends the actual client as its final entry.
   The last comma-separated address is used; absent or invalid headers return
   `400 invalid_peer`, never a socket fallback. Deployment must verify its ingress
   configuration; the dstack gateway header is not established here.
   GET run and evidence reads have a separate limit of 60 per peer per 10 s.
   Bundles contain at most 128 source files.
2. **Pre-authentication RPC**: a semaphore of `PREAUTH_RPC_SLOTS` = 4 bounds the
   concurrent `get_proposal` reads; none free returns `429 busy`. The slot is
   released the moment the commitment check passes.
3. **Authentication**: the sealed box opens and `sha256(plaintext)` equals the
   on-chain `source_commitment` (`400 bad_bundle` otherwise, nothing charged).
4. **Run admission**, only now: an occupied worker is refused first
   (`429 queue_full`, or `409 run_in_progress` for the same proposal) and charges
   nothing; then the per-proposal limiter (one per 10 s, `PROPOSAL_INTERVAL`) and
   the global limiter (10 per 1 s sliding window, `GLOBAL_REQUESTS`/`GLOBAL_WINDOW`)
   are charged, and the single `active` slot is taken (before the Base reads).

Bad requests do not consume the authenticated proposal or global admission budgets
and cannot hold the run slot. They can exhaust a shared peer budget (for example
behind an unconfigured proxy) or occupy the four pre-authentication RPC slots.
`429 rate_limited`, `queue_full`, `cooling_down` and `busy` carry `Retry-After`.
One worker runs at a time. A panic drop guard releases that slot and ends a
pre-submission run `failed`/`worker_panic`; poisoned state locks recover.
Post-signing submission uncertainty is retained as described below.
The newest 64 finished runs are retained (`FINISHED_RUNS`), plus any unfinished
run. IDs are monotonic; evicted status/evidence returns 404, not a lifetime
queue refusal. Export evidence before eviction if it must be retained.
The allowed CORS origin can read `Retry-After` via `Access-Control-Expose-Headers`.

**Cool-down.** Failed and withheld runs without a certificate accrue strikes:
10, 20, 40, then 60 minutes, forgotten after two hours; a signed certificate clears
them. Clearly verifier-owned `sandbox_unavailable`, `worker_panic`, RPC transport
faults, RPC/AI deadline overruns and a failed health canary do not add a strike.
Other faults, including storage errors and incomplete AI review, do. The response
is `429 cooling_down` with `Retry-After`.

**Honest canary.** On a timeout or invalid tool output, the same stage of the
honest fixture is checked after the failure and its completion time is cached.
The second-failure Reject requires a healthy same-stage canary from before the
candidate stage, at most 30 minutes old, and a fresh healthy canary afterwards.
An incomplete solver result uses the same before/after rule. A failed or absent
pre-check leaves the proposal retryable. Ordinary successful runs perform no
extra canary work. A transient host fault between the cached check and the failing
stage remains a residual attribution risk. Before a diagnostic-based compile
Reject the honest fixture must compile; otherwise the run fails without signing.
Canary checks add `canary` stages to run JSON.

### Outcome classification

| Cause | HTTP / run outcome | Certificate / gate |
| --- | --- | --- |
| Bad sealed bundle, malformed plaintext, wrong source commitment | `400 bad_bundle`; no run or authenticated admission charge; peer budget applies | None; Pending |
| Other gate | `400 wrong_gate` before RPC | None |
| Per-peer flood / pre-auth RPC slots busy | `429 rate_limited` / `429 busy`, `Retry-After` | None |
| Authenticated rate / busy worker | `429 rate_limited` / `429 queue_full`, `Retry-After`; same active proposal otherwise 409; a busy refusal charges nothing | None |
| Proposal in cool-down | `429 cooling_down`, `Retry-After`, before any RPC | None |
| Proposal not Pending (Signed, Rejected, Expired, ...) | `409 proposal_not_pending` | None |
| Unsupported rule-set hash, malformed proxy slot, gate/proposal id mismatch, expired proposal | `400 unsupported_proposal` | None |
| Too few blocks for the manifest confirmations, no current implementation yet | `409 not_ready` (retry later) | None |
| Gate/Base RPC transport or protocol fault, evidence block change during reads | `502 rpc_unavailable` | None; Pending |
| Source compilation diagnostic | Signed Reject 0/2 only after healthy canary compile | Terminal Reject, otherwise failed |
| Compiler output without an allowlisted diagnostic | `failed`, `tool_output_invalid`; canary and cooldown apply | None; Pending |
| Halmos solver timeout/unknown | Reject 2/23 only with healthy same-stage canaries before and after | Otherwise failed |
| R5 repeated candidate execution incomplete or witness bound exceeded | R5 Incomplete, reason 51 | Signed Reject |
| R5 provider lag, transport/HTTP/protocol failure, malformed replies, unauthenticated code mismatch or committed data unavailable | `failed`: `rpc_unavailable`, `replay_unavailable`, or `committed_data_unavailable` | None; Pending and retryable |
| Sandbox/Docker unavailable, disk/tempdir failure, missing verifier tools | `failed`: `sandbox_unavailable`, `storage_error`, `tool_unavailable` | None; Pending and retryable |
| Tool wall timeout/OOM (exit 124/137), malformed/inconsistent tool artifacts, unavailable committed harness | `failed`: `infrastructure_timeout`, `tool_output_invalid`; counts toward the cool-down and canary rule | None, or signed Reject once the canary attributes it to the proposal |
| Run exceeds `RUN_DEADLINE` (20 minutes: sandbox stages, replay RPCs, AI candidates) | `failed`, `infrastructure_timeout` | None; Pending and retryable |
| Incomplete required AI review | Withheld, no Pass | None; Pending and retryable |

Replay `Incomplete` distinguishes Environment and Contradiction. Candidate execution
incompleteness, including unwitnessed reads and blockhash, and repeated witness bound
excesses are Contradictions. RPC transport and unauthenticated code mismatches remain
environmental. The same configured provider supplies both observations; an untrusted
provider may repeat misleading tracer hints.

Failure details expose static codes/digests, never tool diagnostics, keys or RPC
URLs. A generic container wall timeout is infrastructure, not a solver timeout.
`failed` is terminal for that run, not terminal for the proposal.

### Run JSON contract for CLI/page consumers

`POST /v1/runs` success is `202 {"run_id": <u64>}`. Admission errors are
plain-text stable codes; rate/busy 429 responses include `Retry-After` integer
seconds. Successful `GET /v1/runs/{id}` returns the following flattened object
(a schema description, not a live receipt):

| Field | JSON shape / meaning |
| --- | --- |
| `stage` | String: `queued`, `unsealing`, `compiling`, `canary`, `r1_layout`, `r2_solvency`, `r3_filter`, `r4_initializer`, `r5_replay`, `ai_review`, `reproducing`, `signing`, `submitting`, `done`, or `failed` (`canary` is transient: it appears in `stages`, and as `stage` only while the honest fixture runs) |
| `started_at` | u64 Unix seconds |
| `stages` | Array of `{name: string, status: "Pass" \| "Fail" \| "Incomplete" \| "NotRun" \| "Skipped", detail: string, elapsed_ms: u64}`; submission result uses stage name `submit` |
| `outcome` | Human-readable string, not a machine discriminator |
| `elapsed_ms` | u64 elapsed milliseconds |
| `error_code` | Optional string, present on `failed`; stable codes from the classification table, including `tool_output_invalid` for malformed verifier-tool artifacts, `infrastructure_timeout` (also for the 20-minute run deadline), `worker_panic`; defensive pipeline-only unseal failures use `bad_bundle` |
| `verdict` | Optional `"Pass"` or `{"Reject":{"rule_id":u8,"reason_code":u16}}`; present only after signing |
| `certificate_b64`, `signature_b64` | Optional base64 strings, canonical certificate bytes and Ed25519 signature |
| `evidence_root` | Optional `0x`-prefixed 32-byte lowercase hex string |
| `near_tx_hash` | Optional observed successful submission hash; never inferred |

Both `done` and **`failed` are terminal run stages**; stop polling either.
A `done` run may be rejected, certified, review-withheld or submission-incomplete:
inspect certificate/verdict and submit status, not just `stage`.
`failed` has no certificate, signature, verdict or on-chain submission; the
proposal remains Pending and may be retried. `Stage.status` is a stage result,
not a proposal state. Absent optional fields are omitted, never fake/null facts.
Evidence is fetched separately from `/v1/runs/{id}/evidence`; it is not embedded
in status JSON. Export either before finished-history eviction.

Each issued certificate/signature pair triggers exactly one relay submission.
Successful execution adds `near_tx_hash` to run JSON. A failed/pending submission
records `submit: Incomplete`, retains the certificate, and never retries
automatically. Inspect the relay's chain transactions before starting another run,
because a lost response may still have landed. No certificate means no submission.
All R2 manifest fields must be resolved before startup; null never becomes a bound.

The no-certificate verifier-fault policy applies **before signing**. Once
`submitting` begins, an RPC error or sender panic cannot prove the transaction
was not broadcast. Such a run ends `done` with a `submit: Incomplete` stage and
retains its signed certificate (not `failed`); inspect relay transactions before
retrying. The worker slot is still released. No automatic resubmission occurs.

## Local end-to-end check

With the prebuilt real gate at
`target/near/constitution_gate/constitution_gate.wasm`, Foundry in
`~/.foundry/bin`, and the existing Docker image, run:

```text
CARGO_BUILD_JOBS=4 cargo test -p constitution-verifier --test chain_e2e -- --nocapture --test-threads=1
```

If the wasm is absent, build it from `crates/gate` with
`cargo near build non-reproducible-wasm`. Each test starts its own NEAR sandbox
and anvil (`chain_id=84532`) and runs `Deploy.s.sol`. The R2 Fail fixture must
produce Reject rule 2 reason 20 on the gate. The honest proposal seeds the corpus
actor before pinning the evidence block, passes R1 through R5, and stays Pending:
without a running verified sidecar, AI is Incomplete and no certificate is issued.
Both tests POST sealed bundles over HTTP and check the proxy is unchanged.
They print exact local runtimes and final gate proposal JSON; a NEAR hash is
printed only when observed. Allow ten minutes per run; R2 uses one CPU.
This is local evidence, never testnet or attestation evidence.

The suite uses the shared local dstack socket simulator solely to obtain a
nonzero release hash for the gate's strict initialization checks. The fixture
quote is rebound to local ephemeral keys and is **not cryptographically valid
DCAP evidence**; this does not turn the test into an attested deployment.

## R5 replay stage

R5 runs after R4 with the current/candidate runtimes already reproduced by the
compiling stage. It parses `rules/r5/corpus.json`, enforces the manifest's seven-call
bound and exact corpus SHA-256, fetches proof-authenticated witness data at the
observed block, then executes the bounded differential replay.

R5 uses `ProposalSource::archive_rpc()` for block observation, code/storage reads
and `debug_traceCall` / `prestateTracer`, including state overrides and diff mode.
It uses `ProposalSource::proof_rpc()` for `eth_getProof`. Both endpoints must report
chain id 84532 and the same hash-verified header at the proposal's fixed numeric
block. Every account/storage proof is checked against that header's `stateRoot`;
the pinned hash is rechecked on both providers after the fetch. AI candidate
witnesses use the same endpoint split and authentication.

Full endpoint URLs are never included in evidence or run details; provenance names
only provider hosts. LocalRegistry has neither endpoint: reaching R5 in offline
mode fails with `rpc_unavailable`, never a pass. An unavailable/rate-limited proof,
expired proof-history window, wrong proof chain/header or invalid proof withholds
the certificate as an environmental failure; there is no fallback to `latest` or
to unauthenticated state.

R5 records Pass 0, divergence Fail 50, or Incomplete 51. Candidate execution
incompleteness and witness bound excesses may Reject after the same contradiction
recurs on a second fetch. Both fetches use the configured provider, not independent
providers. Environment outcomes end the run without a certificate. Evidence hashes
witness bytes or sanitized incomplete details.

After all deterministic rules pass, the verifier calls the loopback AI sidecar.
An unavailable sidecar, gateway signature, timeout, malformed response or incomplete
candidate replay produces `withheld: AI review incomplete; no Pass without provider_tee Clean`.
AI doubt alone never produces a rejection. **Clean means the provider responded and
nothing was reproduced**: it is a best-effort veto layer, not a proof that the review was
meaningful, since model output (and prompt injection from the source) can yield an empty
candidate list. A reproduced R2 violation produces
`Reject { rule_id: 2, reason_code: 22 }` with `AiReview::Vetoed` and a candidate hash.

[I] The committed two-successor-block lag is a minimal demo evidence policy,
not a Base finality claim. The corpus actor is a public anvil account; testnet
activation requires a deliberately seeded testnet actor and a new corpus commitment.
Rule-set hash (CLI observed locally, 2026-11-03, after the security-review fixes):
`0x8060ddbf63635e14a955b4fef81c8e696c01e349c9f959cae135d48056e89455`.

Library witness policy and limits:
[`rules/r5/README.md`](../../../rules/r5/README.md).

## AI sidecar (F6, local-provable integration)

[I] Node 24 and the already specified `@nearai/inference-sdk` **0.1.0** are pinned
in `verifier-image/ai-sidecar/package.json`; the pnpm lock pins transitive packages.
No key is passed to Rust or tool containers. Start the sidecar separately, in the
same network namespace as the verifier:

```sh
corepack pnpm@12.15.0 --dir verifier-image/ai-sidecar install --frozen-lockfile --ignore-scripts
node verifier-image/ai-sidecar/server.mjs
```

It binds only `127.0.0.1:8788`, exposes only `POST /review`, refuses browser Origin
requests, and never logs prompts, source, model output, endpoint credentials or
the API key. It reads `NEARAI_API_KEY`, `NEARAI_BASE_URL` (default
`https://cloud-api.near.ai/v1`) and `NEARAI_MODEL` (only `z-ai/glm-5.3-flash`).
The SDK Node entry point enforces E2EE, gateway/model TCB `UpToDate`, and
`x-no-aliasing: true`. It calls `verifyResponse(completion.id)` before releasing
validated candidates; only `provider_tee` is complete. A missing key, error,
truncated/malformed JSON, timeout or non-provider signature returns structured
Incomplete with no candidates. The timeout covers completion and verification.

Source comments and model text remain untrusted data. Requests are capped at
4 MiB, source at 2 MiB, output candidates at 64 KiB, candidates at the committed
manifest count (currently 4), and steps at 16 per candidate. The only accepted
calls are zero-value `deposit(uint256)` and `withdraw(uint256)` to the proxy,
with uint128 decimal arguments. `attacker` is the first committed R5 corpus actor;
`victim` is address `0x0000000000000000000000000000000000000002`; explicit addresses
must be witnessed EOAs; successful token transfers require their real evidence-block
token balances and approvals. As in R5, gas charging is disabled for property replay;
this does not establish that a sequence could pay current network fees.
The prompt cannot seed balances, alter approvals, execute host code or select
another callee. Transactions use R5's 3,000,000 gas cap and strict revm database.

The verifier authenticates candidate access witnesses against the **same pinned
block hash/state root as R5**, then reuses the R5 upgraded-fork preparation and
transaction engine. Missing accounts/slots/proofs fail closed, never become zero.
Predicate discovery always includes the ERC-7201 token and total-deposits slots,
the token account, and its `balanceOf(proxy)` accesses—even for a candidate that
reverts before touching a token. Initializer discovery and execution use the
committed R5 actor, not the model's first caller.
The R2 predicate is the same aggregate rule as `rules/r2`:
`totalDeposits() <= token.balanceOf(proxy)`, checked before and after every step.
An already-insolvent prestate is incomplete, not an AI veto. Predicate getters run
on discarded fork clones, so their state changes cannot seed candidate execution.
Creation/destruction is refused by R5's strict commit path; upgraded code has
already passed the R3 opcode/slot filter in the pipeline.

AI evidence keeps the exact shared `AiEvidence` type: model, completion id,
informational plaintext prompt/content SHA-256, signature kind, sanitized protocol
receipt digest (`preflight_digest`), and candidate hashes/reproduced flags.
`signature` contains UTF-8 JSON bytes of the bounded, normalized SDK receipt:
`{kind, signingAlgo, signingAddress, signedText, signature}`. No quote, arbitrary
SDK object, source or completion body is published. The provider's **wire-byte**
hashes are in `signedText`; they are distinct from the informational plaintext
prompt/content digests. Provider/outcome/count appear in the AI stage detail.
The protocol response digest also commits to usage and cost metadata. Usage is
returned only when numeric; `costUsd` is null because no documented billed-cost
field is available. Record actual console billing separately, never call an estimate billed.

[V, source inspection, 2026-11-03] Installed SDK 0.1.0 `dist/node.js:4141–4154`
looks up retained completion bytes, `4205–4222` passes those exact request/response
bytes to verification, and `2641–2673` verifies the signature over
`model:hash(request):hash(response)` and binds its signer to verified attestation.
The successful receipt fields are declared in `dist/index-BeV3pihr.d.ts:769–784`.
The sidecar requires the matching completion id and `provider_tee`, and preserves
only allowlisted hex/bounded-text receipt fields. It does not equate the local
content digest with a provider-signed wire hash.

[I] Remaining operational limits: each archive RPC request has a 30-second timeout
(shortened to the remaining run time). The pipeline checks the 20-minute
`RUN_DEADLINE` at stage boundaries and before signing, but blocking sandbox and
sidecar calls can overrun it until their own timeouts. There is no per-proposal AI
spending quota. The one-active-run queue is not account-level
abuse prevention. Independent later appraisal of the signer still needs the
provider's attestation evidence; the normalized receipt alone is not a DCAP proof.
The existing `--insecure-dev-identity` path still attempts socket attestation when
available; its registry has no archive RPC, so R5 withholds Pass independently.

### Deterministic checks and authorized real probe

```sh
node --test verifier-image/ai-sidecar/*.test.mjs
cargo test -p constitution-verifier --test ai --test replay --test attest_guest --lib -- --test-threads=1
cargo test -p constitution-verifier --test chain_e2e local_honest_r1_to_r5_pass_withheld_without_ai -- --nocapture --test-threads=1
```

Local protocol fakes test schema, signature gating, prompt-injection comments,
timeout, and a real proof-authenticated revm reproduction against the **R2 Fail
fixture**. The socket simulator test exercises the attested signing branch but
is not real DCAP or provider evidence.

[V, local, 2026-11-03] Final Rust checks: 16 targeted tests passed
(`--lib`, `--test ai`, `--test replay`, `--test attest_guest`, `--test http`),
`cargo clippy -p constitution-verifier --all-targets -- -D warnings` and
`cargo fmt -p constitution-verifier --check` succeeded. Both serialized
`--test chain_e2e` cases passed: honest R1–R5 Pass/AI Incomplete remained Pending
without certificate/submission (120,008 ms); the deterministic R2 Fail fixture
was rejected on the local real gate (84,767 ms). This is sandbox/anvil/simulator
evidence, not testnet or real AI/TEE evidence.

[V, local, 2026-11-03] `node --test` passed 10 tests on the available WSL Node
24.16.0, including an offline import/strict-constructor check of SDK 0.1.0.
The architecture's Node 24.22.1 pin was not installed locally; no inference,
attestation request, API key or billed-cost observation was used for these tests.

[U] Real NEAR AI completion/receipt and billed cost require founder-authorized B4
credentials. No live inference is performed by the local tests. Once the key is
already exported securely into the environment (never echo or put it on a command
line), the exact opt-in, potentially billable probe is:

```sh
node verifier-image/ai-sidecar/live-review.mjs
```

Expected observation, not yet exercised: exit 0, `status: complete`,
`signatureKind: provider_tee`, an observed completion id and numeric token usage.
This probe supplies no private vault source and cannot issue a verifier Pass.
Keep its timestamp, completion id, request/response hashes and actual billing
observation. Any nonzero exit is incomplete; gateway-only is never sufficient.
SDK integration reference: https://docs.near.ai/cloud/inference-sdk/javascript.md
(read 2026-11-03); constructor/request types also inspected in installed SDK 0.1.0.


## Commitments and limits

- Rule-set hash: SHA-256 over canonical Borsh `RuleSetManifest`.
- Source commitment: SHA-256 over Borsh `SourceBundle`; transport uses crypto_box
  0.9.1 sealed boxes with fresh X25519 transport keys.
- R2 harness digest: SHA-256 over Borsh of sorted `(relative filename,
  SHA-256(file bytes))` entries for every file in `rules/r2`.
- R1 passes exactly the manifest's `unsafe_allow` list to upgrades-core. These are
  committed layout-only exceptions. R3 independently enforces opcode/slot bans.
- Evidence field digest: SHA-256 of the canonical certificate with `evidence_root`
  zeroed. The final certificate binds the resulting evidence root, avoiding a cycle.
- Evidence provenance comes from `ProposalSource::archive_provider()` before
  hashing/signing: registry input is explicitly local; live reads expose only
  RPC hostnames and the finality/confirmation policy, not endpoint credentials.
- Tools receive only staged source and read-only Solidity libraries. They have no
  network, signing key, service environment, dstack socket or service home mount.
  Foundry FFI is disabled and compiler settings are trusted, not proposer-supplied.
- Incomplete tool results are never Pass. Solver unknown, bounded loops, malformed
  JSON, unexpected test identity and incomplete path accounting are reason 23 and, being
  load-sensitive, become a signed Reject only if the honest canary completes the same stage
  (see the canary rule above); a generic container timeout is infrastructure, never reason
  23. A symbolic counterexample is reason 20; a fuzz counterexample is reason 21.
- Evidence keeps content hashes, not private source. Tool diagnostics are hashed
  before status publication so compiler source snippets are not exposed through
  the HTTP status or evidence endpoints.

[I] The local queue allows one active run and retains a bounded in-memory history.
Restart clears history, cool-downs and limiter state, and changes both keys. The per-peer
limiter is only as good as the peer address it sees (see Admission); it is not durable
production rate limiting. Socket identity publication is
not, by itself, independent attestation appraisal.

## Direct dependency evidence

[V] Read live from the official sparse index on 2026-11-03; all selected entries
were non-yanked. Existing workspace dependencies and the existing tokio entry
are reused.

| Added dependency | Pin | Primary index source |
| --- | --- | --- |
| axum | 0.8.9 | https://index.crates.io/ax/um/axum |
| crypto_box | 0.9.1, seal feature | https://index.crates.io/cr/yp/crypto_box |
| ed25519-dalek | 3.0.0 | https://index.crates.io/ed/25/ed25519-dalek |
| sha3 | 0.12.0 | https://index.crates.io/sh/a3/sha3 |
| tempfile | 3.27.0 | https://index.crates.io/te/mp/tempfile |

The chain edges reuse existing workspace pins: `reqwest =0.13.5`,
`near-api =0.8.6`, and dev-only `near-workspaces =0.23.0`; no new version pin
is introduced by the chain integration.
