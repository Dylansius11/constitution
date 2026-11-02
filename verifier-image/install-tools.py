"""Install official binary releases only after validating their recorded SHA256."""
import hashlib
import pathlib
import tarfile
import urllib.request

releases = [
    ("https://raw.githubusercontent.com/ethereum/solc-bin/gh-pages/linux-amd64/solc-linux-amd64-v0.8.38+commit.1835d247", "65c3180a336eabb6bdc4bbd49cc405aeb233976964ae6c11fba2a6c8be8f3243", "solc"),
    ("https://github.com/foundry-rs/foundry/releases/download/v1.8.6/foundry_v1.8.6_linux_amd64.tar.gz", "58850106984038f3a7200f38a4182c8cb423d3a4f4c7848eacbc2453e6bc35fc", "foundry.tar.gz"),
    ("https://nodejs.org/dist/v24.22.1/node-v24.22.1-linux-x64.tar.xz", "6c172a9ab6718f7b801fb5a7bd486af2ee5e32758086a04b84fcce2aa77095e6", "node.tar.xz"),
]
for url, expected, name in releases:
    path = pathlib.Path("/tmp") / name
    with urllib.request.urlopen(url) as response, path.open("wb") as output:
        while chunk := response.read(1024 * 1024):
            output.write(chunk)
    actual = hashlib.file_digest(path.open("rb"), "sha256").hexdigest()
    if actual != expected:
        raise RuntimeError(f"checksum mismatch: {url}: {actual}")
    print(f"SHA256 verified {name}: {actual}", flush=True)
    if name == "solc":
        path.replace("/usr/local/bin/solc")
        pathlib.Path("/usr/local/bin/solc").chmod(0o755)
    elif name.startswith("foundry"):
        with tarfile.open(path) as archive:
            archive.extractall("/usr/local/bin", filter="data")
        path.unlink()
    else:
        with tarfile.open(path) as archive:
            archive.extractall("/opt", filter="data")
        path.unlink()
