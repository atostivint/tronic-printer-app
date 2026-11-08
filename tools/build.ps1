<#
  Run cargo, one build at a time on this machine.

    .\tools\build.ps1 check
    .\tools\build.ps1 test
    .\tools\build.ps1 build --release --bins

  Several agents share this PC. The build waits for any other build started through this script
  (a machine-wide mutex), runs at BelowNormal priority so the desktop stays usable, caps the job
  count at 8, and refuses to start when less than 1.5 GB of RAM is free.
#>
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs)

if (-not $CargoArgs) { throw 'Nothing to build: pass cargo arguments, e.g. .\tools\build.ps1 check' }

# Pushing a build into a machine that is already out of RAM is what froze it, not the CPU: stop.
$free_kb = (Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory
if ($free_kb -lt 1.5MB) {
    Write-Host ('Not enough free memory ({0:N1} GB): close something before building.' -f ($free_kb / 1MB)) -ForegroundColor Red
    exit 3
}

if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '8' }
# Some terminals refuse a priority change; a slow build beats a failed one.
try { (Get-Process -Id $PID).PriorityClass = 'BelowNormal' } catch {}

# A cargo started outside this script does not hold the lock: warn, and let the user decide.
$busy = @(Get-Process cargo -ErrorAction SilentlyContinue).Count
if ($busy) { Write-Host "Another cargo is already running ($busy): expect a slow machine." -ForegroundColor Yellow }

# Global if the session allows it, Local otherwise: the lock only has to span this user's agents.
$lock = try { New-Object System.Threading.Mutex($false, 'Global\TronicPrinterCargoBuild') }
        catch { New-Object System.Threading.Mutex($false, 'Local\TronicPrinterCargoBuild') }
$code = 1
$held = $false
try {
    if (-not $lock.WaitOne(0)) {
        Write-Host 'Waiting for another build to finish...' -ForegroundColor Yellow
        $lock.WaitOne() | Out-Null
    }
    $held = $true
    & cargo @CargoArgs
    $code = $LASTEXITCODE
} finally {
    if ($held) { $lock.ReleaseMutex() }
    $lock.Dispose()
}
exit $code
