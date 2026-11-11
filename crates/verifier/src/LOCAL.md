# Local verifier plumbing

[I] A Pass certificate requires R1–R5 Pass, AI Clean with a verified `provider_tee`
response, complete candidate replay, and a socket-attested signing identity with
a nonzero release hash. Chain-connected startup requires a dstack socket;
without one it stops with `attestation_required`. Only explicit
`--insecure-dev-identity` permits NOT ATTESTED mode (ephemeral boot keys,
zero release hash, no quote claims, local registry and no submitter).
Never register that identity on testnet. Protocol fakes are not attestation.

## Packaged Linux service

[I] `verifier-image/Service.Dockerfile` packages the Rust verifier and the existing
Node 24.22.1 AI sidecar. `run-service.mjs` starts both, stops the sibling if either
exits, and forwards SIGTERM/SIGINT with a ten-second kill deadline. There is no
automatic restart: restarting loses the ephemeral certificate key. The sidecar
receives only PATH, HOME and NEARAI settings; Rust does not receive NEARAI settings.
Only the verifier API is published; the AI sidecar stays on container loopback.

Build from the repository root on Linux/WSL with Docker:

```sh
sh verifier-image/build-service.sh
```

The Rust builder uses the existing digest-pinned Rust 1.99.1 image and one Cargo
job. Dependency downloads are build-time only. Both Docker build contexts use
allowlists: `.env`, credentials, host node_modules and generated artifacts are
not image inputs. The tool image now embeds the trusted OpenZeppelin and
forge-std source trees; rebuild it before using the updated runner. Old images
without those trees are incompatible with the new absolute remappings.

**Docker sibling-container boundary.** The service talks to the host daemon, not
a nested privileged daemon. Its staging directory must be bind-mounted at the
**same absolute path** on both sides; TMPDIR points there. Only each staged bundle
is mounted into its tool container. Rules, reference sources and canary sources
are embedded in the service image; trusted libraries are embedded in the tool
image, not read from a host checkout. The Docker socket is root-equivalent:
it belongs only in the trusted service, never in an untrusted tool container.

Prepare a private service environment file containing PAGE_ORIGIN and, for chain
mode, the RPC/gate/relay settings in "Start" and NEARAI settings below. Do not print
resolved `docker compose config`: it includes the environment file's secrets.
Separately export the Compose interpolation settings:

```sh
export VERIFIER_SERVICE_IMAGE="$(docker image inspect constitution-verifier-service:local --format '{{.Id}}')"
export VERIFIER_TOOL_IMAGE="$(docker image inspect constitution-verifier-spike:local --format '{{.Id}}')"
export DOCKER_GID="$(stat -c %g /var/run/docker.sock)"
export VERIFIER_ENV_FILE=/absolute/private/verifier.env
export VERIFIER_WORK_DIR=/var/lib/constitution/work
```

Pre-create the staging directory owned by UID/GID 10001 with mode 0700; the
service fails rather than creating a missing host bind source. VERIFIER_UID and
VERIFIER_GID can override the service identity for a local host, but must remain
nonroot and match the staging owner. Permit that identity to read the private
environment/registry or relay credential and, in a CVM, connect to dstack.
Preload both images on the target host. Use registry digest references for a
published release, not mutable tags; local image IDs are for local checks.

Explicit **offline** startup uses an operator-controlled local registry and no
relay credential or dstack mount:

```sh
export LOCAL_PROPOSAL_REGISTRY=/absolute/private/local-proposals.json
docker compose -p constitution-local -f verifier-image/compose.service.yaml \
  -f verifier-image/compose.offline.yaml up -d
```

For a separately approved CVM deployment, use `compose.tee.yaml` **instead** of
the offline override, and set VERIFIER_RELAY_KEY_FILE to the host credential
file. Missing dstack stops startup; never substitute a simulator. Do not combine
TEE and offline overrides. The Compose files are deployment inputs, not evidence
that TDX appraisal or Phala ingress works. A public ingress and its peer-header
policy must be independently verified before public use; default socket policy
conservatively shares a proxy's rate budget.

The API publishes only `127.0.0.1:8787` on the host (VERIFIER_PORT overrides the
host port); sidecar port 8788 is not published. The service has a 512 MiB cap and
one CPU; sibling tool containers retain separate one-CPU/1-GiB caps. These are
limits, not measured peak requirements or proof that a 500-MB-free VPS fits.
Shutdown the local check with the same Compose files and `down`; do not stop or
restart an activated verifier without accounting for its lost signing identity.
This packaging does not implement an unattested VPS chain-signing fallback.

