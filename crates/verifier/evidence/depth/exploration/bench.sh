#!/bin/bash
# usage: bench.sh VARIANT_FILE CAND DEPTH LIMIT BTO [extra halmos args...]
var=$(realpath "$1"); cand=$2; depth=$3; limit=$4; bto=$5; shift 5
ROOT="/mnt/c/Users/HP Victus/Documents/personal-projects/hackathon/near-hackathon/constitution"
name="bench-$(basename "$var" .sol)-$cand-d$depth-$RANDOM"
docker run -d --name $name --network none --read-only --cap-drop ALL --security-opt no-new-privileges:true --pids-limit 128 --memory 1536m --cpus 1 --tmpfs '/work:uid=10001,gid=10001,mode=0700,size=536870912' --tmpfs '/tmp:uid=10001,gid=10001,mode=0700,size=268435456' constitution-verifier-spike:local sleep 3000 >/dev/null
docker exec $name mkdir -p /work/src /work/lib/openzeppelin /work/lib/forge-std /work/test
tar -C "$ROOT/contracts/src" -cf - demo | docker exec -i $name tar -C /work/src -xf -
tar -C "$ROOT/contracts/lib/openzeppelin-contracts/contracts" -cf - . | docker exec -i $name tar -C /work/lib/openzeppelin -xf -
tar -C "$ROOT/contracts/lib/forge-std/src" -cf - . | docker exec -i $name tar -C /work/lib/forge-std -xf -
tar -C "$ROOT/rules/r2" -cf - foundry.toml | docker exec -i $name tar -C /work -xf -
docker exec -i $name sh -c 'cat > /work/test/Solvency.t.sol' < "$var"
docker exec $name sh -c "printf 'import {$cand as Candidate} from \"src/demo/$cand.sol\";\n' > /work/candidate.sol"
docker exec -w /work $name sh -c "s=\$(date +%s.%N); timeout --kill-after=5s ${limit}s halmos --contract Solvency --function invariant_solvency --invariant-depth $depth --loop 2 --solver-timeout-branching $bto --solver-timeout-assertion 10s --solver yices --solver-threads 1 --no-status --json-output /work/r.json $* > /work/t.log 2>&1; st=\$?; e=\$(date +%s.%N); echo \"status=\$st wall=\$(echo \$e \$s | awk '{printf \"%.1f\",\$1-\$2}') \$(grep -a -E '^\[(PASS|FAIL|TIMEOUT)' /work/t.log | head -3 | tr '\n' ' ')\""
mkdir -p "$(dirname "$0")/logs"
lg="$(dirname "$0")/logs/$(basename "$var" .sol).$cand.d$depth.$bto.$RANDOM"
docker exec $name cat /work/t.log > "$lg.log" 2>/dev/null
docker exec $name cat /work/r.json > "$lg.json" 2>/dev/null
docker rm -f $name >/dev/null 2>&1
