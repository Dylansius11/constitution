use constitution_verifier::identity::{Identity, release_hash, report_data};
use sha2::{Digest, Sha256};

#[test]
fn attest_report_data_independently_binds_both_keys_and_nonce() {
    let identity = Identity::new();
    let view = identity.view([9; 32]);
    let preimage = [
        b"constitution.v1/key".as_slice(),
        &view.verifier_pubkey.0,
        &view.transport_pubkey.0,
        &identity.boot_nonce,
    ]
    .concat();
    let mut expected = [0; 64];
    expected[..32].copy_from_slice(&Sha256::digest(&preimage));
    assert_eq!(view.report_data_preimage.0, preimage);
    assert_eq!(identity.report_data(), expected);
    assert_eq!(
        report_data(
            identity.pubkey(),
            *identity.transport.public_key().as_bytes(),
            identity.boot_nonce
        ),
        expected
    );
}

#[test]
fn attest_release_hash_independently_binds_only_static_measurements_and_compose() {
    let measurements = [[1; 48], [2; 48], [3; 48], [4; 48]];
    let compose = [5; 32];
    let mut hash = Sha256::new();
    hash.update(b"constitution.v1/release");
    for measurement in measurements {
        hash.update(measurement);
    }
    hash.update(compose);
    assert_eq!(
        release_hash(&measurements, compose),
        <[u8; 32]>::from(hash.finalize())
    );
}

#[test]
fn attest_local_identity_is_ephemeral_and_not_attested() {
    let a = Identity::new();
    let b = Identity::new();
    assert_ne!(a.pubkey(), b.pubkey());
    assert_ne!(a.boot_nonce, b.boot_nonce);
    assert_ne!(a.transport.public_key(), b.transport.public_key());
    assert_eq!(a.release_hash(), [0; 32]);
    let view = a.view([9; 32]);
    assert!(view.quote_hex.is_empty());
    assert!(view.label.contains("not attested"));
}
