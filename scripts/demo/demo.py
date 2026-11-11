"""Testnet-only deployment orchestration. Keys stay inside Foundry keystores."""
import argparse
import base64
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
from urllib.parse import urlparse
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[2]
GATE = "constitution-demo.testnet"
MPC = "v1.signer-prod.testnet"
MPC_PATH = "constitution/base-sepolia/v1"
ACTOR = "0x289541ff1cd4882653a02b049ebb5d6a06374826"


def run(*args, cwd=ROOT, env=None):
    result = subprocess.run(args, cwd=cwd, env=env, text=True, capture_output=True)
    if result.returncode:
        # Provider errors may contain authenticated URLs. Do not log command/output.
        raise RuntimeError(f"{args[0]} {args[1]} failed (exit {result.returncode}); inspect privately")
    return result.stdout.strip()


def rpc(url, method, params):
    request = Request(url, json.dumps({"jsonrpc": "2.0", "id": "demo", "method": method,
                                      "params": params}).encode(),
                      {"Content-Type": "application/json", "User-Agent": "constitution-demo/1"})
    with urlopen(request, timeout=60) as response:
        data = json.load(response)
    if "error" in data:
        raise RuntimeError(f"RPC {method} refused; inspect provider privately")
    return data["result"]


def near_view(url, account, method, args):
    result = rpc(url, "query", {"request_type": "call_function", "finality": "final",
                               "account_id": account, "method_name": method,
                               "args_base64": base64.b64encode(json.dumps(args).encode()).decode()})
    return json.loads(bytes(result["result"]))


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def fill_gallery(gallery, proxy, implementations, codehash):
    updated = copy.deepcopy(gallery)
    if gallery.get("gate") not in (None, GATE):
        raise ValueError("Gallery is bound to another gate")
    for attack in updated["attacks"]:
        implementation = implementations[attack["contract"]].lower()
        if codehash(implementation).lower() != attack["codehash"]:
            raise ValueError(f"Gallery codehash mismatch: {attack['id']}")
        attack["impl"] = implementation
    updated["proxy"] = proxy.lower()
    return updated


def deployment_record(evm, state):
    return {"chain_id": 84532, "governance_delay_days": 1, "gate": GATE,
            "gate_code_hash": state["gate_code_hash"], "mpc_account": MPC,
            "executor": evm["executor"], "executor_runtime_hash": evm["executor_runtime_hash"],
            "proxy": evm["proxy"], "verifiers": state["verifiers"],
            "roles": state["roles"], "planned_changes": []}


def validate(record):
    import jsonschema
    schema = json.loads((ROOT / "deployments/testnet.schema.json").read_text())
    jsonschema.Draft202012Validator(schema).validate(record)


def wallet(args, actor=False, script=False):
    if args.local_unlocked:
        return ["--unlocked", "--sender" if script else "--from", ACTOR if actor else args.deployer]
    prefix = "actor" if actor else "deployer"
    return ["--keystore", str(Path(getattr(args, prefix + "_keystore")).resolve()),
            "--password-file", str(Path(getattr(args, prefix + "_password_file")).resolve())]


def call(args, address, signature, *values):
    return run("cast", "call", address, signature, *values, "--rpc-url", args.rpc).lower()


def number(value):
    return int(value.split()[0], 16 if value.startswith("0x") else 10)


def check_evm(args, evm):
    def require(condition, description):
        if not condition:
            raise ValueError(f"EVM readback failed: {description}")

    require(number(run("cast", "chain-id", "--rpc-url", args.rpc)) == 84532, "chain id")
    for signature, expected in (("signer()(address)", evm["derived_eoa"]),
                                ("proxy()(address)", evm["proxy"]),
                                ("proxyAdmin()(address)", evm["proxy_admin"]),
                                ("currentImplementation()(address)", evm["vault_v1"])):
        require(call(args, evm["executor"], signature) == expected, signature)
    require(call(args, evm["proxy_admin"], "owner()(address)") == evm["executor"], "ProxyAdmin owner")
    implementation_slot = hex(int(run("cast", "keccak", "eip1967.proxy.implementation"), 16) - 1)
    require(run("cast", "storage", evm["proxy"], implementation_slot, "--rpc-url", args.rpc)[-40:].lower()
            == evm["vault_v1"][2:], "ERC-1967 implementation")
    # Initialization itself must have run, not merely installed V1.
    vault_slot = "0x199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416800"
    require(run("cast", "storage", evm["proxy"], vault_slot, "--rpc-url", args.rpc)[-40:].lower()
            == evm["token"][2:], "initialized vault token")
    require(number(call(args, evm["token"], "balanceOf(address)(uint256)", ACTOR)) >= 900, "actor tokens >= 900")
    require(number(call(args, evm["token"], "allowance(address,address)(uint256)", ACTOR, evm["proxy"])) >= 900,
            "actor allowance >= 900")
    require(number(call(args, evm["proxy"], "balanceOf(address)(uint256)", ACTOR)) == 100, "actor deposit = 100")
    require(number(call(args, evm["proxy"], "totalDeposits()(uint256)")) == 100, "total deposits = 100")
    require(number(call(args, evm["token"], "balanceOf(address)(uint256)", evm["proxy"])) == 100, "vault tokens = 100")


