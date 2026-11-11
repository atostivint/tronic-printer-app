//! A label page: designed the way the tag is read, 240 dots across by 112 down (30 × 14 mm),
//! then turned a quarter turn clockwise to print as the 112 × 240 dots the head can lay down.
//! Parts are stacked on the design page, cut or padded to its height, and clipped to the rounded
//! corners the paper is cut with, so a label theme only has to say what it puts where.

// Nothing in the running app builds a label page yet: the label themes that use this come next.
// The tests below are what exercises it until then.
#![allow(dead_code)]

use super::draw;
use super::picture::LABEL_CORNER;
use super::text::{self, Align, FontChoice, TextOpts};
use crate::protocol::Bitmap;

/// The design page of a label, in dots: 30 mm wide, 14 mm tall, because a label is read lying
/// down. The head is only 112 dots wide, so this page is turned before it is printed.
pub const LABEL_PAGE_W: usize = 240;
pub const LABEL_PAGE_H: usize = 112;

/// Blank dots kept each side of a label's text on the design page.
const MARGIN: f32 = 6.0;
/// The rule's own margin: narrower, so a line reads as the full width of the label.
const RULE_MARGIN: usize = 6;
/// A rule is 2 dots thick: one dot is faint once the thermal head bleeds.
const RULE_THICK: usize = 2;
/// The sizes a big line tries, largest first. A word that does not fit is set smaller, never cut
/// in two (`KITCHEN` as `KITC` / `HEN` is unreadable from a shelf).
const BIG_SIZES: [f32; 5] = [40.0, 34.0, 28.0, 22.0, 18.0];

/// A label page, built from the top down on its design page.
pub struct Label {
    parts: Vec<Bitmap>,
}

impl Default for Label {
    fn default() -> Self {
        Label::new()
    }
}

impl Label {
    pub fn new() -> Label {
        Label { parts: Vec::new() }
    }

    fn opts(px: f32, font: FontChoice, align: Align, margin_x: f32) -> TextOpts {
        TextOpts { px, font, align, invert: false, break_anywhere: false, line_gap: 0.0, margin_y: 0.0, margin_x }
    }

    /// Wrapped text across the design page. A blank string, or a machine with no usable font,
    /// keeps the room a line would take so the parts below stay where they were put.
    ///
    /// The builder cannot fail: on a label the picture is the fallback, and a line that cannot be
    /// set must not cost the user the rest of the label.
    fn put(mut self, s: &str, opts: TextOpts) -> Label {
        match text::text_at(LABEL_PAGE_W, s, opts) {
            Ok(b) => self.parts.push(b),
            Err(_) => self.parts.push(Bitmap::blank_at(LABEL_PAGE_W, (opts.px * 0.6) as usize)),
        }
        self
    }

    /// The heading of the label.
    pub fn title(self, s: &str) -> Label {
        self.put(s, Self::opts(26.0, FontChoice::Bold, Align::Center, MARGIN))
    }

    /// A line of detail, set against the left margin like a listing.
    pub fn line(self, s: &str) -> Label {
        self.put(s, Self::opts(18.0, FontChoice::Sans, Align::Left, 4.0))
    }

    /// The one thing the label is about.
    pub fn big(self, s: &str) -> Label {
        self.put(s, Self::opts(Self::big_px(s), FontChoice::Sans, Align::Center, MARGIN))
    }

    /// The largest size of [`BIG_SIZES`] at which `s` still sets on one line across the design
    /// page, or the smallest of them when even that is too wide (the word is then cut by
    /// [`finish`]).
    fn big_px(s: &str) -> f32 {
        BIG_SIZES.into_iter().find(|px| Self::one_line(s, *px)).unwrap_or(BIG_SIZES[BIG_SIZES.len() - 1])
    }

    /// True when `s` stands on one line at `px`: the same number of rows it would take on paper
    /// wide enough never to wrap. The heights are compared instead of measuring the glyphs, so
    /// this holds whatever face the machine actually has.
    fn one_line(s: &str, px: f32) -> bool {
        let opts = Self::opts(px, FontChoice::Sans, Align::Center, MARGIN);
        match (text::text_at(LABEL_PAGE_W, s, opts), text::text_at(LABEL_PAGE_W * 64, s, opts)) {
            (Ok(fitted), Ok(unbounded)) => fitted.height == unbounded.height,
            _ => false,
        }
    }

