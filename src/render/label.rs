//! A label page: designed the way the tag is read, 240 dots across by 112 down (30 × 14 mm),
//! then turned a quarter turn clockwise to print as the 112 × 240 dots the head can lay down.
//! Parts are stacked on the design page, cut or padded to its height, and clipped to the rounded
//! corners the paper is cut with, so a label theme only has to say what it puts where.

// Only the label themes build a label page today: the pieces below that they do not use yet
// (dashes, icons) are exercised by the tests until a theme asks for them.
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
/// The square an icon is drawn in, in dots.
pub const ICON_BOX: usize = 24;
/// A frame's pen: two dots, like a rule, because one dot is faint once the head bleeds.
const FRAME_THICK: usize = 2;
/// The gap between the page edge and a frame; close enough that the frame reads as the tag's edge.
const FRAME_INSET: usize = 3;
/// A dashed frame's dash: four dots of ink. The paper between two dashes is one dot longer: at
/// four and four the frame inked as much as a solid one (the four corners are inked twice there),
/// and the head bleeds a dot into each gap, so a gap has to outlast its dash to read as one.
const FRAME_DASH: usize = 4;
/// Corner marks sit just inside the rounded cut, so the mask does not eat them.
const MARK_INSET: usize = 3;
/// The arm of a corner mark.
const MARK_LEN: usize = 10;

/// A small drawing that says what the tag is for, without words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Star,
    Heart,
    Leaf,
    Snowflake,
}

/// Something drawn on the design page itself rather than stacked with the text: a frame has to
/// sit at the edge of the tag whatever the text does, so it cannot be a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decor {
    Frame { inset: usize, thickness: usize, dash: Option<usize> },
    CornerMarks { inset: usize, len: usize },
}

/// A label page, built from the top down on its design page.
pub struct Label {
    parts: Vec<Bitmap>,
    decor: Vec<Decor>,
}

impl Default for Label {
    fn default() -> Self {
        Label::new()
    }
}

impl Label {
    pub fn new() -> Label {
        Label { parts: Vec::new(), decor: Vec::new() }
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

    /// A solid frame `inset` dots in from the edge, `thickness` dots of pen.
    pub fn frame(mut self, inset: usize, thickness: usize) -> Label {
        self.decor.push(Decor::Frame { inset, thickness: thickness.max(1), dash: None });
        self
    }

    /// A dashed frame, for a tag that reads as a note rather than as a card: `dash` dots of ink,
    /// then one dot more of paper — a gap no longer than its dash inked half as much as a solid
    /// frame, which is not a dashed frame at all.
    pub fn dashed_frame(mut self, inset: usize, dash: usize) -> Label {
        self.decor.push(Decor::Frame { inset, thickness: FRAME_THICK, dash: Some(dash.max(1)) });
        self
    }

    /// A frame at the usual inset, two dots thick.
    pub fn framed(self) -> Label {
        self.frame(FRAME_INSET, FRAME_THICK)
    }

    /// A dashed frame at the usual inset.
    pub fn dashed(self) -> Label {
        self.dashed_frame(FRAME_INSET, FRAME_DASH)
    }

    /// Four little corner brackets, the marks of a printed card. They sit on the rounded cut of
    /// the paper, so they only reach inwards.
    pub fn corner_marks(mut self, len: usize) -> Label {
        self.decor.push(Decor::CornerMarks { inset: MARK_INSET, len: len.max(1) });
        self
    }

    /// White text knocked out of a black band across the whole width: loud, and legible from far
    /// off (`FRAGILE`, a shelf's name). The band is a part, so it takes the room a line takes.
    pub fn band(mut self, s: &str) -> Label {
        // `invert` flips the whole bitmap, margins included, so the part is a black band with
        // the glyphs punched white — no second pass needed.
        let opts = TextOpts { px: 20.0, font: FontChoice::Bold, align: Align::Center, invert: true, break_anywhere: false, line_gap: 0.0, margin_y: 2.0, margin_x: MARGIN };
        self.put(s, opts)
    }

    /// A small drawing on its own row, centred across the tag.
    pub fn icon(mut self, kind: Icon) -> Label {
        self.parts.push(icon_part(kind));
        self
    }

    /// Blank rows.
    pub fn gap(mut self, rows: usize) -> Label {        if rows > 0 {
            self.parts.push(Bitmap::blank_at(LABEL_PAGE_W, rows));
        }
        self
    }

    /// How many design rows the parts take before the page is centred and cut: a theme that
    /// wants to keep all its lines under 112 asks this.
    pub fn stack_height(&self) -> usize {
        self.parts.iter().map(|p| p.height).sum()
    }

    /// The finished label: the parts stacked, centred down the design page (the blank rows split
    /// evenly above and below), the decorations drawn over it, the last rows dropped when they run
    /// past 112, the corners rounded off, and the whole page turned a quarter turn clockwise into
    /// the 112 × 240 dots that go to the head.
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
        // The decorations go over the text: a frame drawn under a line that reaches the edge
        // would be invisible exactly where it matters.
        for d in &self.decor {
            d.draw(&mut out);
        }
        mask_corners(&mut out);
        rotate_cw(&out)
    }
}

