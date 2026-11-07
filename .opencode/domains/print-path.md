# Domain: print-path

## Owns
- `src/ble.rs` — Bluetooth worker (commands in, events out over std mpsc)
- `src/protocol.rs` — bytes (activation, raster, credits, queries)
- `src/jobs.rs`, `src/jobspec.rs` — structured jobs (jobspec shared with render-themes)
- `src/ipc.rs`, `src/client.rs` — named pipe `\\.\pipe\tronic-printer`
- connection and printing logic in `src/app.rs` (shared with ui)

## Invariants (verified on real hardware)
- Write only on characteristic FF02 (write-without-response).
  Notifications: FF01 (query replies), FF03 (credits `01 nn`, battery `02 bb 00`), ignore `AA 0D 0A`.
- Before every command send activation `10 FF F1 03`, then 12 zero bytes in a separate write.
  Stop: `10 FF F1 45`.
- Raster `GS v 0`, 384 dots wide (48 bytes per line), bands of 24 lines, packets of 180 bytes,
  credit-based flow control (wait for FF03 credits).
- Density `10 FF 10 00 n` (0..2). Feed `1B 4A n` (80 dots is about 10 mm).
- Queries: model `10FF20F0`, firmware `10FF20F1`, battery `10FF50F1`, paper `10FF40`
  (reply format unknown, shown as raw hex).
- Paper is 48 mm = 384 dots, so 1 mm = 8 dots.
- Auto-connect to the strongest-signal printer whose name starts with `Mini Pocket Printer`,
  retry every 12 s; `Disconnect` stops auto-connect until `Connect`.

## Known traps
- `src/app.rs` is the serialization point with ui.
- Never print on the physical printer without asking the user.
- Hardware behaviour cannot be proven by unit tests; say what needs the printer and what was not observed.
- `xbackbone.rs` exists on branch `worktree-agent-af3a4dfaaf948b889`, not merged.

## Proof
- `cargo test` (no printer or network needed)
- `$env:CARGO_TARGET_DIR="target\mine"; cargo build`
- snapshot scenes `printing`, `done`, `error`, `connecting` when relevant
- for hardware changes: exact frames sent, what was observed, what was not

## Open threads
- Paper status `10 FF 40` reply format (TP-04).
- Residual gap between live terminal batches (TP-05, branch orch-live-gap).
- XBackBone client merge and listing limitation (TP-01, blocked on user).
