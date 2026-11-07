<#
  Build the app as it is now and open it, so a change can be looked at right away.

    .\tools\qa.ps1              build (release), stop the old QA copy, open the new one
    .\tools\qa.ps1 -NoBuild     just reopen the last build
    .\tools\qa.ps1 -Debug       a debug build (compiles faster, runs slower)

  The QA copy lives in run\ (ignored by git) and has its own build folder, so it never fights
  with the programs that lock target\release (the MCP server) or with other builds.
#>
param(
    [switch]$NoBuild,
    [switch]$Debug
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$profile_dir = if ($Debug) { 'debug' } else { 'release' }
$env:CARGO_TARGET_DIR = Join-Path $root 'target\mine'
$run = Join-Path $root 'run'
New-Item -ItemType Directory -Force $run | Out-Null

if (-not $NoBuild) {
    $flags = @('build', '--bins')
    if (-not $Debug) { $flags += '--release' }
    Write-Host "Building ($profile_dir)..." -ForegroundColor Cyan
    & cargo @flags
    if ($LASTEXITCODE -ne 0) { throw "The build failed, nothing was opened. Fix the errors above." }
}

$built = Join-Path $env:CARGO_TARGET_DIR $profile_dir
Get-Process tp-stable -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 400
Copy-Item (Join-Path $built 'tronic-printer.exe') (Join-Path $run 'tp-stable.exe') -Force
Copy-Item (Join-Path $built 'tronic-print.exe') (Join-Path $run 'tronic-print.exe') -Force

# Windows keeps a console-less app quiet: whatever it says goes to run\stderr.txt.
Start-Process -FilePath (Join-Path $run 'tp-stable.exe') -RedirectStandardError (Join-Path $run 'stderr.txt')
Write-Host "Opened run\tp-stable.exe ($profile_dir). What to look at: run\QA.md" -ForegroundColor Green
