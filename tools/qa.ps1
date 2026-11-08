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

# What the line after the launch claims, and nothing the run cannot prove. A build: the label is
# worked out ONCE, before it starts, and reused after it. build.rs stamps the binary from git at
# compile time, so a git read made once the build has finished can name a commit the binary does
# not contain (the tree moved while it compiled); one value, printed twice, cannot disagree with
# itself. Git may be absent (an unpacked source tree): "unknown" then, never an error.
$source = $null
if (-not $NoBuild) {
    $cargo_version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1)
    $cargo_version = if ($cargo_version) { $cargo_version.Matches[0].Groups[1].Value } else { '0.0.0' }
    $hash = try { (git rev-parse --short HEAD 2>$null) } catch { $null }
    if (-not $hash) { $hash = 'unknown' }
    $count = try { (git rev-list --count HEAD 2>$null) } catch { $null }
    if (-not $count) { $count = '0' }
    $dirty = try { if (git status --porcelain 2>$null) { '*' } else { '' } } catch { '' }
    $label = "v$cargo_version #$count $hash$dirty"

    $flags = @('build', '--bins')
    if (-not $Debug) { $flags += '--release' }
    Write-Host "Building $label..." -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'build.ps1') @flags
    if ($LASTEXITCODE -ne 0) { throw "The build failed, nothing was opened. Fix the errors above." }
    $source = "built from $label"
}

# -NoBuild compiles nothing, and the tree's git state says nothing about the executable about to
# open: the file itself carries only the crate version (no commit, see VersionInfo), so name what
# proves it, its write time, and leave the commit to the window, where build.rs baked it.
if ($NoBuild) {
    $written = (Get-Item (Join-Path $run 'tp-stable.exe') -ErrorAction SilentlyContinue).LastWriteTime
    if ($written) {
        $clock = $written.ToString('HH:mm', [System.Globalization.CultureInfo]::InvariantCulture)
        Write-Host "Opening run\tp-stable.exe, written $clock (its own stamp is in the window, bottom-right)" -ForegroundColor Magenta
    }
}

$built = Join-Path $env:CARGO_TARGET_DIR $profile_dir
Get-Process tp-stable -ErrorAction SilentlyContinue | Stop-Process -Force
# Stop-Process returns before Windows lets go of the file: wait for the process to really end, then
# the retries below cover whatever still holds the executable (a scanner, a slow handle release).
try { Get-Process tp-stable -ErrorAction SilentlyContinue | Wait-Process -Timeout 5 -ErrorAction Stop } catch {}
Start-Sleep -Milliseconds 400
$copied = $false
foreach ($attempt in 1..3) {
    try {
        Copy-Item (Join-Path $built 'tronic-printer.exe') (Join-Path $run 'tp-stable.exe') -Force -ErrorAction Stop
        Copy-Item (Join-Path $built 'tronic-print.exe') (Join-Path $run 'tronic-print.exe') -Force -ErrorAction Stop
        $copied = $true
        break
    } catch { Start-Sleep -Milliseconds 300 }
}
if (-not $copied) { throw "Could not replace run\tp-stable.exe (still in use, or not built?): nothing was opened." }

# Windows keeps a console-less app quiet: whatever it says goes to run\stderr.txt.
Start-Process -FilePath (Join-Path $run 'tp-stable.exe') -RedirectStandardError (Join-Path $run 'stderr.txt')
$origin = if ($source) { " - $source" } else { '' }
Write-Host "Opened run\tp-stable.exe ($profile_dir)$origin. What to look at: run\QA.md" -ForegroundColor Green
