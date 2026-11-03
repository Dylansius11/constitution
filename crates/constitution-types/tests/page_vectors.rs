//! Cross-language vectors for the public page (`web/`), which re-derives every byte independently.
//!
//! * T-ENC-2 `vectors/certificate.json`: canonical Borsh certificates with their SHA-256 and the
//!   decoded JSON. `web/test/certificate.test.ts` decodes them with a separate TypeScript decoder.
//! * T-ENC-3 `vectors/sealed-bundle.json`: sealed-box interoperability between Rust `crypto_box`
//!   0.9.1 and libsodium (`libsodium-wrappers` 0.8.4). Rust seals one ciphertext for the page to
//!   open; the page seals one for Rust to open. Both reject tampering.
//!
//! Every key in the sealed-bundle file is a TEST-ONLY key derived from a public label. It protects
//! nothing and must never be reused.
//!
//! Check mode (default) asserts byte equality with the committed files. Regenerate with
//! `CONSTITUTION_WRITE_VECTORS=1 cargo test -p constitution-types --test page_vectors`, then run
//! `CONSTITUTION_WRITE_VECTORS=1 pnpm --dir web test` once to refresh the libsodium-sealed
//! ciphertext (libsodium seals with a random ephemeral key, so that field is written by the page
//! tests and preserved by this file).
//!
//! The page tests also write a freshly libsodium-sealed ciphertext to
//! `scratch/page-vectors/js-fresh-seal.json` on every run; when that file exists this test opens it
//! too, so a live cross-process check needs no committed bytes.
use base64::{Engine, engine::general_purpose::STANDARD};
use constitution_types::{
    AccountId, AiReview, Certificate, DOMAIN, Verdict, bundle::SourceBundle, certificate_hash,
};
use crypto_box::{
    SecretKey,
    aead::rand_core::{CryptoRng, Error, RngCore},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CERT_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vectors/certificate.json"
);
const SEAL_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vectors/sealed-bundle.json"
);
const FRESH_JS_SEAL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scratch/page-vectors/js-fresh-seal.json"
);

fn writing() -> bool {
    std::env::var("CONSTITUTION_WRITE_VECTORS").as_deref() == Ok("1")
}

fn digest(label: &str) -> [u8; 32] {
    Sha256::digest(label.as_bytes()).into()
}

fn bytes<const N: usize>(name: &str, field: &str) -> [u8; N] {
    let full = digest(&format!("constitution.page-vectors/{name}/{field}"));
    full[..N].try_into().expect("N <= 32")
}

