//! Turns text, pictures and codes into the printer's 1-bit, 384-dot-wide bitmaps. Everything the
//! preview shows and everything the printer gets comes out of this module, so what you see is
//! what you print.

mod codes;
pub mod draw;
mod dither;
pub mod label;
mod picture;
pub mod presets;
mod text;
mod theme;

pub use codes::{CodeKind, CodeOpts, EcLevel, code, code128_modules};
pub use dither::{apply_width, Dither};
pub use label::Label;
pub use picture::{
    crop_window, crop_window_at, cropped, cropped_at, label_picture, label_picture_biased, label_prepare,
    label_tone, load_picture, picture, prepare, prepare_with, tone, Crop, PictureOpts, Prepared, Source,
    DOTS_PER_MM, LABEL_CORNER, LABEL_HEIGHT, LABEL_GAMMA, LABEL_INK_BIAS, LABEL_WIDTH, MAX_SPACE_MM,
};
pub use text::{Align, FontChoice, TextOpts, text, text_at};
pub use theme::{custom, render as themed, render_mapped as themed_mapped, themes_for, Family, Fields, Region, Target, Theme, MAX_FIELDS};

use crate::protocol::{Bitmap, WIDTH_DOTS};

/// Dots per centimetre of paper (203 dpi).
pub const DOTS_PER_CM: f32 = 203.0 / 2.54;

/// Length of a bitmap on paper, in centimetres.
pub fn length_cm(bitmap: &Bitmap) -> f32 {
    bitmap.height as f32 / DOTS_PER_CM
}

/// Stack bitmaps top to bottom with `gap` blank lines between them. All parts must share the
/// same width; the result uses the width of the first (or 384 dots when empty).
pub fn stack(parts: &[Bitmap], gap: usize) -> Bitmap {
    let width = parts.first().map_or(WIDTH_DOTS, |p| p.width);
    let height = parts.iter().map(|p| p.height).sum::<usize>() + gap * parts.len().saturating_sub(1);
    let mut out = Bitmap::blank_at(width, height);
    let mut y = 0;
    for part in parts {
        let wb = out.width_bytes();
        let at = y * wb;
        out.data[at..at + part.data.len()].copy_from_slice(&part.data);
        y += part.height + gap;
    }
    out
}

/// The last `rows` lines of a bitmap.
pub fn tail(bitmap: &Bitmap, rows: usize) -> Bitmap {
    let rows = rows.min(bitmap.height);
    let wb = bitmap.width_bytes();
    let from = (bitmap.height - rows) * wb;
    Bitmap { width: bitmap.width, height: rows, data: bitmap.data[from..].to_vec() }
}

/// A label bitmap: printed on the 112-dot head instead of the pocket roll's 384 dots, and read
/// lying down (see [`label`]).
fn is_label(bitmap: &Bitmap) -> bool {
    bitmap.width != WIDTH_DOTS
}

/// Side margin of the on-screen preview, in dots: the pocket ticket shows the blank paper on
/// either side of the 48 mm head, a label shows its liner.
pub fn preview_margin(width: usize) -> usize {
    if width == WIDTH_DOTS { PAPER_MARGIN } else { LABEL_MARGIN }
}

/// Rows of cut edge above the on-screen preview: the pocket ticket is torn from a roll, a label
/// has no teeth.
pub fn preview_teeth(width: usize) -> usize {
    if width == WIDTH_DOTS { TEETH } else { 0 }
}

/// Blank rows above/below a label on its liner (0 for the pocket roll).
pub fn preview_pad_y(width: usize) -> usize {
    if width == WIDTH_DOTS { 0 } else { LABEL_PAD }
}

/// Width of the on-screen preview of `bitmap`, in dots. A label leaves the head 112 × 240 but is
/// read lying down, so the preview shows it turned: 240 dots across, plus the liner on either
/// side. A pocket ticket shows the whole 58 mm roll.
pub fn preview_width(bitmap: &Bitmap) -> usize {
    let across = if is_label(bitmap) { bitmap.height } else { bitmap.width };
    across + 2 * preview_margin(bitmap.width)
}

