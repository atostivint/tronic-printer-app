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
- `ui.rs` holds every view; layout work is one agent at a time.
- `src/main.rs` is also touched by mcp-cli wiring: serialize.
- `src/app.rs` is the serialization point with print-path.

## Proof
- `cargo test`
- `$env:CARGO_TARGET_DIR="target\mine"; cargo build`
- `.\tools\qa.ps1`
- PNG path for each touched scene
- `run\QA.md` updated in French + explicit "not verified on screen" list

## Open threads
- Tray icon + global hotkey (print clipboard / screenshot) — not started (TP-03).
