import unittest
from classify import classify


def result(code=0, loops=0, models=0):
    return {"exitcode": int(code != 0), "test_results": {"test/Vault.t.sol:PassVault": [{
        "name": "check_solvency(uint128,uint128)", "exitcode": code,
        "num_models": models, "num_paths": [1, 1, 0], "num_bounded_loops": loops,
    }]}}


class Outcomes(unittest.TestCase):
    def test_explicit_complete_success(self):
        self.assertEqual(classify(0, result(), "PassVault", "check_solvency(uint128,uint128)"), "Pass")

    def test_counterexample(self):
        self.assertEqual(classify(1, result(1, models=1), "PassVault", "check_solvency(uint128,uint128)"), "Fail")

    def test_incomplete_never_passes(self):
        for status, data in [(124, result()), (1, result()), (0, None), (0, {}),
                             (0, {"exitcode": 0, "test_results": {}}), (1, result(2)),
                             (1, result(3)), (1, result(4)), (1, result(5)),
                             (0, result(0, loops=1)), (0, result(99)), (137, result())]:
            with self.subTest(status=status, data=data):
                self.assertEqual(classify(status, data, "PassVault", "check_solvency(uint128,uint128)"), "Incomplete")

    def test_unexpected_identity(self):
        self.assertEqual(classify(0, result(), "Other", "check_solvency(uint128,uint128)"), "Incomplete")

    def test_missing_fields(self):
        data = result()
        del data["test_results"]["test/Vault.t.sol:PassVault"][0]["num_bounded_loops"]
        self.assertEqual(classify(0, data, "PassVault", "check_solvency(uint128,uint128)"), "Incomplete")

    def test_boolean_loop_count_is_not_complete(self):
        self.assertEqual(classify(0, result(loops=False), "PassVault", "check_solvency(uint128,uint128)"), "Incomplete")

    def test_non_string_contract_key(self):
        data = result()
        data["test_results"] = {1: next(iter(data["test_results"].values()))}
        self.assertEqual(classify(0, data, "PassVault", "check_solvency(uint128,uint128)"), "Incomplete")


if __name__ == "__main__":
    unittest.main()
