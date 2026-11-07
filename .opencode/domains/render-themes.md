# Domain: render-themes

## Owns
- `src/render/**` — the 1-bit Bitmap pipeline: `text.rs`, `picture.rs`, `dither.rs`, `codes.rs`,
  `draw.rs`, `theme.rs`, `theme/*.rs`
- `themes/**` — user JSON theme files and their README
- the `jobspec::THEMES` entry for each theme (`jobspec.rs` is shared with print-path)

## Invariants
- Everything renders into a 1-bit `Bitmap` 384 dots wide.
- Adding a theme touches, in this order: enum variant, `BUILTIN` (+ array length), `id`, `name`,
  `tagline`, `family`, `fields` (at most `MAX_FIELDS` = 5), `sample`, the `render` match, and
  `jobspec::THEMES`. A test checks the two lists match.
- A theme is repeatable: same text, same ticket (`Ctx::seed`).
- Empty shop fields use the hint; silly forms (`forms.rs`) leave a blank line to write on and never
  tick boxes by themselves.
- Writable parts use `Page::begin(key)` / `Page::end()`; `render_mapped` returns the regions.
- App-only settings travel as reserved field keys starting with `_` (`_shuffle`, `_closing`, `_editing`).
- Every built-in theme draws its sample in a test; add a test for any new behaviour.

## Known traps
- `jobspec.rs` is shared with print-path: serialize.
- The working tree currently has untracked `render/theme/banner.rs` and `render/theme/effects.rs`
  (in-flight effects work). Do not edit or delete them without asking.
- Theme sheets: `$env:THEME_SHEETS="$PWD\target\sheets"; cargo test sheets -- --ignored`.

## Proof
- `cargo test`
- theme sheets for the touched families
- `--snap out.png --scene theme` / `form` and look at the PNG
- `run\QA.md` updated in French

## Open threads
- Custom user theme regions (`"region": "<field key>"`) — TP-06, branch orch-custom-regions.
- Printed-ticket textures (halftone, coffee ring, stamp, perforation, dog-ear) — TP-07,
  branch orch-ticket-effects.
