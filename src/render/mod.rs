//! Turns text, pictures and codes into the printer's 1-bit, 384-dot-wide bitmaps. Everything the
//! preview shows and everything the printer gets comes out of this module, so what you see is
//! what you print.

mod codes;
mod dither;
mod picture;
mod text;

pub use codes::{CodeKind, CodeOpts, EcLevel, code};
pub use dither::Dither;
pub use picture::{PictureOpts, Prepared, Source, load_picture, picture, prepare, tone};
pub use text::{Align, FontChoice, TextOpts, text};

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
