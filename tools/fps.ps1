# How choppy is the window? Records a few seconds of it at 60 frames a second, hashes the frames,
# and prints the share of consecutive frames that came out identical. A frame that repeats itself
# is a frame the view did not redraw, so the higher the number, the choppier the animation reads.
#
#   .\tools\fps.ps1 -Name tp-stable          # the QA copy, 5 seconds
#   .\tools\fps.ps1 -Name tronic-printer     # a build from this tree
#   .\tools\fps.ps1 -Name tp-stable -Keep    # keep the frames under target\fps-frames
#
# The window must be moving: a ticket on the stage with the effects on (the ghost bobs beside it).
# On an idle window the number is near 100 %, which only says the app was resting, not that it
# stutters. Compare two builds on the same machine, in the same scene: only the pair means anything.
#
# The window is found by process name, not by title: two copies of the app carry the same title
# ("Tronic Printer · Connected · 100%"), and `-i title=` would grab whichever one it found first.
param(
    [Parameter(Mandatory)][string]$Name,
    [int]$Seconds = 5,
    [string]$Out = (Join-Path $PSScriptRoot "..\target\fps-frames"),
    [switch]$Keep
)
$ErrorActionPreference = "Stop"

$ffmpeg = "C:\Users\elekt\AppData\Local\Microsoft\WinGet\Links\ffmpeg.exe"
if (-not (Test-Path $ffmpeg)) { throw "ffmpeg not found at $ffmpeg" }

# Measure the window in real pixels: without this the shell reports virtualized coordinates and the
# crop lands somewhere else on a scaled display.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public struct RECT { public int Left, Top, Right, Bottom; }
public struct POINT { public int X, Y; }
public static class Win {
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("shcore.dll")] public static extern int SetProcessDpiAwareness(int value);
}
"@
try { [Win]::SetProcessDpiAwareness(2) | Out-Null } catch { }

$window = Get-Process -Name $Name -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $window) { throw "no window for the process '$Name' (is the app on screen?)" }
$handle = $window.MainWindowHandle
[Win]::ShowWindow($handle, 9) | Out-Null   # SW_RESTORE
# The window has to be the one on top of the stack or the capture records whatever is in front of
# it. `SetForegroundWindow` is refused to a background shell, so raise it as topmost and let it
# fall back at once: on top of the others, without taking the keyboard from whoever is typing.
[Win]::SetWindowPos($handle, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0040) | Out-Null
[Win]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 500
[Win]::SetWindowPos($handle, [IntPtr](-2), 0, 0, 0, 0, 0x0001 -bor 0x0002) | Out-Null

$rect = New-Object RECT
[Win]::GetClientRect($handle, [ref]$rect) | Out-Null
$origin = New-Object POINT
[Win]::ClientToScreen($handle, [ref]$origin) | Out-Null
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
if ($w -le 0 -or $h -le 0) { throw "the window of '$Name' has no size ($w x $h)" }

$Out = [System.IO.Path]::GetFullPath($Out)
if (Test-Path $Out) { Remove-Item $Out -Recurse -Force }
New-Item -ItemType Directory -Path $Out -Force | Out-Null

# Straight to PNG: no video codec in between, so two frames that differ by a pixel stay two frames
# in the count.
& $ffmpeg -hide_banner -loglevel error -y -f gdigrab -framerate 60 `
    -offset_x $origin.X -offset_y $origin.Y -video_size "${w}x${h}" `
    -i desktop -t $Seconds -f image2 (Join-Path $Out "f%05d.png")
if ($LASTEXITCODE -ne 0) { throw "ffmpeg could not record '$Name'" }

$frames = @(Get-ChildItem $Out -Filter "f*.png" | Sort-Object Name)
if ($frames.Count -lt 10) {
    throw "only $($frames.Count) frames of '$Name': the window never drew"
}

$previous = $null
$same = 0
foreach ($frame in $frames) {
    $hash = (Get-FileHash $frame.FullName -Algorithm SHA256).Hash
    if ($null -ne $previous -and $hash -eq $previous) { $same++ }
    $previous = $hash
}

$pairs = $frames.Count - 1
$share = 100.0 * $same / $pairs
"{0} ({1}): {2} frames in {3} s, {4} of {5} repeat the frame before, {6:N1} % identical" -f `
    $Name, $window.MainWindowTitle, $frames.Count, $Seconds, $same, $pairs, $share

if (-not $Keep) { Remove-Item $Out -Recurse -Force }
