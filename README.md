# Tronic Printer

Print from Windows on the **Lidl Tronic / Silvercrest "Mini Pocket Printer"** (model 5890, firmware
`V1.06LY`) over Bluetooth LE. A small Rust app with a [Rune](https://github.com/anomalyco/rune) GPU
interface: text, pictures, QR codes and barcodes, a picture gallery that prints by itself, terminal
output, and an MCP server so an AI assistant can print too.

The Bluetooth protocol comes from [dimer47/PocketPrinter5890](https://github.com/dimer47/PocketPrinter5890)
and was checked on a real unit.

## Build and run

You need Rust and the *Build Tools for Visual Studio* (C++ workload). Then:

```powershell
cargo build --release
.\target\release\tronic-printer.exe
```

Use the release build for daily use: the debug build is much slower (sliders, effects). The build
makes three programs next to each other in `target\release`:

| Program | What it is |
|---|---|
| `tronic-printer.exe` | the app (window) |
| `tronic-print.exe` | prints terminal output through the running app |
| `tronic-mcp.exe` | MCP server so AI assistants can print |

Switch the printer on, close its phone app (it only talks to one device at a time) and open the
app: it connects by itself. Windows 11 is recommended (dark title bar, round corners).

## In the app

- **Text**: five fonts, four sizes, alignment, white on black, and "Surprise me". Or pick a **theme**
  (below): the same text as a shop receipt, a parchment scroll, a boarding pass…
- **Image**: pick a folder as a gallery (scrolls, thumbnails), click a picture, or **drag it onto the
  printer** to print. Look (Photo, Meme, Line art, Soft), brightness and contrast sliders, rotate,
  flip, invert. "Print new pictures automatically" prints pictures you add to the folder; pictures
  dropped on the window from Explorer print at once.
- **Codes**: QR, Code 128, Code 39, EAN-13.
- **Terminal**: see below.
- A live preview of the exact black and white ticket, darkness, copies. The **FX** switch turns the
  GPU effects (fire, plasma, smoke, ghost paper) on or off; it starts off when Windows animations
  are disabled.

## Writing themes

A theme dresses your text as a document, drawn for the printer's 384 black-and-white dots: fonts,
borders, rules, stamps, barcodes, speckle. Pick one in the Text tab (the preview shows it as you
type); themes with details (a store name, a seat number) show a small form. The **gallery** shows
a small picture of every ticket: click one to use it, or pull it onto the printer to print your text
in that theme.

| Family | Themes (id, details) |
|---|---|
| Shop & diner | `receipt` (store, address, date: one `Item 2.50` per line, the total is added), `diner` (place, table, server), `pharmacy` (patient, doctor, refills), `coupon` (offer, code, expires), `fine` (offender, amount, number) |
| Travel tickets | `boarding` (from, to, name, seat), `flight` (same, lying sideways like a real boarding pass with a tear-off stub), `train-old` (the classic card ticket with its magnetic strip: from, to, date, seat; the text is the passenger), `train` (the e-ticket with a QR code: same fields) |
| Parchment & old world | `parchment` (title, signed), `decree` (title, signed), `wanted` (name, reward), `treasure` (place) |
| Fun official papers | `movie` (room, seat, time), `fortune`, `diploma` (name, award, signed), `horoscope` (sign), `fridge`, `checklist` (title), `ransom`, `name-tag` |
| Techy & retro | `terminal` (prompt), `dot-matrix` (title), `label`, `telegram` (to), `punch-card`, `poster` |

Lines starting with `- `, `[ ]` or `[x]` become tick boxes in `checklist` and `fridge`. Fonts come
from Windows; a theme falls back to a plainer font if one is missing.

```powershell
'Coffee 2.50', 'Croissant 1.80' | tronic-print --theme receipt --field store="CAFE LUNE"
'Dig where the shadow ends' | tronic-print --theme treasure --field place="Skull Island"
```

An assistant uses `print_text` with `theme` and `fields`, e.g. *"print a boarding pass from LYS to
BCN for Alex"*.

## From a terminal

Keep the app open and connected.

```powershell
dir | tronic-print                        # print what arrives, once
tronic-print -- git log --oneline -5      # run a command and print its output
tronic-print --live -- cargo build        # print as the output arrives
tronic-print --force ...                  # allow more than 100 cm for one print
tronic-print --theme receipt --field store=SHOP   # dress the text as a document (see Writing themes)
```

## From an AI assistant (MCP)

`tronic-mcp.exe` is an [MCP](https://modelcontextprotocol.io) server over stdio. It talks to the
running app, so **keep Tronic Printer open and connected** (the light is green).

**Tools**

| Tool | What it does |
|---|---|
| `print_text` | text, with `font`, `size`, `align`, `invert`, `copies`, or a `theme` (receipt, parchment, boarding…) with its `fields` |
| `print_code` | QR code or barcode (`type`: qr, code128, code39, ean13), with `caption`, `error_correction` |
| `print_image` | a picture file on this PC (`path`), with `look`, `brightness`, `contrast`, `rotate`, `flip`, `invert` |
| `printer_status` | connected or not, battery, model, firmware, busy |

Anything over 100 cm of paper needs `force: true`, to protect the roll.

`tronic-mcp --config` prints these instructions with the right paths for your machine, and the app's
**Terminal** tab has buttons that copy the command and the JSON.

### Claude Code

```powershell
claude mcp add tronic-printer -- "C:\path\to\tronic-mcp.exe"
```

Check with `claude mcp list`, then ask: "print a QR code for https://example.com".

### Claude Desktop

Settings > Developer > Edit Config, add to `claude_desktop_config.json`, then restart Claude Desktop:

```json
{
  "mcpServers": {
    "tronic-printer": {
      "command": "C:\\path\\to\\tronic-mcp.exe",
      "args": []
    }
  }
}
```

### Cursor, Windsurf, VS Code, Cline and others

Add a stdio MCP server with the same JSON (the file is usually `mcp.json`, or the `mcp` section of the
settings), or give the host the command `C:\path\to\tronic-mcp.exe` with no arguments.

### Check it by hand

```powershell
'{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"printer_status","arguments":{}}}' | .\tronic-mcp.exe
```

## Troubleshooting

- **"the Tronic Printer app is not running"**: open the app and wait for the green light.
- **Printer not found**: switch it on, close its phone app, turn the phone's Bluetooth off, and keep
  it within a metre or two of the PC (the Wi-Fi/Bluetooth antenna must be plugged in on a desktop).
- **Nothing prints but it answers**: check the paper (thermal side facing the head) and the lid.

## Developing

```powershell
cargo test                                  # unit tests for every part
cargo run -- --snap out.png --scene text    # render the window headless; scenes: text, online,
                                            # gallery, drag, printing, ghost, done, egg, error, qr,
                                            # image, terminal, theme, connecting (add --at SECONDS)
.\tools\make_icons.ps1                      # rebuild the logo files (needs Edge and Pillow)
```

The Bluetooth protocol, the printer's quirks and the named-pipe format are documented at the top
of `src/protocol.rs`, `src/ipc.rs` and `src/jobspec.rs`.
