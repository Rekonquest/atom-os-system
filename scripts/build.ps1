$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$programs = @('hello', 'daemon', 'shell', 'fieldmon')

foreach ($p in $programs) {
    Write-Output "== build $p =="
    Push-Location (Join-Path $root "programs\$p")
    try {
        cargo -Zjson-target-spec build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed for $p" }
    } finally {
        Pop-Location
    }
}

$image = Join-Path $root 'image'
New-Item -ItemType Directory -Force -Path $image | Out-Null
foreach ($p in $programs) {
    $bin = Join-Path $root "target\x86_64-atom-user\release\$p"
    Copy-Item -LiteralPath $bin -Destination (Join-Path $image "$p.elf") -Force
}
Write-Output "staged ELFs in $image"
