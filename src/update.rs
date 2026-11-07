//! Looks for a newer release on GitHub, once, in the background. It only tells the user: the
//! banner opens the release page, where the installer (`setup.exe`) is the download.
//!
//! The repository comes from `TRONIC_REPO` ("owner/name"), which the release workflow sets at build
//! time. A local build leaves it unset and never goes online.

use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use rune::Waker;

/// A release newer than the running app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    /// The release page on github.com.
    pub url: String,
}

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Start the check on its own thread. The receiver gets one message, and only if there is an update.
pub fn spawn(waker: Waker) -> Receiver<Update> {
    let (tx, rx) = channel();
    if let Some(repo) = option_env!("TRONIC_REPO").filter(|r| r.contains('/')) {
        let _ = std::thread::Builder::new().name("update-check".into()).spawn(move || {
            if let Some(update) = latest(repo) {
                if tx.send(update).is_ok() {
                    waker();
                }
            }
        });
    }
    rx
}

fn latest(repo: &str) -> Option<Update> {
    let body = ureq::get(&format!("https://api.github.com/repos/{repo}/releases/latest"))
        .set("User-Agent", concat!("tronic-printer/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(8))
        .call()
        .ok()?
        .into_string()
        .ok()?;
    parse(&body, CURRENT)
}

/// The update described by a GitHub "latest release" reply, if it is newer than `current`.
fn parse(body: &str, current: &str) -> Option<Update> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    if json["draft"].as_bool() == Some(true) || json["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = json["tag_name"].as_str()?;
    let url = json["html_url"].as_str()?;
    // Only ever open a github.com page.
    if !url.starts_with("https://github.com/") || !newer(tag, current) {
        return None;
    }
    Some(Update { version: tag.trim_start_matches('v').to_string(), url: url.to_string() })
}

fn numbers(v: &str) -> Option<Vec<u64>> {
    v.trim().trim_start_matches('v').split('.').map(|p| p.split(['-', '+']).next()?.parse().ok()).collect()
}

/// Is version `a` ("v1.2.3" or "1.2.3") newer than `b`?
fn newer(a: &str, b: &str) -> bool {
    match (numbers(a), numbers(b)) {
        (Some(mut x), Some(mut y)) => {
            let n = x.len().max(y.len());
            x.resize(n, 0);
            y.resize(n, 0);
            x > y
        }
        _ => false,
    }
}

/// Open a page in the default browser.
pub fn open(url: &str) {
    if url.starts_with("https://github.com/") {
        let _ = opener_command(url).spawn();
    }
}

/// Show a folder in the system's file manager (Explorer, Finder, or what handles it on Linux).
pub fn open_folder(dir: &std::path::Path) -> std::io::Result<std::process::Child> {
    opener_command(dir.as_os_str()).spawn()
}

/// The command that hands a URL or a folder to the desktop: `rundll32` (URLs) or `explorer`
/// (folders) on Windows, `open` on macOS, `xdg-open` elsewhere.
fn opener_command(target: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let target = target.as_ref();
    let is_url = target.to_str().is_some_and(|t| t.starts_with("https://"));
    let mut cmd;
    if cfg!(windows) {
        if is_url {
            cmd = std::process::Command::new("rundll32");
            cmd.arg("url.dll,FileProtocolHandler");
        } else {
            cmd = std::process::Command::new("explorer");
        }
    } else if cfg!(target_os = "macos") {
        cmd = std::process::Command::new("open");
    } else {
        cmd = std::process::Command::new("xdg-open");
    }
    cmd.arg(target);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_opener_is_the_systems_own() {
        let cmd = opener_command("https://github.com/o/r");
        let program = cmd.get_program().to_string_lossy().into_owned();
        let expected = if cfg!(windows) { "rundll32" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        assert_eq!(program, expected);
        let last = cmd.get_args().last().unwrap().to_string_lossy().into_owned();
        assert_eq!(last, "https://github.com/o/r");
    }

    #[test]
    fn compares_versions() {
        assert!(newer("v0.2.0", "0.1.0"));
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("1.0", "0.9.9"));
        assert!(!newer("v0.1.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.2.0"));
        assert!(!newer("nightly", "0.1.0"));
    }

    #[test]
    fn reads_a_release() {
        let reply = r#"{"tag_name":"v0.2.0","html_url":"https://github.com/o/r/releases/tag/v0.2.0","draft":false,"prerelease":false}"#;
        let u = parse(reply, "0.1.0").unwrap();
        assert_eq!((u.version.as_str(), u.url.as_str()), ("0.2.0", "https://github.com/o/r/releases/tag/v0.2.0"));
        assert!(parse(reply, "0.2.0").is_none());
        assert!(parse(&reply.replace("https://github.com", "https://evil.example"), "0.1.0").is_none());
        assert!(parse(&reply.replace("\"prerelease\":false", "\"prerelease\":true"), "0.1.0").is_none());
    }
}
