# Orchestrator workspace — Tronic Printer

This directory is the orchestrator's memory. It is committed, so it survives sessions,
compactions and worktrees.

| File | Role |
|---|---|
| `POLICY.md` | Hard rules, proof contract, scope rules, routing, reply format. Read at triage. |
| `registry.md` | Domain → child session. The orchestrator's long-term memory. |
| `inbox.md` | Items to triage, seeded from `HANDOFF.md`. |

The domain notes live in `.opencode/domains/`.

## Starting the session
Run OpenCode in `tronic-printer-app`. The project config sets `default_agent: orchestrator`,
so the session starts in the orchestrator. Keep this one session alive per project; do not open a
new one per item. Name it `tronic — orchestrator`.

## The loop
1. `/triage <item>` or "take the next item from `.orchestrator/inbox.md`".
2. The orchestrator classifies it against `.opencode/domains/`, checks `git worktree list`, and
   writes a brief.
3. One `worker` per independent task, in background when parallel; separate worktree and
   `CARGO_TARGET_DIR` per worker.
4. No PROOF block → send back once → otherwise close as failed.
5. One-pass review with `reviewer` (`/ship` reviews the current diff).
6. Orchestrator updates `registry.md`, the domain note, and the inbox status.

## What not to do
- Do not let the orchestrator edit code: it is denied by permission, on purpose.
- Do not run two items on the same shared file (`main.rs`, `ui.rs`, `app.rs`, `jobspec.rs`).
- Do not duplicate work already on an `orch-*` or `worktree-agent-*` branch.
- Do not print on the physical printer, commit or push without the user asking.

## Switching back to direct work
OpenCode's agent picker (Tab) switches to `build` for a normal coding session. The orchestrator
stays configured for the next triage.
