//! Little drawing tools for the themes: lines, boxes, discs, speckle, and copying one bitmap onto
//! another. Everything works on the printer's 1-bit [`Bitmap`], 384 dots wide.

use crate::protocol::{Bitmap, WIDTH_BYTES, WIDTH_DOTS};

/// A tiny deterministic random source, so a theme looks the same every time it is drawn from the
/// same text (and tests can rely on it).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// 0..n
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    /// 0.0..1.0
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A seed from some text (FNV-1a).
pub fn seed_of(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

pub fn clear(b: &mut Bitmap, x: usize, y: usize) {
    if x < WIDTH_DOTS && y < b.height {
        b.data[y * WIDTH_BYTES + x / 8] &= !(0x80 >> (x % 8));
    }
}

fn flip(b: &mut Bitmap, x: usize, y: usize) {
    if x < WIDTH_DOTS && y < b.height {
        b.data[y * WIDTH_BYTES + x / 8] ^= 0x80 >> (x % 8);
    }
}

pub fn put(b: &mut Bitmap, x: i32, y: i32, on: bool) {
    if x < 0 || y < 0 {
        return;
    }
    if on {
        b.set(x as usize, y as usize);
    } else {
        clear(b, x as usize, y as usize);
    }
}

/// `src` drawn onto `dst` with its top-left at (`x`, `y`); black dots win.
pub fn or_at(dst: &mut Bitmap, src: &Bitmap, x: i32, y: i32) {
    blit(dst, src, x, y, Mode::Or);
}

/// `src` XORed onto `dst`: where both are black, the dot turns white. Draws white text on a disc.
pub fn xor_at(dst: &mut Bitmap, src: &Bitmap, x: i32, y: i32) {
    blit(dst, src, x, y, Mode::Xor);
}

#[derive(Clone, Copy)]
enum Mode {
    Or,
    Xor,
}

fn blit(dst: &mut Bitmap, src: &Bitmap, x: i32, y: i32, mode: Mode) {
    for sy in 0..src.height {
        for sx in 0..WIDTH_DOTS {
            if src.get(sx, sy) {
                let (dx, dy) = (sx as i32 + x, sy as i32 + y);
                if dx < 0 || dy < 0 {
                    continue;
                }
                match mode {
                    Mode::Or => dst.set(dx as usize, dy as usize),
                    Mode::Xor => flip(dst, dx as usize, dy as usize),
                }
            }
        }
    }
}

/// Copy the columns `x0..=x1` of `src` to `dst` at (`x`, `y`), keeping the dots that are set.
pub fn copy_cols(dst: &mut Bitmap, src: &Bitmap, x0: usize, x1: usize, x: i32, y: i32) {
    for sy in 0..src.height {
        for sx in x0..=x1.min(WIDTH_DOTS - 1) {
            if src.get(sx, sy) {
                put(dst, x + (sx - x0) as i32, y + sy as i32, true);
            }
        }
    }
}

/// The smallest box (x0, y0, x1, y1) holding every black dot, or `None` for a blank bitmap.
pub fn ink_box(b: &Bitmap) -> Option<(usize, usize, usize, usize)> {
    let mut found: Option<(usize, usize, usize, usize)> = None;
    for y in 0..b.height {
        for x in 0..WIDTH_DOTS {
            if b.get(x, y) {
                found = Some(match found {
                    None => (x, y, x, y),
                    Some((a, c, z, d)) => (a.min(x), c.min(y), z.max(x), d.max(y)),
                });
            }
        }
    }
    found
}

/// The rows `y0..y1` of a bitmap.
pub fn slice(b: &Bitmap, y0: usize, y1: usize) -> Bitmap {
    let y1 = y1.min(b.height);
    let y0 = y0.min(y1);
    Bitmap { height: y1 - y0, data: b.data[y0 * WIDTH_BYTES..y1 * WIDTH_BYTES].to_vec() }
}

/// Blank rows above and below.
pub fn pad(b: &Bitmap, top: usize, bottom: usize) -> Bitmap {
    let mut out = Bitmap::blank(top + b.height + bottom);
    out.data[top * WIDTH_BYTES..(top + b.height) * WIDTH_BYTES].copy_from_slice(&b.data);
    out
}

/// Every dot flipped.
pub fn invert(b: &mut Bitmap) {
    for byte in &mut b.data {
        *byte = !*byte;
    }
}

/// Flip the dots of the rows `y0..y1`.
pub fn invert_rows(b: &mut Bitmap, y0: usize, y1: usize) {
    for y in y0..y1.min(b.height) {
        for byte in &mut b.data[y * WIDTH_BYTES..(y + 1) * WIDTH_BYTES] {
            *byte = !*byte;
        }
    }
}

/// A line of dots with a pen `t` dots thick; `dash` is (on, off), or (0, 0) for solid.
pub fn hline(b: &mut Bitmap, y: usize, x0: usize, x1: usize, t: usize, dash: (usize, usize)) {
    let period = (dash.0 + dash.1).max(1);
    for x in x0..=x1.min(WIDTH_DOTS - 1) {
        if dash == (0, 0) || (x - x0) % period < dash.0 {
            for k in 0..t {
                b.set(x, y + k);
            }
        }
    }
}

pub fn vline(b: &mut Bitmap, x: usize, y0: usize, y1: usize, t: usize, dash: (usize, usize)) {
    let period = (dash.0 + dash.1).max(1);
    for y in y0..=y1.min(b.height.saturating_sub(1)) {
        if dash == (0, 0) || (y - y0) % period < dash.0 {
            for k in 0..t {
                b.set(x + k, y);
            }
        }
    }
}

/// A box outline, `t` dots thick, inside the corners (`x0`,`y0`) and (`x1`,`y1`).
pub fn rect(b: &mut Bitmap, x0: usize, y0: usize, x1: usize, y1: usize, t: usize) {
    hline(b, y0, x0, x1, t, (0, 0));
    hline(b, y1 + 1 - t, x0, x1, t, (0, 0));
    vline(b, x0, y0, y1, t, (0, 0));
    vline(b, x1 + 1 - t, y0, y1, t, (0, 0));
}

pub fn fill_rect(b: &mut Bitmap, x0: usize, y0: usize, x1: usize, y1: usize) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            b.set(x, y);
        }
    }
}

