//! Canonical local source collection, credit reservation and encrypted verifier submission.
use crate::{
    failure::Failure,
    near_tx::Writer,
    rpc::{Evm, Near, Verifier},
};
use constitution_types::{
    Config, Constitution, Hex20, Hex32, HexBytes, ProposeArgs, U128Str,
    bundle::{MAX_BUNDLE, SourceBundle},
};
use crypto_box::{PublicKey, aead::rand_core::OsRng};
use serde_json::json;
use sha3::{Digest, Keccak256};
use std::{fs, path::Path};

fn bundle_error(cause: impl std::fmt::Display) -> Failure {
    Failure::new(
        format!("source bundle: {cause}"),
        "use a directory containing only regular .sol files under src/ and --contract src/path.sol:Name",
    )
}

pub fn build_bundle(dir: &Path, contract: &str) -> Result<SourceBundle, Failure> {
    fn collect(
        root: &Path,
        dir: &Path,
        files: &mut Vec<(String, Vec<u8>)>,
        size: &mut usize,
    ) -> Result<(), Failure> {
        let meta = fs::symlink_metadata(dir).map_err(bundle_error)?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(bundle_error(format!(
                "{} is not a regular source directory",
                dir.display()
            )));
        }
        for entry in fs::read_dir(dir).map_err(bundle_error)? {
            let path = entry.map_err(bundle_error)?.path();
            let meta = fs::symlink_metadata(&path).map_err(bundle_error)?;
            if meta.file_type().is_symlink() {
                return Err(bundle_error(format!("symlink refused: {}", path.display())));
            }
            if meta.is_dir() {
                collect(root, &path, files, size)?;
            } else {
                if !meta.is_file() || path.extension().is_none_or(|ext| ext != "sol") {
                    return Err(bundle_error(format!(
                        "non-.sol or non-regular file: {}",
                        path.display()
                    )));
                }
                if meta.len() > MAX_BUNDLE as u64 || files.len() >= 4096 {
                    return Err(bundle_error("bundle limit exceeded"));
                }
                let content = fs::read(&path).map_err(bundle_error)?;
                *size = size
                    .checked_add(content.len())
                    .ok_or_else(|| bundle_error("bundle limit exceeded"))?;
                if *size > MAX_BUNDLE {
                    return Err(bundle_error("bundle exceeds 2 MiB"));
                }
                let relative = path.strip_prefix(root).map_err(bundle_error)?;
                let name = relative
                    .to_str()
                    .ok_or_else(|| bundle_error("source path is not UTF-8"))?
                    .replace('\\', "/");
                files.push((name, content));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(dir, &dir.join("src"), &mut files, &mut 0)?;
    files.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let (path, name) = contract
        .rsplit_once(':')
        .ok_or_else(|| bundle_error("contract must be src/path.sol:Name"))?;
    if name.is_empty()
        || !name
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit()))
        || !files.iter().any(|(p, _)| p == path)
    {
        return Err(bundle_error(
            "selected contract source or contract name is invalid",
        ));
    }
    let bundle = SourceBundle {
        contract_name: contract.into(),
        files,
        foundry_profile: "default".into(),
    };
    bundle.validate().map_err(bundle_error)?;
    Ok(bundle)
}

pub fn canonical_bytes(bundle: &SourceBundle) -> Result<Vec<u8>, Failure> {
    bundle.validate().map_err(bundle_error)?;
    let bytes = borsh::to_vec(bundle).map_err(bundle_error)?;
    if bytes.len() > MAX_BUNDLE {
        return Err(bundle_error("Borsh bundle exceeds 2 MiB"));
    }
    Ok(bytes)
}

pub fn seal(bytes: &[u8], public_key: [u8; 32]) -> Result<Vec<u8>, Failure> {
    PublicKey::from(public_key)
        .seal(&mut OsRng, bytes)
        .map_err(|_| {
            Failure::new(
                "could not seal source bundle",
                "fetch the verifier identity again and check its transport key",
            )
        })
}

