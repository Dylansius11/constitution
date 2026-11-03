# Local verifier plumbing

[I] The verifier never emits a Pass certificate while AI review/reproduction remain
unavailable. Without a dstack socket, identity is explicitly NOT ATTESTED: boot
keys are ephemeral, release hash is `[0; 32]`, and quote/event-log claims are absent.
Never register this local identity on testnet. Chain connectivity is not attestation.

## Start

Run from the repository root inside WSL as an unprivileged user, with Docker and the
existing `constitution-verifier-spike:local` image available. The listener remains
loopback-only at `127.0.0.1:8787`. Set `PAGE_ORIGIN` to the allowed page origin.

Default mode reads real gate and Base JSON-RPC endpoints and submits issued verdicts:

```text
NEAR_RPC_URL=http://127.0.0.1:<sandbox-port>
BASE_SEPOLIA_RPC_URL=http://127.0.0.1:<anvil-port>
VERIFIER_RELAY_ACCOUNT_ID=<sandbox-relay-account>
VERIFIER_RELAY_KEY_FILE=<near-cli-legacy-credential.json>
cargo run -p constitution-verifier
```

`BASE_SEPOLIA_ARCHIVE_RPC_URL`, when nonempty, takes precedence over
`BASE_SEPOLIA_RPC_URL`. When `VERIFIER_RELAY_KEY_FILE` is unset, `NEAR_NETWORK_ID`
is required and the file defaults to
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

Each issued certificate/signature pair triggers exactly one relay submission.
Successful execution adds `near_tx_hash` to run JSON. A failed/pending submission
records `submit: Incomplete`, retains the certificate, and never retries
automatically. Inspect the relay's chain transactions before starting another run,
because a lost response may still have landed. No certificate means no submission.
All R2 manifest fields must be resolved before startup; null never becomes a bound.

## Local end-to-end check

With the prebuilt real gate at
`target/near/constitution_gate/constitution_gate.wasm`, Foundry in
`~/.foundry/bin`, and the existing Docker image, run:

```text
CARGO_BUILD_JOBS=6 cargo test -p constitution-verifier --test chain_e2e -- --nocapture
```

If the wasm is absent, build it from `crates/gate` with
`cargo near build non-reproducible-wasm`. Each test starts its own NEAR sandbox
and anvil (`chain_id=84532`) and runs `Deploy.s.sol`. The R2 Fail fixture must
produce Reject rule 2 reason 20 on the gate. The honest proposal seeds the corpus
actor before pinning the evidence block, passes R1 through R5, and stays Pending:
AI review is not integrated, so no certificate or submission is produced.
Both tests POST sealed bundles over HTTP and check the proxy is unchanged.
They print exact local runtimes and final gate proposal JSON; a NEAR hash is
printed only when observed. Allow ten minutes per run; R2 uses one CPU.
This is local evidence, never testnet or attestation evidence.

## R5 replay stage

R5 runs after R4 with the current/candidate runtimes already reproduced by the
compiling stage. It parses `rules/r5/corpus.json`, enforces the manifest's seven-call
bound and exact corpus SHA-256, fetches proof-authenticated witness data at the
observed block, then executes the bounded differential replay.

The stage requires an RPC supporting `debug_traceCall` / `prestateTracer`,
`eth_getProof`, and historical code/storage reads. `GateSource` passes its Base
endpoint internally through `ProposalSource::archive_rpc()`; prefer
`BASE_SEPOLIA_ARCHIVE_RPC_URL` for an archive provider. The full URL is never
included in evidence or run details. LocalRegistry has no archive endpoint:
reaching R5 in offline mode produces Incomplete 51, not a pass.

R5 records Pass 0, divergence Fail 50, or Incomplete 51. Missing RPC/corpus,
commitment mismatch, unreadable observations, or an incomplete replay all fail
closed. Codes 50/51 produce signed Reject rule 5 certificates under the existing
rejection policy. Evidence hashes JSON witness bytes for a complete observation,
or the sanitized error message for Incomplete; status exposes only `reason N`
and, for Incomplete, the sanitized message.

After all deterministic rules pass, the outcome remains
`withheld: AI review not integrated; no Pass without AI`.
AI review, reproduction and signing remain NotRun; no verdict is submitted.

[I] The committed two-successor-block lag is a minimal demo evidence policy,
not a Base finality claim. The corpus actor is a public anvil account; testnet
activation requires a deliberately seeded testnet actor and a new corpus commitment.
Rule-set hash (CLI observed locally, 2026-11-03):
`0x0707c9b79926d02204b74fe9c67d840b7ed6a3b8d73862b8a37ae1ecd5a2e1b1`.

Library witness policy and limits:
[`rules/r5/README.md`](../../../rules/r5/README.md).

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
  JSON, unexpected test identity, timeout and incomplete path accounting are reason
  23. A symbolic counterexample is reason 20; a fuzz counterexample is reason 21.
- Evidence keeps content hashes, not private source. Tool diagnostics are hashed
  before status publication so compiler source snippets are not exposed through
  the HTTP status or evidence endpoints.

[I] The local queue allows one active run and retains a bounded in-memory history.
Restart clears history and changes both keys. This is not durable production
storage or rate limiting for a public deployment. Socket identity publication is
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
