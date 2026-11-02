"""Executed as the tool user in a fresh, no-mount, network-none container."""
import errno
import socket


def denied(label, action, expected):
    try:
        action()
    except OSError as error:
        print(f"{label}: {type(error).__name__}: {error}", flush=True)
        if error.errno not in expected:
            raise
    else:
        raise AssertionError(f"sandbox escape: {label} succeeded")


denied("secret", lambda: open("/run/signing.key", "rb"), {errno.ENOENT, errno.EACCES})
with socket.socket(socket.AF_UNIX) as unix:
    denied("dstack socket", lambda: unix.connect("/var/run/dstack.sock"), {errno.ENOENT, errno.EACCES})
with socket.socket(socket.AF_INET) as network:
    network.settimeout(2)
    denied("network", lambda: network.connect(("1.1.1.1", 443)), {errno.ENETUNREACH})
print("sandbox negative test: Pass")
