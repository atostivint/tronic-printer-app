# Registry — Tronic Printer

One row per domain. `session` is the `sessionID` returned by the subagent tool; pass it back to
continue that domain instead of starting from scratch. `files` is the current conflict surface.

| domain | session | last item | files in play | status |
|---|---|---|---|---|
| ui | ses_e359c5307ffeAPYpUuIOOzGj5W | `deviceid` (worker `minimax-m3.1-flash-free`, out of steps before the snapshots; diff reviewed by me): `device_line` = name · kind · model · firmware · dBm, pill reads "label" on a Label device, Print key dark with `voice::label_unprintable()` as help, scene `label-online`. **300 tests green**, 2 new tests. **No PNG**: launching `tronic-printer.exe` is refused by the permission layer, so the screen is unverified | src/ui/mod.rs, src/voice.rs, src/main.rs (scene) | branch `deviceid`, not committed |
| print-path | ses_e359c5308ffebQpUDyphSZJQQj | `labelguard` (worker `flint-40b-free`, out of steps before the build; retested by me + a 2-line fix by `iceman-free`): `App::on_a_label_printer` now gates `can_print`, `print_once`, `flush_terminal` and `run_job`; a `Status` job still answers. **303 tests green**, 5 new tests by name, diff on `src/app.rs` + `src/jobs.rs` only | src/app.rs, src/jobs.rs | branch `labelguard`, not committed |
| ~~print-path (archive)~~ | ses_e39da8580ffeAWvXfaAfcS9n20 | `a2f9790` (worker `beam-free`, sent back once): discovery sees every Tronic advertiser, `Kind::{Pocket,Label}` from the name prefix, dedup + RSSI sort, auto-connect still the strongest Pocket. `App::discovered` holds the list, nothing draws it yet | src/ble.rs, src/protocol.rs, src/app.rs | merged, UI selector next |
| render-themes | ses_e359c5305ffeWhmcGw4mg8fQIj | `labelcrop` (worker `longcat-2.5-free`): `picture::crop_window` / `picture::cropped` — the largest centred window of a given shape, `None` on a nonsense shape, 5 tests, **303 green** (I reran the 4 matching `crop`). Nothing calls it yet; the worker was right to correct my expected value in test 2 (50/100 is wider than 7/15, so it keeps the height) | src/render/picture.rs | branch `labelcrop`, not committed |
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