/// Height of the on-screen preview of `bitmap`, in dots: a pocket ticket adds its toothed cut
/// edge, a label its pad of liner above and below.
pub fn preview_height(bitmap: &Bitmap) -> usize {
    if is_label(bitmap) {
        bitmap.width + 2 * preview_pad_y(bitmap.width)
    } else {
        bitmap.height.max(1) + preview_teeth(bitmap.width)
    }
}

/// A bitmap turned a quarter turn anticlockwise, the inverse of the turn [`label`] prints with:
/// `out(x, y) = in(W - 1 - y, x)`, where `W` is the width of the input. The 112 × 240 dots of a
/// label become the 240 × 112 design page again, so the preview can show the tag lying down.
pub fn rotate_ccw(bitmap: &Bitmap) -> Bitmap {
    let mut out = Bitmap::blank_at(bitmap.height, bitmap.width);
    for y in 0..out.height {
        for x in 0..out.width {
            if bitmap.get(bitmap.width - 1 - y, x) {
                out.set(x, y);
            }
        }
    }
    out
}

/// Black dots on cream paper, as RGBA, for the on-screen preview. The ticket stands up out of
/// the printer, like the real one: its first line is at the top, under a toothed cut edge, and
/// its last line is at the bottom, where it goes back into the slot (a little darker there).
pub fn preview_rgba(bitmap: &Bitmap) -> (u32, u32, Vec<u8>) {
    let height = bitmap.height.max(1);
    let margin = preview_margin(bitmap.width);
    let wide = bitmap.width + 2 * margin;
    if bitmap.width == WIDTH_DOTS {
        let paper = [250.0f32, 246.0, 236.0];
        let ink = [27u8, 22, 32, 255];
        let teeth = TEETH;
        let mut out = Vec::with_capacity(wide * (height + teeth) * 4);
        // The cut edge: a row of teeth pointing up, anti-aliased with alpha.
        let edge = [paper[0] as u8, paper[1] as u8, paper[2] as u8];
        if teeth > 0 {
            for k in 0..teeth {
                for x in 0..wide {
                    let t = (x % TOOTH_PERIOD) as f32;
                    let tri = if t < TOOTH_PERIOD as f32 / 2.0 { t } else { TOOTH_PERIOD as f32 - t };
                    let depth = tri * (teeth as f32 / (TOOTH_PERIOD as f32 / 2.0));
                    let alpha = (depth - (teeth - 1 - k) as f32).clamp(0.0, 1.0);
                    out.extend_from_slice(&[edge[0], edge[1], edge[2], (alpha * 255.0) as u8]);
                }
            }
        }
        for y in 0..height {
            let from_bottom = (height - 1 - y) as f32;
            let shade = 0.80 + 0.20 * (from_bottom / SHADE_ROWS as f32).min(1.0);
            let lit = [(paper[0] * shade) as u8, (paper[1] * shade) as u8, (paper[2] * shade) as u8, 255];
            // The strips the print head cannot reach: a touch darker than the printable paper.
            let rim = [(paper[0] * shade * 0.94) as u8, (paper[1] * shade * 0.94) as u8, (paper[2] * shade * 0.94) as u8, 255];
            for _ in 0..margin {
                out.extend_from_slice(&rim);
            }
            for x in 0..bitmap.width {
                out.extend_from_slice(if bitmap.get(x, y) { &ink } else { &lit });
            }
            for _ in 0..margin {
                out.extend_from_slice(&rim);
            }
        }
        return (wide as u32, (height + teeth) as u32, out);
    }

    // A label: a rounded white label on a beige liner, with a small shadow below. The head lays the
    // tag down at 112 × 240, but a label is read across 30 × 14 mm, so the bitmap is turned back a
    // quarter turn anticlockwise and the preview shows the tag lying down, as it sits on a shelf.
    let page = rotate_ccw(bitmap);
    let height = page.height;
    let margin = preview_margin(bitmap.width);
    let wide = page.width + 2 * margin;
    let pad_y = preview_pad_y(bitmap.width);
    let total_h = height + 2 * pad_y;
    let ink = [27u8, 22, 32, 255];
    let label_paper = [252u8, 250, 243, 255];
    let label_border = [216u8, 204, 180];
    let shadow = [200u8, 188, 160];
    let mut out = Vec::with_capacity(wide * total_h * 4);
    let (lw, lh) = (page.width as i32, height as i32);
    let (radius, margin_i, pad_i) = (10i32, margin as i32, pad_y as i32);
    let inside = |x: i32, y: i32, x0: i32, y0: i32, w: i32, h: i32, r: i32| -> bool {
        if x < x0 || y < y0 || x >= x0 + w || y >= y0 + h {
            return false;
        }
        let (cx, cy) = if x < x0 + r && y < y0 + r {
            (x0 + r, y0 + r)
        } else if x >= x0 + w - r && y < y0 + r {
            (x0 + w - 1 - r, y0 + r)
        } else if x < x0 + r && y >= y0 + h - r {
            (x0 + r, y0 + h - 1 - r)
        } else if x >= x0 + w - r && y >= y0 + h - r {
            (x0 + w - 1 - r, y0 + h - 1 - r)
        } else {
            return true;
        };
        let (dx, dy) = (x - cx, y - cy);
        dx * dx + dy * dy <= r * r
    };
    let is_corner = |x: i32, y: i32, x0: i32, y0: i32, w: i32, h: i32, r: i32| -> bool {
        (x < x0 + r && y < y0 + r)
            || (x >= x0 + w - r && y < y0 + r)
            || (x < x0 + r && y >= y0 + h - r)
            || (x >= x0 + w - r && y >= y0 + h - r)
    };
    let in_label = |x: i32, y: i32| inside(x, y, margin_i, pad_i, lw, lh, radius);
    for y in 0..total_h as i32 {
        // Beige liner, slightly darker at the bottom.
        let t = y as f32 / (total_h.saturating_sub(1).max(1)) as f32;
        let liner = [lerp_u8(232, 220, t), lerp_u8(220, 206, t), lerp_u8(194, 178, t), 255];
        for x in 0..wide as i32 {
            // Liner fills the whole preview (support band).
            if !in_label(x, y) {
                // Small shadow just below the label's bottom edge.
                let below = y - (pad_i + lh - 1);
                let under = below >= 1 && below <= 2 && x >= margin_i + 2 && x < margin_i + lw - 2;
                if under && !in_label(x, y - below) {
                    let mix = if below == 1 { 0.45 } else { 0.2 };
                    let c = [
                        lerp_u8(liner[0], shadow[0], mix),
                        lerp_u8(liner[1], shadow[1], mix),
                        lerp_u8(liner[2], shadow[2], mix),
                        255,
                    ];
                    out.extend_from_slice(&c);
                } else {
                    out.extend_from_slice(&liner);
                }
                continue;
            }
            // 1 px border around the rounded label.
            let border = (x == margin_i || y == pad_i || x == margin_i + lw - 1 || y == pad_i + lh - 1)
                || (is_corner(x, y, margin_i, pad_i, lw, lh, radius)
                    && !inside(x, y, margin_i + 1, pad_i + 1, lw - 2, lh - 2, (radius - 1).max(0)));
            if border {
                out.extend_from_slice(&[label_border[0], label_border[1], label_border[2], 255]);
                continue;
            }
            let bx = (x - margin_i) as usize;
            let by = (y - pad_i) as usize;
            out.extend_from_slice(if page.get(bx, by) { &ink } else { &label_paper });
        }
    }
    (wide as u32, total_h as u32, out)
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

/// A small picture of a ticket for a gallery: every 2 x 2 dots become one pixel, shaded by how
/// much of it is ink. The picture takes the bitmap's real width, so a pocket ticket gives 192
/// pixels across. A label leaves the head at 112 × 240 but is read lying down, so its picture is
/// taken from the page turned back a quarter turn (the same turn as the preview): 120 × 56.
/// The turn happens once here, not in [`thumb_rgba`] itself: the turned page is 240 dots wide,
/// which is still not the pocket's 384.
pub fn thumb_rgba(bitmap: &Bitmap) -> (u32, u32, Vec<u8>) {
    if is_label(bitmap) {
        return thumb_of(&rotate_ccw(bitmap));
    }
    thumb_of(bitmap)
}

/// The halving, shading part of [`thumb_rgba`], on a page already lying the right way.
fn thumb_of(bitmap: &Bitmap) -> (u32, u32, Vec<u8>) {
    let (w, h) = (bitmap.width / 2, bitmap.height.div_ceil(2).max(1));
    let (paper, ink) = ([250.0f32, 246.0, 236.0], [27.0f32, 22.0, 32.0]);
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let covered = (0..4).filter(|k| bitmap.get(x * 2 + k % 2, y * 2 + k / 2)).count() as f32 / 4.0;
            for c in 0..3 {
                out.push((paper[c] + (ink[c] - paper[c]) * covered) as u8);
            }
            out.push(255);
        }
    }
    (w as u32, h as u32, out)
}