/// Retry verification of an existing proposal without another on-chain write.
/// File input is canonical Borsh SourceBundle; directories use the propose collector.
pub fn verifier_run(
    path: &Path,
    contract: Option<&str>,
    gate: &str,
    id: u64,
    near: &Near,
    verifier: &Verifier,
) -> Result<(), Failure> {
    let bytes = if path.is_dir() {
        let contract = contract
            .ok_or_else(|| bundle_error("directory input requires --contract src/path.sol:Name"))?;
        canonical_bytes(&build_bundle(path, contract)?)?
    } else {
        if fs::metadata(path).map_err(bundle_error)?.len() > MAX_BUNDLE as u64 {
            return Err(bundle_error("Borsh bundle exceeds 2 MiB"));
        }
        let raw = fs::read(path).map_err(bundle_error)?;
        let bundle: SourceBundle = borsh::from_slice(&raw).map_err(bundle_error)?;
        canonical_bytes(&bundle)?
    };
    let proposal = crate::receipt::read_proposal(near, gate, id)?;
    if proposal.state != constitution_types::ProposalState::Pending {
        return Err(Failure::new(
            format!("proposal {id} is not Pending"),
            "inspect `constitution status <id>`; only Pending proposals can start another verifier run",
        ));
    }
    if constitution_types::certificate_hash(&bytes) != proposal.source_commitment {
        return Err(Failure::new(
            format!(
                "bundle differs from proposal {id} source commitment; no verifier request was sent"
            ),
            "use exactly the original canonical source bundle and contract selection; do not change the existing proposal",
        ));
    }
    let identity = verifier.identity()?;
    let key: Hex32 =
        serde_json::from_value(identity["transport_pubkey"].clone()).map_err(|_| {
            Failure::new(
                "verifier identity has no valid transport_pubkey",
                "check --verifier-url/VERIFIER_URL and fetch the current identity before retrying",
            )
        })?;
    println!(
        "verifier identity NOT APPRAISED by this command; use constitution verifier appraise (D5)"
    );
    let run_id = verifier.start(gate, id, &seal(&bytes, key.0)?)?;
    println!("verifier run: {run_id}");
    println!(
        "next: constitution status {id} --run {run_id} --watch 600 (use the same --gate, --near-rpc and --verifier)"
    );
    Ok(())
}

pub struct Input<'a> {
    pub bytes: &'a [u8],
    pub implementation: &'a str,
    pub init: &'a str,
    pub account: &'a str,
}

