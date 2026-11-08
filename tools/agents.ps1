<#
  What the parallel agents are doing, read from the disk: the git worktrees are the source of truth.
    .\tools\agents.ps1            one table, now
    .\tools\agents.ps1 -Watch     redraw every 2 s until Ctrl+C (one pass takes a few s on 20+)
  Read-only on purpose: only "worktree list", "status" and "diff --stat" run, with git's optional
  index lock off, so looking at 20 worktrees never touches one of them.
#>
param([switch]$Watch)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$fmt = '{0,-22} {1,-30} {2,5} {3,-11} {4,-52} {5}'

function Limit($text, $width) {
    if ($text.Length -gt $width) { $text.Substring(0, $width - 1) + '~' } else { $text }
}
function When($time) { if ($time) { $time.ToString('MM-dd HH:mm') } else { '-' } }

function Get-Row($path, $branch) {
    $name = if ($path -eq $root) { 'main' } else { Split-Path -Leaf $path }
    $dirty = @(git --no-optional-locks -C $path status --short 2>$null | Where-Object { $_.Trim() }).Count

    $srcTime = $null
    $src = Join-Path $path 'src'
    if (Test-Path -LiteralPath $src) {
        $srcTime = Get-ChildItem -LiteralPath $src -Recurse -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty LastWriteTime
    }

    $stat = '-'
    if ($dirty -gt 0) {   # a clean worktree has no unstaged diff, and a git spawn costs ~100 ms
        $stat = @(git --no-optional-locks -C $path diff --stat 2>$null) | Select-Object -Last 1
        if (-not $stat) { $stat = '-' }
    }

    $sketch = '-'
    $sketches = Join-Path $path 'run\sketches'
    if (Test-Path -LiteralPath $sketches) {
        $png = Get-ChildItem -LiteralPath $sketches -Filter *.png -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($png) { $sketch = "$($png.Name) $($png.LastWriteTime.ToString('MM-dd HH:mm'))" }
    }

    $fmt -f (Limit $name 22), (Limit $branch 30), $dirty, (When $srcTime), (Limit $stat 52),
        (Limit $sketch 33)
}

function Show-Agents {
    Write-Output ('worktrees as of {0}  (dirty = modified + untracked)' -f (Get-Date -Format 'HH:mm:ss'))
    Write-Output ($fmt -f 'NAME', 'BRANCH', 'DIRTY', 'SRC MTIME', 'LAST DIFF LINE', 'NEWEST SKETCH')

    $current = $null
    foreach ($line in (& git -C $root worktree list --porcelain)) {
        if ($line -like 'worktree *') {
            if ($current) { Get-Row $current.Path $current.Branch }
            $path = $line.Substring(9).Replace('/', '\').TrimEnd('\')
            $current = [pscustomobject]@{ Path = $path; Branch = 'detached' }
        } elseif ($line -like 'branch refs/heads/*') {
            $current.Branch = $line.Substring(18)
        }
    }
    if ($current) { Get-Row $current.Path $current.Branch }
}

if ($Watch) {
    while ($true) {
        try { Clear-Host } catch { }   # piped output has no console to clear
        Show-Agents
        Start-Sleep -Seconds 2
    }
} else {
    Show-Agents
}
