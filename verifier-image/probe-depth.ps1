<#
Measures the R2 Halmos invariant harness (rules/r2/Solvency.t.sol) against the demo candidates
inside the same no-network, 1 CPU, 1.5 GiB container profile the verifier uses.
Run from the repository root (comma lists are single strings because of -File):
  powershell -File verifier-image/probe-depth.ps1 -Depths 2,3,4 -Tag v2
Each run starts a fresh container, so cgroup peak memory is per run. Logs, JSON and one line per
run in results.tsv land in crates/verifier/evidence/depth/<Tag>/.
#>
param(
    [string]$Candidates = 'VaultV2Drain,VaultV2Honest',
    [string]$Depths = '4',
    [int]$TimeoutSeconds = 300,
    [string]$Harness = 'rules/r2/Solvency.t.sol',
    [string]$Solver = 'yices',
    [string]$AssertionTimeout = '10s',
    [string]$BranchingTimeout = '1ms',
    [int]$Loop = 2,
    [string]$Extra = '--early-exit',
    [string]$Tag = 'default'
)
$ErrorActionPreference = 'Stop'
$image = 'constitution-verifier-spike:local'
$root = (Get-Location).Path
$results = Join-Path $root "crates/verifier/evidence/depth/$Tag"
New-Item -ItemType Directory -Force $results | Out-Null
$stage = Join-Path ([IO.Path]::GetTempPath()) "constitution-depth-$Tag"
New-Item -ItemType Directory -Force $stage | Out-Null

$runner = @'
#!/bin/sh
cand=$1; depth=$2; limit=$3; solver=$4; ato=$5; bto=$6; loop=$7; shift 7
cd /work
printf 'import {%s as Candidate} from "src/demo/%s.sol";\n' "$cand" "$cand" > candidate.sol
rm -f result.json time.txt tool.log
start=$(date +%s.%N)
/usr/bin/time -v -o time.txt timeout --kill-after=5s "${limit}s" halmos --contract Solvency --function invariant_solvency --invariant-depth "$depth" --loop "$loop" --solver-timeout-branching "$bto" --solver-timeout-assertion "$ato" --solver "$solver" --solver-threads 1 --no-status --statistics --json-output result.json "$@" > tool.log 2>&1
status=$?
end=$(date +%s.%N)
wall=$(echo "$end $start" | awk '{printf "%.1f", $1 - $2}')
rss=$(grep 'Maximum resident' time.txt | awk '{print $NF}')
peak=$(cat /sys/fs/cgroup/memory.peak 2>/dev/null || echo NA)
class=$(python3 /work/classify.py "$status" result.json Solvency 'invariant_solvency()')
printf 'RESULT\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$cand" "$depth" "$status" "$wall" "$rss" "$peak" "$class"
'@
[IO.File]::WriteAllText((Join-Path $stage 'run.sh'), $runner.Replace("`r`n", "`n"))
Copy-Item (Join-Path $root 'spikes/verifier-min/classify.py') (Join-Path $stage 'classify.py') -Force

function D {
    & docker @args
    if ($LASTEXITCODE -ne 0) { throw "docker failed: $args" }
}
function RemoveBox($name) { & cmd /d /c "docker rm -f $name >nul 2>&1" }
function CopyOut($name, $src, $dst) { & cmd /d /c "docker exec $name cat $src > `"$dst`" 2>nul" }
function Put($name, $dir, $dest, $items) {
    & cmd /d /c "tar -C `"$dir`" -cf - $items | docker exec -i $name tar -C $dest -xf -"
    if ($LASTEXITCODE -ne 0) { throw "upload failed: $dir" }
}

$tsv = Join-Path $results 'results.tsv'
if (-not (Test-Path $tsv)) { "marker`tcandidate`tdepth`thalmos_exit`twall_s`tmax_rss_kb`tcgroup_peak_bytes`tclass`tsolver`tassertion_timeout`tbranching_timeout`tloop`textra" | Set-Content -Encoding ascii $tsv }

foreach ($candidate in ($Candidates -split ',')) {
    foreach ($depth in ($Depths -split ',')) {
        $name = "constitution-depth-$Tag-$candidate-d$depth".ToLower()
        RemoveBox $name
        try {
            D run -d --name $name --network none --read-only --cap-drop ALL --security-opt no-new-privileges:true --pids-limit 128 --memory 1536m --cpus 1 --tmpfs '/work:uid=10001,gid=10001,mode=0700,size=536870912' --tmpfs '/tmp:uid=10001,gid=10001,mode=0700,size=268435456' $image sleep 3000 | Out-Null
            D exec $name mkdir -p /work/src /work/lib/openzeppelin /work/lib/forge-std /work/test
            Put $name 'contracts/src' '/work/src' 'demo'
            Put $name 'contracts/lib/openzeppelin-contracts/contracts' '/work/lib/openzeppelin' '.'
            Put $name 'contracts/lib/forge-std/src' '/work/lib/forge-std' '.'
            Put $name (Split-Path -Parent (Join-Path $root $Harness)) '/work/test' (Split-Path -Leaf $Harness)
            Put $name 'rules/r2' '/work' 'foundry.toml'
            Put $name $stage '/work' 'run.sh classify.py'
            $extraArgs = @($Extra -split ' ' | Where-Object { $_ })
            $out = & docker exec $name sh /work/run.sh $candidate $depth $TimeoutSeconds $Solver $AssertionTimeout $BranchingTimeout $Loop @extraArgs
            $line = ($out | Where-Object { $_ -like "RESULT*" } | Select-Object -Last 1)
            if (-not $line) { throw "no RESULT line for $candidate depth $depth" }
            $base = "$candidate.d$depth"
            CopyOut $name /work/tool.log (Join-Path $results "$base.log")
            CopyOut $name /work/result.json (Join-Path $results "$base.json")
            CopyOut $name /work/time.txt (Join-Path $results "$base.time.txt")
            $fields = $line.Split("`t")
            "$($fields[0])`t$($fields[1])`t$($fields[2])`t$($fields[3])`t$($fields[4])`t$($fields[5])`t$($fields[6])`t$($fields[7])`t$Solver`t$AssertionTimeout`t$BranchingTimeout`t$Loop`t$Extra" | Add-Content -Encoding ascii $tsv
            Write-Host ($line -replace "`t", ' | ')
        } finally { RemoveBox $name }
    }
}
