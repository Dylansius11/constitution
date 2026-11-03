//! [I] Only staged source, trusted dependencies and output enter the no-network tool
//! container. No service environment, keys, home, or dstack socket is mounted.
use crate::sha256;
use constitution_types::bundle::SourceBundle;
use constitution_types::{
    Hex32,
    manifest::{RuleSetManifest, StageStatus},
};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Stable public codes; raw Docker/host diagnostics must not enter evidence.
pub fn infrastructure_error(error: &str) -> Option<&'static str> {
    match error.strip_prefix("infrastructure:")? {
        "sandbox_unavailable" => Some("sandbox_unavailable"),
        "infrastructure_timeout" => Some("infrastructure_timeout"),
        "storage_error" => Some("storage_error"),
        "tool_unavailable" => Some("tool_unavailable"),
        "tool_output_invalid" => Some("tool_output_invalid"),
        _ => None,
    }
}

fn storage_error(_: std::io::Error) -> String {
    "infrastructure:storage_error".into()
}

pub struct Sandbox {
    pub root: tempfile::TempDir,
    pub repo: PathBuf,
    pub image: String,
}
impl Sandbox {
    pub fn new(
        repo: &Path,
        bundle: &SourceBundle,
        manifest: &RuleSetManifest,
    ) -> Result<Self, String> {
        crate::bundle::validate_sources(bundle, manifest)?;
        let root = tempfile::tempdir().map_err(storage_error)?;
        crate::bundle::materialize(bundle, root.path())
            .map_err(|_| "infrastructure:storage_error".to_owned())?;
        let config = format!(
            "[profile.default]\nsrc='src'\ntest='test'\nout='out'\nlibs=['lib']\nsolc='/usr/local/bin/solc'\nevm_version='prague'\noptimizer=true\noptimizer_runs={}\nbytecode_hash='none'\ncbor_metadata=false\nffi=false\nast=true\ndynamic_test_linking=false\nbuild_info=true\nextra_output=['storageLayout']\nremappings=['@openzeppelin/contracts/=lib/openzeppelin-contracts/contracts/','forge-std/=lib/forge-std/src/']\n",
            manifest.optimizer_runs
        );
        std::fs::write(root.path().join("foundry.toml"), config).map_err(storage_error)?;
        // The container runs as the unprivileged staging owner, so disposable outputs
        // remain removable by TempDir even when a stage times out.
        Ok(Self {
            root,
            repo: repo.canonicalize().map_err(storage_error)?,
            image: "constitution-verifier-spike:local".into(),
        })
    }
    pub fn run(&self, script: &str, args: &[&str]) -> Result<Output, String> {
        let mount = format!("{}:/work/project", self.root.path().display());
        let libs = format!(
            "{}:/work/project/lib:ro",
            self.repo.join("contracts/lib").display()
        );
        #[cfg(unix)]
        let (user, home) = {
            use std::os::unix::fs::MetadataExt;
            let meta = std::fs::metadata(self.root.path()).map_err(storage_error)?;
            if meta.uid() == 0 {
                return Err("infrastructure:sandbox_unavailable".into());
            }
            (
                format!("{}:{}", meta.uid(), meta.gid()),
                format!(
                    "/work/home:rw,noexec,nosuid,uid={},gid={},size=16m",
                    meta.uid(),
                    meta.gid()
                ),
            )
        };
        #[cfg(not(unix))]
        let (user, home) = (
            "10001:10001".to_owned(),
            "/work/home:rw,noexec,nosuid,uid=10001,gid=10001,size=16m".to_owned(),
        );
        let output = Command::new("timeout")
            .args([
                "--kill-after=5s",
                "180s",
                "docker",
                "run",
                "--pull=never",
                "--rm",
                "--network",
                "none",
                "--read-only",
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges",
                "--user",
                &user,
                "--cpus",
                "1",
                "--memory",
                "1g",
                "--pids-limit",
                "128",
                "--ulimit",
                "fsize=67108864:67108864",
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,size=256m",
                "--tmpfs",
                &home,
                "-v",
                &mount,
                "-v",
                &libs,
                "-w",
                "/work/project",
                &self.image,
                "timeout",
                "--kill-after=3s",
                "150s",
                "sh",
                "-c",
                script,
                "runner",
            ])
            .args(args)
            .output()
            .map_err(|_| "infrastructure:sandbox_unavailable".to_owned())?;
        match output.status.code() {
            Some(124 | 137) => return Err("infrastructure:infrastructure_timeout".into()),
            Some(125) | None => return Err("infrastructure:sandbox_unavailable".into()),
            Some(126 | 127) => return Err("infrastructure:tool_unavailable".into()),
            _ => {}
        }
        Ok(output)
    }
    pub fn compile(&self) -> Result<Vec<u8>, String> {
        let output = self.run(
            "solc --version && forge --version && forge build --force --offline --build-info",
            &[],
        )?;
        let mut log = output.stdout;
        log.extend(output.stderr);
        if !output.status.success() {
            let text = String::from_utf8_lossy(&log);
            if [
                "No space left on device",
                "Read-only file system",
                "Input/output error",
            ]
            .iter()
            .any(|message| text.contains(message))
            {
                return Err("infrastructure:storage_error".into());
            }
            return Err(String::from_utf8_lossy(&log).into_owned());
        }
        Ok(log)
    }
    pub fn runtime(&self, contract: &str) -> Result<Vec<u8>, String> {
        let name = contract.rsplit(':').next().ok_or("bad contract")?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err("bad contract name".into());
        }
        let mut matches = Vec::new();
        for dir in std::fs::read_dir(self.root.path().join("out")).map_err(storage_error)? {
            let path = dir
                .map_err(storage_error)?
                .path()
                .join(format!("{name}.json"));
            if path.is_file() {
                matches.push(path);
            }
        }
        if matches.len() != 1 {
            return Err("runtime artifact ambiguous or missing".into());
        }
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&matches[0]).map_err(storage_error)?)
                .map_err(|_| "infrastructure:tool_output_invalid".to_owned())?;
        let text = v
            .pointer("/deployedBytecode/object")
            .and_then(|v| v.as_str())
            .ok_or("runtime missing")?;
        let text = if text.starts_with("0x") {
            text.to_owned()
        } else {
            format!("0x{text}")
        };
        let bytes: constitution_types::HexBytes =
            serde_json::from_value(serde_json::Value::String(text)).map_err(|e| e.to_string())?;
        if bytes.0.is_empty() {
            return Err("empty runtime".into());
        }
        Ok(bytes.0)
    }
    pub fn layout(
        &self,
        candidate: &str,
        reference: &str,
        unsafe_allow: &[String],
    ) -> Result<(StageStatus, Vec<u8>), String> {
        let allows = unsafe_allow.join(",");
        let out=self.run("openzeppelin-upgrades-core validate out/build-info --contract \"$1\" --reference \"$2\" --requireReference --unsafeAllow \"$3\"",&[candidate,reference,&allows])?;
        let success = out.status.success();
        let failed = out.status.code() == Some(1);
        let mut log = out.stdout;
        log.extend(out.stderr);
        let text = String::from_utf8_lossy(&log);
        let status = if success && text.contains("SUCCESS") {
            StageStatus::Pass
        } else if failed
            && text.lines().any(|line| line == "FAILED")
            && text.contains("(upgrades from ")
        {
            StageStatus::Fail
        } else {
            return Err("infrastructure:tool_output_invalid".into());
        };
        Ok((status, log))
    }
    pub fn solvency(
        &self,
        manifest: &RuleSetManifest,
        bundle: &SourceBundle,
    ) -> Result<(StageStatus, u16, Vec<u8>), String> {
        let harness = self.repo.join("rules/r2");
        let digest = harness_digest(&harness)?;
        if Some(Hex32(digest)) != manifest.r2.harness_sha256 {
            return Err("infrastructure:tool_output_invalid".into());
        }
        // The expected hash is derived from the verifier's first compiler artifact,
        // never from candidate.sol or another proposer-controlled configuration.
        if !self.root.path().join("out").exists() {
            self.compile()?;
        }
        let expected = hex::encode(crate::keccak(&self.runtime(&bundle.contract_name)?));
        let test = self.root.path().join("test");
        std::fs::create_dir(&test).map_err(storage_error)?;
        for entry in std::fs::read_dir(&harness).map_err(storage_error)? {
            let entry = entry.map_err(storage_error)?;
            if entry.path().extension().is_some_and(|s| s == "sol") {
                let bytes = std::fs::read(entry.path()).map_err(storage_error)?;
                std::fs::write(test.join(entry.file_name()), bytes).map_err(storage_error)?;
            }
        }
        let (file, name) = bundle
            .contract_name
            .rsplit_once(':')
            .ok_or("expected src/path.sol:Contract")?;
        std::fs::write(
            self.root.path().join("candidate.sol"),
            format!("pragma solidity 0.8.38; import {{{name} as Candidate}} from \"{file}\";\nlibrary CandidateConfig {{ bytes32 internal constant EXPECTED_RUNTIME_HASH = 0x{expected}; }}\n"),
        )
        .map_err(storage_error)?;
        let r = &manifest.r2;
        let depth = r.invariant_depth.ok_or("missing depth")?.to_string();
        let loops = r.loop_bound.ok_or("missing loop bound")?.to_string();
        let seconds = format!(
            "{}s",
            r.solver_timeout_assertion_s.ok_or("missing timeout")?
        );
        let fuzz = r.fuzz_runs.ok_or("missing fuzz runs")?.to_string();
        let fuzz_depth = r.fuzz_depth.ok_or("missing fuzz depth")?.to_string();
        let seed = serde_json::to_value(r.fuzz_seed.ok_or("missing seed")?)
            .map_err(|e| e.to_string())?
            .as_str()
            .ok_or("seed")?
            .to_owned();
        let arrays = r.array_lengths.as_deref().ok_or("missing array lengths")?;
        // [V] Halmos 0.3.3 source (config.py, sevm.py ~L2838): 1ms is the branching default and an
        // undecided branch stays feasible, so it adds paths, never hides one. An unbounded branch
        // timeout (0) made depth 4 miss 300 s (crates/verifier/evidence/depth/README.md).
        // --early-exit stops at the first counterexample.
        let out=self.run("halmos --contract Solvency --function invariant_solvency --solver yices --solver-threads 1 --invariant-depth \"$1\" --loop \"$2\" --array-lengths \"$3\" --solver-timeout-branching 1ms --solver-timeout-assertion \"$4\" --early-exit --no-status --json-output halmos.json; status=$?; printf '%s' \"$status\" > halmos.exit; [ \"$status\" = 0 ] && FOUNDRY_INVARIANT_RUNS=\"$5\" FOUNDRY_INVARIANT_DEPTH=\"$7\" forge test --offline --match-contract Solvency --fuzz-seed \"$6\" > fuzz.log 2>&1",&[&depth,&loops,arrays,&seconds,&fuzz,&seed,&fuzz_depth])?;
        let mut log = out.stdout;
        log.extend(out.stderr);
        let data = std::fs::read(self.root.path().join("halmos.json"))
            .map_err(|_| "infrastructure:tool_output_invalid".to_owned())?;
        let status = std::fs::read_to_string(self.root.path().join("halmos.exit"))
            .map_err(storage_error)?
            .parse()
            .map_err(|_| "infrastructure:tool_output_invalid".to_owned())?;
        let parsed: serde_json::Value = serde_json::from_slice(&data)
            .map_err(|_| "infrastructure:tool_output_invalid".to_owned())?;
        let (mut result, mut reason) = crate::solvency::classify_tool_output(
            status,
            &parsed,
            "Solvency",
            "invariant_solvency()",
        )?;
        if result == StageStatus::Pass {
            let fuzz = std::fs::read(self.root.path().join("fuzz.log")).map_err(storage_error)?;
            let text = String::from_utf8_lossy(&fuzz);
            if !out.status.success() || !text.contains("[PASS] invariant_solvency()") {
                (result, reason) =
                    if text.contains("[FAIL") && text.contains("invariant_solvency()") {
                        (StageStatus::Fail, 21)
                    } else {
                        return Err("infrastructure:tool_output_invalid".into());
                    };
            }
            log.extend(fuzz);
        }
        log.extend(data);
        Ok((result, reason, log))
    }
}
/// [I] Hash sorted relative paths and content digests using canonical Borsh.
pub fn harness_digest(root: &Path) -> Result<[u8; 32], String> {
    let mut entries = Vec::new();
    for e in std::fs::read_dir(root).map_err(storage_error)? {
        let e = e.map_err(storage_error)?;
        if !e.file_type().map_err(storage_error)?.is_file() {
            return Err("infrastructure:tool_output_invalid".into());
        }
        entries.push((
            e.file_name()
                .to_str()
                .ok_or("infrastructure:tool_output_invalid")?
                .to_owned(),
            sha256(&std::fs::read(e.path()).map_err(storage_error)?),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sha256(&borsh::to_vec(&entries).map_err(|e| e.to_string())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(source: &[u8], contract: &str) -> (RuleSetManifest, SourceBundle) {
        let manifest = serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        let bundle = SourceBundle {
            contract_name: contract.into(),
            foundry_profile: "default".into(),
            files: vec![("src/A.sol".into(), source.to_vec())],
        };
        (manifest, bundle)
    }

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn tool_exit_codes_are_infrastructure_not_rule_failures() {
        let (manifest, bundle) = fixture(b"pragma solidity 0.8.38; contract A {}", "src/A.sol:A");
        let sandbox = Sandbox::new(&repo(), &bundle, &manifest).unwrap();
        for (status, expected) in [
            ("124", "infrastructure_timeout"),
            ("125", "sandbox_unavailable"),
            ("126", "tool_unavailable"),
            ("127", "tool_unavailable"),
            ("137", "infrastructure_timeout"),
        ] {
            let error = sandbox.run("exit \"$1\"", &[status]).unwrap_err();
            assert_eq!(infrastructure_error(&error), Some(expected));
        }
        assert_eq!(sandbox.run("exit 1", &[]).unwrap().status.code(), Some(1));
        assert_eq!(infrastructure_error("raw compiler diagnostic"), None);
        assert_eq!(infrastructure_error("infrastructure:unknown"), None);
    }

    #[test]
    fn source_compile_failure_is_not_an_infrastructure_fault() {
        let (manifest, bundle) = fixture(b"pragma solidity 0.8.38; contract A {", "src/A.sol:A");
        let sandbox = Sandbox::new(&repo(), &bundle, &manifest).unwrap();
        let error = sandbox.compile().unwrap_err();
        assert_eq!(infrastructure_error(&error), None);
    }

    #[test]
    fn r2_refuses_rebuild_that_differs_from_the_first_runtime_artifact() {
        let (manifest, bundle) = fixture(
            include_bytes!("../../../contracts/src/demo/VaultV2Honest.sol"),
            "src/A.sol:VaultV2Honest",
        );
        let sandbox = Sandbox::new(&repo(), &bundle, &manifest).unwrap();
        sandbox.compile().unwrap();
        let artifact = sandbox.root.path().join("out/A.sol/VaultV2Honest.json");
        let mut compiled: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&artifact).unwrap()).unwrap();
        // Inject a different first-compilation artifact; the actual harness rebuild
        // still deploys the real honest code. Without the code-hash guard this passes.
        compiled["deployedBytecode"]["object"] = "0x00".into();
        std::fs::write(&artifact, serde_json::to_vec(&compiled).unwrap()).unwrap();
        let error = sandbox.solvency(&manifest, &bundle).unwrap_err();
        assert_eq!(infrastructure_error(&error), Some("tool_output_invalid"));
        assert!(!sandbox.root.path().join("fuzz.log").exists());
    }
}
