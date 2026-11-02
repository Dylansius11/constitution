#!/bin/sh
set -eu
solc --version
forge --version
halmos --version
python --version
python -c 'import importlib.metadata as m; print("z3-solver", m.version("z3-solver")); print("yices-solver", m.version("yices-solver"))'
yices-smt2 --version
z3 --version
node --version
node -p 'require("/opt/tools/node_modules/@openzeppelin/upgrades-core/package.json").version'
