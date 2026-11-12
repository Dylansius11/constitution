"""Focused offline regression checks; integration is dry-run.sh on real Anvil."""
import copy
import json
import unittest
import jsonschema
import demo


class Records(unittest.TestCase):
    def test_gallery_only_fills_owned_fields_and_fails_closed(self):
        original = {"version": 1, "proxy": None, "attacks": [
            {"id": "a", "contract": "src/A.sol:A", "codehash": "0x" + "ab" * 32,
             "impl": None, "observed": None}], "upgrades": [{"observed": None}]}
        before = copy.deepcopy(original)
        addresses = {"src/A.sol:A": "0x" + "11" * 20}
        with self.assertRaisesRegex(ValueError, "codehash"):
            demo.fill_gallery(original, "0x" + "22" * 20, addresses, lambda _: "0x" + "cd" * 32)
        self.assertEqual(original, before)
        actual = demo.fill_gallery(original, "0x" + "22" * 20, addresses, lambda _: "0x" + "ab" * 32)
        expected = copy.deepcopy(before)
        expected["proxy"] = "0x" + "22" * 20
        expected["attacks"][0]["impl"] = "0x" + "11" * 20
        self.assertEqual(actual, expected)

    def test_record_uses_demo_bindings_and_existing_schema(self):
        evm = {"executor": "0x" + "11" * 20, "proxy": "0x" + "22" * 20,
               "executor_runtime_hash": "0x" + "33" * 32}
        state = {"gate_code_hash": "11111111111111111111111111111111", "verifiers": [],
                 "roles": {"governor": "governor.testnet", "guardian": "guardian.testnet"}}
        record = demo.deployment_record(evm, state)
        schema = json.loads((demo.ROOT / "deployments/testnet.schema.json").read_text())
        jsonschema.Draft202012Validator(schema).validate(record)
        self.assertEqual(record["gate"], "constitution-demo.testnet")
        self.assertEqual(record["governance_delay_days"], 1)
        self.assertEqual(record["executor"], evm["executor"])
        self.assertEqual(record["verifiers"], [])


if __name__ == "__main__":
    unittest.main()
