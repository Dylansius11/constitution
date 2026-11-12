import json
import unittest
from unittest.mock import patch
import demo
import derive


class Derivation(unittest.TestCase):
    def test_archived_vectors_match_offline_point_and_eoa(self):
        data = json.loads((demo.ROOT / "vectors/mpc-derivation.json").read_text())
        for vector in data["vectors"]:
            with self.subTest(gate=vector["predecessor"], path=vector["path"]):
                point = derive.derived_point(data["root"]["decoded"], vector["predecessor"], vector["path"])
                self.assertEqual("04" + point.hex(), vector["public_key_uncompressed_hex"])
                self.assertEqual(derive.eoa(point), vector["evm_address"])

    def test_bad_curve_points_are_refused(self):
        with self.assertRaises(ValueError):
            derive.decode_key("secp256k1:" + "1" * 64)

    def test_out_of_range_tweak_fails_instead_of_modulo_reduction(self):
        data = json.loads((demo.ROOT / "vectors/mpc-derivation.json").read_text())
        # A real SHA3 preimage for this astronomically rare branch is unavailable.
        with patch("derive.hashlib.sha3_256") as digest:
            digest.return_value.digest.return_value = derive.N.to_bytes(32, "big")
            with self.assertRaisesRegex(ValueError, "scalar"):
                derive.derived_point(data["root"]["decoded"])


if __name__ == "__main__":
    unittest.main()
