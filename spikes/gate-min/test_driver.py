import json
from http.server import BaseHTTPRequestHandler, HTTPServer
import threading
import unittest

from driver import independent_preimage, keccak, rlp, rpc


class DriverEncodingTests(unittest.TestCase):
    def test_independent_preimage_matches_original_real_mpc_payload(self):
        item = {
            "executor": [0x11] * 20, "certificate_id": [0x22] * 32,
            "new_impl": [0x33] * 20, "new_impl_codehash": [0x44] * 32,
            "expected_current_impl": [0x55] * 20, "deadline_s": 1800000000,
            "init_calldata": [0xde, 0xad, 0xbe, 0xef], "nonce": 7,
            "gas_limit": 300000, "max_fee_per_gas": "2000000000",
            "max_priority_fee_per_gas": "1000000",
        }
        self.assertEqual(keccak("0x" + independent_preimage(item).hex()),
            "0xf4cabf1bbeafae5b1b847737b03d31ac62d5805cb3204b04faefe5e22bb750c3")

    def test_rlp_integer_and_long_length_boundaries(self):
        self.assertEqual(rlp(0), bytes.fromhex("80"))
        self.assertEqual(rlp(127), bytes.fromhex("7f"))
        self.assertEqual(rlp(128), bytes.fromhex("8180"))
        self.assertEqual(rlp([]), bytes.fromhex("c0"))
        self.assertEqual(rlp(bytes(56))[:2], bytes.fromhex("b838"))
        self.assertEqual(rlp([bytes(56)])[:2], bytes.fromhex("f83a"))

    def test_json_rpc_error_survives_non_200_http_status(self):
        error = {"jsonrpc": "2.0", "id": "test", "error": {"code": -32601, "message": "rpc method is not allowed"}}

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers["Content-Length"]))
                self.send_response(403)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(error).encode())

            def log_message(self, *_):
                pass

        with HTTPServer(("127.0.0.1", 0), Handler) as server:
            thread = threading.Thread(target=server.serve_forever)
            thread.start()
            try:
                url = f"http://127.0.0.1:{server.server_port}"
                self.assertEqual(rpc(url, "eth_chainId", [], allow_error=True), error)
                with self.assertRaisesRegex(RuntimeError, "rpc method is not allowed"):
                    rpc(url, "eth_chainId", [])
            finally:
                server.shutdown()
                thread.join()


if __name__ == "__main__":
    unittest.main()