def deploy(args):
    resuming = Path(args.output).exists()
    if resuming and not args.resume:
        raise ValueError("Output exists; pass --resume to continue a partial testnet deployment")
    if number(run("cast", "chain-id", "--rpc-url", args.rpc)) != 84532:
        raise ValueError("Expected chain 84532")
    if args.local_unlocked:
        if urlparse(args.rpc).hostname not in ("localhost", "127.0.0.1"):
            raise ValueError("Unlocked mode is local Anvil only")
        if "anvil" not in rpc(args.rpc, "web3_clientVersion", []).lower():
            raise ValueError("Unlocked mode is local Anvil only")
    else:
        actor = run("cast", "wallet", "address", *wallet(args, True)).lower()
        if actor != ACTOR:
            raise ValueError("Actor keystore does not match committed R5 actor")
    if run("cast", "code", ACTOR, "--rpc-url", args.rpc) != "0x":
        raise ValueError("R5 actor must have no code")
    gallery = json.loads(Path(args.gallery).read_text())
    contracts = list(dict.fromkeys([a["contract"] for a in gallery["attacks"]] + [
        "src/demo/VaultV2Honest.sol:VaultV2Honest", "src/demo/VaultV3Honest.sol:VaultV3Honest"]))
    # Fail before spending if any candidate is absent or its compiled hash differs.
    hashes = {}
    for contract in contracts:
        bytecode = run("forge", "inspect", contract, "deployedBytecode", "--offline", cwd=ROOT / "contracts")
        hashes[contract] = run("cast", "keccak", bytecode)
    for attack in gallery["attacks"]:
        if hashes[attack["contract"]] != attack["codehash"]:
            raise ValueError(f"Compiled gallery codehash mismatch: {attack['id']}")
    if resuming:
        # Bootstrap already confirmed on-chain; reuse the persisted observations.
        evm = json.loads(Path(args.output).read_text())
        if evm["derived_eoa"] != args.signer.lower():
            raise ValueError("Resume record belongs to another signer")
        receipts = evm["receipts"]
    else:
        env = dict(os.environ, SIGNER=args.signer)
        run("forge", "script", "script/DeployDemo.s.sol:DeployDemo", "--rpc-url", args.rpc,
            *wallet(args, script=True), "--broadcast", "--slow", "--offline", cwd=ROOT / "contracts", env=env)
        broadcast = json.loads((ROOT / "contracts/broadcast/DeployDemo.s.sol/84532/run-latest.json").read_text())
        created = {t["contractName"]: t["contractAddress"].lower() for t in broadcast["transactions"]
                   if t["transactionType"] == "CREATE"}
        receipts = list(broadcast["receipts"])
        evm = {"executor": created["ConstitutionExecutor"], "token": created["MockToken"],
               "vault_v1": created["VaultV1"], "derived_eoa": args.signer.lower(), "implementations": {},
               "receipts": receipts}
        evm["proxy"] = call(args, evm["executor"], "proxy()(address)")
        evm["proxy_admin"] = call(args, evm["executor"], "proxyAdmin()(address)")
        # Persist bootstrap observations immediately; never blindly redeploy after partial failure.
        save(args.output, evm)
    for contract in contracts:
        if contract in evm["implementations"]:
            continue
        result = json.loads(run("forge", "create", contract, "--json", "--broadcast", "--offline",
                                "--rpc-url", args.rpc, *wallet(args), cwd=ROOT / "contracts"))
        evm["implementations"][contract] = result["deployedTo"].lower()
        receipts.append(rpc(args.rpc, "eth_getTransactionReceipt", [result["transactionHash"]]))
        save(args.output, evm)

    def send(to, *values, actor=False):
        receipt = json.loads(run("cast", "send", to, *values, "--json", "--rpc-url", args.rpc,
                                 *wallet(args, actor)))
        if number(str(receipt["status"])) != 1:
            raise ValueError("Transaction reverted")
        receipts.append(receipt)
        evm["receipts"] = receipts
        save(args.output, evm)

    send(args.signer, "--value", args.signer_funding)
    send(ACTOR, "--value", args.actor_funding)
    if args.local_unlocked:
        rpc(args.rpc, "anvil_impersonateAccount", [ACTOR])
    try:
        if number(call(args, evm["proxy"], "balanceOf(address)(uint256)", ACTOR)) != 0:
            raise ValueError("New proxy already has actor deposits")
        send(evm["token"], "mint(address,uint256)", ACTOR, "1000")
        send(evm["token"], "approve(address,uint256)", evm["proxy"], "1000", actor=True)
        send(evm["proxy"], "deposit(uint256)", "100", actor=True)
    finally:
        if args.local_unlocked:
            rpc(args.rpc, "anvil_stopImpersonatingAccount", [ACTOR])
    evm["executor_runtime_hash"] = run("cargo", "run", "-p", "constitution-cli", "--",
                                     "executor-runtime-hash").splitlines()[-1]
    evm["initial_evm_nonce"] = number(run("cast", "nonce", args.signer, "--rpc-url", args.rpc))
    evm["gas_used"] = sum(number(str(r["gasUsed"])) for r in receipts)
    evm["receipts"] = receipts
    check_evm(args, evm)
    save(args.output, evm)
    filled = fill_gallery(gallery, evm["proxy"], evm["implementations"],
                          lambda address: run("cast", "codehash", address, "--rpc-url", args.rpc))
    save(args.gallery, filled)
    print(f"Local/testnet EVM readbacks passed; {len(receipts)} transactions; gas_used={evm['gas_used']}")
    print("R5 token balance=900 allowance=900 vault balance=100; proxy V1 initialized; ProxyAdmin owner=executor")
    print(f"Observed EVM record: {args.output}; no NEAR deployment or attestation is implied")


