$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$readobj = 'C:\Users\jgali\.rustup\toolchains\nightly-x86_64-pc-windows-msvc\lib\rustlib\x86_64-pc-windows-msvc\bin\llvm-readobj.exe'
$image = Join-Path $root 'image'

if (-not (Test-Path -LiteralPath $readobj)) { throw "llvm-readobj not found at $readobj" }
if (-not (Test-Path -LiteralPath $image)) { throw "image\ not found - run scripts\build.ps1 first" }

$failed = $false
foreach ($elf in Get-ChildItem -LiteralPath $image -Filter *.elf) {
    $hdr = & $readobj --file-headers --program-headers $elf.FullName | Out-String
    $checks = @(
        @{ Name = 'ELF64';        Ok = $hdr -match 'Class:\s+64-bit' },
        @{ Name = 'x86_64';       Ok = $hdr -match 'EM_X86_64' },
        @{ Name = 'ET_EXEC';      Ok = $hdr -match 'Type:\s+Executable' },
        @{ Name = 'entry-in-user-range'; Ok = $hdr -match 'Entry:\s+0xFFFFFFFF801[0-9A-F]{5}' },
        @{ Name = 'PT_LOAD-at-link-base'; Ok = $hdr -match 'VirtualAddress:\s+0xFFFFFFFF80100000' }
    )
    foreach ($c in $checks) {
        if (-not $c.Ok) {
            Write-Output "FAIL $($elf.Name): $($c.Name)"
            $failed = $true
        }
    }
    if (-not $failed) { Write-Output "ok   $($elf.Name)" }
}
if ($failed) { exit 1 }
Write-Output "all ELFs pass static loader-compatibility checks (headers/PHDRs only; not execution)"