### Observed packaging smoke (2026-11-05)

[V, locally exercised on WSL2/Docker Desktop 28.5.1; not a CVM] Both images built
with Rust 1.99.1 and Node 24.22.1. Local image IDs and Docker inspect `.Size`:

| Image | Local image ID | Size bytes |
| --- | --- | --- |
| Tool sandbox | `sha256:1ae70d99b3451cbc2e09067b001efff1e30afe9bb545cd6471f8ecb5843bc4d8` | 319069062 |
| Verifier service | `sha256:f561cf088f9ed4889e21f46531f9c5361b225b542fe84170bd42e1a3e96593bb` | 130922929 |

These IDs record the earlier packaging smoke, not the latest source release.
They predate the initial chain RPC body-bound correction and the AI review
policy update. The replacement local build is recorded below; neither build
has been appraised inside a CVM.

- The actual offline Compose service accepted a sealed synthetic source bundle
  over its host-published API, ran the real compiler and honest compile canary in
  sibling Docker sandboxes, and issued `Reject { rule_id: 0, reason_code: 2 }`
  in 5,115 ms. An independent Node client verified the Ed25519 signature over
  SHA-256 of the canonical certificate bytes. AI was NotRun and no verdict was
  submitted. The offline identity had no quote, release measurement or compose hash.
- The real sidecar refused a browser Origin and returned Incomplete for a valid
  request without an API key. The packaged SDK imported with networking disabled;
  no inference or attestation call was made.
- The packaged chain-mode invocation without dstack exited 1 with
  `attestation_required`, not an offline downgrade. Stopping the actual sidecar
  caused the supervisor and verifier container to exit 1; the disposable Compose
  deployment was removed afterwards.
- Docker reported only API port 8787 published on host loopback (test port 19787),
  read-only root and restart policy `no`. Idle service usage was 27.96 MiB under
  its 512 MiB cap; this is not a workload peak or a whole-host requirement.
- Launcher lifecycle regression tests: 10 passed, including sibling failure,
  spawn failure, signal handling and forced kill. Both Compose variants parsed;
  the TEE variant was not deployed. Two independent scoped read-only reviews
  returned no findings; they are not a security audit.
- Focused Rust checks: startup 7 passed, socket/secret sandbox isolation 1 passed,
  sandbox integration 4 passed (including the committed R2 honest/failing pair
  and strict incomplete classification), and tool-boundary unit checks 5 passed.
  The final invalid-address/redaction regression was rerun after removing
  incidental error-format assertions: 1 passed. Verifier rustfmt check and
  binary-target clippy with `-D warnings` passed. No full-workspace suite is claimed.

No testnet transaction, paid inference, TDX/DCAP proof, final activation or
unattested VPS signing support follows from this smoke.

### Updated packaging smoke (2026-11-06)

[V, locally exercised on WSL2/Docker Desktop 28.5.1; not a CVM]
`sh verifier-image/build-service.sh` completed in 1,240.33 s, including
19 min 42 s for the single-job optimized Rust build. Local image references:

| Image | Local image ID | Size bytes |
| --- | --- | --- |
| Tool sandbox | `sha256:b4d4afc801f93a55dbb0f3acc45caa5ea4cb7fac54e5ff507e32c5994bc4d9ca` | 319070300 |
| Verifier service | `sha256:0e528a8567df25613e219d57f941e74559cb42ad672d987442271225f8d2c171` | 130922644 |

- The rebuilt service advertised the then-current rule-set commitment
  `0xa8a4bfb14f17a60b8dac20aaba63dfda2f89152fe7b7d4829f9d33edbb2cbf93`.
  A sealed invalid-Solidity bundle passed source-commitment validation, ran
  the honest compilation canary, and returned `Reject { rule_id: 0, reason_code: 2 }`.
  The client observed completion in 5,070 ms and independently verified the
  Ed25519 certificate signature. AI was NotRun; submission was local-only.
- With networking disabled, the packaged sidecar's real loopback HTTP handler
  completed a disposable SDK-protocol-double check for `max_tokens: 45000`,
  `thinking_token_budget: null` and the 300 s request deadline. This checks
  request construction, not a real provider response or attestation.
- The running sidecar without an API key returned `incomplete/missing_api_key`.
  A separate chain-mode startup without dstack exited 1 with `attestation_required`.
  No paid inference, chain write, offline-to-chain downgrade or CVM proof occurred.
