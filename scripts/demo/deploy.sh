#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
source "$HOME/.cargo/env"
export PATH="$HOME/.foundry/bin:$PATH"
exec python3 -B scripts/demo/demo.py deploy "$@"
