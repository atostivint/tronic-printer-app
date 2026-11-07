# Domain: mcp-cli

## Owns
- `src/mcp.rs`, `src/mcp_http.rs`, `src/mcp_setup.rs` — MCP server (stdio and HTTP at
  `127.0.0.1:7391/mcp`)
- `src/bin/tronic-mcp.rs` — the MCP binary
- `src/bin/tronic-print.rs` — the CLI (`dir | tronic-print --theme receipt --field store=SHOP`)
- README examples that document the CLI and MCP

## Invariants
- The HTTP endpoint stays `127.0.0.1:7391/mcp`; the "Port busy: pick another" path must keep working.
- Job fields and MCP tool schemas must stay compatible with `jobs.rs` and `jobspec.rs`.
- CLI examples in README must remain runnable. The old `tools/make_*` scripts are gone;
  only `tools/make_icons.py` remains.
- The app has no console window; errors go to `run\stderr.txt` when started by `qa.ps1`.

## Known traps
- `target\release` is locked by running `tronic-mcp.exe` processes: build with `target\mine`.
- `src/main.rs` wiring for MCP is shared with ui: serialize.
- Changing a tool schema is a breaking change for clients; flag it in the brief.

## Proof
- `cargo test`
- `$env:CARGO_TARGET_DIR="target\mine"; cargo build --bins`
- one real CLI invocation with a sample job (no print): exact command + output
- MCP handshake if the protocol changed

## Open threads
- Published 2026-11-07: https://github.com/atostivint/tronic-printer-app (public, `main`). README is the
  landing page with 6 `--snap` screenshots in `docs/img/`; MIT `LICENSE` added; `opencode.json` untracked
  and git-ignored (machine-specific agent/model ids).
- Still missing before the repo looks finished: no tag pushed, so `.github/workflows/release.yml`
  (installer + portable zip) has never run; the README must not claim releases until then.
- `tools/` and `themes/README.md` were checked for stale `make_*` script names in TP-11: clean.
