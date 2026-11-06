//! Turns text, pictures and codes into the printer's 1-bit, 384-dot-wide bitmaps. Everything the
//! preview shows and everything the printer gets comes out of this module, so what you see is
//! what you print.

mod codes;
pub mod draw;
mod dither;
mod picture;
mod text;
mod theme;

pub use codes::{CodeKind, CodeOpts, EcLevel, code, code128_modules};
pub use dither::Dither;
pub use picture::{PictureOpts, Prepared, Source, load_picture, picture, prepare, tone};
pub use text::{Align, FontChoice, TextOpts, text};
pub use theme::{Family, Fields, MAX_FIELDS, Theme, render as themed};

use crate::protocol::{Bitmap, WIDTH_DOTS};

/// Dots per centimetre of paper (203 dpi).
pub const DOTS_PER_CM: f32 = 203.0 / 2.54;

/// Length of a bitmap on paper, in centimetres.
pub fn length_cm(bitmap: &Bitmap) -> f32 {
    bitmap.height as f32 / DOTS_PER_CM
}

/// Stack bitmaps top to bottom with `gap` blank lines between them.
pub fn stack(parts: &[Bitmap], gap: usize) -> Bitmap {
    let height = parts.iter().map(|p| p.height).sum::<usize>() + gap * parts.len().saturating_sub(1);
    let mut out = Bitmap::blank(height);
    let mut y = 0;
    for part in parts {
        let at = y * crate::protocol::WIDTH_BYTES;
        out.data[at..at + part.data.len()].copy_from_slice(&part.data);
        y += part.height + gap;
    }
    out
}

/// The last `rows` lines of a bitmap.
pub fn tail(bitmap: &Bitmap, rows: usize) -> Bitmap {
    let rows = rows.min(bitmap.height);
    let from = (bitmap.height - rows) * crate::protocol::WIDTH_BYTES;
    Bitmap { height: rows, data: bitmap.data[from..].to_vec() }
}

/// Black dots on cream paper, as RGBA, for the on-screen preview. The ticket stands up out of
/// the printer, like the real one: its first line is at the top, under a toothed cut edge, and
/// its last line is at the bottom, where it goes back into the slot (a little darker there).
pub fn preview_rgba(bitmap: &Bitmap) -> (u32, u32, Vec<u8>) {
    let paper = [250.0f32, 246.0, 236.0];
    let ink = [27u8, 22, 32, 255];
    let height = bitmap.height.max(1);
    let mut out = Vec::with_capacity(WIDTH_DOTS * (height + TEETH) * 4);
    // The cut edge: a row of teeth pointing up, anti-aliased with alpha.
    let edge = [paper[0] as u8, paper[1] as u8, paper[2] as u8];
    for k in 0..TEETH {
        for x in 0..WIDTH_DOTS {
            let t = (x % TOOTH_PERIOD) as f32;
            let tri = if t < TOOTH_PERIOD as f32 / 2.0 { t } else { TOOTH_PERIOD as f32 - t };
            let depth = tri * (TEETH as f32 / (TOOTH_PERIOD as f32 / 2.0));
            let alpha = (depth - (TEETH - 1 - k) as f32).clamp(0.0, 1.0);
            out.extend_from_slice(&[edge[0], edge[1], edge[2], (alpha * 255.0) as u8]);
        }
    }
    for y in 0..height {
        let from_bottom = (height - 1 - y) as f32;
        let shade = 0.80 + 0.20 * (from_bottom / SHADE_ROWS as f32).min(1.0);
        let lit = [(paper[0] * shade) as u8, (paper[1] * shade) as u8, (paper[2] * shade) as u8, 255];
        for x in 0..WIDTH_DOTS {
            out.extend_from_slice(if bitmap.get(x, y) { &ink } else { &lit });
        }
    }
    (WIDTH_DOTS as u32, (height + TEETH) as u32, out)
}

/// A small picture of a ticket for a gallery: every 2 x 2 dots become one pixel, shaded by how
/// much of it is ink.
pub fn thumb_rgba(bitmap: &Bitmap) -> (u32, u32, Vec<u8>) {
    let (w, h) = (WIDTH_DOTS / 2, bitmap.height.div_ceil(2).max(1));
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
    fn the_preview_marks_black_dots_darker_than_the_paper() {
        let mut b = Bitmap::blank(1);
        b.set(0, 0);
        let (w, h, px) = preview_rgba(&b);
        assert_eq!((w, h), (WIDTH_DOTS as u32, 1 + TEETH as u32));
        let ticket = &px[TEETH * WIDTH_DOTS * 4..];
        assert!(ticket[0] < 100 && ticket[4] > 150, "ink is dark, paper is light");
        let alphas: Vec<u8> = px[..WIDTH_DOTS * 4].chunks(4).map(|p| p[3]).collect();
        assert!(alphas.iter().any(|a| *a == 255) && alphas.iter().any(|a| *a < 255), "the top edge is toothed");
    }
}