pub fn clear_rect(b: &mut Bitmap, x0: usize, y0: usize, x1: usize, y1: usize) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            clear(b, x, y);
        }
    }
}

/// A straight line with a square pen `t` dots across.
pub fn line(b: &mut Bitmap, x0: i32, y0: i32, x1: i32, y1: i32, t: i32) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=steps {
        let x = x0 + (x1 - x0) * i / steps;
        let y = y0 + (y1 - y0) * i / steps;
        for dy in 0..t {
            for dx in 0..t {
                put(b, x + dx - t / 2, y + dy - t / 2, true);
            }
        }
    }
}

/// A filled disc, or a ring `t` thick when `ring` is set.
pub fn disc(b: &mut Bitmap, cx: i32, cy: i32, r: i32, ring: Option<i32>) {
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = dx * dx + dy * dy;
            let inside = d2 <= r * r;
            let hollow = ring.is_some_and(|t| d2 < (r - t) * (r - t));
            if inside && !hollow {
                put(b, cx + dx, cy + dy, true);
            }
        }
    }
}

/// A white disc, punched out of whatever was there.
pub fn punch(b: &mut Bitmap, cx: i32, cy: i32, r: i32) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                put(b, cx + dx, cy + dy, false);
            }
        }
    }
}

/// Random single dots, like age spots and dust.
pub fn speckle(b: &mut Bitmap, rng: &mut Rng, count: usize, big: usize) {
    for _ in 0..count {
        let (x, y) = (rng.below(WIDTH_DOTS) as i32, rng.below(b.height.max(1)) as i32);
        let size = if big > 0 && rng.below(big) == 0 { 2 } else { 1 };
        for dy in 0..size {
            for dx in 0..size {
                put(b, x + dx, y + dy, true);
            }
        }
    }
}

/// A ragged, burnt edge down both sides, `max` dots deep at worst.
pub fn ragged_edges(b: &mut Bitmap, rng: &mut Rng, max: usize) {
    let mut depth = max / 2;
    for y in 0..b.height {
        let step = rng.below(3) as i32 - 1;
        depth = (depth as i32 + step).clamp(0, max as i32) as usize;
        for x in 0..depth {
            b.set(x, y);
            b.set(WIDTH_DOTS - 1 - x, y);
        }
    }
}

/// A frame inside the paper: two lines with diamonds at the corners.
pub fn ornate_border(b: &mut Bitmap, inset: usize) {
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    rect(b, inset, inset, w - inset, h - inset, 3);
    rect(b, inset + 7, inset + 7, w - inset - 7, h - inset - 7, 1);
    for (cx, cy) in [(inset + 3, inset + 3), (w - inset - 3, inset + 3), (inset + 3, h - inset - 3), (w - inset - 3, h - inset - 3)] {
        punch(b, cx as i32, cy as i32, 11);
        disc(b, cx as i32, cy as i32, 10, Some(2));
        disc(b, cx as i32, cy as i32, 4, None);
    }
}

