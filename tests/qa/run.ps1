param(
    [string]$Luwu = "",
    [string]$Luau = "",
    [string]$Filter = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$cases = Join-Path $PSScriptRoot "cases"
$experimental = Join-Path $PSScriptRoot "experimental"
$outRoot = Join-Path $PSScriptRoot ".out"
$buildRoot = Join-Path $PSScriptRoot ".build"

if (-not $Luwu) {
    $Luwu = Join-Path $root "target/release/luwu.exe"
    if (-not (Test-Path $Luwu)) {
        $Luwu = Join-Path $root "target/debug/luwu.exe"
    }
}
if (-not (Test-Path $Luwu)) {
    throw "luwu binary not found; run 'cargo build --release' first"
}

if (-not $Luau) {
    $Luau = $env:LUAU
}
if (-not $Luau) {
    $cmd = Get-Command luau -ErrorAction SilentlyContinue
    if ($cmd) {
        $Luau = $cmd.Source
    }
}
if (-not $Luau) {
    throw "luau executable not found; set `$env:LUAU or pass -Luau <path>"
}

New-Item -ItemType Directory -Force -Path $outRoot, $buildRoot | Out-Null
$files = @(Get-ChildItem (Join-Path $cases "*.luwu"))
if (Test-Path $experimental) {
    $files += @(Get-ChildItem (Join-Path $experimental "*.luwu.txt"))
}
if ($Filter) {
    $files = $files | Where-Object { $_.Name -like "*$Filter*" }
}

$failures = 0
foreach ($file in $files) {
    $name = $file.Name -replace '\.luwu(\.txt)?$', ''
    $outDir = Join-Path $outRoot $name
    $buildDir = Join-Path $buildRoot $name
    Remove-Item -Recurse -Force $outDir, $buildDir -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $outDir, $buildDir | Out-Null
    Copy-Item $file.FullName (Join-Path $buildDir "$name.luwu")

    $build = & $Luwu build $buildDir -o $outDir --no-config --no-assets -q 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Output "FAIL (transpile) $name"
        $build | ForEach-Object { Write-Output "  $_" }
        $failures++
        continue
    }

    $output = & $Luau (Join-Path $outDir "$name.luau") 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Output "FAIL (runtime) $name"
        $output | ForEach-Object { Write-Output "  $_" }
        $failures++
        continue
    }

    Write-Output "ok   $name"
    $output | ForEach-Object { Write-Output "  $_" }
}

if ($failures -gt 0) {
    Write-Output "$failures fixture(s) failed"
    exit 1
}
Write-Output "all $($files.Count) fixtures passed"
