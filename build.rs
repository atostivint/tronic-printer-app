//! Puts the logo into the Windows executables (Explorer, the taskbar and Alt+Tab show it), fills
//! in the file properties, and stamps which build this is for `src/version.rs`. The icon part does
//! nothing on other systems.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    stamp_version();
    #[cfg(windows)]
    embed_icon();
}

/// Bake the git state and the build time into the binary, so the window can say which build it is.
/// Every git call may fail (no git installed, a source tarball): the line then says "unknown" and
/// the build carries on.
fn stamp_version() {
    let hash = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let count = git(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "0".into());
    let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.trim().is_empty());
    println!("cargo:rustc-env=TRONIC_GIT_HASH={hash}");
    println!("cargo:rustc-env=TRONIC_GIT_COUNT={count}");
    println!("cargo:rustc-env=TRONIC_GIT_DIRTY={}", if dirty { "1" } else { "" });
    println!("cargo:rustc-env=TRONIC_BUILD_DATE={}", local_now());

    // A commit moves the branch ref (HEAD itself stays `ref: refs/heads/...`) and a `git add`
    // rewrites the index: either way the stamp is stale, so watch the files git actually writes.
    // This also covers a worktree, whose git dir is not `.git` but `.git/worktrees/<name>`.
    let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) else { return };
    let dir = Path::new(&dir);
    let mut watched = vec![dir.join("HEAD"), dir.join("index")];
    match git(&["symbolic-ref", "-q", "HEAD"]) {
        Some(r) => watched.push(dir.join(r)),
        // A detached HEAD has no ref file; commits on a packed branch land in packed-refs.
        None => watched.push(dir.join("packed-refs")),
    }
    for path in watched {
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

/// The first line of `git <args>` in the crate root, trimmed; `None` when git is missing, this is
/// not a checkout, or the command failed.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(std::env::var_os("CARGO_MANIFEST_DIR")?)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

/// `YYYY-MM-DD HH:MM` in the wall clock of whoever builds, so the version line reads like the clock
/// in the corner of the same screen. Rust has no timezone database without a dependency, so ask the
/// system: `Get-Date` on Windows, `date` elsewhere. If that shell is missing or prints something
/// else, fall back to UTC: a wrong hour beats a build that will not compile.
fn local_now() -> String {
    let (program, args): (&str, &[&str]) = if cfg!(windows) {
        ("powershell", &["-NoProfile", "-Command", "(Get-Date).ToString('yyyy-MM-dd HH:mm')"])
    } else {
        ("date", &["+%Y-%m-%d %H:%M"])
    };
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|text| stamped(text.trim()).map(str::to_string))
        .unwrap_or_else(utc_now)
}

/// `text` when it is exactly `YYYY-MM-DD HH:MM`: a localized `date`, a warning or a stray line must
/// never reach the version string, where it would render as no time at all.
fn stamped(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes.get(range).is_some_and(|b| b.iter().all(u8::is_ascii_digit))
    };
    let layout = bytes.len() == 16
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b' '
        && bytes[13] == b':'
        && digits(0..4)
        && digits(5..7)
        && digits(8..10)
        && digits(11..13)
        && digits(14..16);
    layout.then_some(text)
}

/// `YYYY-MM-DD HH:MM` in UTC, the fallback when the local clock cannot be asked.
fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rest / 3600, (rest % 3600) / 60)
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day of era, 0..=146096
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // 0..=399
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365
    let mp = (5 * doy + 2) / 153; // 0..=11, March is 0
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

// `winresource` is a build-dependency on Windows hosts only, so the code must not exist elsewhere.
#[cfg(windows)]
fn embed_icon() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Tronic Printer");
        res.set("FileDescription", "Tronic Printer: print from Windows on the Tronic Mini Pocket Printer");
        res.set("LegalCopyright", "MIT");
        if let Err(e) = res.compile() {
            // A missing resource compiler must not stop the build: the app just gets no icon.
            println!("cargo:warning=could not embed the icon: {e}");
        }
    }
}