- Docker inspect confirmed read-only root, restart policy `no`, a 512 MiB
  service cap, one CPU and only API port 8787 published on host loopback
  (test port 19789). These are limits, not measured workload peaks or proof
  that a 500 MiB host can run the service and sibling tools.
- The disposable Compose container and network were removed after the smoke.
  R2/solver and the full workspace suite were not rerun.

Machine-readable observations:
[`review-policy-2026-11-06.json`](../evidence/ai/review-policy-2026-11-06.json).


## Start

Run from the repository root inside WSL as an unprivileged user, with Docker and the
existing `constitution-verifier-spike:local` image available. The native listener
defaults to `127.0.0.1:8787`; VERIFIER_LISTEN_ADDR accepts an explicit IP:port
(not a hostname). Set PAGE_ORIGIN to the allowed page origin.

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

[I] `VERIFIER_MANIFEST` selects a manifest relative to the repository root/current
working directory; omission serves `rules/manifest.json`. For the separate live
demo use `VERIFIER_MANIFEST=rules/demo/manifest.json` in the private Compose
environment file, or as the Phala Compose interpolation variable. Empty, absolute,
traversing and canonicalized symlink-escape paths refuse startup.

V1 JSON/Borsh and its final-gate hash are unchanged. V2 retains the policy fields
and appends `reference_lineage` (VaultV1, VaultV2Honest, VaultV3Honest); its first
entry equals `reference`. SHA-256 covers the complete Borsh v2 struct, beginning
with u32 version 2 and without an enum tag. Rule 0 validates lineage source pins
and picks the first compiled runtime matching the observed current codehash;
no match is (0,4). R1, R5 and AI then use that current reference.
The image allowlist already includes `rules/**` and `contracts/src/demo/*.sol`;
the service Dockerfile copies both trees at build and runtime. No service image
build or CVM deployment follows from this source change.

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
| Run exceeds `RUN_DEADLINE` (30 minutes: sandbox stages, replay RPCs, AI candidates) | `failed`, `infrastructure_timeout` | None; Pending and retryable |
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
| `error_code` | Optional string, present on `failed`; stable codes from the classification table, including `tool_output_invalid` for malformed verifier-tool artifacts, `infrastructure_timeout` (also for the 30-minute run deadline), `worker_panic`; defensive pipeline-only unseal failures use `bad_bundle` |
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

### Demo reference-lineage check

[V, locally tested 2026-11-11; requires a working WSL Docker engine/tool image]
The test reused the existing gate wasm byte-for-byte. Do not rebuild or modify the
final gate. Serialize heavy checks across workers:

```sh
VERIFIER_MANIFEST=rules/demo/manifest.json CARGO_BUILD_JOBS=1 \
  flock --close /tmp/constitution-e2e.lock cargo test -p constitution-verifier \
  --test lineage_e2e -- --nocapture --test-threads=1
```

Use `--close` so a daemonized sandbox cannot inherit and retain the lock if its
test process is interrupted.

The new test reads real proposals from a local NEAR sandbox and proof-authenticated
Anvil state, then runs the production sealed-bundle pipeline and real tool image.
It checks drain (2,20) at V1, honest V1-to-V2 and V2-to-V3 R1–R5 Pass with AI
Incomplete, then all four attack rules at V3. An unavailable AI transport prevents
paid inference and Pass issuance. Between honest checks it explicitly impersonates
the local Anvil executor signer to advance the proxy; this is **not** a gate/MPC
approved upgrade, attestation, testnet transaction or bypass of the real gate.
No fabricated verdict is submitted to NEAR. Local case/stage durations are printed.

[I] V3 adds only a capacity-preview view and inherits V2 storage and existing
behavior. The committed R2 harness directly initializes the candidate proxy and
its V2 cap; it does not migrate from V1. This unchanged harness remains applicable
to V3's unchanged deposit/withdraw logic within its existing bounds. The unchanged
R5 corpus observes exactly V2-compatible calls. Empty-init V3-to-V3 and V3-to-V2
are not forbidden by lineage: lineage selects supported current code, not a
forward-only upgrade policy. V2/V3-to-V1 removes the cap field and fails R1.

[V, locally exercised 2026-11-11] The serialized lineage test passed in 490.91 s
(490,862 ms internal total). Case durations include proposal registration and the
real pipeline:

