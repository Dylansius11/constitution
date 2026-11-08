use constitution_types::{bundle::SourceBundle, manifest::*, *};
use constitution_verifier::{
    identity::Identity, pipeline, proposals::LocalProposal, sha256, tools::Sandbox,
};
use std::path::{Path, PathBuf};
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn manifest() -> RuleSetManifest {
    serde_json::from_slice(&std::fs::read(repo().join("rules/manifest.json")).unwrap()).unwrap()
}
fn bundle() -> SourceBundle {
    let root = repo();
    let mut files = vec![];
    for name in ["VaultV1.sol", "VaultV2Honest.sol", "MockToken.sol"] {
        files.push((
            format!("src/demo/{name}"),
            std::fs::read(root.join(format!("contracts/src/demo/{name}"))).unwrap(),
        ));
    }
    SourceBundle {
        contract_name: "src/demo/VaultV2Honest.sol:VaultV2Honest".into(),
        files,
        foundry_profile: "default".into(),
    }
}
#[test]
fn t_rule_1_and_3_real_pinned_compiler_pass_and_fail_fixtures() {
    let m = manifest();
    let mut b = bundle();
    for name in ["LayoutBreaking", "Delegatecall", "ImplementationSlot"] {
        b.files.push((
            format!("src/{name}.sol"),
            std::fs::read(repo().join(format!("crates/verifier/tests/fixtures/{name}.sol")))
                .unwrap(),
        ));
    }
    let sandbox = Sandbox::new(&repo(), &b, &m).unwrap();
    let start = std::time::Instant::now();
    let log = sandbox.compile().unwrap();
    eprintln!(
        "compile {}ms {}",
        start.elapsed().as_millis(),
        String::from_utf8_lossy(&log)
    );
    for (name, want) in [
        (b.contract_name.as_str(), StageStatus::Pass),
        ("src/LayoutBreaking.sol:LayoutBreaking", StageStatus::Fail),
    ] {
        let (status, log) = sandbox
            .layout(name, &m.reference.contract_name, &m.r1.unsafe_allow)
            .unwrap();
        eprintln!("layout {name}: {}", String::from_utf8_lossy(&log));
        assert_eq!(status, want);
    }
    for (name, want) in [
        (b.contract_name.as_str(), Ok(())),
        ("src/Delegatecall.sol:Delegatecall", Err(30)),
        ("src/ImplementationSlot.sol:ImplementationSlot", Err(31)),
    ] {
        let runtime = sandbox.runtime(name).unwrap();
        assert_eq!(
            constitution_verifier::rules::filter(
                &runtime,
                &m.r3.forbidden_opcodes,
                &m.r3.forbidden_constants
            ),
            want
        );
    }
}
#[test]
fn t_rule_2_committed_depth_four_harness_passes_honest_and_fails_drain() {
    let m = manifest();
    let mut b = bundle();
    b.files.push((
        "src/demo/VaultV2Drain.sol".into(),
        std::fs::read(repo().join("contracts/src/demo/VaultV2Drain.sol")).unwrap(),
    ));
    let mut failures = Vec::new();
    for (name, want, reason) in [
        (
            "src/demo/VaultV2Honest.sol:VaultV2Honest",
            StageStatus::Pass,
            0,
        ),
        (
            "src/demo/VaultV2Drain.sol:VaultV2Drain",
            StageStatus::Fail,
            20,
        ),
    ] {
        b.contract_name = name.into();
        let sandbox = Sandbox::new(&repo(), &b, &m).unwrap();
        let start = std::time::Instant::now();
        let result = sandbox.solvency(&m, &b);
        let elapsed_ms = start.elapsed().as_millis();
        let label = if want == StageStatus::Pass {
            "honest"
        } else {
            "r2-fail"
        };
        eprintln!(
            "r2 {label}: {}ms {:?}",
            elapsed_ms,
            result.as_ref().map(|(s, c, _)| (s, c))
        );
        if let Ok(dir) = std::env::var("RULES_R2_EVIDENCE_DIR") {
            let dir = repo().join(dir);
            std::fs::create_dir_all(&dir).unwrap();
            let log = match &result {
                Ok((_, _, log)) => log.as_slice(),
                Err(error) => error.as_bytes(),
            };
            std::fs::write(dir.join(format!("{label}.log")), log).unwrap();
            std::fs::write(
                dir.join(format!("{label}.json")),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "elapsed_ms": elapsed_ms,
                    "depth": m.r2.invariant_depth,
                    "result": result.as_ref().map(|(status, reason, _)| (status, reason)),
                }))
                .unwrap(),
            )
            .unwrap();
        }
        match result {
            Ok((status, code, _)) if (status, code) == (want, reason) => {}
            other => failures.push(format!("{label}: {other:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
fn proposal(m: &RuleSetManifest, b: &SourceBundle) -> LocalProposal {
    LocalProposal {
        gate: "local.testnet".parse().unwrap(),
        proposal_id: 1,
        chain_id: 84532,
        proxy: Hex20([1; 20]),
        executor: Hex20([2; 20]),
        expected_current_impl: Hex20([3; 20]),
        observed_current_impl: Hex20([3; 20]),
        current_impl_codehash: Hex32([0; 32]),
        new_impl: Hex20([4; 20]),
        new_impl_codehash: Hex32([0; 32]),
        init_calldata: HexBytes(vec![]),
        source_commitment: Hex32(sha256(&borsh::to_vec(b).unwrap())),
        rule_set_hash: Hex32(rule_set_hash(m).unwrap()),
        evidence_block_number: 0,
        evidence_block_hash: Hex32([0; 32]),
        expires_at_s: pipeline::now() + 300,
    }
}
#[test]
fn precheck_tamper_never_signs_and_runtime_mismatch_signs_decodable_reject() {
    use ed25519_dalek::Verifier;
    let m = manifest();
    let b = bundle();
    let identity = Identity::new();
    let sealed = identity
        .transport
        .public_key()
        .seal(
            &mut crypto_box::aead::rand_core::OsRng,
            &borsh::to_vec(&b).unwrap(),
        )
        .unwrap();
    let mut p = proposal(&m, &b);
    p.source_commitment = Hex32([9; 32]);
    for (p, reason) in [(p, 1), (proposal(&m, &b), 2)] {
        let run = pipeline::execute(
            &repo(),
            &m,
            &identity,
            &p,
            &sealed,
            1,
            "local registry, not chain observed",
            None,
            None,
        );
        if reason == 1 {
            assert_eq!(run.stage, "failed");
            assert_eq!(run.error_code.as_deref(), Some("bad_bundle"));
            assert!(run.verdict.is_none());
            assert!(run.certificate_b64.is_none());
            assert!(run.signature_b64.is_none());
            continue;
        }
        assert_eq!(
            run.verdict,
            Some(Verdict::Reject {
                rule_id: 0,
                reason_code: reason
            })
        );
        if reason == 2 {
            assert!(
                run.stages
                    .iter()
                    .any(|s| s.name == "compiling" && s.status == StageStatus::Fail)
            );
        }
        let bytes = &run.certificate_b64.as_ref().unwrap().0;
        let mut cert = Certificate::from_canonical_bytes(bytes).unwrap();
        assert_eq!(cert.ai, AiReview::NotRun);
        let key = ed25519_dalek::VerifyingKey::from_bytes(&identity.pubkey()).unwrap();
        key.verify(
            &sha256(bytes),
            &ed25519_dalek::Signature::from_bytes(&run.signature_b64.unwrap().0),
        )
        .unwrap();
        assert_eq!(
            cert.evidence_root,
            evidence_root(&run.evidence.unwrap()).unwrap()
        );
        assert!(
            run.stages
                .iter()
                .any(|s| s.name == "r4_initializer" && s.status == StageStatus::Skipped)
        );
        cert.verdict = Verdict::Pass;
        assert!(identity.sign_reject(&cert).is_err());
    }
    let mut damaged = sealed;
    damaged[49] ^= 1;
    assert!(constitution_verifier::bundle::open(&identity.transport, &damaged).is_err());
}
#[test]
fn t_rule_2_neutral_spike_pass_fail_timeout_strict_classifier() {
    let b = bundle();
    let m = manifest();
    let sandbox = Sandbox::new(&repo(), &b, &m).unwrap();
    for (contract, want, reason) in [
        ("PassVault", StageStatus::Pass, 0),
        ("FailVault", StageStatus::Fail, 20),
        ("TimeoutVault", StageStatus::Incomplete, 23),
    ] {
        let start = std::time::Instant::now();
        std::fs::create_dir_all(sandbox.root.path().join("test")).unwrap();
        std::fs::copy(
            repo().join("spikes/verifier-min/test/Vault.t.sol"),
            sandbox.root.path().join("test/Vault.t.sol"),
        )
        .unwrap();
        let timeout = if contract == "TimeoutVault" {
            "1ms"
        } else {
            "10s"
        };
        let out=sandbox.run("halmos --contract \"$1\" --function check_solvency --solver yices --solver-threads 1 --solver-timeout-branching 0 --solver-timeout-assertion \"$2\" --invariant-depth 2 --loop 2 --array-lengths xs=2 --json-output result.json; code=$?; printf '%s' \"$code\" > exit.txt",&[contract,timeout]).unwrap();
        eprintln!(
            "{contract} {}ms {}",
            start.elapsed().as_millis(),
            String::from_utf8_lossy(&out.stderr)
        );
        let data = serde_json::from_slice(
            &std::fs::read(sandbox.root.path().join("result.json")).unwrap(),
        )
        .unwrap();
        let exit = std::fs::read_to_string(sandbox.root.path().join("exit.txt"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            constitution_verifier::solvency::classify(
                exit,
                &data,
                contract,
                "check_solvency(uint128,uint128)"
            ),
            (want, reason)
        );
    }
}
