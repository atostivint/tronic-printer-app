<#
  Keep the QA copy in step with main: when main moves, build release and reopen the window.

    .\tools\watch.ps1           poll main every 5 s; build + reopen after each new commit (15 s debounce)
    .\tools\watch.ps1 -Once     check once, exit: build only if main moved since the last recorded hash
    .\tools\watch.ps1 -Force    build and reopen now, whatever main says

  Read-only on the repository: only rev-parse runs, the working tree is never switched or reset.
  One line per build goes to run\watch.log; the last built hash lives in run\watch.hash.
#>
param([switch]$Once, [switch]$Force)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$run = Join-Path $root 'run'
$log = Join-Path $run 'watch.log'
$state = Join-Path $run 'watch.hash'
$qa = Join-Path $PSScriptRoot 'qa.ps1'
$poll = 5
$debounce = 15   # a burst of merges should build once, not five times
New-Item -ItemType Directory -Force $run | Out-Null

function Get-MainHash {
    # rev-parse only reads a ref; git absent (unpacked tree) gives "unknown", never an error.
    $h = git -C $root rev-parse main 2>$null
    if ($h) { "$h".Trim() } else { 'unknown' }
}

function Get-Recorded { if (Test-Path $state) { (Get-Content $state -Raw).Trim() } else { '' } }
function Short([string]$h) { if ($h -and $h.Length -ge 7) { $h.Substring(0, 7) } else { $h } }

# Build through qa.ps1 (build.ps1: one build at a time, jobs 8, BelowNormal) and reopen the app.
# Run as a child process for one reason: qa.ps1 speaks with Write-Host, which a parent script
# cannot capture; a child's console output can, so the "Built v..." line reaches the log.
function Invoke-Qa([string]$old, [string]$new) {
    Write-Host "main moved $(Short $old) -> $(Short $new): building..." -ForegroundColor Cyan
    $out = & (Get-Process -Id $PID).Path -NoProfile -ExecutionPolicy Bypass -File $qa 2>&1
    $code = $LASTEXITCODE
    $out | Where-Object { "$_" -ne '' } | ForEach-Object { Write-Host "$_" }
    $built = ($out | Where-Object { "$_" -match '^Built v' } | Select-Object -Last 1)
    Add-Content -LiteralPath $log -Value ('{0}  {1} -> {2}  exit {3}  {4}' -f
        (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'), (Short $old), (Short $new), $code, $built)
    Set-Content -LiteralPath $state -Value $new
    if ($code -ne 0) { Write-Host "Build failed (exit $code), logged in run\watch.log" -ForegroundColor Red }
    return $code
}

$hash = Get-MainHash
if ($Force) { exit (Invoke-Qa (Get-Recorded) $hash) }

$known = Get-Recorded
if (-not $known) {
    # Nothing to compare with: remember today's main instead of rebuilding what qa.ps1 just built.
    Set-Content -LiteralPath $state -Value $hash
    Write-Host "Nothing recorded yet: remembering $(Short $hash), nothing to do." -ForegroundColor Yellow
    exit 0
}
if ($Once) {
    if ($hash -eq $known) { Write-Host "main is up to date at $(Short $hash), nothing to do." -ForegroundColor Green; exit 0 }
    exit (Invoke-Qa $known $hash)
}

Write-Host "Watching main every $poll s from $(Short $hash). Ctrl+C to stop." -ForegroundColor Green
while ($true) {
    Start-Sleep -Seconds $poll
    if ((Get-MainHash) -eq $hash) { continue }
    Start-Sleep -Seconds $debounce
    $now = Get-MainHash
    Invoke-Qa $hash $now | Out-Null
    $hash = $now
}
