<#
.SYNOPSIS
    Полная проверка примера Roblox-проекта на Luwu.

.DESCRIPTION
    1. luwu build      -- транспиляция .luwu -> .luau
    2. luau-compile    -- проверка синтаксиса сгенерированных файлов
    3. lune run        -- поведенческие проверки общих модулей
    4. rojo build      -- сборка place (проверяет дерево проекта и model.json)

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts/verify-example.ps1
#>
param(
    [switch]$SkipRojo
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$project = Join-Path $root "ExampleRojoProject"
$luwu = Join-Path $root "target\debug\luwu.exe"

if (-not (Test-Path $luwu)) {
    Write-Host "building transpiler..."
    Push-Location $root
    & cargo build --quiet
    Pop-Location
}

Write-Host "1/4 transpiling Luwu -> Luau..."
Push-Location $project
& $luwu build --quiet
Pop-Location
if ($LASTEXITCODE -ne 0) { Write-Error "transpilation failed"; exit 1 }

Write-Host "2/4 checking generated syntax..."
$files = Get-ChildItem -Path (Join-Path $project "build") -Recurse -Filter *.luau
foreach ($file in $files) {
    $output = & luau-compile --only-parse $file.FullName 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Write-Error "syntax error in $($file.Name):`n$output"
        exit 1
    }
}
Write-Host "     $($files.Count) file(s) parsed"

Write-Host "3/4 running behavioural checks..."
Push-Location $project
$output = & lune run tests/smoke.luau 2>&1 | Out-String
$code = $LASTEXITCODE
Pop-Location
Write-Host "     $($output.Trim())"
if ($code -ne 0) { Write-Error "behavioural checks failed"; exit 1 }

if (-not $SkipRojo) {
    Write-Host "4/4 building the place with Rojo..."
    Push-Location $project
    $output = & rojo build -o out.rbxl 2>&1 | Out-String
    $code = $LASTEXITCODE
    Pop-Location
    if ($code -ne 0) { Write-Host $output; Write-Error "rojo build failed"; exit 1 }
    Write-Host "     place built: $project\out.rbxl"
}
else {
    Write-Host "4/4 rojo build skipped"
}

Write-Host ""
Write-Host "example project is green" -ForegroundColor Green
exit 0
