# Handoff (snapshot of 2026-11-07)

Read `AGENTS.md` first (rules, build, code map). This file is the state of the work and the backlog.
Update it when you finish something; keep it short and true.

## Repository state
- Branch in the main tree: `feat/fx-revamp` (last commit `bba0b92`). **A lot of work is not committed** (themes of silly forms, UI
  changes, new icon, palettes in progress). Nothing was pushed. The user decides when to commit.
- Other branches: `worktree-agent-af3a4dfaaf948b889` (XBackBone client, committed, 10 unit tests),
  `worktree-agent-a626cda8a697e9db5` (Linux/macOS port, in progress, worktree locked while its agent runs).
- Worktrees live in `.claude/worktrees/` (should be git-ignored: add `.claude/` to `.gitignore`).

## What works (verified)
- Printing on real hardware: text, pictures (4 dithers), QR/barcodes, terminal one-shot and live tail, themes (about 40),
  auto-connect to the closest printer, battery/model/firmware (paper status is raw hex), CLI `tronic-print`, MCP (stdio + HTTP).
- 145 unit tests pass; snapshots via `--snap`.
- Silly forms (`render/theme/forms.rs`): 11 themes (while-out, permission, incident, passive, pep, excuse, complaint, apology, iou, review,
  butthurt) that print blank to write on by hand; `boxes` and `footer` fields; Shuffle; closing line switch; horoscope with custom/dumb signs.

## In flight (done by a Claude subagent, to be reviewed by the user as QA; see `run\QA.md`)
Owner of `src/ui.rs`, `src/app.rs`, `src/main.rs`, `src/render/**`: the Claude UI subagent, until it reports. Queue:
1. Layout: themes/image gallery at the bottom, printer scaled down for long tickets, ticket scrolls beyond that (done, to verify by hand).
2. Static ghost bubble (no trembling), random phrases (done).
3. Writing on the preview for ALL themes, in place, no big card (asked after the user rejected the inline card). Fallback slim bar for themes that cannot be mapped.
4. Ctrl+P / Cmd+P prints, also while typing in a box.
5. "White space before" a picture (mm, 0 to 50), in the Image tab and the `image` job / MCP (`space_before_mm`).
6. Print button glow much weaker (it hid the controls above it).
7. Seven colour palettes with a selector, default "Discount" (blue/yellow/red parody, no logos): Discount, Night, Professional, Christmas, Halloween, Terminal, Matrix.
8. Release build for smoothness (the debug build was choppy) and fewer idle redraws (done, measure in a real window).

## Backlog, in the user's order of interest
- **XBackBone** (self-hosted ShareX-style upload service). Client module is done on its branch. Facts: upload works with a token on both
  3.x (`POST /upload`, fields `token` + `file`) and the new `master` API (`POST /api/v1/upload`, `Authorization: Bearer`); **listing uploads is
  impossible with a token** (HTML behind a session). Plan: settings (server URL, token in Windows Credential Manager), gallery source made of
  remembered/pasted share URLs, click to download and print; optional "upload this picture". Blocking calls must run on a thread. Waiting for the user's yes.
- **Linux and macOS**: branch above. Needs: Unix socket instead of the Windows named pipe, platform config dirs, cfg-gate `windows`/`chrome.rs`,
  cross-platform fonts (`render/text.rs`), CI workflow (ubuntu, macos, windows), macOS Bluetooth permission (`NSBluetoothAlwaysUsageDescription`, .app bundle).
  Nothing has run on Linux/macOS yet.
- Tray icon + global hotkey (print clipboard / screenshot): not started.
- Paper status reply of `10 FF 40` is undocumented: find its format (needs the printer with and without paper) to show "paper OK / empty".
- Residual small gap between live terminal batches.
- Custom user themes (`themes/` folder files) cannot use in-place regions yet; they use the fallback bar.
- Old docs may mention the deleted `tools/make_*` scripts: the single script is now `tools/make_icons.py` (renders `assets/icon.svg` with Edge headless + Pillow).

## Working with OpenCode and DeepSeek
- OpenCode reads `AGENTS.md` automatically; `CLAUDE.md` just imports it. `opencode.json` sets DeepSeek as the default model. Connect once with `/connect`
  (pick DeepSeek, paste your own API key: never put keys in the repo), then check the model list with `/models`.
- Good tasks to hand to a cheaper/other model: theme drawing code (`render/theme/*.rs`, tests and sheets), microcopy, README/docs, the XBackBone wiring
  once the user agrees, the Linux/macOS CI fixes. Keep UI layout work (`ui.rs`) with one agent at a time.
- Ground rules for every agent: own branch or worktree, own `CARGO_TARGET_DIR`, tests green, snapshot read, report what was not verified.
- Hard rules: no printing on the real printer without asking, no commit/push without asking, no secrets in files.
