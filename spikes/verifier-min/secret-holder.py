"""Test-only secret and real Unix socket in the trusted sibling container."""
import pathlib
import socket
import time

path = pathlib.Path("/run/signing.key")
path.write_text("PLANTED-TEST-SECRET-NOT-A-KEY\n")
path.chmod(0o644)
with socket.socket(socket.AF_UNIX) as server:
    server.bind("/var/run/dstack.sock")
    server.listen()
    pathlib.Path("/var/run/dstack.sock").chmod(0o666)
    print("planted readable secret and listening Unix socket", flush=True)
    time.sleep(300)
