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

/// `v0.1.0 · #137 · fa1d941`, with a `*` right after the hash when the tree was dirty.
pub fn short() -> String {
    let star = if DIRTY.is_empty() { "" } else { "*" };
    format!("v{VERSION} · #{COUNT} · {HASH}{star}")
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
}