    /// A horizontal rule between the label's margins.
    pub fn rule(mut self) -> Label {
        let mut b = Bitmap::blank_at(LABEL_PAGE_W, RULE_THICK + 2 * 2);
        draw::hline(&mut b, 2, RULE_MARGIN, LABEL_PAGE_W - 1 - RULE_MARGIN, RULE_THICK, (0, 0));
        self.parts.push(b);
        self
    }

    /// Blank rows.
    pub fn gap(mut self, rows: usize) -> Label {
        if rows > 0 {
            self.parts.push(Bitmap::blank_at(LABEL_PAGE_W, rows));
        }
        self
    }

    /// The finished label: the parts stacked, centred down the design page (the blank rows split
    /// evenly above and below), the last rows dropped when they run past 112, the corners rounded
    /// off, and the whole page turned a quarter turn clockwise into the 112 × 240 dots that go to
    /// the head.
    pub fn finish(self) -> Bitmap {
        let page = super::stack(&self.parts, 0);
        let mut out = Bitmap::blank_at(LABEL_PAGE_W, LABEL_PAGE_H);
        let (wb, rows) = (out.width_bytes(), page.height.min(LABEL_PAGE_H));
        // 240 dots is 30 whole bytes, so a line of a wider part (a 384-dot piece stacked in by
        // mistake) copies cleanly into the page width instead of shifting the rows.
        let bytes = wb.min(page.width_bytes());
        // The stack sits in the middle of the page: a label with a rule and two lines reads as one
        // block, not as a line glued to the top edge. A stack taller than the page keeps its top
        // and is truncated at 112, as before.
        let top = LABEL_PAGE_H.saturating_sub(page.height) / 2;
        for y in 0..rows {
            let (to, from) = ((top + y) * wb, y * page.width_bytes());
            out.data[to..to + bytes].copy_from_slice(&page.data[from..from + bytes]);
        }
        mask_corners(&mut out);
        rotate_cw(&out)
    }
}

/// A bitmap turned a quarter turn clockwise: `out(x, y) = in(y, H - 1 - x)`, where `H` is the
/// height of the input. The 240 × 112 design page becomes the 112 × 240 dots of one label, so the
/// text that reads left to right on the page reads top to bottom on the paper.
pub(super) fn rotate_cw(bitmap: &Bitmap) -> Bitmap {
    let mut out = Bitmap::blank_at(bitmap.height, bitmap.width);
    for y in 0..out.height {
        for x in 0..out.width {
            // `get` answers false outside the bitmap, so the padding of the last byte cannot leak.
            if bitmap.get(y, bitmap.height - 1 - x) {
                out.set(x, y);
            }
        }
    }
    out
}

