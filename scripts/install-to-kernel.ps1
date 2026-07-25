param(
    [string]$KernelPath = 'C:\Projects\atom-os-kernel',
    [switch]$Force
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$kernelTarget = Join-Path $KernelPath 'target\x86_64-os\release'
if (-not (Test-Path -LiteralPath $kernelTarget)) {
    throw "kernel target dir not found: $kernelTarget (build the kernel's payload/daemon once first)"
}

$map = @(
    @{ Src = Join-Path $root 'image\shell.elf';    Dst = Join-Path $kernelTarget 'payload' },
    @{ Src = Join-Path $root 'image\daemon.elf';   Dst = Join-Path $kernelTarget 'daemon' },
    @{ Src = Join-Path $root 'image\hello.elf';    Dst = Join-Path $kernelTarget 'hello' },
    @{ Src = Join-Path $root 'image\fieldmon.elf'; Dst = Join-Path $kernelTarget 'fieldmon' }
)

foreach ($m in $map) {
    if (-not (Test-Path -LiteralPath $m.Src)) {
        throw "missing $($m.Src) - run scripts\build.ps1 first"
    }
}

Write-Output "This replaces the kernel's embedded userspace binaries:"
foreach ($m in $map) {
    Write-Output "  $($m.Src) -> $($m.Dst)"
}
Write-Output "Kernel source is NOT modified. Rebuild x86_64-kernel afterwards to boot this system."

if (-not $Force) {
    Write-Output "dry run; re-run with -Force to copy"
    exit 0
}

foreach ($m in $map) {
    Copy-Item -LiteralPath $m.Src -Destination $m.Dst -Force
}
Write-Output "installed. NOTE: if cargo rebuilds the kernel's own payload/daemon crates it will overwrite these files; build only the x86_64-kernel crate."
