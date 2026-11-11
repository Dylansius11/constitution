"""Run a real disposable chain; never writes local addresses under deployments/."""
import hashlib
import json
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
from types import SimpleNamespace

from demo import ROOT, check_evm, deployment_record, rpc, save, validate


def base58(raw):
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    value, out = int.from_bytes(raw, "big"), ""
    while value:
        value, remainder = divmod(value, 58)
        out = alphabet[remainder] + out
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + out


def main():
    folder = Path(tempfile.mkdtemp(prefix="demo-anvil-", dir=ROOT / "scratch"))
    gallery = folder / "demo-gallery.json"
    shutil.copyfile(ROOT / "deployments/demo-gallery.json", gallery)
    # Bind an unused port, then let Anvil own it. Fail rather than using an existing chain.
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    process = subprocess.Popen(["anvil", "--chain-id", "84532", "--port", str(port), "--silent"],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    url = f"http://127.0.0.1:{port}"
    try:
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError("Anvil exited before readiness")
            try:
                rpc(url, "eth_chainId", [])
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError("Anvil startup timed out")
        signer = subprocess.check_output(["python3", "-B", "scripts/demo/derive.py", "--offline"],
                                         cwd=ROOT, text=True).strip()
        subprocess.run(["bash", "scripts/demo/deploy.sh", "--rpc", url, "--signer", signer,
                        "--local-unlocked", "--gallery", str(gallery), "--output", str(folder / "evm.json")],
                       cwd=ROOT, check=True)
        evm = json.loads((folder / "evm.json").read_text())
        check_evm(SimpleNamespace(rpc=url), evm)
        wasm = ROOT / "target/near/constitution_gate/constitution_gate.wasm"
        # Schema-only pre-NEAR record: local build commitment, explicitly no registered verifier.
        # This is NOT finalize's observed NEAR output, attestation, or an audit PASS.
        state = {"gate_code_hash": base58(hashlib.sha256(wasm.read_bytes()).digest()), "verifiers": [],
                 "roles": {"governor": "constitution-governor.testnet", "guardian": "constitution-guardian.testnet"}}
        record = deployment_record(evm, state)
        validate(record)
        save(folder / "demo.json", record)
        print("PASS schema: deployments/testnet.schema.json (Draft202012Validator)")
        print(f"PASS gallery runtime hashes: {len(json.loads(gallery.read_text())['attacks'])} attacks")
        print(f"Derived demo EOA (archived root; local only): {signer}")
        print(f"Evidence directory (LOCAL ONLY): {folder.relative_to(ROOT)}")
        print(f"EVM execution gas estimate: {evm['gas_used']} gas; Base L1 data fee NOT estimated")
    finally:
        process.terminate()
        process.wait(timeout=10)


if __name__ == "__main__":
    main()
