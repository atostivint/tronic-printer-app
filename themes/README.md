# Your own themes

A theme is one `.json` file in `%APPDATA%\TronicPrinter\themes\` (the **Open folder** button under
**My themes** in the app opens it). Drop a file in to add a theme, delete it to remove it, edit it
to change it: the app notices within a couple of seconds (or press **Reload**). A file with a
mistake is skipped and the problem is shown under **My themes**; the other themes keep working.
Copy the files in `examples/` to start. Files whose name starts with `_` or `.` are ignored.

Try themes without touching the real folder: set `TRONIC_THEMES_DIR` to another folder.

## The file

```json
{
  "id": "tea-room",              // required: letters, digits, dashes (CLI / MCP name)
  "name": "Tea room",            // shown in the gallery
  "tagline": "…",                // one line for the picker and MCP
  "family": "shop",              // shop | travel | old-world | papers | retro (default: My themes)
  "sample": "Earl Grey 3.20",    // example text, also used for the gallery picture
  "body_optional": false,        // true: printing an empty box still makes a ticket
  "fields": [                    // the little form, at most 5
    { "key": "room", "label": "Tea room", "hint": "The Gilded Cup" }
  ],
  "layout": "page",              // page (default) or sheet
  "steps": [ … ]
}
```

An `id` already used by a built-in theme or another file is refused.

### Text holes

Any text value can contain `{…}`: `{body}` the whole text, `{today}`, `{total}` the sum of the
prices found at the end of the lines, `{number:8}` a number made from the text (digits, optional
second number to get a different one), and `{key}` for each field (an empty field gives its
`hint`; a hint of `today` gives the date). Add `|upper` for capitals: `{room|upper}`.
Inside an `each` step: `{line}`, `{name}` and `{price}` (for `Coffee 2.50`: `Coffee` and `2.50`) and
`{index}`. Write `{{` and `}}` for literal braces.

### `"layout": "page"`: a stack of lines

`"margin"` (default 24) is the blank space left and right. `steps` run top to bottom:

| op | what | settings |
|---|---|---|
| `text` | wrapped text | `text`, `font`, `size`, `align` (left/center/right), `line_gap`, `break_anywhere` |
| `band` | white on a black band | `text`, `font`, `size`, `align` |
| `shout` | each line as large as fits | `text`, `font`, `size` (the maximum) |
| `leader` | `left ....... right` | `left`, `right`, `font`, `size` |
| `rule` | horizontal rule | `thick`, `space`, `dash` as `[on, off]` |
| `flourish` | rule with a diamond | |
| `gap` | blank rows | `rows` |
| `barcode` | Code 128 | `data`, `rows` |
| `code` | QR or barcode picture | `data`, `kind` (qr, code128, code39, ean13) |
| `each` | its `steps` once per non-empty line of the text | `steps` |

`"finish"` is an optional list drawn over the finished ticket: `border` (`inset`), `speckle`
(`count`, `big`), `ragged` (`max`), `zigzag` (`tooth`), `tractor` (`pitch`).

### `"layout": "sheet"`: a sideways ticket

Drawn lengthways on a `width` × `height` sheet (height at most 384, the paper's width), then turned
to print down the paper, like the boarding pass. Coordinates are in printer dots (203 per inch) from
the top-left; for `text`, `y` is the top of the letters. `steps`:

| op | settings |
|---|---|
| `rect` | `x0 y0 x1 y1 thick` |
| `fill` | `x0 y0 x1 y1` |
| `hline` / `vline` | `y x0 x1` / `x y0 y1`, `thick`, `dash` |
| `line` | `x0 y0 x1 y1 thick` |
| `text` | `text x y size font anchor` (left/center/right), `knockout` (white on black) |
| `bars` | `data x y h module` (Code 128) |
| `punch` | `x y r`: a round notch |
| `disc` | `x y r ring` |
| `round` | `radius`: round the corners of the whole sheet |

Fonts: sans, bold, mono, serif, script, old-english, stencil, impact, playbill, typewriter,
chancery, papyrus, ocr, marker, symbols.