def finalize(args):
    evm = json.loads(Path(args.evm).read_text())
    config = near_view(args.near_rpc, GATE, "get_config", {})
    expected = {"chain_id": 84532, "executor": evm["executor"], "proxy": evm["proxy"],
                "proxy_admin": evm["proxy_admin"], "derived_eoa": evm["derived_eoa"],
                "mpc_account": MPC, "mpc_path": MPC_PATH, "mpc_domain_id": 0, "governance_delay_days": 1}
    if config != expected:
        raise ValueError("Observed gate config does not match demo EVM deployment")
    account = rpc(args.near_rpc, "query", {"request_type": "view_account", "finality": "final", "account_id": GATE})
    if account["code_hash"] != args.expected_gate_code_hash:
        raise ValueError("Gate wasm commitment mismatch")
    verifiers = near_view(args.near_rpc, GATE, "list_verifiers", {"from_index": 0, "limit": 50})
    if not verifiers:
        raise ValueError("Register and appraise the demo verifier before finalizing")
    state = {"gate_code_hash": account["code_hash"], "roles": near_view(args.near_rpc, GATE, "get_roles", {}),
             "verifiers": [{"pubkey": key, "release_hash": value["release_hash"]} for key, value in verifiers]}
    record = deployment_record(evm, state)
    validate(record)
    save(args.output, record)
    print(f"Schema-valid demo record written: {args.output}; run the full audit before deleting keys")


def main():
    parser = argparse.ArgumentParser(__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    dep = commands.add_parser("deploy")
    dep.add_argument("--rpc", required=True)
    dep.add_argument("--signer", required=True)
    dep.add_argument("--gallery", default="deployments/demo-gallery.json")
    dep.add_argument("--output", default="scratch/demo/evm.json")
    dep.add_argument("--signer-funding", default="0.0005ether")
    dep.add_argument("--actor-funding", default="0.002ether")
    dep.add_argument("--local-unlocked", action="store_true")
    dep.add_argument("--resume", action="store_true", help="continue a partial testnet deployment from --output")
    dep.add_argument("--deployer", default="0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
    for name in ("deployer", "actor"):
        dep.add_argument(f"--{name}-keystore", default=f".secrets/evm/{'r5-actor' if name == 'actor' else name}")
        dep.add_argument(f"--{name}-password-file", default=f".secrets/evm/{'r5-actor' if name == 'actor' else name}.password")
    fin = commands.add_parser("finalize")
    fin.add_argument("--near-rpc", required=True)
    fin.add_argument("--evm", default="scratch/demo/evm.json")
    fin.add_argument("--expected-gate-code-hash", required=True)
    fin.add_argument("--output", default="deployments/demo.json")
    args = parser.parse_args()
    (deploy if args.command == "deploy" else finalize)(args)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # urllib exceptions can expose secret-bearing provider URLs.
        detail = str(error) if isinstance(error, (ValueError, RuntimeError)) else type(error).__name__
        print(f"Demo step stopped: {detail}. Inspect privately before any retry.", file=sys.stderr)
        sys.exit(1)