pub fn propose(
    input: Input<'_>,
    gate: &str,
    writer: &Writer,
    near: &Near,
    evm: &Evm,
    verifier: &Verifier,
) -> Result<(), Failure> {
    let bytes = input.bytes;
    let address: Hex20 = serde_json::from_value(json!(input.implementation)).map_err(|e| {
        Failure::new(
            format!("--impl: {e}"),
            "pass a lowercase 0x-prefixed 20-byte address",
        )
    })?;
    let init: HexBytes = serde_json::from_value(json!(input.init)).map_err(|e| {
        Failure::new(
            format!("--init: {e}"),
            "pass lowercase 0x-prefixed calldata",
        )
    })?;
    if address.0 == [0; 20] || init.0.len() > constitution_types::MAX_INIT_CALLDATA {
        return Err(Failure::new(
            "zero implementation or init calldata exceeds 1024 bytes",
            "choose a deployed implementation and valid initializer calldata",
        ));
    }
    let read_error = |e| Failure::new(e, "check --gate, --near-rpc and --evm-rpc and retry");
    let config: Config = near
        .view(gate, "get_config", json!({}))
        .map_err(read_error)?;
    let constitution: Constitution = near
        .view(gate, "get_constitution", json!({}))
        .map_err(read_error)?;
    let chain = evm.chain_id().map_err(read_error)?;
    if chain != config.chain_id {
        return Err(Failure::new(
            format!(
                "EVM RPC chain {chain} differs from gate chain {}",
                config.chain_id
            ),
            "point --evm-rpc at the gate's configured chain",
        ));
    }
    let code = evm.code(address.0).map_err(read_error)?;
    if code.is_empty() {
        return Err(Failure::new(
            "implementation has no runtime bytecode",
            "deploy the implementation on the configured local or testnet chain first",
        ));
    }
    let current = evm
        .address_getter(config.executor, "currentImplementation()")
        .map_err(read_error)?;
    let args = ProposeArgs {
        new_impl: address,
        new_impl_codehash: Hex32(Keccak256::digest(&code).into()),
        expected_current_impl: Hex20(current),
        init_calldata: init,
        source_commitment: Hex32(constitution_types::certificate_hash(bytes)),
    };
    let identity = verifier.identity()?;
    let key: Hex32 = serde_json::from_value(
        identity
            .get("transport_pubkey")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|e| {
        Failure::new(
            format!("verifier transport_pubkey: {e}"),
            "check --verifier / VERIFIER_URL; nothing was written on chain",
        )
    })?;
    let credit: U128Str = near
        .view(gate, "get_credit", json!({"account": input.account}))
        .map_err(read_error)?;
    let proposal_fee: U128Str = near
        .view(gate, "get_proposal_fee", json!({}))
        .map_err(read_error)?;
    let mut shortfall = constitution
        .params
        .proposal_deposit
        .saturating_sub(credit.0);
    if credit.0 == 0 {
        // A zero balance may be a missing entry. The gate takes a one-time entry fee out of the
        // first deposit to a missing entry; if the entry exists, the same amount is only extra
        // refundable credit, so paying it is safe either way.
        let entry_fee: U128Str = near
            .view(
                gate,
                "get_credit_entry_fee",
                json!({"account": input.account}),
            )
            .map_err(read_error)?;
        shortfall = shortfall.saturating_add(entry_fee.0);
    }
    if shortfall > 0 {
        println!("depositing credit: {shortfall} yoctoNEAR");
        writer.call(
            gate,
            "deposit_credit",
            json!({"account": input.account}),
            shortfall,
            "fund the signing account or top up credit with a full-access key, then retry",
        )?;
    }
    println!(
        "paying the non-refundable proposal storage fee: {} yoctoNEAR",
        proposal_fee.0
    );
    let called = writer.call(gate, "propose", json!({"args": args}), proposal_fee.0, "wait for your active proposal to finish (or expire it after its deadline), then propose again; inspect with constitution status <id>")?;
    let id = called.value.as_u64().ok_or_else(|| {
        Failure::new(
            "propose landed but returned no proposal id",
            "inspect the NEAR transaction; do not submit a duplicate proposal",
        )
    })?;
    println!("proposal {id}");
    println!("NEAR propose tx: {}", called.tx_hash);
    println!(
        "verifier identity NOT APPRAISED by this command; use constitution verifier appraise (D5)"
    );
    let sealed = seal(bytes, key.0)?;
    let run_id = verifier.start(gate, id, &sealed)?;
    println!("verifier run: {run_id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_box::SecretKey;

    #[test]
    fn bundle_is_sorted_src_only_and_commitment_stable() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::write(dir.path().join("src/Z.sol"), b"contract Z {}").unwrap();
        fs::write(dir.path().join("src/nested/A.sol"), b"contract A {}").unwrap();
        fs::write(dir.path().join("outside.sol"), b"excluded").unwrap();
        let bundle = build_bundle(dir.path(), "src/Z.sol:Z").unwrap();
        assert_eq!(
            bundle
                .files
                .iter()
                .map(|(p, _)| p.as_str())
                .collect::<Vec<_>>(),
            ["src/Z.sol", "src/nested/A.sol"]
        );
        let bytes = canonical_bytes(&bundle).unwrap();
        assert_eq!(borsh::from_slice::<SourceBundle>(&bytes).unwrap(), bundle);
        // Independently encoded with Python struct.pack('<I', len) and hashlib.sha256.
        assert_eq!(
            hex::encode(constitution_types::certificate_hash(&bytes)),
            "aeb14b717101cde46ef0ce9e452f0497afd63b9d06da7911e34b9a663ef0fcfa"
        );
        fs::write(dir.path().join("src/not-source.txt"), b"refuse").unwrap();
        assert!(build_bundle(dir.path(), "src/Z.sol:Z").is_err());
    }

    #[test]
    fn sealing_round_trip_preserves_exact_canonical_bytes() {
        let bundle = SourceBundle {
            contract_name: "src/A.sol:A".into(),
            files: vec![("src/A.sol".into(), b"contract A {}".to_vec())],
            foundry_profile: "default".into(),
        };
        let bytes = canonical_bytes(&bundle).unwrap();
        let key = SecretKey::generate(&mut OsRng);
        let sealed = seal(&bytes, *key.public_key().as_bytes()).unwrap();
        assert_eq!(key.unseal(&sealed).unwrap(), bytes);
        let other = SecretKey::generate(&mut OsRng);
        assert!(other.unseal(&sealed).is_err());
    }

    #[test]
    fn rejects_missing_contract_and_encoded_size_over_limit() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/A.sol"), b"contract A {}").unwrap();
        assert!(build_bundle(dir.path(), "src/missing.sol:A").is_err());
        assert!(build_bundle(dir.path(), "src/A.sol:").is_err());
        let mut bundle = build_bundle(dir.path(), "src/A.sol:A").unwrap();
        bundle.files[0].1.resize(MAX_BUNDLE, 0);
        assert!(
            canonical_bytes(&bundle).is_err(),
            "Borsh framing counts toward the 2 MiB limit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_escape_and_symlink_source_directory() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(outside.path().join("A.sol"), b"private").unwrap();
        symlink(outside.path().join("A.sol"), dir.path().join("src/A.sol")).unwrap();
        assert!(build_bundle(dir.path(), "src/A.sol:A").is_err());
        fs::remove_file(dir.path().join("src/A.sol")).unwrap();
        fs::remove_dir(dir.path().join("src")).unwrap();
        symlink(outside.path(), dir.path().join("src")).unwrap();
        assert!(build_bundle(dir.path(), "src/A.sol:A").is_err());
    }
}
