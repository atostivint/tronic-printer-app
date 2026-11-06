"""Draws the Tronic Printer logo (assets/logo.svg).

The mark is the printer as it sits on a desk: a white pebble with a steel tear bar and a green
light, and a ticket standing out of it with a toothed cut edge and a few lines of print, on a
coral-to-pink tile. Run `tools/make_icons.ps1` afterwards to produce the PNG and ICO files.
"""

from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "assets" / "logo.svg"

# The ticket: 80 wide, standing on the steel bar, cut edge toothed.
PAPER_L, PAPER_R, PAPER_TOP, PAPER_BOTTOM = 88, 168, 52, 150
TOOTH = 5  # half a tooth, in x


def paper_path() -> str:
    pts = [(PAPER_L, PAPER_TOP)]
    x, up = PAPER_L, False
    while x < PAPER_R:
        x += TOOTH
        pts.append((x, PAPER_TOP - 8 if not up else PAPER_TOP))
        up = not up
    pts += [(PAPER_R, PAPER_BOTTOM), (PAPER_L, PAPER_BOTTOM)]
    return "M" + " L".join(f"{x} {y}" for x, y in pts) + " Z"


SVG = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ff6a3d"/>
      <stop offset="1" stop-color="#ff3d8b"/>
    </linearGradient>
    <linearGradient id="body" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#ffffff"/>
      <stop offset="1" stop-color="#d8d5df"/>
    </linearGradient>
    <linearGradient id="steel" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#f1f1f5"/>
      <stop offset="1" stop-color="#8d8d98"/>
    </linearGradient>
    <linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#ffffff" stop-opacity="0.28"/>
      <stop offset="0.5" stop-color="#ffffff" stop-opacity="0"/>
    </linearGradient>
    <filter id="drop" x="-20%" y="-20%" width="140%" height="160%">
      <feDropShadow dx="0" dy="7" stdDeviation="6" flood-color="#3a0a1f" flood-opacity="0.35"/>
    </filter>
    <filter id="glow" x="-100%" y="-300%" width="300%" height="700%">
      <feGaussianBlur stdDeviation="4"/>
    </filter>
  </defs>

  <rect width="256" height="256" rx="58" fill="url(#tile)"/>
  <rect width="256" height="128" rx="58" fill="url(#sheen)"/>

  <!-- the ticket -->
  <g filter="url(#drop)">
    <path d="{paper_path()}" fill="#fbf6ea"/>
  </g>
  <g fill="#2a2233">
    <rect x="98" y="72" width="52" height="7" rx="3.5"/>
    <rect x="98" y="88" width="38" height="7" rx="3.5"/>
    <rect x="98" y="104" width="58" height="7" rx="3.5"/>
    <rect x="98" y="120" width="30" height="7" rx="3.5" fill="#ff4f6a"/>
  </g>

  <!-- the printer -->
  <rect x="64" y="143" width="128" height="11" rx="5.5" fill="url(#steel)"/>
  <g filter="url(#drop)">
    <rect x="50" y="151" width="156" height="62" rx="28" fill="url(#body)"/>
  </g>
  <rect x="115" y="190" width="26" height="8" rx="4" fill="#3ddc97" filter="url(#glow)"/>
  <rect x="115" y="190" width="26" height="8" rx="4" fill="#3ddc97"/>
</svg>
"""

if __name__ == "__main__":
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(SVG, encoding="utf-8")
    print(f"wrote {OUT}")
