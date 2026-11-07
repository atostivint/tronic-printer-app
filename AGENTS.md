# Tronic Printer: guide for any coding agent (Claude Code, OpenCode, DeepSeek, humans)

A desktop app that prints text, pictures, codes and "tickets" on the Lidl Tronic / Silvercrest 5890
"Mini Pocket Printer" (BLE name `Mini Pocket Printer_BLE`, model A2Y) over Bluetooth LE.
Rust 2024, GUI by the Rune framework (GPU, wgpu/winit), btleplug for Bluetooth, tokio on worker threads.
The current state of the work and the backlog are in `HANDOFF.md`. Read it second.

## The user and how to work with them
- Alexandre writes English and French. Answer in the language of the last message (mostly French), short and plain.
- The user is the QA: they must SEE every change. After a batch of changes run `.\tools\qa.ps1`
  (builds release, reopens the app) and update `run\QA.md` (French checklist of what to look at).
  Always say what you could NOT verify on screen.
- Never print on the physical printer on your own. Never commit or push unless asked. Commit messages
  of Claude end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`; other agents add their own trailer.
- Humour is welcome in the product (ticket texts, microcopy in `src/voice.rs`) but not "AI-ish" filler:
  no gratuitous closing lines, sparkles or gradients. The user dislikes anything that looks "vibe coded".
- Several agents may work at once. Do not edit a file another agent is editing: see "Working in parallel".

## Build, test, run (Windows, the main platform)
- **Run cargo from PowerShell**, not Git Bash (Git Bash's GNU `link` shadows the MSVC linker). MSVC Build Tools are installed.
- Use your own target folder so builds never fight: `$env:CARGO_TARGET_DIR="target\mine"` (the user's QA copy builds there too).
  `target\release` is locked by running `tronic-mcp.exe` processes: do not build there.
- Tests: `cargo test` (about 145 unit tests, no printer or network needed). Keep them green.
- Snapshot of the UI without a window: `tronic-printer.exe --snap out.png --scene NAME` (scenes in `src/main.rs`: text, online, gallery,
  drag, printing, ghost, ghost-online, connecting, done, egg, error, qr, image, terminal, theme, form, write...). Look at the PNG.
  Snapshots never start Bluetooth scanning.
- Sheets of all themes as PNG: `$env:THEME_SHEETS="$PWD\target\sheets"; cargo test sheets -- --ignored`.
- The app has no console window (`windows_subsystem = "windows"` always on Windows); Rune logs go to `run\stderr.txt` when started by `qa.ps1`.
- The QA copy is `run\tp-stable.exe` (+ `run\tronic-print.exe`); `run\` and `target\` are git-ignored.

## Printer protocol (verified on real hardware, in `src/protocol.rs` and `src/ble.rs`)
- Write only on characteristic FF02 (write-without-response). Notifications: FF01 (query replies), FF03 (credits `01 nn`, battery `02 bb 00`, ignore `AA 0D 0A`).
- Before every command send the activation `10 FF F1 03`, then 12 zero bytes in a separate write. Stop: `10 FF F1 45`.
- Raster `GS v 0`, 384 dots wide (48 bytes per line), bands of 24 lines, packets of 180 bytes, credit-based flow control (wait for FF03 credits).
- Density `10 FF 10 00 n` (0..2). Feed `1B 4A n` (80 dots is about 10 mm). Queries: model `10FF20F0`, firmware `10FF20F1`, battery `10FF50F1`, paper `10FF40` (reply format unknown, shown as raw hex).
- Paper is 48 mm = 384 dots, so 1 mm = 8 dots. The preview draws 1 dot as 1 point.
- The app auto-connects to the strongest-signal printer whose name starts with `Mini Pocket Printer`, retries every 12 s, and `Disconnect` stops auto-connect until `Connect`.

## Code map (src/)
- `main.rs` setup, poll loop, snapshot scenes. `app.rs` all state (`App`), connection logic, printing, editing state. `ui.rs` every view (Rune `fn(&S, &mut Cx<S>) -> El`). `style.rs` colours and the Rune Kit. `fx.rs` GPU effects slots. `chrome.rs` window chrome (Windows only). `voice.rs` all microcopy.
- `ble.rs` the Bluetooth worker (commands in, events out over std mpsc). `protocol.rs` bytes. `ipc.rs` + `client.rs` named pipe `\\.\pipe\tronic-printer` (header line then payload: `once`, `job`, `live`). `mcp.rs`, `mcp_http.rs`, `mcp_setup.rs`, `bin/tronic-mcp.rs` MCP server (stdio and HTTP at `127.0.0.1:7391/mcp`). `bin/tronic-print.rs` CLI (`dir | tronic-print --theme receipt --field store=SHOP`). `jobs.rs`, `jobspec.rs` structured jobs (the client list of themes lives in `jobspec::THEMES`).
- `render/` turns anything into a 1-bit `Bitmap` (384 wide): `text.rs` (fontdue), `picture.rs` + `dither.rs`, `codes.rs` (QR, Code128/39, EAN13), `draw.rs` (primitives), `theme.rs` (the writing themes and the `Page` builder), `theme/*.rs` (shop, world, travel, papers, retro, forms, custom).
- `term.rs`, `ansi.rs` terminal printing (one-shot and live tail). `gallery.rs` picture folder. `settings.rs` persisted settings. `update.rs` update notice. `egg.rs` easter eggs. `xbackbone.rs` (on branch `worktree-agent-af3a4dfaaf948b889`, not merged yet).
- `themes/` user theme files (JSON) and their README.

## Adding or changing a theme
Edit `src/render/theme.rs`: enum variant, `BUILTIN` (and its array length), `id`, `name`, `tagline`, `family`, `fields` (at most `MAX_FIELDS` = 5), `sample`, the `render` match, and `jobspec::THEMES` (a test checks the two lists match). Write the drawing in a `theme/*.rs` file with `Page`.
- A theme is repeatable: same text, same ticket (`Ctx::seed`). Empty field: shops use the hint; the silly forms (`forms.rs`) leave a blank line to write on, never tick boxes by themselves.
- Writable parts: `Page::begin(key)` / `Page::end()` record a region; `render_mapped` returns them so the UI can put a text box on the paper. App-only settings travel as reserved field keys starting with `_` (`_shuffle`, `_closing`, `_editing`).
- Test pattern: every built-in theme draws its sample (see `theme.rs` tests); add a test for any new behaviour.

## Style of change
- Small, tested, readable. Comments explain why. English in code and UI strings. No new dependency without a reason (check `Cargo.lock` first; `ureq 2` is already used for HTTP).
- Rune is pinned to one git revision in `Cargo.toml`; its source is in `%USERPROFILE%\.cargo\git\checkouts\rune-*`. Read it for the real API instead of guessing.
- Keep Windows working. Linux and macOS support is being added on a separate branch (see `HANDOFF.md`).

## Working in parallel (humans and agents)
- One branch or git worktree per agent (`git worktree add .claude/worktrees/NAME -b NAME`); the main tree is for the person who owns the UI files at that moment.
- Never run two agents on the same file. Ownership is written in `HANDOFF.md`. Merge by branch, never by copying files.
- Each agent builds in its own `CARGO_TARGET_DIR`. Report in French: what changed per file, how it was verified, what was not.
