# Orchestration policy — Tronic Printer

The orchestrator reads this file at triage time. Workers only get what the brief quotes from it.

## Hard rules (from AGENTS.md, repeated because they are not negotiable)
- Never print on the physical printer without asking the user.
- Never commit or push unless the user asks; the user decides.
- Run cargo from PowerShell, with your own `CARGO_TARGET_DIR` (`target\mine`).
  `target\release` is locked by running `tronic-mcp.exe` processes: never build there.
- The user is the QA. A visual change is not done until `.\tools\qa.ps1` ran and `run\QA.md`
  says, in French, what to look at.
- Answer in French.

## Proof contract
A task is accepted only if its PROOF block contains:
1. `git diff --stat` (files changed).
2. Every command run, with exit code and the relevant output. Minimum:
   - `cargo test`
   - `$env:CARGO_TARGET_DIR="target\mine"; cargo build`
3. For a visual change: `.\tools\qa.ps1`, the PNG produced by `--snap out.png --scene NAME`,
   and `run\QA.md` updated in French.
4. Tests that ran, by name, for anything new.
5. `gaps`: what could not be verified (screen, hardware, Linux/macOS).

A summary without commands is rejected once. A second time, the task is closed as failed.

## Scope rules
- The brief lists exact files. No file outside the list may be touched.
- Shared files, one active task at a time on each: `src/main.rs`, `src/ui.rs`, `src/app.rs`,
  `src/jobspec.rs`.
- Two tasks in parallel need separate worktrees and separate `CARGO_TARGET_DIR`s.
- Before creating a task, run `git worktree list` and read `HANDOFF.md`: the work may already
  exist on an `orch-*` or `worktree-agent-*` branch. Never duplicate it.

## Routing
| The item is about | Domain |
|---|---|
| layout, views, scenes, colours, microcopy, tray/hotkey UI | `ui` |
| BLE, protocol bytes, jobs, IPC, printing, XBackBone client | `print-path` |
| bitmaps, dithering, codes, themes, ticket textures | `render-themes` |
| MCP server, HTTP endpoint, CLI, README/CLI docs | `mcp-cli` |
| build.rs, CI, macOS bundle, Linux/macOS port | `platform` |

## Reply format (triage)
At most 8 lines:
```
#ID (one-line restatement) — related: #OTHER.
Domain: <domain> (session: <id or new>).
Covers: <what the fix changes>.
Unverified: <screen/hardware/platform>.
```

## After each task
- Update `.orchestrator/registry.md` (row for the domain).
- Update the domain note's "Open threads" if it moved.
- The worker updates `HANDOFF.md` and `run\QA.md` when the brief asks for it; the orchestrator
  cannot edit them (it only writes in `.orchestrator/` and `.opencode/domains/`).
