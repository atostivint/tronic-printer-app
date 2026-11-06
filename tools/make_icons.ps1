# Rebuilds every logo file from tools/make_logo.py: the SVG, a 512 px PNG (rendered by Edge, so
# it needs no extra software), then icon.ico and the small PNG the app shows. Needs Pillow:
#   python -m pip install pillow
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
python "$root\tools\make_logo.py"

$edge = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe"
$svg = (Resolve-Path "$root\assets\logo.svg").Path -replace '\\', '/'
$html = Join-Path $env:TEMP 'tronic-logo.html'
"<!doctype html><html><body style='margin:0;background:transparent'><img src='file:///$svg' width='512' height='512' style='display:block'></body></html>" | Set-Content $html -Encoding utf8
& $edge --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 `
    --screenshot="$root\assets\logo-512.png" --window-size=512,512 "file:///$($html -replace '\\', '/')" | Out-Null
Start-Sleep 2

python "$root\tools\make_ico.py"
