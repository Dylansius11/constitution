use constitution_types::{Base64, Base64Bytes, manifest::StageStatus};
use constitution_verifier::{
    pipeline::Run,
    submit::{SubmittedRun, VerdictSubmitter},
};
use std::sync::atomic::{AtomicUsize, Ordering};
struct Refusing(AtomicUsize);
impl VerdictSubmitter for Refusing {
    fn submit(
        &self,
        _: &constitution_types::AccountId,
        certificate: &Base64Bytes,
        signature: &Base64<64>,
    ) -> Result<String, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        assert_eq!(certificate.0, vec![1, 2]);
        assert_eq!(signature.0, [3; 64]);
        Err("gate refused certificate".into())
    }
}
#[test]
fn failed_submission_is_incomplete_and_not_retried() {
    let mut run = Run::queued();
    run.certificate_b64 = Some(Base64Bytes(vec![1, 2]));
    run.signature_b64 = Some(Base64([3; 64]));
    let relay = Refusing(AtomicUsize::new(0));
    let result = SubmittedRun::submit(run, &"gate.testnet".parse().unwrap(), &relay);
    assert_eq!(relay.0.load(Ordering::SeqCst), 1);
    assert!(result.near_tx_hash.is_none());
    assert_eq!(result.run.stage, "done");
    let stage = result
        .run
        .stages
        .iter()
        .find(|s| s.name == "submit")
        .unwrap();
    assert_eq!(stage.status, StageStatus::Incomplete);
    assert_eq!(stage.detail, "gate refused certificate");
    assert!(result.run.outcome.contains("submission incomplete"));
}
#[test]
fn no_certificate_is_never_submitted() {
    let relay = Refusing(AtomicUsize::new(0));
    let result = SubmittedRun::submit(Run::queued(), &"gate.testnet".parse().unwrap(), &relay);
    assert_eq!(relay.0.load(Ordering::SeqCst), 0);
    assert!(result.near_tx_hash.is_none());
}
