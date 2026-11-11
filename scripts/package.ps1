<#
.SYNOPSIS
    Builds the release binary and puts it into dist/ for distribution.

.DESCRIPTION
    1. cargo build --release
    2. copy target/release/luwu.exe (or luwu) into dist/
    3. optionally zip it together with the README

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts/package.ps1
    powershell -ExecutionPolicy Bypass -File scripts/package.ps1 -Zip
#>
param(
    [switch]$Zip,
    [switch]$SkipTests
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root "dist"

Push-Location $root
if (-not $SkipTests) {
    Write-Host "running tests..."
    & cargo test --quiet
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error "tests failed"; exit 1 }
}

Write-Host "building release binary..."
& cargo build --release --quiet
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error "release build failed"; exit 1 }
Pop-Location

$binary = Join-Path $root "target\release\luwu.exe"
if (-not (Test-Path $binary)) {
    $binary = Join-Path $root "target\release\luwu"
}
if (-not (Test-Path $binary)) { Write-Error "release binary not found"; exit 1 }

New-Item -ItemType Directory -Force $dist | Out-Null
$target = Join-Path $dist "luwu.exe"
Copy-Item $binary $target -Force

$size = [math]::Round((Get-Item $target).Length / 1MB, 2)
Write-Host "packaged $target ($size MB)" -ForegroundColor Green

if ($Zip) {
    $archive = Join-Path $dist "luwu-$((Get-Content (Join-Path $root 'Cargo.toml') | Select-String '^version' | ForEach-Object { ($_ -split '=')[1].Trim().Trim('"') })).zip"
    if (Test-Path $archive) { Remove-Item $archive -Force }
    Copy-Item (Join-Path $root "README.md") $dist -Force
    Compress-Archive -Path (Join-Path $dist "luwu.exe"), (Join-Path $dist "README.md") -DestinationPath $archive
    Write-Host "packaged $archive" -ForegroundColor Green
}

Write-Host ""
Write-Host "Add the folder to PATH to use it anywhere:"
Write-Host "  `$env:PATH += ';$dist'"
