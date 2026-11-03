# Attestation appraisal evidence

## Sources read 2026-11-03

[V] `dcap-qvl =0.7.1` exists and is not yanked, observed at https://crates.io/api/v1/crates/dcap-qvl/0.7.1 on 2026-11-03. Production appraisal uses its Intel production root; there is no CLI custom-root or insecure signature bypass. Default collateral service: `https://pccs.phala.network`, matching https://github.com/Phala-Network/dcap-qvl/blob/05c200b752ecb824b73e5b1ae63cc047f1702ab2/src/collateral.rs . Override with `--pccs-url` or `PCCS_URL`. PCCS supplies Intel-signed collateral, not a new trust anchor.

[V] Guest API/socket researched at https://github.com/dstack-tee/dstack/blob/d7e15dc9020101bb37d93ea9eec5f1f4fefe93e7/sdk/curl/api.md and https://github.com/dstack-tee/dstack/blob/d7e15dc9020101bb37d93ea9eec5f1f4fefe93e7/docs/guest-api-v0.md on 2026-11-03. `/var/run/dstack.sock` exposes the frozen unversioned TDX `POST /GetQuote` (hex `report_data`, padded to 64 bytes) and `GET /Info` (`compose_hash`). Current v1 uses `Attest`, not `GetQuote`; this integration deliberately uses the supported frozen TDX API required by the project. Info is not evidence: the CLI authenticates its compose hash against runtime measurements. Only the verifier service may receive this socket, never tool containers.

[V] Runtime encoding/replay source: https://github.com/dstack-tee/dstack/blob/d7e15dc9020101bb37d93ea9eec5f1f4fefe93e7/dstack/cc-eventlog/src/runtime_events.rs and `tdx.rs`, read 2026-11-03. V1 hashes `event_type_le || ':' || name || ':' || payload`; v2 hashes canonical JSON and requires a matching preimage. RTMR3 starts with 48 zero bytes, extends SHA-384 over previous measurement and each authenticated digest. Compose hash is extracted from the unique `compose-hash` runtime event, not equated with RTMR3 itself.

## Published real Intel-signed evidence

[V] The following files are downloaded byte-for-byte on 2026-11-03 from Phala's maintained DCAP verifier, commit `05c200b752ecb824b73e5b1ae63cc047f1702ab2`:

- `tdx_quote`: https://github.com/Phala-Network/dcap-qvl/blob/05c200b752ecb824b73e5b1ae63cc047f1702ab2/sample/tdx_quote
- `tdx_quote_collateral.json`: same commit, `sample/tdx_quote_collateral.json`.
- `tdx_quote_outdated`: same commit, `sample/tdx_quote_outdated`.
- `tdx_quote_outdated_collateral.json`: same commit, `sample/tdx_quote_outdated_collateral.json`.
- `sgx_quote` and `sgx_quote_collateral.json`: same commit, `sample/sgx_quote` and `sample/sgx_quote_collateral.json`. These additionally provide Intel-signed `ConfigurationAndSWHardeningNeeded` evidence for the strict status-policy negative test; they are SGX, not TDX.

[I] Offline tests verify these archived quotes at an explicit historical time within their collateral validity windows. They do not claim current collateral validity or quote freshness. The normal CLI always uses the current system time and freshly fetched collateral. Network collateral fetching is a separately ignored, opt-in test (`cargo test -p constitution-cli --test attest attest_network_collateral_fetch -- --ignored`).

[I] The real evidence proves Intel certificate/signature/collateral verification, rejection of tampered signed bytes, TCB classification and expiry enforcement. The upstream fixture does not publish a `constitution.v1/key` preimage or corresponding boot-generated keys, so it cannot honestly demonstrate a full CONSTITUTION identity PASS. No quote is modified and then represented as Intel-signed.

[V] The published `tdx_quote_outdated` pair yields `No matching TCB level found` in dcap-qvl 0.7.1 at a valid collateral time; the test requires this fail-closed rejection rather than inventing an authenticated OutOfDate status. A clean non-UpToDate TDX classification fixture was not available in the researched upstream samples. The status-policy test uses the real Intel-signed SGX classification above, and unit tests reject every non-UpToDate status explicitly.

## Official dstack simulator output — NOT Intel-signed, NOT a CVM

[V] `simulator_identity.json`, `simulator_collateral.json`, `simulator_root.der` were generated locally on 2026-11-03 with the unmodified official dstack `TdxGenerator::from_seed` and `attest_with_measurements` implementation from https://github.com/dstack-tee/dstack/blob/d7e15dc9020101bb37d93ea9eec5f1f4fefe93e7/dstack/crates/mock-attestation/src/tdx.rs . Its deterministic key helper was copied verbatim from `src/lib.rs`; the driver supplied report data, MRTD and RTMRs. The simulator PKI seed is the public test value `[0x42;32]`; no private keys are committed. The source is Apache-2.0, copyright 2026 Phala Network. See https://github.com/dstack-tee/dstack/blob/d7e15dc9020101bb37d93ea9eec5f1f4fefe93e7/dstack/crates/mock-attestation/README.md .

[I] Driver inputs: public synthetic identity keys `[1;32]`, `[2;32]`, nonce `[3;32]`; compose hash `sha256("simulator-only compose fixture")`; one v1 compose-hash event; MRTD `[0x11;48]`, RTMR0 `[0x20;48]`, RTMR1 `[0x21;48]`, RTMR2 `[0x22;48]`, RTMR3 from that event. The official simulator signs the resulting report and collateral. The test verifies that chain against the committed simulator root **only in test code**, then runs the same production key/preimage, RTMR3, compose and release checks. Another test proves production appraisal rejects this non-Intel root. These are simulator proofs, not real Intel or Phala CVM evidence.

[I] Guest-socket tests use a local Unix HTTP responder and a patched simulator report to verify request/response handling and independent identity formulas. They are transport/unit tests, not attestation proofs. Tool isolation is exercised through the real local Docker sandbox, not a CVM.

## Remaining real Phala CVM prerequisite (B3)

[U] No real Phala CVM has been deployed or appraised by this work. B3 still gates a full positive appraisal over boot-generated CONSTITUTION keys, live Intel-signed TDX evidence, current UpToDate collateral, measured compose and tool sandbox isolation in the CVM. A PASS authenticates measurements, not owner approval: supply `--expect-compose-hash` from an independently reviewed compose commitment before registration. Evidence has no verifier-chosen freshness challenge in this interface; registration operators must obtain a current boot identity through their expected service endpoint.

## Observed local checks (2026-11-03, WSL Ubuntu)

- [V] `cargo test -p constitution-cli`: 44 passed, 1 ignored (the opt-in PCCS network test). Includes the Intel-signed TDX positive chain, signed SGX non-UpToDate policy rejection, simulator binding checks, CLI JSON/exit behavior and existing CLI sandbox tests.
- [V] `cargo test -p constitution-verifier --test 'attest*'`: 7 passed (3 guest HTTP transport tests, 3 independent formula/ephemeral identity tests, 1 real local Docker socket/key isolation test).
- [V] `cargo test -p constitution-verifier --lib identity`: zero matching unit tests, 4 filtered out; independent formula coverage is in `attest_identity.rs`, not an invented unit-test count.
- [V] `cargo clippy -p constitution-cli --all-targets -- -D warnings` and the equivalent `constitution-verifier` command: exit 0.
- [V] `cargo fmt -p constitution-cli -p constitution-verifier -- --check`: exit 0.
- [U] Public PCCS network test was not run. No testnet deployment, real Phala CVM, live Intel quote over these boot keys, or CVM certificate/gate verification was exercised.