| Current proxy | Proposal | Observed result | Duration ms |
| --- | --- | --- | --- |
| V1 | Drain | Reject (2,20), R1 Pass | 67,151 |
| V1 | Honest V2 | R1–R5 Pass; AI Incomplete; no certificate | 84,565 |
| V2 | Honest V3 | R1–R5 Pass; AI Incomplete; no certificate | 83,070 |
| V3 | Drain | Reject (2,20), R1 Pass | 59,858 |
| V3 | Storage shuffle | Reject (1,10) | 4,310 |
| V3 | Self-destruct | Reject (3,30), R1/R2 Pass | 76,727 |
| V3 | Forbidden initializer | Reject (4,41), R1–R3 Pass | 77,650 |

Every local NEAR proposal remained Pending: the test does not submit its
unattested rejection certificates. The two local-only executor transactions
advanced Anvil to V3; final reads found 100 token units in the vault and capacity
999,900. The actual invocation used `flock` without `--close` and took
39m53.555s including queue/lock wait and compilation; the safer documented command
above closes the inherited lock descriptor. This is local deterministic evidence,
not a real AI completion, CVM appraisal, MPC signature or testnet evidence.


[V, locally tested 2026-11-11] Final focused regression commands:
`cargo test -p constitution-types -- --test-threads=1` (31 passed);
`cargo test -p constitution-cli rules::tests -- --test-threads=1` (4 passed, 0.05 s);
`forge test --root contracts --offline --match-contract VaultV3Test`
(2 passed, 8.75 ms suite; existing forge-std compiler warnings remain); and
`flock /tmp/constitution-e2e.lock cargo test -p constitution-verifier --lib -- --test-threads=1`
(71 passed, 1 existing ignored real-R2 canary, 17.50 s).
An earlier run failed while Docker was unavailable; after the lead restored it,
all five Docker-dependent regressions passed in this final run.

`cargo run -p constitution-cli -- rules hash rules/manifest.json` returned
`0x1efe635ac3eb0eb59b0df0d56c7243b2728922898b55487bc5cf8063db94a410`
both before and after the changes. The demo manifest command,
`cargo run -p constitution-cli -- rules hash rules/demo/manifest.json`, returned
`0xa988551688a3c7a7c61faa25591b933a419d1233779ce6e5bac38456474d8cc8`.



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
[V, local CLI, 2026-11-06] The previous 300 s rule-set hash was
`0xa8a4bfb14f17a60b8dac20aaba63dfda2f89152fe7b7d4829f9d33edbb2cbf93`.
[I] Recompute the changed 600 s manifest with
`cargo run -p constitution-cli -- rules hash rules/manifest.json`.

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
Incomplete with no candidates and an allowlisted reason in stage detail, committed
by the preflight digest. The committed timeout is 600 s and covers SDK setup,
completion and verification; the Rust HTTP caller adds 2 s transport grace.
The request uses `max_tokens: 100000` and `thinking_token_budget: null`: [U] the
provider's model output maximum is unverified. Reasoning has no separate cap but
remains inside the total completion budget. The 64 KiB candidate JSON and 96 KiB
sidecar-envelope limits remain independent safeguards.
No inference retry is automatic, and timeout does not prove upstream billing stopped.

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
(shortened to the remaining run time). The pipeline checks the 30-minute
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


## Local R2 readiness rerun (2026-11-05)

[V, WSL2/Docker Desktop 28.5.1, not TDX or testnet] The existing tool image
`sha256:1b2a2c17ba038dfb1047e5089faadeb92a5ea713eddb438e34c9ee7263b7e038`
passed the focused honest/drain depth-four sandbox test: honest Pass/0 in
109,801 ms; drain Fail/20 in 94,857 ms. Honest symbolic exploration completed
341 paths, then 2,000 seeded fuzz runs passed. The explicitly enabled real
Docker canary also passed compile/layout/solvency in 146,723 ms total.

Reproduce each once, sequentially, with the existing image and one Cargo job:

```sh
CARGO_BUILD_JOBS=1 cargo test -p constitution-verifier --test sandbox t_rule_2_committed_depth_four_harness_passes_honest_and_fails_drain --offline --locked -- --exact --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test -p constitution-verifier --lib pipeline::replay_stage_tests::real_docker_canary_completes_the_honest_fixture --offline --locked -- --exact --ignored --nocapture --test-threads=1
```

The production tool runner caps each sandbox at 1 CPU / 1 GiB; those are
limits, not measured peak consumption. No image rebuild, AI request or cloud
deployment was needed. Final deployed-image/TDX timing remains unverified.

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
