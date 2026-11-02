#!/usr/bin/env python3
"""P1.4 evidence driver. Local fork uses real NEAR MPC signatures, not live Base writes."""
import argparse
import base64
import datetime
import json
import os
from pathlib import Path
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
SPIKE = ROOT / "spikes/gate-min"
TARGET = SPIKE / "target/driver"
NEAR = ROOT / ".tools/bin/near"
NEAR_RPC = "https://test.rpc.fastnear.com"
SOURCE_RPC = "https://sepolia.base.org"
SPIKE_ACCOUNT = "constitution-spike.testnet"
SIGNER = "0x8221c9477886cc456d7e1cccc0866e0401b81070"
ANVIL_DEPLOYER = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
IMPLEMENTATION_SLOT = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"


def rpc(url, method, params, *, allow_error=False):
    request = {"jsonrpc": "2.0", "id": "constitution-spike", "method": method, "params": params}
    try:
        with urllib.request.urlopen(urllib.request.Request(url, json.dumps(request).encode(),
                {"Content-Type": "application/json", "User-Agent": "constitution-spike/0.1"}), timeout=120) as response:
            body = json.load(response)
    except urllib.error.HTTPError as error:
        body = json.load(error)
        if not isinstance(body, dict) or "error" not in body:
            raise
    if "error" in body and not allow_error:
        raise RuntimeError(f"{method}: {body['error']}")
    return body


def call(url, method, params):
    return rpc(url, method, params)["result"]


