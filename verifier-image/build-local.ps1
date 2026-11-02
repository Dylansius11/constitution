$ErrorActionPreference = 'Stop'
$timer = [System.Diagnostics.Stopwatch]::StartNew()
docker build --progress=plain -f verifier-image/Dockerfile -t constitution-verifier-spike:local .
$code = $LASTEXITCODE
$timer.Stop()
Write-Output ('BUILD_SECONDS=' + $timer.Elapsed.TotalSeconds)
if ($code -eq 0) { docker image inspect constitution-verifier-spike:local --format 'IMAGE_ID={{.Id}} SIZE_BYTES={{.Size}}' }
exit $code