fn hex0(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn certificate(name: &str, gate: &str, ai: AiReview, verdict: Verdict) -> Certificate {
    Certificate {
        domain: DOMAIN,
        gate: gate.parse::<AccountId>().expect("valid account id"),
        proposal_id: 7,
        attempt: 1,
        chain_id: 84_532,
        proxy: bytes(name, "proxy"),
        executor: bytes(name, "executor"),
        current_impl: bytes(name, "current_impl"),
        current_impl_codehash: bytes(name, "current_impl_codehash"),
        new_impl: bytes(name, "new_impl"),
        new_impl_codehash: bytes(name, "new_impl_codehash"),
        init_calldata_hash: bytes(name, "init_calldata_hash"),
        source_commitment: bytes(name, "source_commitment"),
        rule_set_hash: bytes(name, "rule_set_hash"),
        verifier_pubkey: bytes(name, "verifier_pubkey"),
        verifier_release_hash: bytes(name, "verifier_release_hash"),
        evidence_block_number: 41_000_000,
        evidence_block_hash: bytes(name, "evidence_block_hash"),
        evidence_root: bytes(name, "evidence_root"),
        ai,
        verdict,
        issued_at_s: 1_793_700_000,
        expires_at_s: 1_793_703_600,
    }
}

fn certificates() -> Vec<(&'static str, Certificate)> {
    let n1 = "pass_ai_clean_provider_tee";
    let pass = certificate(
        n1,
        "gate.constitution.testnet",
        AiReview::Clean {
            evidence_hash: bytes(n1, "ai_evidence"),
            signature_kind: 1,
        },
        Verdict::Pass,
    );
    let n2 = "reject_r2_ai_vetoed";
    let mut veto = certificate(
        n2,
        "gate.constitution.testnet",
        AiReview::Vetoed {
            evidence_hash: bytes(n2, "ai_evidence"),
            counterexample_hash: bytes(n2, "ai_counterexample"),
        },
        Verdict::Reject {
            rule_id: 2,
            reason_code: 22,
        },
    );
    veto.proposal_id = 8;
    let n3 = "reject_r1_ai_not_run";
    let mut layout = certificate(
        n3,
        "gate.constitution.testnet",
        AiReview::NotRun,
        Verdict::Reject {
            rule_id: 1,
            reason_code: 10,
        },
    );
    layout.proposal_id = 9;
    layout.attempt = 2;
    // Largest values every integer field can carry, a 64-character account id, signature kind 2.
    let n4 = "pass_ai_clean_gateway_boundary_values";
    let mut max = certificate(
        n4,
        &"a".repeat(64),
        AiReview::Clean {
            evidence_hash: bytes(n4, "ai_evidence"),
            signature_kind: 2,
        },
        Verdict::Pass,
    );
    max.proposal_id = u64::MAX;
    max.attempt = u32::MAX;
    max.chain_id = u64::MAX;
    max.evidence_block_number = u64::MAX;
    max.issued_at_s = u64::MAX;
    max.expires_at_s = u64::MAX;
    // Smallest account id, all-zero hashes, largest rule and reason codes.
    let n5 = "reject_extreme_ids_all_zero";
    let mut zero = certificate(
        n5,
        "ab",
        AiReview::NotRun,
        Verdict::Reject {
            rule_id: 255,
            reason_code: 65_535,
        },
    );
    zero.proposal_id = 0;
    zero.attempt = 0;
    zero.chain_id = 0;
    zero.proxy = [0; 20];
    zero.executor = [0; 20];
    zero.current_impl = [0; 20];
    zero.current_impl_codehash = [0; 32];
    zero.new_impl = [0; 20];
    zero.new_impl_codehash = [0; 32];
    zero.init_calldata_hash = [0; 32];
    zero.source_commitment = [0; 32];
    zero.rule_set_hash = [0; 32];
    zero.verifier_pubkey = [0; 32];
    zero.verifier_release_hash = [0; 32];
    zero.evidence_block_number = 0;
    zero.evidence_block_hash = [0; 32];
    zero.evidence_root = [0; 32];
    zero.issued_at_s = 0;
    zero.expires_at_s = 0;
    vec![(n1, pass), (n2, veto), (n3, layout), (n4, max), (n5, zero)]
}

/// Byte strings that Rust refuses and the page must refuse too.
fn invalid_certificates() -> Vec<(&'static str, &'static str, Vec<u8>)> {
    let valid = certificates();
    let good = valid[0].1.to_canonical_bytes().expect("encode");
    let not_run = valid[2].1.to_canonical_bytes().expect("encode");
    // Tail of the NotRun/Reject certificate: ai tag (1), verdict tag + rule + reason (4), two u64.
    let verdict_at = not_run.len() - 16 - 4;
    let gate_at = 16 + 4;
    let mut bad_ai = not_run.clone();
    bad_ai[verdict_at - 1] = 3;
    let mut bad_verdict = not_run.clone();
    bad_verdict[verdict_at] = 2;
    let mut upper_gate = good.clone();
    upper_gate[gate_at] = b'G';
    let mut non_utf8_gate = good.clone();
    non_utf8_gate[gate_at] = 0xff;
    let mut huge_gate_length = good.clone();
    huge_gate_length[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut trailing = good.clone();
    trailing.push(0);
    vec![
        ("empty", "no bytes", vec![]),
        (
            "truncated_one_byte",
            "last byte missing",
            good[..good.len() - 1].to_vec(),
        ),
        (
            "trailing_byte",
            "one extra byte after a complete certificate",
            trailing,
        ),
        ("unknown_ai_variant", "AiReview tag 3", bad_ai),
        ("unknown_verdict_variant", "Verdict tag 2", bad_verdict),
        (
            "uppercase_gate_account",
            "account id with an uppercase letter",
            upper_gate,
        ),
        (
            "non_utf8_gate_account",
            "account id bytes are not UTF-8",
            non_utf8_gate,
        ),
        (
            "gate_length_beyond_input",
            "account id length u32::MAX",
            huge_gate_length,
        ),
    ]
}

fn certificate_vectors() -> Value {
    let vectors: Vec<Value> = certificates()
        .into_iter()
        .map(|(name, cert)| {
            let encoded = cert.to_canonical_bytes().expect("encode");
            assert_eq!(
                Certificate::from_canonical_bytes(&encoded).expect("decode"),
                cert,
                "{name} must round trip"
            );
            json!({
                "name": name,
                "certificate_b64": STANDARD.encode(&encoded),
                "certificate_hash": hex0(&certificate_hash(&encoded)),
                "decoded": serde_json::to_value(&cert).expect("serde"),
            })
        })
        .collect();
    let invalid: Vec<Value> = invalid_certificates()
        .into_iter()
        .map(|(name, why, bytes)| {
            assert!(
                Certificate::from_canonical_bytes(&bytes).is_err(),
                "Rust must refuse {name}"
            );
            json!({"name": name, "why": why, "bytes_b64": STANDARD.encode(bytes)})
        })
        .collect();
    json!({
        "label": "T-ENC-2 certificate vectors produced by constitution-types (borsh 1.8.1); decoded independently by web/test/certificate.test.ts. decoded is the Rust serde form: domain is a byte array, hashes and addresses are 0x hex, verifier_pubkey is base64, enums are externally tagged.",
        "vectors": vectors,
        "invalid": invalid,
    })
}

fn compare_with_file(file: &str, produced: &Value, what: &str) {
    if writing() {
        std::fs::write(file, serde_json::to_string_pretty(produced).unwrap() + "\n").unwrap();
    }
    let committed: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(produced, &committed, "{what} drifted from {file}");
}

#[test]
fn t_enc_2_certificate_vectors_match_committed_file() {
    compare_with_file(CERT_FILE, &certificate_vectors(), "certificate encoding");
}

/// Deterministic `CryptoRng` so the Rust-sealed vector is reproducible. Test-only.
struct SeededRng {
    seed: [u8; 32],
    counter: u64,
}

impl RngCore for SeededRng {
    fn next_u32(&mut self) -> u32 {
        let mut word = [0u8; 4];
        self.fill_bytes(&mut word);
        u32::from_le_bytes(word)
    }
    fn next_u64(&mut self) -> u64 {
        let mut word = [0u8; 8];
        self.fill_bytes(&mut word);
        u64::from_le_bytes(word)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(32) {
            let mut hash = Sha256::new();
            hash.update(self.seed);
            hash.update(self.counter.to_le_bytes());
            self.counter += 1;
            chunk.copy_from_slice(&hash.finalize()[..chunk.len()]);
        }
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for SeededRng {}

fn test_key(label: &str) -> SecretKey {
    SecretKey::from_bytes(digest(label))
}

fn plaintext() -> Vec<u8> {
    let bundle = SourceBundle {
        contract_name: "src/VaultV2Honest.sol:VaultV2Honest".into(),
        files: vec![
            (
                "src/VaultV2Honest.sol".into(),
                "// SPDX-License-Identifier: MIT\npragma solidity 0.8.38;\ncontract VaultV2Honest {}\n"
                    .as_bytes()
                    .to_vec(),
            ),
            ("src/Lib.sol".into(), vec![0, 1, 2, 253, 254, 255]),
        ],
        foundry_profile: "default".into(),
    };
    borsh::to_vec(&bundle).expect("encode bundle")
}

fn key_json(key: &SecretKey) -> Value {
    json!({
        "secret_key": hex0(&key.to_bytes()),
        "public_key": hex0(key.public_key().as_bytes()),
    })
}

/// Flip one bit at each structurally distinct position, truncate, and try the wrong recipient.
fn assert_rejects_tampering(key: &SecretKey, sealed: &[u8], what: &str) {
    for position in [0, 31, 32, 47, 48, sealed.len() - 1] {
        let mut tampered = sealed.to_vec();
        tampered[position] ^= 0x01;
        assert!(
            key.unseal(&tampered).is_err(),
            "{what}: bit flip at {position} must fail"
        );
    }
    for length in [0, 31, 32, 47, sealed.len() - 1] {
        assert!(
            key.unseal(&sealed[..length]).is_err(),
            "{what}: truncation to {length} must fail"
        );
    }
    let stranger = test_key("constitution.page-vectors/stranger");
    assert!(
        stranger.unseal(sealed).is_err(),
        "{what}: wrong recipient must fail"
    );
}

fn field_bytes(value: &Value, pointer: &str) -> Vec<u8> {
    let text = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{pointer} missing"));
    match text.strip_prefix("0x") {
        Some(digits) => hex::decode(digits).expect("hex"),
        None => STANDARD.decode(text).expect("base64"),
    }
}

#[test]
fn t_enc_3_sealed_bundle_vectors_interoperate_with_libsodium() {
    let rust_recipient = test_key("constitution.page-vectors/rust-recipient");
    let js_recipient = test_key("constitution.page-vectors/js-recipient");
    let plain = plaintext();
    let mut rng = SeededRng {
        seed: digest("constitution.page-vectors/rust-seal-rng"),
        counter: 0,
    };
    let rust_to_js = js_recipient
        .public_key()
        .seal(&mut rng, &plain)
        .expect("seal");
    assert_eq!(js_recipient.unseal(&rust_to_js).expect("self open"), plain);
    assert_rejects_tampering(&js_recipient, &rust_to_js, "rust_to_js");

    let produced = |js_to_rust: Value| {
        json!({
            "label": "T-ENC-3 sealed-box vectors. TEST-ONLY keys derived from public labels; they protect nothing and must never be reused. rust_to_js is sealed by crypto_box 0.9.1 with a deterministic RNG; js_to_rust is sealed by libsodium-wrappers 0.8.4 and written by web/test/seal.test.ts when CONSTITUTION_WRITE_VECTORS=1.",
            "plaintext_hex": hex0(&plain),
            "rust_recipient": key_json(&rust_recipient),
            "js_recipient": key_json(&js_recipient),
            "rust_to_js": {"sealed_b64": STANDARD.encode(&rust_to_js)},
            "js_to_rust": js_to_rust,
        })
    };
    let existing: Option<Value> = std::fs::read_to_string(SEAL_FILE)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    let preserved = existing
        .as_ref()
        .and_then(|value| value.get("js_to_rust").cloned())
        .unwrap_or(Value::Null);
    if writing() {
        let text = serde_json::to_string_pretty(&produced(preserved.clone())).unwrap() + "\n";
        std::fs::write(SEAL_FILE, text).unwrap();
    }
    let committed: Value =
        serde_json::from_str(&std::fs::read_to_string(SEAL_FILE).unwrap()).unwrap();
    assert_eq!(
        produced(committed["js_to_rust"].clone()),
        committed,
        "Rust-side sealed-bundle fields drifted from {SEAL_FILE}"
    );

    if committed["js_to_rust"].is_null() {
        assert!(
            writing(),
            "js_to_rust is empty: run `CONSTITUTION_WRITE_VECTORS=1 pnpm --dir web test`"
        );
        return;
    }
    let js_to_rust = field_bytes(&committed, "/js_to_rust/sealed_b64");
    assert_eq!(
        rust_recipient
            .unseal(&js_to_rust)
            .expect("open libsodium seal"),
        plain
    );
    assert_rejects_tampering(&rust_recipient, &js_to_rust, "js_to_rust");
}

#[test]
fn t_enc_3_opens_a_fresh_libsodium_seal_written_by_the_page_tests() {
    let Ok(text) = std::fs::read_to_string(FRESH_JS_SEAL) else {
        eprintln!(
            "no fresh libsodium seal at {FRESH_JS_SEAL}; run `pnpm --dir web test` first (committed vector still covers the direction)"
        );
        return;
    };
    let fresh: Value = serde_json::from_str(&text).expect("fresh seal json");
    let rust_recipient = test_key("constitution.page-vectors/rust-recipient");
    assert_eq!(
        field_bytes(&fresh, "/recipient_public_key"),
        rust_recipient.public_key().as_bytes().to_vec(),
        "the page sealed to a different key than the Rust vector key"
    );
    let sealed = field_bytes(&fresh, "/sealed_b64");
    assert_eq!(
        rust_recipient
            .unseal(&sealed)
            .expect("open fresh libsodium seal"),
        field_bytes(&fresh, "/plaintext_hex")
    );
    assert_rejects_tampering(&rust_recipient, &sealed, "fresh js seal");
}
