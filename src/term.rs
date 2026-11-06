//! The terminal buffer: lines that arrive from `tronic-print`, and when they go to paper.

use std::time::{Duration, Instant};

/// Lines kept for the preview.
const KEEP: usize = 400;
/// A live burst longer than this prints without waiting for a pause.
pub const MAX_BATCH: usize = 40;
/// How long the output must be quiet before a live batch prints.
pub const IDLE: Duration = Duration::from_millis(900);

pub struct Term {
    lines: Vec<String>,
    printed: usize,
    last_push: Option<Instant>,
    /// Print live output as it arrives.
    pub auto: bool,
    /// Index into [`SIZES`].
    pub size: usize,
    /// A `tronic-print --live` session is connected.
    pub live: bool,
    /// Live batches were printed without the paper advance; one is due when the stream ends.
    pub owes_feed: bool,
}

/// Glyph heights for the monospace font, in dots: about 46, 38 and 31 columns.
pub const SIZES: [f32; 3] = [15.0, 18.0, 22.0];

impl Default for Term {
    fn default() -> Self {
        Term { lines: Vec::new(), printed: 0, last_push: None, auto: true, size: 1, live: false, owes_feed: false }
    }
}

impl Term {
    pub fn push(&mut self, line: &str, now: Instant) {
        for l in crate::ansi::clean(line).split('\n') {
            self.lines.push(l.to_string());
        }
        self.last_push = Some(now);
        if self.lines.len() > KEEP {
            let drop = self.lines.len() - KEEP;
            self.lines.drain(..drop);
            self.printed = self.printed.saturating_sub(drop);
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.printed = 0;
    }

    pub fn unprinted(&self) -> usize {
        self.lines.len() - self.printed
    }

    pub fn total(&self) -> usize {
        self.lines.len()
    }

    /// Everything, for the preview.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// What has not been printed yet.
    pub fn pending_text(&self) -> String {
        self.lines[self.printed..].join("\n")
    }

    pub fn mark_printed(&mut self) {
        self.printed = self.lines.len();
    }

    /// A live batch is due: enough lines, or the output went quiet.
    pub fn due(&self, now: Instant) -> bool {
        let n = self.unprinted();
        n > 0 && (n >= MAX_BATCH || self.last_push.is_some_and(|t| now.duration_since(t) >= IDLE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_waits_for_a_pause_or_for_enough_lines() {
        let t0 = Instant::now();
        let mut term = Term::default();
        term.push("one", t0);
        assert!(!term.due(t0), "just arrived");
        assert!(term.due(t0 + IDLE), "quiet for long enough");
        for i in 0..MAX_BATCH {
            term.push(&format!("line {i}"), t0);
        }
        assert!(term.due(t0), "a long burst doesn't wait");
    }

    #[test]
    fn printed_lines_are_not_printed_twice() {
        let t0 = Instant::now();
        let mut term = Term::default();
        term.push("a\nb", t0);
        assert_eq!(term.pending_text(), "a\nb");
        term.mark_printed();
        assert_eq!(term.unprinted(), 0);
        term.push("c", t0);
        assert_eq!(term.pending_text(), "c");
        assert_eq!(term.text(), "a\nb\nc");
    }

    #[test]
    fn colour_codes_never_reach_the_buffer_and_old_lines_fall_off() {
        let t0 = Instant::now();
        let mut term = Term::default();
        term.push("\x1b[32mok\x1b[0m", t0);
        assert_eq!(term.text(), "ok");
        for i in 0..KEEP + 10 {
            term.push(&i.to_string(), t0);
        }
        assert_eq!(term.total(), KEEP);
    }
}
