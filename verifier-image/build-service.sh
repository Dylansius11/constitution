#!/bin/sh
# Run from repository root on Linux/WSL; no credentials are used as build inputs.
set -eu
[ -f Cargo.lock ] && [ -f verifier-image/Service.Dockerfile ] || {
  echo 'Run from the repository root.' >&2
  exit 1
}
docker build --platform linux/amd64 --progress=plain -f verifier-image/Dockerfile -t constitution-verifier-spike:local .
docker build --platform linux/amd64 --progress=plain -f verifier-image/Service.Dockerfile \
  --build-arg TOOLS_IMAGE=constitution-verifier-spike:local \
  -t constitution-verifier-service:local .
docker image inspect constitution-verifier-spike:local constitution-verifier-service:local \
  --format 'IMAGE_ID={{.Id}} SIZE_BYTES={{.Size}}'
