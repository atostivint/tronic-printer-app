"""Rebuilds every icon file from assets/icon.svg: Edge headless renders 16/32/48/96/256 px PNGs
(transparent), then Pillow writes icon.ico and logo-96.png (the header logo). Needs Edge + Pillow."""
import subprocess, tempfile, os, time
from pathlib import Path
from PIL import Image

ASSETS = Path(__file__).resolve().parent.parent / "assets"
EDGE = os.path.expandvars(r"%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe")
SIZES = (16, 32, 48, 96, 256)
svg = (ASSETS / "icon.svg").resolve().as_posix()
tmp = Path(tempfile.mkdtemp())
for s in SIZES:
    html = tmp / f"i{s}.html"
    html.write_text(f"<!doctype html><body style='margin:0;background:transparent'><img src='file:///{svg}' width='{s}' height='{s}' style='display:block'>", encoding="utf8")
    subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--default-background-color=00000000",
                    f"--screenshot={ASSETS / f'icon-{s}.png'}", f"--window-size={s},{s}", f"file:///{html.as_posix()}"],
                   check=True, capture_output=True)
time.sleep(1)
ims = {s: Image.open(ASSETS / f"icon-{s}.png").convert("RGBA") for s in SIZES}
ims[256].save(ASSETS / "icon.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)],
              append_images=[ims[s] for s in (16, 32, 48, 96)])
ims[96].save(ASSETS / "logo-96.png", optimize=True)
print("wrote icon-*.png, icon.ico, logo-96.png")
