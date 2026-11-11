<#
.SYNOPSIS
    Installs the Luwu transpiler and makes the `luwu` command available.

.DESCRIPTION
    Copies luwu.exe into a directory on your machine (by default
    %LOCALAPPDATA%\Luwu on Windows) and adds that directory to the user PATH.

    Run this script from the extracted release archive, or pass -Source to
    point at a luwu.exe you have already downloaded/built.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\install.ps1
    powershell -ExecutionPolicy Bypass -File .\install.ps1 -Source .\dist\luwu.exe
    powershell -ExecutionPolicy Bypass -File .\install.ps1 -Dir "C:\tools\luwu"
#>
param(
    [string]$Source,
    [string]$Dir,
    [switch]$NoPath
)

$ErrorActionPreference = "Stop"

if (-not $Dir) {
    $Dir = Join-Path $env:LOCALAPPDATA "Luwu"
}

if (-not $Source) {
    $here = Split-Path -Parent $MyInvocation.MyCommand.Path
    $candidates = @(
        (Join-Path $here "luwu.exe"),
        (Join-Path $here "..\dist\luwu.exe"),
        (Join-Path $here "..\target\release\luwu.exe"),
        (Join-Path $here "..\target\debug\luwu.exe")
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $Source = $c; break }
    }
}

if (-not $Source -or -not (Test-Path $Source)) {
    Write-Error "luwu.exe not found. Extract the release archive and run this script from it, or pass -Source <path-to-luwu.exe>."
    exit 1
}

$Source = (Resolve-Path $Source).Path
New-Item -ItemType Directory -Force $Dir | Out-Null
$target = Join-Path $Dir "luwu.exe"
Copy-Item $Source $target -Force
Write-Host "installed $target" -ForegroundColor Green

if (-not $NoPath) {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($userPath) { $parts = $userPath -split ";" | Where-Object { $_ } }
    if ($parts -notcontains $Dir) {
        $newPath = if ([string]::IsNullOrEmpty($userPath)) { $Dir } else { $userPath.TrimEnd(";") + ";" + $Dir }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")

        # Tell running shells that the environment changed.
        $src = @'
using System;
using System.Runtime.InteropServices;
public class EnvBroadcast {
    [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
    public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint flags, uint timeout, out UIntPtr result);
}
'@
        Add-Type -TypeDefinition $src -ErrorAction SilentlyContinue
        $result = [UIntPtr]::Zero
        [EnvBroadcast]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, "Environment", 2, 2000, [ref]$result) | Out-Null

        Write-Host "added $Dir to the user PATH"
        Write-Host "Open a new terminal, then run: luwu --version"
    } else {
        Write-Host "PATH already contains $Dir"
        Write-Host "Ready. Run: luwu --version"
    }
} else {
    Write-Host "PATH was not changed (-NoPath)."
}

exit 0
