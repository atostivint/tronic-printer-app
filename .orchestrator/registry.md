# Registry — Tronic Printer

One row per domain. `session` is the `sessionID` returned by the subagent tool; pass it back to
continue that domain instead of starting from scratch. `files` is the current conflict surface.

| domain | session | last item | files in play | status |
|---|---|---|---|---|
| ui | ses_e452b7c0effeCdRoKps98t7tWn | TP-20 stage-first shell + drawer (branch `drawer`) | src/ui.rs, src/app.rs, src/main.rs | in-flight |
| print-path | — | — | — | idle |
| render-themes | — | — | — | idle |
| mcp-cli | ses_e489636aaffe5u1cKXLADxXhrI | TP-11 published: README + 6 screenshots, repo public, main pushed | README.md, docs/img/, LICENSE | done |
| platform | ses_e487ab1acffegsRUoWvlArmDRY | TP-02 merged (`60292ce`); watching CI ubuntu/macos/windows | README.md, src/settings.rs, src/chrome.rs, src/ipc.rs, Cargo.toml, build.rs | merged |
| infra (hors dépôt) | ses_e44e90116ffe3MgyBi27qNKdmv | TP-21 provider `bc250` migré en forme native v2 `providers` (13 modèles, timeouts, défaut `bc250/gpt-oss-20b-MXFP4`) | none (config globale OpenCode) | done |

## Rules
- Fill `session` as soon as the first worker returns its `sessionID`.
- A dead or wrong session: clear the cell and start a new one. Never guess an ID.
- One active item per domain. Queue the rest in `inbox.md`.
- Update the row before replying to the user.
- After a compaction, this table is the memory: read it before triaging anything.
