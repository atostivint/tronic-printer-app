# User theme scripts

Optional: a page-layout user theme can generate its `steps` with a small Rhai script instead of listing them in JSON.

```json
{
  "name": "Ticket dynamique",
  "layout": "page",
  "script": "let out = []; let i = 0; while i < 3 { out.push(#{ op: \"text\", text: \"Ligne \" + i.to_string() }); i += 1; } out"
}
```

The script **does not draw**. It **returns the same list of steps** as the JSON `"steps"` array. Rendering is unchanged (same step kinds and fields as `themes/README.md` / page layout): `text`, `band`, `shout`, `leader`, `rule`, `flourish`, `gap`, `barcode`, `code`, `each`, with the same properties (use `kind` for a code step's kind: `qr`, `code128`, `code39`, `ean13`).

## Evaluation

- Runs **once at load time** (not on every render). The produced steps are reused like a normal theme.
- Sandboxed and deterministic: evaluated with Rhai's standard library (strings/arrays/maps/math) and capped at **100,000 operations**. No extra packages are registered, so there is no filesystem, network, OS, or process API available.
- Deterministic: do not use date/time/randomness in the script (there is none available). Use template holes like `{today}` or `{number:8}` in the generated text instead.
- A script error or a return value that is not a list of steps is reported clearly when loading the theme (the theme is skipped, never a panic). Errors show up in the theme report / under **My themes**.

## Example

See `script-example.json` in this folder for a minimal theme that renders three lines.