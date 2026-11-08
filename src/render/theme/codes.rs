//! A ticket whose body mixes prose and codes: a line starting with `qr`, `code128`, `code39` or
//! `ean13` becomes that symbol, every other line stays text. One ticket can carry several codes.

use super::{Ctx, Page, Theme};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw;
use crate::render::text::{Align, FontChoice};
use crate::render::{CodeKind, CodeOpts};

/// The keyword at the start of a line, and the data after it. `qr https://…` and `qr: https://…`
/// both work; `qrcode` does not, so a word that merely starts with a keyword stays text.
fn code_line(line: &str) -> Option<(CodeKind, &str)> {
    let lower = line.to_ascii_lowercase();
    for (keyword, kind) in [("code128", CodeKind::Code128), ("code39", CodeKind::Code39), ("ean13", CodeKind::Ean13), ("qr", CodeKind::Qr)] {
        let Some(rest) = lower.strip_prefix(keyword) else { continue };
        if !(rest.starts_with(':') || rest.starts_with(' ') || rest.starts_with('\t')) {
            continue;
        }
        // Byte offsets are the same in `line`: only ASCII letters were lowered, so the split point
        // from the lower-cased copy is valid in the original, case and accents kept.
        let data = line[keyword.len()..].trim_start_matches(|ch: char| ch == ':' || ch.is_whitespace()).trim();
        if !data.is_empty() {
            return Some((kind, data));
        }
    }
    None
}

pub fn codes(c: &Ctx) -> Result<Bitmap, String> {
    let caption = c.field(Theme::Codes, "caption");
    let mut p = Page::new(16.0);
    p.gap(10);
    p.band("  codes", FontChoice::Mono, 20.0, Align::Left)?;
    p.gap(8);
    p.shout(&caption.to_uppercase(), FontChoice::Bold, 56.0)?;
    p.gap(6);
    p.rule((0, 0), 2, 4);
    p.gap(6);
    // The whole body is one writable part: a text box over it edits the prose and the code lines.
    p.begin("body");
    let mut drew_something = false;
    for raw in c.body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            p.gap(8);
            continue;
        }
        if let Some((kind, data)) = code_line(line) {
            // A code that cannot be made (an EAN-13 with the wrong number of digits, a QR with
            // too much text) falls back to the line as text: one bad line never loses the ticket.
            if let Ok(b) = crate::render::code(data, CodeOpts { kind, caption: false, ..CodeOpts::default() }) {
                p.push(b);
                p.text_any(data, FontChoice::Mono, 15.0)?;
                p.gap(12);
                drew_something = true;
                continue;
            }
        }
        p.text(line, FontChoice::Mono, 22.0, Align::Left)?;
        drew_something = true;
    }
    if !drew_something {
        // An emptied text box keeps the frame and leaves room to write in by hand.
        p.gap(64);
    }
    p.end();
    p.gap(8);
    p.rule((0, 0), 1, 4);
    p.text(&format!("NO. {}", c.number(16, 6)), FontChoice::Mono, 16.0, Align::Center)?;
    p.gap(12);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::rect(&mut b, 0, 0, w, h, 2);
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::text::TextOpts;
    use super::super::{Fields, render};

    fn draw(body: &str) -> Bitmap {
        render(Theme::Codes, body, &Fields::new(), TextOpts::default()).unwrap_or_else(|e| panic!("codes: {e}"))
    }

    /// Tall blocks of ink inside the frame: one per symbol (a QR or a barcode is over 100 rows
    /// high; a caption or a rule is not). The border is left out of the count.
    fn tall_blocks(b: &Bitmap) -> usize {
        let ink = |y: usize| (40..WIDTH_DOTS - 40).any(|x| b.get(x, y));
        let (mut runs, mut run) = (0, 0);
        for y in 0..b.height {
            if ink(y) {
                run += 1;
            } else {
                if run > 100 {
                    runs += 1;
                }
                run = 0;
            }
        }
        if run > 100 {
            runs += 1;
        }
        runs
    }

    #[test]
    fn code_lines_need_a_separator_and_data() {
        assert_eq!(code_line("qr: https://example.com"), Some((CodeKind::Qr, "https://example.com")));
        assert_eq!(code_line("QR   HELLO"), Some((CodeKind::Qr, "HELLO")));
        assert_eq!(code_line("code128: TRONIC-2026"), Some((CodeKind::Code128, "TRONIC-2026")));
        assert_eq!(code_line("ean13 123456789012"), Some((CodeKind::Ean13, "123456789012")));
        assert_eq!(code_line("qrcode is a word"), None);
        assert_eq!(code_line("qr:"), None, "nothing to encode is not a code line");
        assert_eq!(code_line("just prose"), None);
    }

    #[test]
    fn two_qr_lines_draw_two_separate_symbol_blocks() {
        assert_eq!(tall_blocks(&draw("qr: ONE")), 1, "one qr line, one symbol");
        let two = draw("qr: ONE\nqr: TWO");
        assert_eq!(tall_blocks(&two), 2, "two qr lines, two symbols");
        assert_eq!(two, draw("qr: ONE\nqr: TWO"), "the same text gives the same ticket");
        assert_ne!(two, draw("qr: ONE\nqr: THREE"), "the data is encoded, not just shown");
    }

    #[test]
    fn prose_and_codes_share_the_ticket() {
        let b = draw("hello there\nqr: ONE");
        assert_eq!(tall_blocks(&b), 1);
        let text = draw("hello there");
        assert!(text.height > 60 && text.height < b.height, "the code makes the ticket taller");
    }
}
