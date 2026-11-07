# Inbox — Tronic Printer

Items to triage, seeded from `HANDOFF.md` on 2026-11-07. The orchestrator takes them in this
order unless the user says otherwise. Status values: `ready`, `in-flight`, `blocked`, `triage`.

| id | item | domain | status | notes |
|---|---|---|---|---|
| TP-01 | XBackBone: gallery of share URLs (read only, no upload) | print-path | ready | User's decision 2026-11-07: XBackBone is **not** an upload tool here; it feeds the picture gallery with the images already exported (share URLs remembered/pasted, click → download → print). Listing with a token stays impossible (HTML behind a session), so no server browser. PR #1 (`worktree-agent-af3a4dfaaf948b889`) has the download/normalisation client + 10 tests, mergeable; its upload half must be stripped before merge. Settings + gallery wiring not started. |
| TP-02 | Linux and macOS: merged; CI on 3 systems, then a real run | platform | in-flight | PR #2 merged as `60292ce` after a rebase (conflicts only in `README.md` and `src/settings.rs`); tests 181+21+6 green on Windows. The branch's own `ci.yml` now runs ubuntu/macos/windows. Still missing: a real run on both systems, and the two `packaging/patches/*.diff` (fonts, custom themes dir) that were never applied. |
| TP-03 | Tray icon + global hotkey (print clipboard / screenshot) | ui | ready | Not started. Needs a design decision on Windows first. |
| TP-04 | Paper status `10 FF 40` reply format | print-path | blocked | Needs the printer with and without paper, and the user present. Do not guess the format. |
| TP-05 | Residual gap between live terminal batches | print-path | in-flight | Branch `orch-live-gap` (worktree under `%LOCALAPPDATA%\opencode\worktree\`). Do not duplicate. |
| TP-06 | Custom user themes: writable regions (`"region": "<field key>"`) | render-themes | in-flight | Branch `orch-custom-regions`. Do not duplicate. |
| TP-07 | Printed-ticket textures (halftone, coffee ring, stamp, perforation, dog-ear) | render-themes | in-flight | Branch `orch-ticket-effects`. Untracked `banner.rs` / `effects.rs` in the main tree. Do not edit or delete them. |
| TP-08 | README refresh (deleted scripts, palettes, Ctrl+P, space-before, forms, qa.ps1) | mcp-cli | done | Absorbed by TP-11: the orch-docs text is in the published README. |
| TP-09 | Reviewer findings: bugs, UI/UX, performance | triage | triage | Three read-only reviewers on free models (`beam-free`, `glm-5.3-flash-free`, `iceman-free`). Split findings into the domains above before acting. |
| TP-10 | Docs still mention deleted `tools/make_*` scripts | mcp-cli | ready | The single script is now `tools/make_icons.py` (Edge headless + Pillow). Check README and `themes/README.md`. |
| TP-11 | README with real screenshots + publish on GitHub | mcp-cli | done | User asked 2026-11-07: "make a readme and publish it, i let you do the pictures", "publie les branches et fais le readme sur main", repo name keeps the folder name (`tronic-printer-app`). Worker: README.md + `docs/img/`, then reviewer, then `gh repo create atostivint/tronic-printer-app --public`, `main` = the README commit, push all local branches. Absorbs TP-08 (orch-docs README, same content) and TP-10 (stale scripts). |
| TP-12 | UI rework: compact side options, no dead space, ghost beside the printer | ui | in-flight | Variant A first (worktree `.claude/worktrees/ui-layout`, branch `ui-layout`): icon rail + ~300 px options panel holding the theme and picture galleries, printer+ticket in the remaining space, ghost next to the printer when the preview is empty. Variants B (stacked sidebar) and C (floating controls) only if the user asks after seeing A. |
| TP-13 | Native Windows title bar keeps Night's colours after a palette switch | ui | done | Fixed on `ui-sketches` (1df9b7e): each palette owns a `Frame` triple in `style.rs`, `chrome::repaint()` re-applies it from `App::set_palette`. Tests 175+16+3 green. The in-app header was measured as correct in four transitions; only the native bar was stale. Not visible in `--snap`: needs the user's eyes. |
| TP-14 | Print button label not centred (sat against the rounded cap, first letters cut) | ui | in review | User's screenshot. `hero_button` grew the label then left-aligned it. Fix: `px(28)` + `align(Center)` in `ui.rs`, snapshots `label-form.png` / `label-terminal.png` show it centred. On `ui-sketches`, being rebased onto `main` before merge. |

## Before starting any item
1. `git worktree list` and `git branch -a`: the work may already exist.
2. Read the domain note in `.opencode/domains/`.
3. Write the brief with GOAL / SCOPE / ACCEPTANCE / PROOF / STOP.
4. One item per domain at a time.
