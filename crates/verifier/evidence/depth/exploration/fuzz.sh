#!/bin/bash
# usage: fuzz.sh CAND RUNS DEPTH SEED [TIMEOUT]
cand=$1; runs=$2; depth=$3; seed=$4; limit=${5:-280}
ROOT="/mnt/c/Users/HP Victus/Documents/personal-projects/hackathon/near-hackathon/constitution"
name="fuzz-$RANDOM"
docker run -d --name $name --network none --read-only --cap-drop ALL --security-opt no-new-privileges:true --pids-limit 128 --memory 1536m --cpus 1 --tmpfs '/work:uid=10001,gid=10001,mode=0700,size=536870912' --tmpfs '/tmp:uid=10001,gid=10001,mode=0700,size=268435456' constitution-verifier-spike:local sleep 3000 >/dev/null
docker exec $name mkdir -p /work/src /work/lib/openzeppelin /work/lib/forge-std /work/test
tar -C "$ROOT/contracts/src" -cf - demo | docker exec -i $name tar -C /work/src -xf -
tar -C "$ROOT/contracts/lib/openzeppelin-contracts/contracts" -cf - . | docker exec -i $name tar -C /work/lib/openzeppelin -xf -
tar -C "$ROOT/contracts/lib/forge-std/src" -cf - . | docker exec -i $name tar -C /work/lib/forge-std -xf -
tar -C "$ROOT/rules/r2" -cf - foundry.toml Solvency.t.sol | docker exec -i $name tar -C /work -xf -
docker exec $name sh -c "mv /work/Solvency.t.sol /work/test/Solvency.t.sol; printf 'import {$cand as Candidate} from \"src/demo/$cand.sol\";\n' > /work/candidate.sol"
docker exec -w /work $name sh -c "export HOME=/tmp FOUNDRY_INVARIANT_RUNS=$runs FOUNDRY_INVARIANT_DEPTH=$depth FOUNDRY_FUZZ_SEED=$seed FOUNDRY_INVARIANT_FAIL_ON_REVERT=false; s=\$(date +%s.%N); timeout --kill-after=5s ${limit}s /usr/bin/time -v -o /work/time.txt forge test --match-contract Solvency --match-test invariant_solvency -vv > /work/f.log 2>&1; st=\$?; e=\$(date +%s.%N); echo \"cand=$cand runs=$runs depth=$depth seed=$seed status=\$st wall=\$(echo \$e \$s | awk '{printf \"%.1f\",\$1-\$2}') rss_kb=\$(grep 'Maximum resident' /work/time.txt | awk '{print \$NF}') \$(grep -a -E '^\[(PASS|FAIL)' /work/f.log | head -2 | cut -c1-150 | tr '\n' ' ')\""
mkdir -p "$(dirname "$0")/logs"
docker exec $name cat /work/f.log > "$(dirname "$0")/logs/fuzz.$cand.r$runs.d$depth.s$seed.log" 2>/dev/null
docker rm -f $name >/dev/null 2>&1