/// Clear the dots outside the rounded rectangle the paper is cut to.
///
/// `picture::mask_label_corners` draws the same shape but is private to its module; the two must
/// stay in step (both read [`LABEL_CORNER`]).
fn mask_corners(bitmap: &mut Bitmap) {
    let (w, h, r) = (bitmap.width, bitmap.height, LABEL_CORNER);
    let wb = bitmap.width_bytes();
    for y in 0..h {
        for x in 0..w {
            let in_corner = (x < r && y < r)
                || (x >= w.saturating_sub(r) && y < r)
                || (x < r && y >= h.saturating_sub(r))
                || (x >= w.saturating_sub(r) && y >= h.saturating_sub(r));
            if !in_corner {
                continue;
            }
            let (cx, cy) = if x < r && y < r {
                (r as f32 - 0.5, r as f32 - 0.5)
            } else if x >= w - r && y < r {
                ((w - r) as f32 - 0.5, r as f32 - 0.5)
            } else if x < r && y >= h - r {
                (r as f32 - 0.5, (h - r) as f32 - 0.5)
            } else {
                ((w - r) as f32 - 0.5, (h - r) as f32 - 0.5)
            };
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy > (r as f32) * (r as f32) {
                bitmap.data[y * wb + x / 8] &= !(0x80 >> (x % 8));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::render::picture::{LABEL_HEIGHT, LABEL_WIDTH};
    use super::*;

    /// A design page with every dot inked.
    fn black_page() -> Bitmap {
        let mut b = Bitmap::blank_at(LABEL_PAGE_W, LABEL_PAGE_H);
        for y in 0..LABEL_PAGE_H {
            for x in 0..LABEL_PAGE_W {
                b.set(x, y);
            }
        }
        b
    }

    /// A bar of ink `rows` deep across the whole design page: one line at the widest the page
    /// allows, so its position and thickness are easy to read back after the turn.
    fn block(rows: usize) -> Bitmap {
        let mut b = Bitmap::blank_at(LABEL_PAGE_W, rows);
        for y in 0..rows {
            for x in 0..LABEL_PAGE_W {
                b.set(x, y);
            }
        }
        b
    }

    fn cols_with_ink(page: &Bitmap) -> (usize, usize) {
        let ink = |x: usize| (0..page.height).any(|y| page.get(x, y));
        let first = (0..page.width).find(|&x| ink(x)).expect("the page has ink");
        let last = (0..page.width).rev().find(|&x| ink(x)).expect("the page has ink");
        (first, last)
    }

    #[test]
    fn a_label_page_is_printed_as_112_by_240() {
        let page = Label::new().title("Coffee").line("1.50").rule().big("OK").gap(4).finish();
        assert_eq!((page.width, page.height), (LABEL_WIDTH, LABEL_HEIGHT));
        assert_eq!(page.data.len(), 14 * LABEL_HEIGHT, "14 bytes a line, 240 lines");
    }

    #[test]
    fn the_design_is_landscape() {
        // The page is laid out lying down: 30 mm across, 14 mm down, so the parts are set on 240
        // dots and the finished bitmap is the same page turned a quarter turn clockwise.
        assert_eq!((LABEL_PAGE_W, LABEL_PAGE_H), (240, 112), "30 × 14 mm at 8 dots a millimetre");
        assert!(LABEL_PAGE_W > LABEL_PAGE_H, "the design page is wider than it is tall");
        // One inked row of the design comes out as one inked column of the print: the page did
        // turn, rather than being laid out in the orientation of the head.
        let page = Label { parts: vec![block(1)] }.finish();
        let (first, last) = cols_with_ink(&page);
        assert_eq!((first, last), (56, 56), "the centred row lands in the middle column");
        assert!((0..page.height).all(|y| page.get(56, y)), "the row spans the whole 240 dots, now downwards");
        for y in 0..page.height {
            assert_eq!((0..page.width).filter(|&x| page.get(x, y)).count(), 1, "row {y} holds one dot, not a line");
        }
        // And the other way round: a column of the design is a row of the print, so the 240 dots
        // of the page become the 240 rows down the paper.
        let mut design = Bitmap::blank_at(LABEL_PAGE_W, LABEL_PAGE_H);
        for y in 0..LABEL_PAGE_H {
            design.set(LABEL_PAGE_W - 1, y);
        }
        let page = Label { parts: vec![design] }.finish();
        // The corners of the design page are rounded off, so the last row of the print is inked
        // between them rather than from edge to edge: the ends of that column were cut away.
        assert!((12..page.width - 12).all(|x| page.get(x, LABEL_HEIGHT - 1)), "the right column of the design is the last row of the print");
        assert!(!page.get(0, LABEL_HEIGHT - 1) && !page.get(page.width - 1, LABEL_HEIGHT - 1), "the cut corners of that column are blank");
        // And the blank left column of the design is the first row of the print, edge to edge.
        assert!((0..page.width).all(|x| !page.get(x, 0)), "the blank left column of the design is a blank first row of the print");
    }

    #[test]
    fn a_page_shorter_than_the_label_leaves_the_rest_blank() {
        // A short stack keeps blank paper on both sides of the ink: what used to be blank rows
        // above and below the text is now blank columns left and right of it.
        let page = Label { parts: vec![block(20)] }.finish();
        let (first, last) = cols_with_ink(&page);
        assert_eq!(last - first + 1, 20);
        assert!(first > 20 && last < LABEL_WIDTH - 20, "{first}..{last}: the ink is an island");
    }

    #[test]
    fn the_corners_of_a_label_page_are_masked() {
        let page = Label { parts: vec![black_page()] }.finish();
        assert!(!page.get(0, 0) && !page.get(LABEL_WIDTH - 1, 0), "the top corners are cut");
        assert!(!page.get(0, LABEL_HEIGHT - 1) && !page.get(LABEL_WIDTH - 1, LABEL_HEIGHT - 1), "the bottom corners are cut");
        assert!(page.get(LABEL_WIDTH / 2, LABEL_HEIGHT / 2), "the middle keeps its ink");
        assert!(page.get(LABEL_WIDTH / 2, 0) && page.get(0, LABEL_HEIGHT / 2), "the edges away from the corners keep theirs");
    }

    #[test]
    fn a_long_big_line_shrinks_instead_of_breaking() {
        // A heading too wide for the tag is set smaller until it stands on one line: `DELICATESSEN`
        // must not come out as `DELICAT` / `ESSEN`, which cannot be read from a shelf.
        for s in ["DELICATESSEN", "Sourdough loaf"] {
            let widest = BIG_SIZES[0];
            assert!(!Label::one_line(s, widest), "{s:?} wraps at {widest}: the check has something to catch");
            let px = Label::big_px(s);
            assert!(px < widest, "{s:?} shrank to {px} instead of breaking at {widest}");
            assert!(Label::one_line(s, px), "{s:?} at {px}: the size it settled on is still wrapped");
            // The part really is one line tall, not a stack of pieces.
            let part = Label::new().big(s).parts.remove(0);
            let one = text::text_at(LABEL_PAGE_W, s, Label::opts(px, FontChoice::Sans, Align::Center, MARGIN)).unwrap().height;
            assert_eq!(part.height, one, "{s:?} at {px} is drawn on one line");
            let wrapped = text::text_at(LABEL_PAGE_W, s, Label::opts(widest, FontChoice::Sans, Align::Center, MARGIN)).unwrap().height;
            assert!(one < wrapped, "{s:?}: the line is shorter than the heading it would have been");
        }
        // `one_line` counts the rows of word-wrapped text, so the label must never ask for a
        // letter wrap: with `break_anywhere` the font would split a word at any letter.
        assert!(!Label::opts(18.0, FontChoice::Sans, Align::Center, MARGIN).break_anywhere, "one_line assumes WrapStyle::Word");
        // When even the smallest size wraps, the smallest is used and the word overflows the tag,
        // clipped at its edge: shrinking stops somewhere rather than looping.
        assert_eq!(Label::big_px(&"M".repeat(40)), BIG_SIZES[BIG_SIZES.len() - 1], "nothing fits: the smallest size is used");
    }

    #[test]
    fn the_page_is_centred_across_the_label() {
        // A bar of ink sits in the middle of the 112 design rows with the same blank band on each
        // side (the odd row goes on the right of the print), so a tag with a rule and two lines
        // reads as one block instead of a line glued to the edge.
        for rows in [39usize, 40, 41] {
            let page = Label { parts: vec![block(rows)] }.finish();
            let (first, last) = cols_with_ink(&page);
            assert_eq!(last - first + 1, rows, "{rows} rows came out whole");
            let after = LABEL_WIDTH - 1 - last;
            assert!(first > 0 && after > 0, "{rows} rows: {first} blank columns before, {after} after");
            assert!(first.abs_diff(after) <= 1, "{rows} rows: {first} before, {after} after");
        }
        // A stack taller than the page keeps its top and is cut at 112 (see `a_long_line_…`).
        let page = Label { parts: vec![block(LABEL_PAGE_H + 60)] }.finish();
        assert!(page.get(LABEL_WIDTH - 1, LABEL_HEIGHT / 2), "a page that overflows starts at the first row");
    }

    #[test]
    fn a_long_line_is_truncated_not_panicking() {
        let page = Label::new().title("Something far too long for fourteen millimetres of paper").line(&"x".repeat(400)).gap(400).finish();
        assert_eq!((page.width, page.height), (LABEL_WIDTH, LABEL_HEIGHT));
    }
}
