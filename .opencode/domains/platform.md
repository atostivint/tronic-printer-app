# Domain: platform

## Owns
- `build.rs` and platform-specific code paths
- CI workflows under `.github/` (none yet)
- macOS bundle, `Info.plist` and the Bluetooth permission
- the Linux/macOS port branch

## Invariants
- Windows is the main platform and must keep working.
- Linux and macOS are being added on branch `worktree-agent-a626cda8a697e9db5`
  ("Make the app build and run on Linux and macOS"); nothing has run there yet.
- Never claim Linux/macOS works without a real run: list it as unverified.
- Cargo only from PowerShell on Windows; MSVC Build Tools are installed.
- Each agent builds in its own `CARGO_TARGET_DIR`.

## Open threads
- CI: ubuntu, macos, windows (TP-02).
- macOS: `NSBluetoothAlwaysUsageDescription` and a real `.app` bundle.
- Real run on both systems, with what was and was not observed.

## Proof
- `cargo test` on Windows
- CI run URL once the workflow exists
- explicit statement of what has not run on Linux/macOS