/// Rounded corners on the filled box (`x0`,`y0`)–(`x1`,`y1`): the dots outside a radius-`r` curve go white.
pub fn round_corners(b: &mut Bitmap, x0: usize, y0: usize, x1: usize, y1: usize, r: usize) {
    for dy in 0..r {
        for dx in 0..r {
            let (ox, oy) = (r as i32 - 1 - dx as i32, r as i32 - 1 - dy as i32);
            if ox * ox + oy * oy > (r * r) as i32 - 2 {
                clear(b, x0 + dx, y0 + dy);
                clear(b, x1 - dx, y0 + dy);
                clear(b, x0 + dx, y1 - dy);
                clear(b, x1 - dx, y1 - dy);
            }
        }
    }
}

/// A zig-zag cut edge along the bottom, the way receipt paper is torn.
pub fn zigzag_bottom(b: &mut Bitmap, tooth: usize) {
    let h = b.height;
    for x in 0..WIDTH_DOTS {
        let t = x % (tooth * 2);
        let depth = if t < tooth { t } else { tooth * 2 - t };
        for k in 0..(tooth / 2).max(1) {
            if depth / 2 >= (tooth / 2).saturating_sub(k + 1) {
                continue;
            }
            clear(b, x, h - 1 - k);
        }
    }
}

/// Holes along both sides, like the tractor strip of continuous paper.
pub fn tractor_holes(b: &mut Bitmap, pitch: usize) {
    let mut y = pitch / 2;
    while y + 6 < b.height {
        for cx in [10, WIDTH_DOTS as i32 - 11] {
            disc(b, cx, y as i32, 5, Some(2));
        }
        y += pitch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(b: &Bitmap) -> u32 {
        b.data.iter().map(|x| x.count_ones()).sum()
    }

    #[test]
    fn the_random_source_repeats_for_the_same_seed() {
        let (mut a, mut b) = (Rng::new(seed_of("hello")), Rng::new(seed_of("hello")));
        assert_eq!((a.next(), a.below(10)), (b.next(), b.below(10)));
        assert_ne!(seed_of("a"), seed_of("b"));
        assert!((0..50).all(|_| (0.0..1.0).contains(&a.unit())));
    }

    #[test]
    fn lines_boxes_and_discs_leave_the_expected_dots() {
        let mut b = Bitmap::blank(20);
        hline(&mut b, 5, 10, 19, 1, (0, 0));
        assert_eq!(ink(&b), 10);
        let mut d = Bitmap::blank(20);
        hline(&mut d, 5, 0, 19, 1, (2, 2));
        assert_eq!(ink(&d), 10, "dashes are on half the time");
        let mut r = Bitmap::blank(10);
        rect(&mut r, 0, 0, 9, 9, 1);
        assert!(r.get(0, 5) && r.get(9, 5) && r.get(5, 0) && r.get(5, 9) && !r.get(5, 5));
        let mut c = Bitmap::blank(21);
        disc(&mut c, 10, 10, 5, None);
        punch(&mut c, 10, 10, 2);
        assert!(c.get(10, 6) && !c.get(10, 10));
    }

    #[test]
    fn copying_and_xor_combine_bitmaps() {
        let mut a = Bitmap::blank(4);
        a.set(1, 1);
        let mut dst = Bitmap::blank(8);
        or_at(&mut dst, &a, 3, 2);
        assert!(dst.get(4, 3));
        xor_at(&mut dst, &a, 3, 2);
        assert!(!dst.get(4, 3), "xor of the same dot clears it");
        assert_eq!(slice(&dst, 2, 5).height, 3);
        assert_eq!(pad(&a, 2, 3).height, 9);
    }

    #[test]
    fn the_decorations_stay_on_the_paper() {
        let mut b = Bitmap::blank(120);
        ornate_border(&mut b, 4);
        ragged_edges(&mut b, &mut Rng::new(3), 4);
        speckle(&mut b, &mut Rng::new(9), 40, 4);
        zigzag_bottom(&mut b, 8);
        tractor_holes(&mut b, 30);
        assert!(ink(&b) > 500);
        assert_eq!(b.data.len(), 120 * WIDTH_BYTES);
    }
}
