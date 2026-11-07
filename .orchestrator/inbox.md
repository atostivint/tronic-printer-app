# Inbox — Tronic Printer

Items to triage, seeded from `HANDOFF.md` on 2026-11-07. The orchestrator takes them in this
order unless the user says otherwise. Status values: `ready`, `in-flight`, `blocked`, `triage`.

| id | item | domain | status | notes |
|---|---|---|---|---|
| TP-01 | XBackBone client: merge + settings + gallery of share URLs | print-path | blocked | Branch `worktree-agent-af3a4dfaaf948b889`, 10 unit tests. Upload works on 3.x (`POST /upload`, `token` + `file`) and master (`POST /api/v1/upload`, Bearer). Listing uploads is impossible with a token (HTML behind a session). Needs the user's yes. |
| TP-02 | Linux and macOS: CI, macOS Bluetooth permission, real run | platform | in-flight | Branch `worktree-agent-a626cda8a697e9db5`. Missing: CI (ubuntu/macos/windows), `NSBluetoothAlwaysUsageDescription` + `.app` bundle, real run on both systems. Nothing has run there yet. |
| TP-03 | Tray icon + global hotkey (print clipboard / screenshot) | ui | ready | Not started. Needs a design decision on Windows first. |
| TP-04 | Paper status `10 FF 40` reply format | print-path | blocked | Needs the printer with and without paper, and the user present. Do not guess the format. |
| TP-05 | Residual gap between live terminal batches | print-path | in-flight | Branch `orch-live-gap` (worktree under `%LOCALAPPDATA%\opencode\worktree\`). Do not duplicate. |
| TP-06 | Custom user themes: writable regions (`"region": "<field key>"`) | render-themes | in-flight | Branch `orch-custom-regions`. Do not duplicate. |
| TP-07 | Printed-ticket textures (halftone, coffee ring, stamp, perforation, dog-ear) | render-themes | in-flight | Branch `orch-ticket-effects`. Untracked `banner.rs` / `effects.rs` in the main tree. Do not edit or delete them. |
| TP-08 | README refresh (deleted scripts, palettes, Ctrl+P, space-before, forms, qa.ps1) | mcp-cli | in-flight | Branch `orch-docs`, done, waiting to merge. |
| TP-09 | Reviewer findings: bugs, UI/UX, performance | triage | triage | Three read-only reviewers on free models (`beam-free`, `glm-5.3-flash-free`, `iceman-free`). Split findings into the domains above before acting. |
| TP-10 | Docs still mention deleted `tools/make_*` scripts | mcp-cli | ready | The single script is now `tools/make_icons.py` (Edge headless + Pillow). Check README and `themes/README.md`. |
| TP-11 | README with real screenshots + publish on GitHub | mcp-cli | in-flight | User asked 2026-11-07: "make a readme and publish it, i let you do the pictures", "publie les branches et fais le readme sur main", repo name keeps the folder name (`tronic-printer-app`). Worker: README.md + `docs/img/`, then reviewer, then `gh repo create atostivint/tronic-printer-app --public`, `main` = the README commit, push all local branches. Absorbs TP-08 (orch-docs README, same content) and TP-10 (stale scripts). |

## Before starting any item
1. `git worktree list` and `git branch -a`: the work may already exist.
2. Read the domain note in `.opencode/domains/`.
3. Write the brief with GOAL / SCOPE / ACCEPTANCE / PROOF / STOP.
4. One item per domain at a time.
