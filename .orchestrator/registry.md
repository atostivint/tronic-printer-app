# Registry — Tronic Printer

One row per domain. `session` is the `sessionID` returned by the subagent tool; pass it back to
continue that domain instead of starting from scratch. `files` is the current conflict surface.

| domain | session | last item | files in play | status |
|---|---|---|---|---|
| ui | ses_e352597c1ffeG1606XIONabi10 | A2 mergé dans main (feat/fx-revamp, non commité) : `label_online` unifié, help label = `device_line` + refus, sélecteur `discovered`→`prefer`/`prefer_auto`, scène `label-online` réaliste. Build+tests verts. Écran/snapshot/QA non vérifiés ; `discovered` non alimenté par BLE (picker inerte en réel). | src/ui/mod.rs, src/voice.rs, src/main.rs | mergé, gaps écran/BLE |
| print-path | ses_e34c1b134ffeo9R03XjBkvl9AJ | A1c mergé/vérifié dans main (feat/fx-revamp, non commité) : `print`/`drain_gallery_queue`/`flush_terminal` via `on_a_label_printer()`, `tick_terminal` feed-only gardé, `send_job` en défense, dette coupée, tests label ajoutés. Build+tests verts (351 pass au total signalés). Revue faite (correctifs appliqués). | src/app.rs, src/jobs.rs | mergé, revu |
| ~~print-path (archive)~~ | ses_e39da8580ffeAWvXfaAfcS9n20 | `a2f9790` (worker `beam-free`, sent back once): discovery sees every Tronic advertiser, `Kind::{Pocket,Label}` from the name prefix, dedup + RSSI sort, auto-connect still the strongest Pocket. `App::discovered` holds the list, nothing draws it yet | src/ble.rs, src/protocol.rs, src/app.rs | merged, UI selector next |
| render-themes | ses_e34ad9999ffeVsebFMTUmnxM6x | A3 mergé dans main (feat/fx-revamp, non commité) : API crop/export/tests + câblage `compose_image` (`prepare_with`, crop dans clé cache) présents, `render::picture` et suite verts. Câblage UI `pic_opts.crop` (éditeur label/TP-57) non fait. Origine intégration/worker ; revalider en revue si besoin. | src/render/picture.rs, src/render/mod.rs, src/app.rs | socle mergé, câblage UI à faire |
| mcp-cli | ses_e489636aaffe5u1cKXLADxXhrI | TP-11 published: README + 6 screenshots, repo public, main pushed | README.md, docs/img/, LICENSE | done |
| platform | ses_e39f0f97dfferZu8YJECPwxL1d | `5df97ac` (worker `longcat-2.5-free`): the two font-dependent assertions now skip with a measured reason instead of failing — the travel layout test pins every other field so only the long values differ, the banner test probes the resolver before assuming two faces exist. Proven against DejaVu, a one-font folder and an empty font folder | src/render/theme/travel.rs, src/render/theme/banner.rs (tests only) | merged, real Linux runner still untested |
| infra (hors dépôt) | ses_e44e90116ffe3MgyBi27qNKdmv | TP-21 provider `bc250` migré en forme native v2 `providers` (13 modèles, timeouts, défaut `bc250/gpt-oss-20b-MXFP4`) | none (config globale OpenCode) | done |

## Model pool (decided with the user, 2026-11-08)
- **Orchestrator**: `opencode/deepseek-v4.1-flash-free` — triage, briefs, proof review, merges.
- **Workers**: the other free models, never a DeepSeek one. Free and tool-capable: `opencode/flint-40b-free`,
  `opencode/longcat-2.5-free`, `opencode/minimax-m3.1-flash-free`, `opencode/mimo-v2.6-flash-free`,
  `opencode/beam-free`, `opencode/inkling-2-small-free`, `opencode/iceman-free`,
  `opencode/nemotron-3.5-super-vl-free`, plus `opencode-go/{longcat-2.5,beam}-free`.
- Rotation, not habit: a proof-less or fabricated report means that model is not used again for the same
  kind of task (local models have fabricated both edits and green test output before).
- Paid models only if the user asks for them by name.

## Rules
- Fill `session` as soon as the first worker returns its `sessionID`.
- A dead or wrong session: clear the cell and start a new one. Never guess an ID.
- One active item per domain. Queue the rest in `inbox.md`.
- Update the row before replying to the user.
- After a compaction, this table is the memory: read it before triaging anything.
