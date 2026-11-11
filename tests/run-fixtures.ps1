<#
.SYNOPSIS
    Compiles the Luwu test fixtures and checks the generated Luau.

.DESCRIPTION
    For every *.luwu file under tests/fixtures the script:
      1. compiles it with the luwu transpiler,
      2. runs `luau-analyze` over the generated file (must report no errors),
      3. runs the generated file with `lune` and compares the output with the
         matching *.expected file.

    It exits with a non-zero code when anything fails.

.EXAMPLE
    pwsh -File tests/run-fixtures.ps1
#>
param(
    [string]$Fixtures = (Join-Path $PSScriptRoot "fixtures"),
    [switch]$SkipAnalyze
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "target\debug\luwu.exe"

Write-Host "building transpiler..."
Push-Location $root
& cargo build --quiet
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error "cargo build failed"; exit 1 }
Pop-Location

$outDir = Join-Path ([System.IO.Path]::GetTempPath()) ("luwu-fixtures-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $outDir | Out-Null

$failures = 0
$total = 0

try {
    Push-Location $root
    & $bin build $Fixtures --out $outDir --no-config | Out-Null
    Pop-Location
    if ($LASTEXITCODE -ne 0) { Write-Error "transpilation failed"; exit 1 }

    $sources = Get-ChildItem -Path $Fixtures -Recurse -Filter *.luwu
    foreach ($source in $sources) {
        $total++
        $relative = $source.FullName.Substring((Resolve-Path $Fixtures).Path.Length + 1)
        $generated = Join-Path $outDir ($relative -replace '\.luwu$', '.luau')
        $expectedPath = Join-Path $source.DirectoryName ($source.BaseName + ".expected")

        if (-not (Test-Path $generated)) {
            Write-Host "FAIL $relative : no output produced" -ForegroundColor Red
            $failures++
            continue
        }

        if (-not $SkipAnalyze) {
            $analysis = & luau-analyze $generated 2>&1 | Out-String
            if ($LASTEXITCODE -ne 0) {
                Write-Host "FAIL $relative : luau-analyze reported errors" -ForegroundColor Red
                Write-Host $analysis
                $failures++
                continue
            }
        }

        if (-not (Test-Path $expectedPath)) {
            Write-Host "FAIL $relative : missing $expectedPath" -ForegroundColor Red
            $failures++
            continue
        }

        $actual = (& lune run $generated 2>&1 | Out-String).TrimEnd()
        $expected = (Get-Content -Raw $expectedPath).TrimEnd()

        if ($actual -ne $expected) {
            Write-Host "FAIL $relative : output differs" -ForegroundColor Red
            Write-Host "--- expected ---"
            Write-Host $expected
            Write-Host "--- actual ---"
            Write-Host $actual
            $failures++
            continue
        }

        Write-Host "ok   $relative" -ForegroundColor Green
    }
}
finally {
    Remove-Item -Recurse -Force $outDir -ErrorAction SilentlyContinue
}

Write-Host ""
if ($failures -gt 0) {
    Write-Host "$failures of $total fixture(s) failed" -ForegroundColor Red
    exit 1
}

Write-Host "all $total fixture(s) passed" -ForegroundColor Green
exit 0
