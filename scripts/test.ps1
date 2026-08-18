$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo test -p atom-rt
    if ($LASTEXITCODE -ne 0) { throw "atom-rt host tests failed" }
} finally {
    Pop-Location
}
