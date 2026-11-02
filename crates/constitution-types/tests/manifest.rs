use constitution_types::manifest::{RuleSetManifest, rule_set_hash};
use std::{
    io::Write,
    process::{Command, Stdio},
};
// [I] Independent Python Borsh writer, not generated from Rust serialization.
const PYTHON: &str = r#"
import sys,json,hashlib,struct
m=json.load(sys.stdin)
u32=lambda n:struct.pack('<I',n)
u64=lambda n:struct.pack('<Q',n)
s=lambda v:u32(len(v.encode()))+v.encode()
h=lambda v:bytes.fromhex(v[2:])
v=lambda xs,f:u32(len(xs))+b''.join(f(x) for x in xs)
o=lambda x,f:b'\0' if x is None else b'\1'+f(x)
b=u32(m['version'])+s(m['name'])+s(m['solc'])+s(m['evm_version'])+u32(m['optimizer_runs'])
b+=b''.join(s(m['tools'][k]) for k in ['foundry','halmos','upgrades_core','z3','yices'])
b+=s(m['reference']['contract_name'])+v(sorted(m['reference']['source_sha256s'].items()),lambda x:s(x[0])+h(x[1]))
b+=v(m['r1']['unsafe_allow'],s)
r=m['r2']
b+=o(r['harness_sha256'],h)+o(r['invariant_depth'],u32)+o(r['loop_bound'],u32)+o(r['array_lengths'],s)+o(r['solver_timeout_assertion_s'],u32)+o(r['fuzz_runs'],u32)+o(r['fuzz_seed'],h)
b+=v(m['r3']['forbidden_opcodes'],lambda x:bytes([x]))+v(m['r3']['forbidden_constants'],h)
b+=v(m['r4']['allowlist'],lambda x:v(list(h(x['selector'])),lambda y:bytes([y]))+v(x['args'],lambda a:s(a['type'])+s(a['max'])))+bytes([m['r4']['allow_empty']])
r=m['r5'];b+=h(r['corpus_sha256'])+u32(r['max_txs'])+u32(r['confirmations'])
a=m['ai'];b+=bytes([a['required']])+s(a['model'])+s(a['signature_kind'])+u32(a['max_candidates'])+u32(a['timeout_s'])+u64(m['max_cert_ttl_s'])
print(hashlib.sha256(b).hexdigest())
"#;
#[test]
fn golden_rule_set_hash_matches_independent_python_borsh() {
    let m: RuleSetManifest =
        serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
    let mut child = Command::new("python3")
        .args(["-c", PYTHON])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&m).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let hash = rule_set_hash(&m).unwrap();
    let hex = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(hex, String::from_utf8(output.stdout).unwrap().trim());
    let bytes = borsh::to_vec(&m).unwrap();
    assert_eq!(borsh::from_slice::<RuleSetManifest>(&bytes).unwrap(), m);
    let mut changed = m;
    changed.optimizer_runs += 1;
    assert_ne!(rule_set_hash(&changed).unwrap(), hash);
}