impl Decor {
    /// Draw this decoration on the design page. `Bitmap::set` clips, so an inset past the edge
    /// simply draws nothing rather than panicking.
    fn draw(&self, page: &mut Bitmap) {
        let (w, h) = (page.width, page.height);
        match *self {
            Decor::Frame { inset, thickness, dash } => {
                let t = thickness.max(1);
                if w <= 2 * inset + t || h <= 2 * inset + t {
                    return;
                }
                let (x0, y0, x1, y1) = (inset, inset, w - 1 - inset, h - 1 - inset);
                // A dashed frame restarts its dashes at each corner, so the four sides look
                // alike; `(0, 0)` is what `draw` takes for a solid line, and the paper between
                // two dashes is a dot wider than the dash (see `FRAME_DASH`).
                let d = dash.map_or((0, 0), |n| (n, n + 1));
                draw::hline(page, y0, x0, x1, t, d);
                draw::hline(page, y1 + 1 - t, x0, x1, t, d);
                draw::vline(page, x0, y0, y1, t, d);
                draw::vline(page, x1 + 1 - t, y0, y1, t, d);
            }
            Decor::CornerMarks { inset, len } => {
                if w <= 2 * inset + 2 || h <= 2 * inset + 2 {
                    return;
                }
                let (x0, y0, x1, y1) = (inset, inset, w - 1 - inset, h - 1 - inset);
                let (t, len) = (FRAME_THICK, len.min(x1.saturating_sub(x0)).min(y1.saturating_sub(y0)));
                // Each bracket is two arms meeting in the corner; the right and bottom ones are
                // pulled inside by the pen so they do not fall off the page.
                for (x, y, dx, dy) in [(x0, y0, 1isize, 1isize), (x1 + 1 - t, y0, -1, 1), (x0, y1 + 1 - t, 1, -1), (x1 + 1 - t, y1 + 1 - t, -1, -1)] {
                    if dx > 0 {
                        draw::hline(page, y, x, x + len, t, (0, 0));
                    } else {
                        draw::hline(page, y, x.saturating_sub(len), x, t, (0, 0));
                    }
                    if dy > 0 {
                        draw::vline(page, x, y, y + len, t, (0, 0));
                    } else {
                        draw::vline(page, x, y.saturating_sub(len), y, t, (0, 0));
                    }
                }
            }
        }
    }
}