def run(args, *, cwd=ROOT, env=None):
    result = subprocess.run([str(x) for x in args], cwd=cwd, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        # Never echo argv: the live branch refers to keystore/password file paths only.
        raise RuntimeError(f"{Path(args[0]).name} failed ({result.returncode}):\n{result.stdout}\n{result.stderr}")
    return result.stdout + result.stderr


def cast(*args):
    return run(["cast", *args]).strip()


def keccak(data):
    return cast("keccak", data)


def address_word(address):
    return bytes(12) + bytes.fromhex(address.removeprefix("0x"))


def word(value):
    return value.to_bytes(32, "big")


def rlp(value):
    if isinstance(value, list):
        payload = b"".join(rlp(item) for item in value)
        base = 192
    else:
        if isinstance(value, int):
            value = value.to_bytes((value.bit_length() + 7) // 8, "big")
        payload = value
        base = 128
        if len(payload) == 1 and payload[0] < 128:
            return payload
    if len(payload) < 56:
        return bytes([base + len(payload)]) + payload
    length = len(payload).to_bytes((len(payload).bit_length() + 7) // 8, "big")
    return bytes([base + 55 + len(length)]) + length + payload


def independent_preimage(item):
    data = bytes.fromhex("90c70cd8") + bytes(item["certificate_id"])
    data += bytes(12) + bytes(item["new_impl"]) + bytes(item["new_impl_codehash"])
    data += bytes(12) + bytes(item["expected_current_impl"]) + word(item["deadline_s"]) + word(192)
    init = bytes(item["init_calldata"])
    data += word(len(init)) + init + bytes((-len(init)) % 32)
    return b"\x02" + rlp([84532, item["nonce"], int(item["max_priority_fee_per_gas"]),
        int(item["max_fee_per_gas"]), item["gas_limit"], bytes(item["executor"]), 0, data, []])


def near_sign(item, label):
    args_file = TARGET / f"{label}-args.json"
    args_file.write_text(json.dumps({"input": item}) + "\n")
    output = run([NEAR, "contract", "call-function", "as-transaction", SPIKE_ACCOUNT,
        "sign_execute", "file-args", args_file, "prepaid-gas", "100 Tgas", "attached-deposit",
        "0 NEAR", "sign-as", SPIKE_ACCOUNT, "network-config", "testnet", "sign-with-legacy-keychain", "send"])
    match = re.search(r"Transaction ID:\s*([1-9A-HJ-NP-Za-km-z]+)", output)
    if not match:
        raise RuntimeError("NEAR CLI returned no transaction id: " + output)
    tx_hash = match.group(1)
    query = {"tx_hash": tx_hash, "sender_account_id": SPIKE_ACCOUNT, "wait_until": "FINAL"}
    response = rpc(NEAR_RPC, "tx", query)
    status = response["result"]["status"]
    if "SuccessValue" not in status:
        raise RuntimeError(f"NEAR transaction did not succeed: {status}")
    signed = json.loads(base64.b64decode(status["SuccessValue"]))
    if signed["status"] != "signed":
        raise RuntimeError(f"MPC callback failed: {signed}")
    expected = independent_preimage(item)
    assert signed["preimage"] == "0x" + expected.hex(), "in-contract preimage differs from independent oracle"
    assert signed["signing_hash"] == keccak("0x" + expected.hex())
    assert signed["tx_hash"] == keccak(signed["raw_tx"])
    mpc = signed["mpc_response"]
    signature = "0x" + mpc["big_r"]["affine_point"][2:] + mpc["s"]["scalar"] + f"{27 + mpc['recovery_id']:02x}"
    verification = cast("wallet", "verify", "--address", SIGNER, "--no-hash", signed["signing_hash"], signature)
    assert "Validation succeeded." in verification, verification
    return {"input": item, "near_tx_hash": tx_hash, "near_tx_query": query,
        "near_tx_response": response, "signed": signed, "sender_verification": verification}


def wait_receipt(url, tx_hash):
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        receipt = call(url, "eth_getTransactionReceipt", [tx_hash])
        if receipt is not None:
            return receipt
        time.sleep(1)
    raise RuntimeError(f"No receipt after 120 seconds: {tx_hash}")


def deployment(url, mode):
    env = dict(os.environ, SIGNER=SIGNER, FOUNDRY_OUT=str(TARGET / "foundry-out"),
        FOUNDRY_CACHE_PATH=str(TARGET / "foundry-cache"), FOUNDRY_BROADCAST=str(TARGET / "broadcast"))
    if mode == "fork":
        wallet = ["--unlocked", "--sender", ANVIL_DEPLOYER]
    else:
        wallet = ["--keystore", str(ROOT / ".secrets/evm/deployer"),
            "--password-file", str(ROOT / ".secrets/evm/deployer.password")]
    output = run(["forge", "script", "script/Deploy.s.sol:Deploy", "--rpc-url", url,
        "--broadcast", "--slow", "--non-interactive", *wallet], cwd=ROOT / "contracts", env=env)
    addresses = {}
    for name in ["executor", "proxy", "proxyAdmin", "vaultV1Implementation", "token"]:
        match = re.search(rf"\b{name}\s+(0x[0-9a-fA-F]{{40}})", output)
        if not match:
            raise RuntimeError(f"Deployment output lacks {name}: {output}")
        addresses[name] = match.group(1).lower()
    create_wallet = ["--unlocked", "--from", ANVIL_DEPLOYER] if mode == "fork" else wallet
    created = run(["forge", "create", "src/demo/VaultV2Honest.sol:VaultV2Honest", "--rpc-url", url,
        "--broadcast", *create_wallet], cwd=ROOT / "contracts", env=env)
    match = re.search(r"Deployed to:\s*(0x[0-9a-fA-F]{40})", created)
    tx = re.search(r"Transaction hash:\s*(0x[0-9a-fA-F]{64})", created)
    if not match or not tx:
        raise RuntimeError("VaultV2Honest deployment output is incomplete: " + created)
    addresses["vaultV2Honest"] = match.group(1).lower()
    admin_owner = cast("call", addresses["proxyAdmin"], "owner()(address)", "--rpc-url", url).lower()
    authorized = cast("call", addresses["executor"], "signer()(address)", "--rpc-url", url).lower()
    assert admin_owner == addresses["executor"]
    assert authorized == SIGNER
    return {"addresses": addresses, "bootstrap_output": output, "v2_output": created,
        "v2_deploy_tx": tx.group(1), "proxy_admin_owner": admin_owner, "authorized_signer": authorized}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["fork", "testnet"], required=True)
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--relay-url", help="defaults to the fork RPC, or the Base Sepolia sequencer in testnet mode")
    parser.add_argument("--output", type=Path, default=ROOT / "vectors/evm-tx-spike.json")
    args = parser.parse_args()
    relay_url = args.relay_url or ("https://sepolia-sequencer.base.org" if args.mode == "testnet" else args.rpc_url)
    if args.mode == "fork":
        assert urllib.parse.urlparse(args.rpc_url).hostname in ("localhost", "127.0.0.1"), "fork RPC must be local"
        assert relay_url == args.rpc_url, "fork relay must use the same local fork"
    TARGET.mkdir(parents=True, exist_ok=True)
    assert int(call(args.rpc_url, "eth_chainId", []), 16) == 84532, "Base Sepolia chain id required"
    label = "local fork of Base Sepolia, real MPC signature" if args.mode == "fork" else "Base Sepolia testnet, real MPC signature"
    evidence = {"label": label, "observed_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "rpc_url": args.rpc_url, "relay_url": relay_url, "near_account": SPIKE_ACCOUNT, "signer": SIGNER,
        "source_fee_observation": {"block": call(SOURCE_RPC, "eth_getBlockByNumber", ["latest", False]),
            "max_priority_fee_per_gas": call(SOURCE_RPC, "eth_maxPriorityFeePerGas", [])}}
    if args.mode == "fork":
        evidence["anvil_node_info"] = call(args.rpc_url, "anvil_nodeInfo", [])
        call(args.rpc_url, "anvil_setBalance", [SIGNER, hex(10**18)])
    else:
        # Only run this branch after the lead confirms funding and authorizes the live stage.
        assert args.rpc_url.startswith("https://"), "live RPC must use HTTPS"
        assert relay_url.startswith("https://"), "live relay must use HTTPS"
        assert int(call(args.rpc_url, "eth_getBalance", [SIGNER, "latest"]), 16) > 0, "derived EOA is unfunded"
    evidence["deployment"] = deployment(args.rpc_url, args.mode)
    addresses = evidence["deployment"]["addresses"]
    init_calldata = cast("calldata", "initializeV2(uint256)", str(1000 * 10**18))
    codehash = keccak(call(args.rpc_url, "eth_getCode", [addresses["vaultV2Honest"], "latest"]))
    evidence["cases"] = []
    for name, cert_byte, expected in [("honest_upgrade", 0x22, addresses["vaultV1Implementation"]),
                                      ("expired_deadline", 0x66, addresses["vaultV2Honest"])]:
        block = call(args.rpc_url, "eth_getBlockByNumber", ["latest", False])
        nonce = int(call(args.rpc_url, "eth_getTransactionCount", [SIGNER, "latest"]), 16)
        priority = int(call(args.rpc_url, "eth_maxPriorityFeePerGas", []), 16)
        max_fee = 2 * int(block["baseFeePerGas"], 16) + priority
        item = {"executor": list(bytes.fromhex(addresses["executor"][2:])),
            "certificate_id": [cert_byte] * 32,
            "new_impl": list(bytes.fromhex(addresses["vaultV2Honest"][2:])),
            "new_impl_codehash": list(bytes.fromhex(codehash[2:])),
            "expected_current_impl": list(bytes.fromhex(expected[2:])),
            "deadline_s": int(block["timestamp"], 16) + (3600 if name == "honest_upgrade" else -1),
            "init_calldata": list(bytes.fromhex(init_calldata[2:])), "nonce": nonce, "gas_limit": 300000,
            "max_fee_per_gas": str(max_fee), "max_priority_fee_per_gas": str(priority)}
        case = near_sign(item, name)
        case.update(name=name, fee_observation={"block_number": block["number"], "base_fee_per_gas": block["baseFeePerGas"],
            "priority_fee_per_gas": priority, "max_fee_per_gas": max_fee})
        evidence["cases"].append(case)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(evidence, indent=2) + "\n")
        tx_hash = call(relay_url, "eth_sendRawTransaction", [case["signed"]["raw_tx"]])
        assert tx_hash == case["signed"]["tx_hash"]
        case["receipt"] = wait_receipt(args.rpc_url, tx_hash)
        case["transaction"] = call(args.rpc_url, "eth_getTransactionByHash", [tx_hash])
        assert case["transaction"]["from"].lower() == SIGNER
        case["nonce_after"] = int(call(args.rpc_url, "eth_getTransactionCount", [SIGNER, "latest"]), 16)
        assert case["nonce_after"] == nonce + 1
        slot = call(args.rpc_url, "eth_getStorageAt", [addresses["proxy"], IMPLEMENTATION_SLOT, "latest"])
        case["implementation_slot"] = slot
        assert "0x" + slot[-40:].lower() == addresses["vaultV2Honest"]
        if name == "honest_upgrade":
            assert int(case["receipt"]["status"], 16) == 1
            topic = keccak("UpgradeExecuted(bytes32,address,address,bytes32)")
            logs = [log for log in case["receipt"]["logs"] if log["address"].lower() == addresses["executor"] and log["topics"][0] == topic]
            assert len(logs) == 1, "UpgradeExecuted missing"
            assert logs[0]["topics"][1] == "0x" + bytes(item["certificate_id"]).hex()
            assert logs[0]["topics"][2] == "0x" + address_word(expected).hex()
            assert logs[0]["topics"][3] == "0x" + address_word(addresses["vaultV2Honest"]).hex()
            assert logs[0]["data"] == codehash
            assert int(cast("call", addresses["proxy"], "depositCap()(uint256)", "--rpc-url", args.rpc_url).split()[0]) == 1000 * 10**18
            case["duplicate_send_response"] = rpc(relay_url, "eth_sendRawTransaction", [case["signed"]["raw_tx"]], allow_error=True)
            assert "error" in case["duplicate_send_response"], "duplicate raw tx was not rejected"
        else:
            assert int(case["receipt"]["status"], 16) == 0
            case["revert_call_response"] = rpc(args.rpc_url, "eth_call", [{"from": SIGNER, "to": addresses["executor"],
                "data": case["transaction"]["input"]}, case["receipt"]["blockNumber"]], allow_error=True)
            error = case["revert_call_response"].get("error", {})
            selector = cast("sig", "DeadlinePassed(uint64,uint256)")
            assert error.get("data", "").startswith(selector), error
        args.output.write_text(json.dumps(evidence, indent=2) + "\n")
        print(f"{label}: {name}, NEAR {case['near_tx_hash']}, EVM {tx_hash}, status {case['receipt']['status']}", flush=True)
    print(f"Evidence: {args.output}")


if __name__ == "__main__":
    main()
