# Domain: ui

## Owns
- `src/main.rs` — setup, poll loop, snapshot scenes
- `src/ui.rs` — every view (Rune `fn(&S, &mut Cx<S>) -> El`)
- `src/style.rs` — colours and the Rune Kit
- `src/chrome.rs` — window chrome (Windows only)
- `src/voice.rs` — all microcopy
- editing state in `src/app.rs` (shared with print-path)

## Does not own
- `src/ble.rs`, `src/protocol.rs`, printing and connection logic in `src/app.rs` → print-path
- `src/render/**`, `themes/**` → render-themes
- `src/mcp*.rs`, `src/bin/**` → mcp-cli

## Invariants
- The user is the QA; a visual change ends with `.\tools\qa.ps1` and a French line in `run\QA.md`.
- Snapshot without a window: `tronic-printer.exe --snap out.png --scene NAME`; scenes are in `src/main.rs`.
  Snapshots never start Bluetooth scanning.
- Humour is welcome in microcopy; no AI-ish filler, sparkles or gradients.
- Ctrl+P / Cmd+P prints, including while a field is focused.
- Long tickets scale to fit ("fit NN%"); clicking a writable part returns to 100%.
- Keep Windows working.

## Known traps
- `src/ui.rs` is now `src/ui/mod.rs` + `src/ui/ghost.rs` + `src/ui/write.rs`; layout work is one agent at a time.
- **`memo::layout*` answers `None` outside a view build** (`rune/src/memo.rs:438`, `read_outside_cx` at 475): never read a
  layout from an event closure (`on_press`, `on_drag`, `on_click`). Pass the size in from the view build instead — the ghost's
  drag was dead for exactly that reason (`63d5e33`).
- `Drag::x/y` are local to the element: an element that follows the pointer never sees them change. Use `Drag::dx/dy`
  (travel since the press, in window points).
- The drawer holds galleries only; settings live on a strip under the stage (`a0f6a95`). Do not put them back.
- `src/main.rs` is also touched by mcp-cli wiring: serialize.
- `src/app.rs` is the serialization point with print-path.
- `tools/qa.ps1` stamps the version **before** the build; reading git after it can name a commit the binary lacks.

## Proof
- `cargo test`
- `$env:CARGO_TARGET_DIR="target\mine"; cargo build`
- `.\tools\qa.ps1`
- PNG path for each touched scene
- `run\QA.md` updated in French + explicit "not verified on screen" list

## Open threads
- Tray icon + global hotkey (print clipboard / screenshot) — not started (TP-03).
