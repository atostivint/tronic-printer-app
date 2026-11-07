# Handoff (snapshot of 2026-11-07, evening)

Read `AGENTS.md` first (rules, build, code map). This file is the state of the work and the backlog.
Update it when you finish something; keep it short and true.

## Repository state
- Branch in the main tree: `feat/fx-revamp`, commit `588972a`, working tree clean. Nothing was pushed.
  The user decides when to commit or push.
- Other branches/worktrees:
  - `worktree-agent-af3a4dfaaf948b889` — XBackBone client (committed, 10 unit tests).
  - `worktree-agent-a626cda8a697e9db5` — Linux/macOS port (committed: "Make the app build and run on Linux and macOS";
    nothing has run on Linux/macOS yet).
  - Transient free-model worktrees under `%LOCALAPPDATA%\opencode\worktree\...`: `orch-live-gap`, `orch-custom-regions`,
    `orch-docs` (see "In flight"). They are disposable; merge by branch, never by copying files.
- Tests: 159 in the lib (156 pass, 3 ignored), plus 16 and 3 in the other targets. All green on 2026-11-07.

## What works (verified by snapshots on 2026-11-07)
- Printing on real hardware: text, pictures (4 dithers), QR/barcodes, terminal one-shot and live tail, themes (about 40),
  auto-connect to the closest printer, battery/model/firmware (paper status is raw hex), CLI `tronic-print`, MCP (stdio + HTTP).
- 7 colour palettes with the chip selector (default Discount), Ctrl+P/Cmd+P Print shortcut, in-place writing on the preview,
  image "white space before" (0-50 mm, `space_before_mm` in the image job and MCP), quieter Print glow.
- Terminal tab: the MCP port status and the copy buttons now fit on one line ("Port busy: pick another", "Copy command", "Copy JSON").
- Effects: each palette has a slow background weather (aurora for Discount/Night/Christmas/Halloween/Matrix, crawling grain for
  Professional/Terminal); the FX switch turns all of it off. The line under TRONIC types itself out and changes every minute; the
  ghost changes every 15 minutes (or on a click), has a receipt face with a barcode, and its eyes follow the pointer.
- Long tickets are scaled to fit the stage whole (the title says "fit NN%"); clicking a part to write on brings it back to 100%.
- The butthurt form has a Provider line as well as the model (form, ticket and job fields).
- Drag and drop: a picture dropped on the window, or dragged from the gallery onto the preview, is only shown; Print does the rest.
- Silly forms (`render/theme/forms.rs`): 11 themes (while-out, permission, incident, passive, pep, excuse, complaint, apology, iou, review,
  butthurt) that print blank to write on by hand; `boxes` and `footer` fields; Shuffle; closing line switch; horoscope with custom/dumb signs.
- Theme gallery and picture gallery at the bottom of the preview, scrollable; the printer shrinks for long tickets and the ticket scrolls.
- Release build + `tools/qa.ps1`; `run\QA.md` is the French checklist for the user.

## In flight (free-model agents, branches off `feat/fx-revamp`)
1. `orch-live-gap` — remove the residual gap between live terminal batches (tight vertical margin for live batches only).
2. `orch-custom-regions` — let user JSON themes declare writable regions (`"region": "<field key>"`) instead of the fallback bar.
3. `orch-docs` — refresh `README.md` (deleted `tools/make_*` scripts, palettes, Ctrl+P, space-before, forms, QA script). Done, to merge.
4. `orch-ticket-effects` — printed-ticket textures (halftone, coffee ring, rubber stamp, perforation, dog-ear) on a few themes.
5. Three read-only reviewers on free models: bugs (`beam-free`), UI/UX (`glm-5.3-flash-free`), performance (`iceman-free`).
Each agent has its own worktree and `CARGO_TARGET_DIR`; none may commit, push or print. To review: diff the branch, run `cargo test`,
then merge or ask the user. Update this file when they report.

## Backlog, in the user's order of interest
- **XBackBone** (self-hosted ShareX-style upload service). Client module is done on its branch. Facts: upload works with a token on both
  3.x (`POST /upload`, fields `token` + `file`) and the new `master` API (`POST /api/v1/upload`, `Authorization: Bearer`); **listing uploads is
  impossible with a token** (HTML behind a session). Plan: settings (server URL, token in Windows Credential Manager), gallery source made of
  remembered/pasted share URLs, click to download and print; optional "upload this picture". Blocking calls must run on a thread. Waiting for the user's yes.
- **Linux and macOS**: branch above. Still needs: CI workflow (ubuntu, macos, windows), macOS Bluetooth permission
  (`NSBluetoothAlwaysUsageDescription`, .app bundle), and a real run on both systems.
- Tray icon + global hotkey (print clipboard / screenshot): not started.
- Paper status reply of `10 FF 40` is undocumented: find its format (needs the printer with and without paper) to show "paper OK / empty".
- Residual small gap between live terminal batches: being fixed on `orch-live-gap`.
- Custom user themes (`themes/` folder files) cannot use in-place regions yet: being fixed on `orch-custom-regions`.
- Old docs may mention the deleted `tools/make_*` scripts: the single script is now `tools/make_icons.py` (renders `assets/icon.svg` with Edge headless + Pillow).

## Working with OpenCode and free models
- OpenCode reads `AGENTS.md` automatically; `CLAUDE.md` just imports it. `opencode.json` sets DeepSeek as the default model.
- Free models available: `opencode/*-free` and `opencode-go/*-free` (see `tools.opencode.models` in OpenCode). Give each agent:
  its own worktree, its own `CARGO_TARGET_DIR`, a short brief with the exact files, tests to run, and the hard rules.
- Good tasks for cheaper/free models: theme drawing code (`render/theme/*.rs`, tests and sheets), docs, small self-contained bugs.
  Keep UI layout work (`ui.rs`) with one agent at a time and review its snapshots.
- Hard rules for every agent: no printing on the real printer without asking, no commit/push without asking, no secrets in files.
