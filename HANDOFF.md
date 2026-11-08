# Handoff (snapshot of 2026-11-08, after the stage-first shell landed)

Read `AGENTS.md` first (rules, build, code map). This file is the state of the work and the backlog.
Update it when you finish something; keep it short and true.

## Repository state
- Published: `origin` = https://github.com/atostivint/tronic-printer-app (public), default branch `main`.
- `main` = `6115001` "Make every generated train ticket attribute editable", pushed. Local `main`, `origin/main` and the
  checked-out `feat/fx-revamp` all point at that commit and that tree: no code is waiting on a branch tip.
- Working tree clean apart from the two tracked `.orchestrator` notes (`inbox.md`, `registry.md`), which the orchestrator
  rewrites; nothing in `src/` is modified.
- History since the publish (`061f182`): Linux/macOS PR #2, palette on the native title bar, centred Print label, ghost
  beside the printer, left column (`52b10dd`), `Printing`/`JobKind` enums, greentext, ticket-as-form (`31b50d5`),
  stage-first shell (`b145feb`), train fields (`6115001`).
- Tests: 205 in the lib, green on `main`.
- No tag has been pushed, so `.github/workflows/release.yml` has never run.
- Worktrees under `.claude/worktrees/` (all off `main` unless said):
  - `fx-2` (`a8ecf05`) — per-family effects ported to the new top bar. Committed, builds, 203 tests green. **Not merged.**
  - `ghost-2` (`b145feb`; `src/app.rs`, `src/main.rs`, `src/ui.rs` modified, uncommitted) — ghost beside the ticket, partial.
  - `empty-body` (`31b50d5`; `src/app.rs`, `src/render/theme.rs` modified, uncommitted) — empty field keeps the ticket; 2 tests red.
  - `tagline-2` (`b145feb`, untouched) — tagline rising; brief TP-25 in `.orchestrator/inbox.md`.
  - `agent-a626cda8a697e9db5` (`666c6de`) — Linux/macOS, merged as `60292ce`, keep for reference.
  - `agent-af3a4dfaaf948b889` (`6661bea`) — XBackBone client, 10 unit tests, not merged.
  - **Disposable, do not merge or reuse**: `layout-a`, `layout-b`, `layout-c`, `ui-sketches`, `ui-layout`, `ui-state`,
    `paper-form`, `drawer`, `theme-greentext`, `fx-themes`, `ghost-wander`, `tagline-rise`, `train-fields`, `state-jobs`
    (merged into `main` or superseded by it).
  - `%LOCALAPPDATA%\opencode\worktree\ac7702\orch-*` are stale: live gap, custom theme regions and docs are merged;
    the ticket textures from `orch-ticket-effects` are in `src/render/theme/effects.rs`. Do not merge them again.

## What works (verified by snapshots and tests on 2026-11-08)
- One top bar and a stage: brand, connection chip, the four modes, `cm · fit NN%`, palette chips, FX, the drawer button,
  Print. The stage below takes the whole window; the old left column is gone.
- Left drawer (360 pt), closed by default: theme gallery (Text), picture gallery (Image), codes/terminal extras, the
  silly-form switches (Shuffle / Blank form / Closing line), density and copies. Its top-bar button is labelled by mode
  ("Themes", "Pictures", "Codes", "Terminal") and "Close" once open — this is the regression in backlog item 1.
- The ticket is the form: theme fields live in `App::theme_edits`, keyed by field id and typed on the paper
  (`App::edit_for`); the duplicated sidebar inputs are gone. A slim bar under the ticket remains for parts with no region.
- Printing is one explicit `Printing` state with one guard; the job type is `JobKind`; `MAX_CM`/`MAX_COPIES` live in
  `jobs.rs` (the window stops at 5 copies). The title bar follows the palette.
- 43 built-in themes, including `greentext` and `banner`. Train tickets expose `operator`, `train`, `price`, `class`
  (`MAX_FIELDS = 8`), each with a writable region and a generated fallback; same text gives the same ticket (`Ctx::seed`).
- Long tickets are scaled to fit the stage whole ("fit NN%" turns coral); the ghost stands beside the printer on a wide
  stage; clicking a part to write on brings the ticket back to 100%.
- Unchanged and still true: printing on real hardware (text, pictures with 4 dithers, QR/barcodes, terminal one-shot and
  live tail, themes), auto-connect to the closest printer, battery/model/firmware (paper status is raw hex), CLI
  `tronic-print`, MCP (stdio + HTTP), 7 palettes, Ctrl+P, drag and drop, silly forms blank to write on, `tools/qa.ps1`
  and the French checklist in `run\QA.md`.

