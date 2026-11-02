"""Real process failures must never turn into Pass at the adapter boundary."""
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


class RunnerFailures(unittest.TestCase):
    def adapt(self, command, contents):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "halmos.json"
            if contents is not None:
                output.write_text(contents)
            process = subprocess.run(command, check=False, capture_output=True)
            adapter = subprocess.run([sys.executable, str(pathlib.Path(__file__).with_name("classify.py")),
                                      str(process.returncode), str(output), "PassVault",
                                      "check_solvency(uint128,uint128)"], check=True, capture_output=True, text=True)
            return adapter.stdout.strip()

    def test_nonzero_process(self):
        self.assertEqual(self.adapt([sys.executable, "-c", "raise SystemExit(7)"], None), "Incomplete")

    def test_missing_output(self):
        self.assertEqual(self.adapt([sys.executable, "-c", "pass"], None), "Incomplete")

    def test_malformed_output(self):
        self.assertEqual(self.adapt([sys.executable, "-c", "pass"], "[PASS]"), "Incomplete")

    def test_unexpected_json(self):
        self.assertEqual(self.adapt([sys.executable, "-c", "pass"], json.dumps({"pass": True})), "Incomplete")

    @unittest.skipIf(sys.platform == "win32", "GNU timeout is exercised inside Linux image")
    def test_wall_timeout(self):
        self.assertEqual(self.adapt(["timeout", "0.01s", sys.executable, "-c", "import time; time.sleep(5)"], None), "Incomplete")


if __name__ == "__main__":
    unittest.main()
