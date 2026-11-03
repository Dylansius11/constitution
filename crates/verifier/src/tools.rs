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
        let root = tempfile::tempdir().map_err(|e| e.to_string())?;
        crate::bundle::materialize(bundle, root.path())?;
        let config = format!(
            "[profile.default]\nsrc='src'\ntest='test'\nout='out'\nlibs=['lib']\nsolc='/usr/local/bin/solc'\nevm_version='prague'\noptimizer=true\noptimizer_runs={}\nbytecode_hash='none'\ncbor_metadata=false\nffi=false\nast=true\ndynamic_test_linking=false\nbuild_info=true\nextra_output=['storageLayout']\nremappings=['@openzeppelin/contracts/=lib/openzeppelin-contracts/contracts/','forge-std/=lib/forge-std/src/']\n",
            manifest.optimizer_runs
        );
        std::fs::write(root.path().join("foundry.toml"), config).map_err(|e| e.to_string())?;
        // The container runs as the unprivileged staging owner, so disposable outputs
        // remain removable by TempDir even when a stage times out.
        Ok(Self {
            root,
            repo: repo.canonicalize().map_err(|e| e.to_string())?,
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
            let meta = std::fs::metadata(self.root.path()).map_err(|e| e.to_string())?;
            if meta.uid() == 0 {
                return Err("sandbox requires an unprivileged host user".into());
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
        Command::new("timeout")
            .args([
                "--kill-after=5s",
                "180s",
                "docker",
                "run",
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
            .map_err(|e| e.to_string())
    }
    pub fn compile(&self) -> Result<Vec<u8>, String> {
        let output = self.run(
            "solc --version && forge --version && forge build --force --offline --build-info",
            &[],
        )?;
        let mut log = output.stdout;
        log.extend(output.stderr);
        if !output.status.success() {
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
        for dir in std::fs::read_dir(self.root.path().join("out")).map_err(|e| e.to_string())? {
            let path = dir
                .map_err(|e| e.to_string())?
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
            serde_json::from_slice(&std::fs::read(&matches[0]).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
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
            StageStatus::Incomplete
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
            return Err("R2 harness commitment mismatch".into());
        }
        let test = self.root.path().join("test");
        std::fs::create_dir(&test).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(&harness).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().is_some_and(|s| s == "sol") {
                let bytes = std::fs::read(entry.path()).map_err(|e| e.to_string())?;
                std::fs::write(test.join(entry.file_name()), bytes).map_err(|e| e.to_string())?;
            }
        }
        let (file, name) = bundle
            .contract_name
            .rsplit_once(':')
            .ok_or("expected src/path.sol:Contract")?;
        std::fs::write(
            self.root.path().join("candidate.sol"),
            format!("pragma solidity 0.8.38; import {{{name} as Candidate}} from \"{file}\";\n"),
        )
        .map_err(|e| e.to_string())?;
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
        let data = std::fs::read(self.root.path().join("halmos.json")).unwrap_or_default();
        let status = std::fs::read_to_string(self.root.path().join("halmos.exit"))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(-1);
        let parsed = serde_json::from_slice(&data).unwrap_or(serde_json::Value::Null);
        let (mut result, mut reason) =
            crate::solvency::classify(status, &parsed, "Solvency", "invariant_solvency()");
        if result == StageStatus::Pass {
            let fuzz = std::fs::read(self.root.path().join("fuzz.log")).unwrap_or_default();
            let text = String::from_utf8_lossy(&fuzz);
            if !out.status.success() || !text.contains("[PASS] invariant_solvency()") {
                (result, reason) =
                    if text.contains("[FAIL") && text.contains("invariant_solvency()") {
                        (StageStatus::Fail, 21)
                    } else {
                        (StageStatus::Incomplete, 23)
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
    for e in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        if !e.file_type().map_err(|e| e.to_string())?.is_file() {
            return Err("unsafe harness entry".into());
        }
        entries.push((
            e.file_name()
                .to_str()
                .ok_or("non UTF-8 harness name")?
                .to_owned(),
            sha256(&std::fs::read(e.path()).map_err(|e| e.to_string())?),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sha256(&borsh::to_vec(&entries).map_err(|e| e.to_string())?))
}
