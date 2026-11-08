//! Which build this is, stamped by `build.rs` and shown bottom-right of the window. The point is
//! to answer "does the copy I just opened have the change I asked for?" without guessing.

/// From `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Short git hash of `HEAD`, or "unknown" outside a checkout.
pub const HASH: &str = env!("TRONIC_GIT_HASH");
/// Commits on `HEAD`, or "0" when git could not tell.
pub const COUNT: &str = env!("TRONIC_GIT_COUNT");
/// "1" when the tree had uncommitted changes at build time, "" otherwise.
pub const DIRTY: &str = env!("TRONIC_GIT_DIRTY");
/// When it was built, `YYYY-MM-DD HH:MM` UTC, or "unknown".
pub const BUILT: &str = env!("TRONIC_BUILD_DATE");

/// `v0.1.0 · #137 · fa1d941 · 14:52`, with a `*` right after the hash when the tree was dirty.
/// The clock is the compile time (UTC, as `build.rs` stamps it), so two copies opened minutes apart
/// can be told apart without hovering for the tooltip.
pub fn short() -> String {
    short_with(BUILT)
}

/// The short line for a given stamp; split out so a malformed date can be tested without env vars.
fn short_with(built: &str) -> String {
    let star = if DIRTY.is_empty() { "" } else { "*" };
    format!("v{VERSION} · #{COUNT} · {HASH}{star}{}", with_time(built))
}

/// The ` · HH:MM` suffix of `BUILT`, or nothing when the stamp is "unknown" or not `YYYY-MM-DD HH:MM`:
/// an unpacked source tree must still get a readable line, never "unknown" or a stray colon.
fn with_time(built: &str) -> String {
    let Some((_, clock)) = built.split_once(' ') else { return String::new() };
    let clock = clock.trim();
    let Some((h, m)) = clock.split_once(':') else { return String::new() };
    let two_digits = |s: &str| s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit());
    if two_digits(h) && two_digits(m) {
        format!(" · {clock}")
    } else {
        String::new()
    }
}

/// The short line, plus the build time and the profile: what the tooltip shows.
pub fn long() -> String {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    format!("{} · built {BUILT} UTC · {profile}", short())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_line_names_the_version_and_the_commit() {
        let line = short();
        // `short()` puts the `v` in front, so compare what follows it.
        let rest = line.strip_prefix('v').unwrap_or(&line);
        assert!(rest.starts_with(VERSION), "{line} does not start with {VERSION}");
        assert!(line.contains('#'), "{line} has no commit count");
        assert!(line.contains(HASH), "{line} has no git hash ({HASH})");
        assert!(!BUILT.is_empty(), "the build date was not stamped");
        assert!(long().starts_with(&line), "the long line lost the short one");
    }

    #[test]
    fn a_stamped_build_appends_the_compile_time() {
        assert_eq!(with_time("2026-11-08 14:52"), " · 14:52");
        assert_eq!(with_time("2026-11-08 00:05"), " · 00:05");
        let time = with_time(BUILT);
        assert!(!time.is_empty(), "the build date was not stamped ({BUILT})");
        assert!(short().ends_with(&time), "{} lost the build time", short());
        assert!(long().starts_with(&short()), "the long line lost the short one");
    }

    #[test]
    fn a_malformed_build_date_adds_no_time() {
        for built in [
            "unknown",
            "",
            "2026-11-08",
            "2026-11-08 14",
            "2026-11-08 14:5",
            "2026-11-08 14:52:33",
            "nonsense",
        ] {
            assert_eq!(with_time(built), "", "{built:?} produced a time");
            assert_eq!(short_with(built), short_with("unknown"), "{built:?} changed the line");
            assert!(!short_with(built).contains(':'), "{built:?} left a colon in the line");
        }
    }
}
