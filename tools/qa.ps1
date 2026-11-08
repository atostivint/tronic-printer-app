<#
  Build the app as it is now and open it, so a change can be looked at right away.

    .\tools\qa.ps1              build (release), stop the old QA copy, open the new one
    .\tools\qa.ps1 -NoBuild     just reopen the last build
    .\tools\qa.ps1 -Debug       a debug build (compiles faster, runs slower)

  The QA copy lives in run\ (ignored by git) and has its own build folder, so it never fights
  with the programs that lock target\release (the MCP server) or with other builds.

  The build goes through tools\build.ps1: one build at a time on the machine, jobs = 8,
  below-normal priority, and nothing starts when less than 1.5 GB of RAM is free.
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
    & (Join-Path $PSScriptRoot 'build.ps1') @flags
    if ($LASTEXITCODE -ne 0) { throw "The build failed, nothing was opened. Fix the errors above." }
}

# Say which build is about to open, the same line the window shows bottom-right. Git may be
# absent (an unpacked source tree): "unknown" then, never an error.
$cargo_version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1)
$cargo_version = if ($cargo_version) { $cargo_version.Matches[0].Groups[1].Value } else { '0.0.0' }
$hash = try { (git rev-parse --short HEAD 2>$null) } catch { $null }
if (-not $hash) { $hash = 'unknown' }
$count = try { (git rev-list --count HEAD 2>$null) } catch { $null }
if (-not $count) { $count = '0' }
$dirty = try { if (git status --porcelain 2>$null) { '*' } else { '' } } catch { '' }
# The compile time, on the local clock like build.rs stamps it, so this line and the window agree:
# the moment the build finished, or (with -NoBuild) the moment the binary that is about to open
# was written.
$stamp = if (-not $NoBuild -or -not (Test-Path (Join-Path $run 'tp-stable.exe'))) {
    [DateTime]::Now
} else {
    (Get-Item (Join-Path $run 'tp-stable.exe')).LastWriteTime
}
$clock = $stamp.ToString('HH:mm', [System.Globalization.CultureInfo]::InvariantCulture)
Write-Host "Built v$cargo_version #$count $hash$dirty at $clock" -ForegroundColor Magenta

$built = Join-Path $env:CARGO_TARGET_DIR $profile_dir
Get-Process tp-stable -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 400
Copy-Item (Join-Path $built 'tronic-printer.exe') (Join-Path $run 'tp-stable.exe') -Force
Copy-Item (Join-Path $built 'tronic-print.exe') (Join-Path $run 'tronic-print.exe') -Force

# Windows keeps a console-less app quiet: whatever it says goes to run\stderr.txt.
Start-Process -FilePath (Join-Path $run 'tp-stable.exe') -RedirectStandardError (Join-Path $run 'stderr.txt')
Write-Host "Opened run\tp-stable.exe ($profile_dir). What to look at: run\QA.md" -ForegroundColor Green
