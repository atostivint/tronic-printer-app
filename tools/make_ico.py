"""assets/logo-512.png -> assets/icon.ico (16 to 256 px) and assets/logo-64.png for the UI."""

from pathlib import Path

from PIL import Image

ASSETS = Path(__file__).resolve().parent.parent / "assets"
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]

src = Image.open(ASSETS / "logo-512.png").convert("RGBA")
src.save(ASSETS / "icon.ico", sizes=[(s, s) for s in SIZES])
src.resize((96, 96), Image.LANCZOS).save(ASSETS / "logo-96.png", optimize=True)
print("wrote icon.ico and logo-96.png")
