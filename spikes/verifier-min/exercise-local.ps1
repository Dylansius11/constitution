$ErrorActionPreference = 'Stop'
$image = 'constitution-verifier-spike:local'
$evidence = Join-Path $PSScriptRoot 'evidence'
New-Item -ItemType Directory -Force $evidence | Out-Null
function Run-Docker {
    & docker @args
    if ($LASTEXITCODE -ne 0) { throw "docker failed: $args" }
}
$flags = @('--network', 'none', '--read-only', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges:true', '--pids-limit', '128', '--memory', '1536m', '--cpus', '1', '--tmpfs', '/work:uid=10001,gid=10001,mode=0700,size=536870912', '--tmpfs', '/tmp:uid=10001,gid=10001,mode=0700,size=268435456')
Run-Docker run --rm @flags $image
Run-Docker run --rm @flags $image python -m unittest discover -s /opt/spike -p 'test_*.py'
foreach ($profile in @('default', 'osaka')) {
    foreach ($fixture in @('PassVault', 'FailVault', 'TimeoutVault')) {
        $name = "constitution-spike-$profile-$fixture".ToLower()
        try {
            $log = & docker run --name $name @flags -e "FOUNDRY_PROFILE=$profile" $image sh /opt/spike/run.sh $fixture 2>&1
            if ($LASTEXITCODE -ne 0) { throw "fixture container failed: $log" }
            $log | Tee-Object -FilePath (Join-Path $evidence "$profile-$fixture.log")
            $expected = @{PassVault='Pass';FailVault='Fail';TimeoutVault='Incomplete'}[$fixture]
            # tmpfs disappears at exit; collect trusted runner stdout, not docker cp.
            $observed = @($log | Where-Object { $_ -cmatch '^(Pass|Fail|Incomplete)$' })
            if ($observed.Count -ne 1 -or $observed[0] -cne $expected) { throw "wrong outcome for $fixture" }
        } finally { & docker rm -f $name | Out-Null }
    }
}
Run-Docker run --rm @flags $image sh -c 'cp -R /opt/spike/test /work/test; cp /opt/spike/foundry.toml /work/foundry.toml; forge build; openzeppelin-upgrades-core validate out/build-info --contract VaultV2 --reference VaultV1 --requireReference'
# Keep an unsupported opcode as explicit evidence, not an aborted sandbox test.
$clz = & docker run --rm @flags -e FOUNDRY_PROFILE=osaka $image sh -c 'cp -R /opt/spike/osaka /work/test; cp /opt/spike/foundry.toml /work/foundry.toml; halmos --contract OsakaOpcode --function check_clz --solver-timeout-branching 0 --solver-timeout-assertion 10s --json-output /work/osaka.json; status=$?; cat /work/osaka.json; echo HALMOS_EXIT=$status; python /opt/spike/classify.py $status /work/osaka.json OsakaOpcode check_clz\(uint256\)'
$clz | Tee-Object -FilePath (Join-Path $evidence 'osaka-clz.log')
if (@($clz | Where-Object { $_ -ceq 'Incomplete' }).Count -ne 1) { throw 'CLZ must fail closed' }
try {
    Run-Docker run -d --name constitution-secret-holder --user 0:0 --network none $image python /opt/spike/secret-holder.py
    Run-Docker exec constitution-secret-holder sh -c 'test -f /run/signing.key && test -S /var/run/dstack.sock && echo trusted-sibling-file-and-socket-present'
    Run-Docker run --rm @flags $image python /opt/spike/sandbox-probe.py
} finally { & docker rm -f constitution-secret-holder | Out-Null }