/// One icon as a part: a [`ICON_BOX`]-tall band of paper, the drawing centred across the page.
/// Every shape is built from lines and discs, so it stays crisp at one bit.
fn icon_part(kind: Icon) -> Bitmap {
    let mut b = Bitmap::blank_at(LABEL_PAGE_W, ICON_BOX);
    let (cx, cy) = ((LABEL_PAGE_W / 2) as i32, (ICON_BOX / 2) as i32);
    let pi = std::f32::consts::PI;
    match kind {
        Icon::Star => {
            // Five points, alternating long and short radii, joined by a 2-dot pen.
            let mut pts = [(0i32, 0i32); 10];
            for (i, p) in pts.iter_mut().enumerate() {
                let r = if i % 2 == 0 { 11.0 } else { 4.4 };
                let a = -pi / 2.0 + i as f32 * pi / 5.0;
                *p = (cx + (r * a.cos()).round() as i32, cy + (r * a.sin()).round() as i32);
            }
            for i in 0..10 {
                let (a, z) = (pts[i], pts[(i + 1) % 10]);
                draw::line(&mut b, a.0, a.1, z.0, z.1, 2);
            }
            draw::disc(&mut b, cx, cy, 2, None);
        }
        Icon::Heart => {
            // The classic implicit heart, scaled to the box: filled, so it reads at arm's length.
            for yy in -11i32..=11 {
                for xx in -11i32..=11 {
                    let x = xx as f32 / 10.0 * 1.2;
                    let y = -(yy as f32) / 10.0 * 1.2 + 0.3;
                    let t = x * x + y * y - 1.0;
                    if t * t * t - x * x * y * y * y <= 0.0 {
                        draw::put(&mut b, cx + xx, cy + yy, true);
                    }
                }
            }
        }
        Icon::Leaf => {
            // An almond: rows narrow towards the tip, with a slanted midrib cleared out of it.
            for dyy in -10i32..=10 {
                let t = dyy as f32 / 10.0;
                let half = (6.0 * (1.0 - t * t).max(0.0).sqrt()).round() as i32;
                draw::hline(&mut b, (cy + dyy) as usize, (cx - half) as usize, (cx + half) as usize, 1, (0, 0));
            }
            for dyy in -8i32..=8 {
                let off = (dyy as f32 * 0.35).round() as i32;
                draw::clear(&mut b, (cx + off) as usize, (cy + dyy) as usize);
            }
            // A stem below the leaf, and one diagonal vein to show which way it grows.
            draw::line(&mut b, cx, cy + 9, cx + 2, cy + 11, 1);
        }
        Icon::Snowflake => {
            // Three axes at 60°, each with a pair of branches near its ends.
            for k in 0..3 {
                let a = k as f32 * pi / 3.0;
                let (dx, dy) = ((a.cos() * 10.0).round() as i32, (a.sin() * 10.0).round() as i32);
                draw::line(&mut b, cx - dx, cy - dy, cx + dx, cy + dy, 2);
            }
            for k in 0..6 {
                let a = k as f32 * pi / 3.0;
                let (ex, ey) = (cx + (a.cos() * 7.0).round() as i32, cy + (a.sin() * 7.0).round() as i32);
                for s in [-1.0f32, 1.0] {
                    let ba = a + s * 0.9;
                    draw::line(&mut b, ex, ey, ex + (ba.cos() * 4.0).round() as i32, ey + (ba.sin() * 4.0).round() as i32, 2);
                }
            }
        }
    }
    b
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

    /// A label made of parts alone, no decoration: the tests below read the page the stack makes.
    fn page_of(parts: Vec<Bitmap>) -> Label {
        Label { parts, decor: Vec::new() }
    }

    #[test]
    #[ignore]
    fn print_label_heights() {
        let stacks: [(&str, Label); 8] = [
            ("address", Label::new().line("TO").line("A. Person").line("12 Rue du Pain").line("75001 Paris").rule()),
            ("gift", Label::new().line("TO").big("Alex").line("FROM").line("Me").icon(Icon::Heart)),
            ("gift-title", Label::new().title("TO").big("Alex").line("FROM").line("Me").icon(Icon::Heart)),
            ("shelf", Label::new().band("MILK").line("2026-11").icon(Icon::Star)),
            ("name", Label::new().line("HELLO, MY NAME IS").big("Alex").line("Say hello").icon(Icon::Star)),
            ("parcel", Label::new().line("TO").line("A. Person").line("12 Rue du Pain").band("FRAGILE")),
            ("freezer", Label::new().icon(Icon::Snowflake).big("CHILI").line("2026-11")),
            ("plant", Label::new().icon(Icon::Leaf).big("BASIL").line("Water weekly")),
        ];
        for (name, l) in stacks {
            println!("{name}: {}", l.stack_height());
        }
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
        let page = page_of(vec![block(1)]).finish();
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
        let page = page_of(vec![design]).finish();
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
        let page = page_of(vec![block(20)]).finish();
        let (first, last) = cols_with_ink(&page);
        assert_eq!(last - first + 1, 20);
        assert!(first > 20 && last < LABEL_WIDTH - 20, "{first}..{last}: the ink is an island");
    }

    #[test]
    fn the_corners_of_a_label_page_are_masked() {
        let page = page_of(vec![black_page()]).finish();
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
            let page = page_of(vec![block(rows)]).finish();
            let (first, last) = cols_with_ink(&page);
            assert_eq!(last - first + 1, rows, "{rows} rows came out whole");
            let after = LABEL_WIDTH - 1 - last;
            assert!(first > 0 && after > 0, "{rows} rows: {first} blank columns before, {after} after");
            assert!(first.abs_diff(after) <= 1, "{rows} rows: {first} before, {after} after");
        }
        // A stack taller than the page keeps its top and is cut at 112 (see `a_long_line_…`).
        let page = page_of(vec![block(LABEL_PAGE_H + 60)]).finish();
        assert!(page.get(LABEL_WIDTH - 1, LABEL_HEIGHT / 2), "a page that overflows starts at the first row");
    }

    #[test]
    fn a_long_line_is_truncated_not_panicking() {
        let page = Label::new().title("Something far too long for fourteen millimetres of paper").line(&"x".repeat(400)).gap(400).finish();
        assert_eq!((page.width, page.height), (LABEL_WIDTH, LABEL_HEIGHT));
    }

    /// Ink in the box (`x0`,`y0`)–(`x1`,`y1`) of the printed bitmap.
    fn ink_in(page: &Bitmap, x0: usize, y0: usize, x1: usize, y1: usize) -> u32 {
        let mut n = 0;
        for y in y0..=y1.min(page.height - 1) {
            for x in x0..=x1.min(page.width - 1) {
                if page.get(x, y) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn each_decoration_inks_a_tag_of_its_own() {
        // Every primitive has to change the page it is asked to decorate: a frame, its dashes,
        // the corner marks, the black band and the four icons. And none of them may move the page
        // off 112 × 240, since that is what the head can lay down.
        let blank = Label::new().finish();
        let decorated = [
            ("frame", Label::new().frame(4, 2)),
            ("dashed frame", Label::new().dashed_frame(4, 4)),
            ("corner marks", Label::new().corner_marks(10)),
            ("band", Label::new().band("FRAGILE")),
            ("star", Label::new().icon(Icon::Star)),
            ("heart", Label::new().icon(Icon::Heart)),
            ("leaf", Label::new().icon(Icon::Leaf)),
            ("snowflake", Label::new().icon(Icon::Snowflake)),
        ];
        let mut seen: Vec<(&str, Bitmap)> = Vec::new();
        for (name, label) in decorated {
            let b = label.finish();
            assert_eq!((b.width, b.height), (LABEL_WIDTH, LABEL_HEIGHT), "{name}: off the label");
            assert_ne!(b.data, blank.data, "{name}: drew nothing");
            assert!(ink_in(&b, 0, 0, LABEL_WIDTH - 1, LABEL_HEIGHT - 1) > 40, "{name}: barely any ink");
            assert!(seen.iter().all(|(_, s)| s.data != b.data), "{name}: looks exactly like an earlier decoration");
            seen.push((name, b));
        }
    }

    #[test]
    fn a_frame_goes_all_the_way_round_the_tag() {
        // `frame(4, 2)` on the design page: rows 4–5 and 106–107 across, columns 4–5 and 234–235
        // down. A quarter turn later, the design's rows are the print's columns and the other way
        // round, so the frame is a frame on the paper too — the 240 dots of the page are the
        // 240 rows down the paper.
        let page = Label::new().frame(4, 2).finish();
        for (x, y, what) in [(107, 120, "top"), (4, 120, "bottom"), (60, 4, "left"), (60, 235, "right")] {
            assert!(page.get(x, y), "{what} edge of the frame is missing at ({x}, {y})");
        }
        assert!(!page.get(60, 120), "the middle of the tag is left blank");
    }

    #[test]
    fn a_dashed_frame_leaves_paper_between_its_dashes() {
        let solid = Label::new().frame(4, 2).finish();
        let dashed = Label::new().dashed_frame(4, 4).finish();
        // Same frame, half the ink: the dashes really are gaps.
        let (a, b) = (ink_in(&solid, 0, 0, LABEL_WIDTH - 1, LABEL_HEIGHT - 1), ink_in(&dashed, 0, 0, LABEL_WIDTH - 1, LABEL_HEIGHT - 1));
        assert!(b * 2 < a, "a dashed frame ({b}) should ink less than half a solid one ({a})");
        assert!(b > 40, "and still draw a frame");
        // The dashes reach every side, not just the top.
        for (cx, cy) in [(107, 120), (4, 120), (60, 4), (60, 235)] {
            assert!(ink_in(&dashed, cx - 1, cy - 1, cx + 1, cy + 1) > 0, "no dash near ({cx}, {cy})");
        }
    }

    #[test]
    fn a_band_is_black_with_white_letters() {
        // A band is a black strip with its text knocked out: it has to cover the tag's full width
        // and leave white dots where the glyphs are. On the print it runs down the paper, because
        // the whole design is turned a quarter turn.
        let page = Label::new().band("FRAGILE").finish();
        // Its height is 20 px + 2 dots of margin on each side, so ~24 design rows, centred: print
        // columns 44..67. Check the middle of that strip over the paper's length.
        let strip = ink_in(&page, 50, 30, 61, LABEL_HEIGHT - 31);
        let full = (12 * (LABEL_HEIGHT - 60)) as u32;
        assert!(strip > full / 2, "the band is mostly ink ({strip} of {full})");
        assert!(strip < full, "the letters are knocked out of it ({strip} of {full})");
        // And the paper beside the band stays blank.
        assert_eq!(ink_in(&page, 80, 30, 100, LABEL_HEIGHT - 31), 0, "the band is a strip, not a wash");
    }

    #[test]
    fn the_corner_marks_sit_in_the_four_corners() {
        let page = Label::new().corner_marks(10).finish();
        for (x0, y0) in [(0, 0), (LABEL_WIDTH - 10, 0), (0, LABEL_HEIGHT - 10), (LABEL_WIDTH - 10, LABEL_HEIGHT - 10)] {
            assert!(ink_in(&page, x0, y0, x0 + 9, y0 + 9) > 20, "no mark at ({x0}, {y0})");
        }
        // They are brackets, not a frame: the middle of each edge stays blank.
        assert_eq!(ink_in(&page, 40, 0, LABEL_WIDTH - 41, 3), 0);
        assert_eq!(ink_in(&page, 40, LABEL_HEIGHT - 4, LABEL_WIDTH - 41, LABEL_HEIGHT - 1), 0);
    }

    #[test]
    fn the_text_keeps_its_room_between_the_decorations() {
        // A frame is drawn over the stack, so it must not swallow the text: the lines of a tag
        // with a frame still show inside it, and the frame does not move the stack.
        let plain = Label::new().line("MILK").line("2026-11").finish();
        let framed = Label::new().frame(4, 2).line("MILK").line("2026-11").finish();
        // The frame only adds ink; nothing of the text moved or was covered.
        assert!(ink_in(&plain, 20, 20, LABEL_WIDTH - 21, LABEL_HEIGHT - 21) > 0, "the sample text draws something");
        for y in 0..LABEL_HEIGHT {
            for x in 0..LABEL_WIDTH {
                if plain.get(x, y) {
                    assert!(framed.get(x, y), "the frame covered a dot of text at ({x}, {y})");
                }
            }
        }
    }
}