/// Blank paper each side of the 48 mm the head prints: the roll is 58 mm (464 dots) wide.
pub const PAPER_MARGIN: usize = 40;
/// Liner shown each side of a label in the preview.
const LABEL_MARGIN: usize = 6;
/// Liner shown above and below a label in the preview.
const LABEL_PAD: usize = 4;
/// Width of the preview: the whole roll, not just the printable part.
pub const PREVIEW_WIDTH: usize = WIDTH_DOTS + 2 * PAPER_MARGIN;
/// Rows of cut edge added above the ticket by [`preview_rgba`].
pub const TEETH: usize = 12;
const TOOTH_PERIOD: usize = 24;
const SHADE_ROWS: usize = 28;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacking_keeps_every_part_and_the_gap() {
        let mut a = Bitmap::blank(2);
        a.set(0, 0);
        let mut b = Bitmap::blank(3);
        b.set(5, 2);
        let s = stack(&[a, b], 4);
        assert_eq!(s.height, 2 + 4 + 3);
        assert!(s.get(0, 0));
        assert!(s.get(5, 2 + 4 + 2));
    }

    #[test]
    fn the_tail_keeps_the_last_rows() {
        let mut b = Bitmap::blank(10);
        b.set(3, 9);
        let t = tail(&b, 4);
        assert_eq!(t.height, 4);
        assert!(t.get(3, 3));
        assert_eq!(tail(&b, 99).height, 10, "asking for more keeps everything");
    }

    #[test]
    fn a_ticket_ten_centimetres_long_is_about_800_lines() {
        let n = (10.0 * DOTS_PER_CM).round() as usize;
        assert!((795..=805).contains(&n));
    }

    #[test]
    fn a_gallery_picture_halves_the_ticket_and_shades_by_ink() {
        let mut b = Bitmap::blank(4);
        for x in 0..2 {
            for y in 0..2 {
                b.set(x, y);
            }
        }
        b.set(2, 0);
        let (w, h, px) = thumb_rgba(&b);
        assert_eq!((w as usize, h), (WIDTH_DOTS / 2, 2));
        assert!(px[0] < 60, "a fully inked square is dark");
        assert!(px[4] > 100 && px[4] < 230, "a quarter-inked one is a light grey");
        assert!(px[(WIDTH_DOTS / 2) * 4] > 240, "a blank one is paper");
    }

    #[test]
    fn a_label_thumbnail_is_landscape() {
        // A label leaves the head at 112 × 240; its gallery picture is turned the same way as its
        // preview, so the tag lies down there too: 240 / 2 across, 112 / 2 down.
        let mut b = Bitmap::blank_at(LABEL_WIDTH, LABEL_HEIGHT);
        for (x, y) in [(56, 2), (56, 3), (57, 2), (57, 3)] {
            b.set(x, y);
        }
        let (w, h, px) = thumb_rgba(&b);
        assert_eq!((w as usize, h as usize), (120, 56), "240 / 2 across and 112 / 2 down");
        assert_eq!(px.len(), 120 * 56 * 4, "one RGBA pixel a 2 x 2 block of dots");
        let dark = |x: usize, y: usize| px[(y * 120 + x) * 4] < 60;
        // The same anticlockwise turn as the preview: the print's top rows are the tag's left.
        assert!(dark(1, 27), "the ink lands at (2..4, 54..56) on the turned page");
        assert!(!dark(118, 28), "and not at the right, where a clockwise turn would put it");
    }

    #[test]
    fn the_preview_marks_black_dots_darker_than_the_paper() {
        let mut b = Bitmap::blank(1);
        b.set(0, 0);
        let (w, h, px) = preview_rgba(&b);
        assert_eq!((w, h), (PREVIEW_WIDTH as u32, 1 + TEETH as u32));
        let ticket = &px[(TEETH * PREVIEW_WIDTH + PAPER_MARGIN) * 4..];
        assert!(ticket[0] < 100 && ticket[4] > 150, "ink is dark, paper is light");
        let alphas: Vec<u8> = px[..PREVIEW_WIDTH * 4].chunks(4).map(|p| p[3]).collect();
        assert!(alphas.iter().any(|a| *a == 255) && alphas.iter().any(|a| *a < 255), "the top edge is toothed");
    }

    #[test]
    fn a_label_preview_is_landscape() {
        // The head lays a label down at 112 × 240, but a tag is read across 30 × 14 mm: the preview
        // turns it back a quarter turn, so it shows 240 dots across plus the liner, 112 down plus
        // its pad.
        let b = Bitmap::blank_at(LABEL_WIDTH, LABEL_HEIGHT);
        assert_eq!((preview_width(&b), preview_height(&b)), (252, 120), "240 + 2 × 6 across, 112 + 2 × 4 down");
        let (w, h, px) = preview_rgba(&b);
        assert_eq!((w as usize, h as usize), (252, 120), "the preview image is the label lying down");
        assert_eq!(px.len(), 252 * 120 * 4, "one RGBA pixel a dot");
        // The turn is anticlockwise, the inverse of the one that made the print: a dot near the top
        // of the print (its first rows) lands at the left of the tag, where the first column of the
        // design page is, not at its right.
        let mut ink = Bitmap::blank_at(LABEL_WIDTH, LABEL_HEIGHT);
        ink.set(56, 2);
        let (_, _, px) = preview_rgba(&ink);
        let dark = |x: usize, y: usize| {
            let i = (y * 252 + x) * 4;
            px[i] < 100
        };
        assert!(dark(6 + 2, 4 + 55), "the print's top rows are the tag's left, at (8, 59)");
        assert!(!dark(6 + 237, 4 + 56), "and not its right, where a clockwise turn would put them");
    }

    #[test]
    fn the_corners_of_a_label_preview_show_the_liner() {
        let b = Bitmap::blank_at(LABEL_WIDTH, LABEL_HEIGHT);
        let (w, h, px) = preview_rgba(&b);
        let (w, h) = (w as usize, h as usize);
        let at = |x: usize, y: usize| {
            let i = (y * w + x) * 4;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            let c = at(x, y);
            assert!(c[0] >= 215 && c[1] >= 200 && c[2] >= 170 && c[3] == 255, "({x}, {y}) is beige liner, got {c:?}");
            assert_ne!(c, [252, 250, 243, 255], "({x}, {y}) is not label paper");
        }
        // The liner is a band around the tag, not only its corners.
        let top = at(w / 2, 0);
        assert!(top[0] >= 215 && top[1] >= 200 && top[2] >= 170, "the band above the tag is liner, got {top:?}");
        // The middle is the blank tag itself.
        let centre = at(w / 2, h / 2);
        assert!(centre[0] >= 240 && centre[1] >= 238 && centre[2] >= 220, "the centre is label paper, got {centre:?}");
    }
}
