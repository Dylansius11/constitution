# Local verifier plumbing

[I] This service is not attested and never emits a Pass certificate. R5, AI review,
AI reproduction, and NEAR submission are intentionally not implemented in this local
assignment. A run through R4 ends `withheld: required stages not implemented`.
Deterministic rejection certificates use a fresh in-memory Ed25519 key on each boot.
Never register this identity on testnet. Quote, event log, compose and release claims
are absent or empty, not fabricated.

## Start

Run from the repository root inside WSL as an unprivileged user, with Docker and the
local `constitution-verifier-spike:local` image available. Set `PAGE_ORIGIN` to the
page origin and `LOCAL_PROPOSAL_REGISTRY` to an operator-controlled JSON array of
`LocalProposal` records (see `proposals.rs`). Then run:

```text
cargo run -p constitution-verifier -- --insecure-dev-identity
```

The listener is loopback-only. Registry observations are explicitly local input,
not chain evidence. POST accepts only gate, proposal id and sealed bundle. The
operator registry supplies all proposal commitments and local chain observations.
All R2 manifest fields must be resolved before startup. Null never becomes a bound
or a successful result. The committed manifest remains unresolved until the lead
receives measured parameters from the harness owner.

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
storage, rate limiting for a public deployment, or an attestation implementation.

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
