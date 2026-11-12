"""Derive the demo EOA; default compares an offline tweak with a finalized MPC view."""
import argparse
import hashlib
import json
import sys
from demo import GATE, MPC, MPC_PATH, ROOT, near_view, run

P = 0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f
N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
G = (0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798,
     0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8)


def decode_key(key):
    if not key.startswith("secp256k1:"):
        raise ValueError("Expected secp256k1 public key")
    encoded = key.split(":", 1)[1]
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    value = 0
    for char in encoded:
        value = value * 58 + alphabet.index(char)
    raw = b"\0" * (len(encoded) - len(encoded.lstrip("1"))) + value.to_bytes((value.bit_length() + 7) // 8, "big")
    if len(raw) != 64:
        raise ValueError("Expected 64-byte MPC point")
    x, y = int.from_bytes(raw[:32], "big"), int.from_bytes(raw[32:], "big")
    if x >= P or y >= P or (y*y - x*x*x - 7) % P:
        raise ValueError("Invalid secp256k1 point")
    return x, y


def add(a, b):
    if a is None:
        return b
    if b is None:
        return a
    x, y = a
    u, v = b
    if x == u and (y + v) % P == 0:
        return None
    slope = ((3*x*x) * pow(2*y, -1, P) if a == b else (v-y) * pow(u-x, -1, P)) % P
    nx = (slope*slope - x-u) % P
    return nx, (slope*(x-nx) - y) % P


def derived_point(root, gate=GATE, path=MPC_PATH):
    prefix = "near-mpc-recovery v0.1.0 epsilon derivation:"
    scalar = int.from_bytes(hashlib.sha3_256(f"{prefix}{gate},{path}".encode()).digest(), "big")
    if scalar >= N:
        raise ValueError("MPC tweak scalar is out of range")
    point, base = None, G
    while scalar:
        if scalar & 1:
            point = add(point, base)
        base = add(base, base)
        scalar >>= 1
    result = add(decode_key(root), point)
    if result is None:
        raise ValueError("Derived identity point")
    return b"".join(coordinate.to_bytes(32, "big") for coordinate in result)


def eoa(point):
    return "0x" + run("cast", "keccak", "0x" + point.hex())[-40:]


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("--near-rpc", default="https://test.rpc.fastnear.com")
    parser.add_argument("--offline", action="store_true", help="Archived root, local Anvil only; not live MPC evidence")
    args = parser.parse_args()
    if args.offline:
        root = json.loads((ROOT / "vectors/mpc-derivation.json").read_text())["root"]["decoded"]
    else:
        root = near_view(args.near_rpc, MPC, "public_key", {"domain_id": 0})
    point = derived_point(root)
    if not args.offline:
        live = decode_key(near_view(args.near_rpc, MPC, "derived_public_key", {
            "predecessor": GATE, "path": MPC_PATH, "domain_id": 0}))
        if point != b"".join(c.to_bytes(32, "big") for c in live):
            raise ValueError("Offline derivation differs from finalized MPC view")
    print(eoa(point))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"MPC derivation stopped: {type(error).__name__}; inspect provider privately", file=sys.stderr)
        sys.exit(1)