## In flight (branches, not merged into `main`)
1. `fx-2` — per-family effects on the new bar. Read the four snapshots in `run\sketches\fx2-*.png`, then merge.
2. `ghost-2` — ghost anchored beside the ticket on the new shell. Partial: app state plus part of `ui.rs`. Check where
   it stopped, finish it, snapshot it.
3. `empty-body` — an emptied field must keep the template instead of dropping the ticket. 2 tests red: one in `src/jobs.rs`
   (out of that branch's scope: an empty themed body must now answer `ERR nothing to print` at print time) and a `ransom`
   case. Fix `jobs.rs`, rerun, then merge.
4. `tagline-2` — not started; the brief is TP-25 in `.orchestrator/inbox.md`. Start it from `main` (the worktree is behind).
Each agent has its own worktree and `CARGO_TARGET_DIR`; none may commit, push or print. Review: diff the branch, run
`cargo test`, then merge or ask the user. Update this file when they report.

## Backlog, in the user's order of interest
1. **Drawer discoverability** (the regression the user hit: he could not find how to pick a theme). The drawer button
   follows the mode, so "Themes" becomes "Pictures" on the Image tab and "Close" when open (`src/ui.rs:886`). Make it one
   permanent, obvious control ("Themes" / "Options") that never depends on the tab and never loses its name.
2. **Tagline rising** (TP-25): the line under TRONIC should climb from the slit like paper, not type itself out; keep the
   one-minute text change, no cursor.
3. **Ghost wandering** (TP-23): beside the ticket, above the paper, changing anchor every 20-30 s and on a poke, never
   overlapping the ticket; keep the gaze, the mood, the tap and the empty-state bubble.
4. **Finishing the effects** (TP-24): merge `fx-2` after the three conditions above are seen on screen; per-family weather,
   press shockwave and printing electric kept, halo never touching the controls.
5. **Empty field keeps the ticket**: finish and merge `empty-body`.
- **XBackBone**: gallery fed by remembered/pasted share URLs, click to download and print. The client is on
  `worktree-agent-af3a4dfaaf948b889`; its upload half must be stripped (upload is not the goal). Listing with a token stays
  impossible (HTML behind a session). Settings (server URL, token in Credential Manager) and gallery wiring not started.
  Blocking calls must run on a thread. Waiting for the user's yes.
- **Linux and macOS**: merged (`60292ce`); `ci.yml` runs ubuntu/macos/windows. Missing: a real run on both systems, and the
  two `packaging/patches/*.diff` (fonts, custom themes dir) still unapplied.
- Rotated write boxes for `flight`, `train-old` (body) and `banner` (font), instead of the slim bar: needs a rotated region
  from the render and a rotated input in the UI (TP-21).
- Tray icon + global hotkey (print clipboard / screenshot): not started, needs a Windows design decision.
- Paper status reply of `10 FF 40` is undocumented: find its format (needs the printer with and without paper) to show
  "paper OK / empty". Do not guess it.
- State machines still implicit from the TP-15 audit: `TerminalSession`, `Link` health, `McpEndpoint`. Batch 3 waits for a
  quiet `app.rs`.
- Old docs may mention the deleted `tools/make_*` scripts: the single script is now `tools/make_icons.py` (renders
  `assets/icon.svg` with Edge headless + Pillow).

## Before starting any item
1. `git worktree list` and `git branch -a`: the work may already exist (see "Disposable" above).
2. Read the domain note in `.opencode/domains/`.
3. Write the brief with GOAL / SCOPE / ACCEPTANCE / PROOF / STOP.
4. One item per domain at a time.

## Working with OpenCode and free models
- OpenCode reads `AGENTS.md` automatically; `CLAUDE.md` just imports it. `opencode.json` (untracked, local to this machine)
  sets the default model and is git-ignored.
- Give each agent: its own worktree, its own `CARGO_TARGET_DIR`, a short brief with the exact files, tests to run, and the
  hard rules. Good tasks for cheaper models: theme drawing (`render/theme/*.rs`, tests, sheets), docs, small bugs. Keep UI
  layout work (`ui.rs`) with one agent at a time and review its snapshots.
- Hard rules for every agent: no printing on the real printer without asking, no commit/push without asking, no secrets in
  files.
