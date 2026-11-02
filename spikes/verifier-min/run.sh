#!/bin/sh
# Trusted local spike runner. Source tree is copied, never run from a secret-bearing mount.
set -eu
fixture=${1:?PassVault, FailVault or TimeoutVault}
case "$fixture" in
  PassVault|FailVault) solver_timeout=10s ;;
  TimeoutVault) solver_timeout=1ms ;;
  *) echo 'Unknown fixture' >&2; exit 2 ;;
esac
mkdir -p /work/project /work/results
cp -R /opt/spike/test /work/project/test
cp /opt/spike/foundry.toml /work/project/foundry.toml
cd /work/project
status=0
/usr/bin/time -v -o /work/results/time.txt timeout --kill-after=5s 90s halmos \
  --contract "$fixture" --function check_solvency \
  --solver-timeout-branching 0 --solver-timeout-assertion "$solver_timeout" \
  --solver yices --solver-threads 1 \
  --invariant-depth 2 --loop 2 --array-lengths 'xs=2' \
  --json-output /work/results/halmos.json > /work/results/tool.log 2>&1 || status=$?
cat /work/results/tool.log
cat /work/results/time.txt
outcome=$(python /opt/spike/classify.py "$status" /work/results/halmos.json "$fixture" 'check_solvency(uint128,uint128)')
printf '%s\n' "$outcome" | tee /work/results/outcome.txt
printf 'tool_exit=%s\n' "$status"
