//! Paper effects for a finished ticket: a faint halftone screen, a coffee ring, a rubber stamp,
//! a tear-off perforation and a folded corner. They give plain tickets a bit of character
//! without touching what is already printed: every effect only ever *sets* dots, clips to the
//! 384-dot-wide paper and draws the same way for the same seed, so a theme stays repeatable.

use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw::{self, Rng};
use crate::render::text::{Align, FontChoice, TextOpts};

const TAU: f32 = std::f32::consts::TAU;

/// A faint grid of dots, like a cheap print screen. It only fills white paper, so text and
/// pictures keep their shape: `step` is the pitch between the dots (in dots), `dot` how many
/// dots across each one is. `(8, 2)` is a screen you notice, `(9, 1)` one you barely do.
pub fn halftone(b: &mut Bitmap, step: usize, dot: usize) {
    let last = b.height.saturating_sub(1);
    halftone_in(b, 0, 0, WIDTH_DOTS - 1, last, step, dot);
}

/// The same screen, only inside the box (`x0`,`y0`)–(`x1`,`y1`): texture behind one part of a
/// ticket (the discount of a coupon, say) without dulling the rest of the paper.
pub fn halftone_in(b: &mut Bitmap, x0: usize, y0: usize, x1: usize, y1: usize, step: usize, dot: usize) {
    // An empty or inside-out box draws nothing; `step_by(0)` would panic, so never 0.
    if x1 <= x0 || y1 <= y0 || b.height == 0 {
        return;
    }
    let (step, dot) = (step.max(1), dot.max(1));
    let (x1, y1) = (x1.min(WIDTH_DOTS - 1), y1.min(b.height - 1));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    // The grid sits in the middle of the box, not on its first row.
    let (gx0, gy0) = (x0 + step / 2, y0 + step / 2);
    for gy in (gy0..=y1).step_by(step) {
        for gx in (gx0..=x1).step_by(step) {
            for dy in 0..dot {
                for dx in 0..dot {
                    let (px, py) = (gx + dx, gy + dy);
                    if px > x1 || py > y1 || b.get(px, py) {
                        continue;
                    }
                    b.set(px, py);
                }
            }
        }
    }
}

/// A ring left standing on the ticket: a wobbly circle one or two dots thick, broken in one or
/// two places, with a few splashes around it. It only sets dots, so anything already printed
/// shows through the stain. Same `rng`, same ring.
pub fn coffee_ring(b: &mut Bitmap, cx: i32, cy: i32, r: i32, rng: &mut Rng) {
    let r = r.max(6);
    // One or two places where the ring did not take.
    let mut gaps: Vec<(f32, f32)> = Vec::new();
    for _ in 0..1 + rng.below(2) {
        gaps.push((rng.unit() * TAU, 0.14 + rng.unit() * 0.22));
    }
    // Two sine terms make the circle wobble without looking like a gear.
    let (p1, p2) = (rng.unit() * TAU, rng.unit() * TAU);
    let steps = (TAU * r as f32 * 2.0).max(24.0) as usize;
    for i in 0..steps {
        let a = i as f32 / steps as f32 * TAU;
        if gaps.iter().any(|(at, wide)| angle_between(a, *at) < *wide) {
            continue;
        }
        let wob = (3.0 * a + p1).sin() * 1.7 + (7.0 * a + p2).sin() * 0.9;
        let rr = r as f32 + wob * (r as f32 / 30.0).max(1.0);
        // Thicker here and there, the way a stain dries.
        let thick = if (11.0 * a + p1).sin() > 0.3 { 2 } else { 1 };
        for k in 0..thick {
            let rr = rr + k as f32;
            draw::put(b, cx + (rr * a.cos()) as i32, cy + (rr * a.sin()) as i32, true);
        }
    }
    // A few splashes thrown off the cup.
    for _ in 0..2 + rng.below(2) {
        let a = rng.unit() * TAU;
        let d = r as f32 * (1.12 + rng.unit() * 0.4);
        let (x, y) = (cx + (d * a.cos()) as i32, cy + (d * a.sin()) as i32);
        draw::put(b, x, y, true);
        if rng.below(3) == 0 {
            draw::put(b, x + 1, y, true);
            draw::put(b, x, y + 1, true);
        }
    }
}

/// The shortest distance between two angles, in radians.
fn angle_between(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % TAU;
    d.min(TAU - d)
}

/// A rubber stamp: a double frame with broken edges and a short word inside, laid on the ticket
/// turned by `angle` degrees. The ink of the word comes from the word itself, so `PAID` always
/// stamps the same way. Nothing under it is erased: the stamp only sets dots.
pub fn stamp(b: &mut Bitmap, cx: i32, cy: i32, w: usize, h: usize, text: &str, angle: f32) {
    if b.height == 0 {
        return;
    }
    let (w, h) = (w.clamp(24, WIDTH_DOTS), h.max(20));
    let pad = 5usize;
    let mut layer = Bitmap::blank(h + 2 * pad);
    let (x0, y0) = ((WIDTH_DOTS - w) / 2, pad);
    let (x1, y1) = (x0 + w - 1, pad + h - 1);
    // The wear comes from the word, so the same stamp always looks the same.
    let mut rng = Rng::new(draw::seed_of(&format!("{text}|{w}x{h}")));
    rough_rect(&mut layer, x0, y0, x1, y1, 2, &mut rng);
    rough_rect(&mut layer, x0 + 5, y0 + 5, x1 - 5, y1 - 5, 1, &mut rng);
    stamp_word(&mut layer, text, x0 + w / 2, y0 + h / 2, w, h);
    lay_rotated(b, &layer, cx, cy, angle);
}

/// The word of a stamp, shrunk until it fits inside the frame, then centred on (`cx`,`cy`).
fn stamp_word(layer: &mut Bitmap, word: &str, cx: usize, cy: usize, w: usize, h: usize) {
    let mut px = (h as f32 * 0.5).clamp(14.0, 72.0);
    for _ in 0..8 {
        let opts = TextOpts {
            px,
            font: FontChoice::Stencil,
            align: Align::Center,
            margin_x: 0.0,
            margin_y: 0.0,
            break_anywhere: true,
            ..TextOpts::default()
        };
        let Ok(glyphs) = crate::render::text::text(word, opts) else { return };
        let (gx0, gy0, gx1, gy1) = match draw::ink_box(&glyphs) {
            Some(b) => b,
            None => return,
        };
        let wide = gx1 - gx0 + 1;
        if wide <= w.saturating_sub(14) || px <= 14.0 {
            let dx = cx as i32 - (gx0 + gx1) as i32 / 2;
            let dy = cy as i32 - (gy0 + gy1) as i32 / 2;
            draw::or_at(layer, &glyphs, dx, dy);
            return;
        }
        px *= 0.82;
    }
}

/// A rectangle whose line is broken here and there, like a stamp that did not take evenly. The
/// wear is cut out of the stamp's own layer, never out of the ticket.
fn rough_rect(b: &mut Bitmap, x0: usize, y0: usize, x1: usize, y1: usize, t: usize, rng: &mut Rng) {
    if x1 < x0 || y1 < y0 || x1 - x0 + 1 < t || y1 - y0 + 1 < t {
        return;
    }
    draw::rect(b, x0, y0, x1, y1, t);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let on_edge = x < x0 + t || x + t > x1 || y < y0 + t || y + t > y1;
            if on_edge && rng.below(24) == 0 {
                draw::clear(b, x, y);
            }
        }
    }
}

/// Lay `layer` on `b` around (`cx`,`cy`), turned by `angle` degrees. The layer is sampled
/// backwards, so every dot of the stamp lands exactly once, dots outside the paper are skipped
/// and nothing on the ticket is cleared.
fn lay_rotated(b: &mut Bitmap, layer: &Bitmap, cx: i32, cy: i32, angle: f32) {
    if b.height == 0 || layer.height == 0 {
        return;
    }
    let (s, c) = (angle.to_radians().sin(), angle.to_radians().cos());
    let (lw, lh) = (WIDTH_DOTS as f32, layer.height as f32);
    let (lcx, lcy) = (lw / 2.0, lh / 2.0);
    // The box the turned layer covers, clipped to the paper.
    let (mut minx, mut miny) = (f32::MAX, f32::MAX);
    let (mut maxx, mut maxy) = (f32::MIN, f32::MIN);
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let (px, py) = (sx * lw / 2.0, sy * lh / 2.0);
        minx = minx.min(px * c - py * s + cx as f32);
        maxx = maxx.max(px * c - py * s + cx as f32);
        miny = miny.min(px * s + py * c + cy as f32);
        maxy = maxy.max(px * s + py * c + cy as f32);
    }
    let (x0, x1) = ((minx.floor() as i32).max(0), (maxx.ceil() as i32).min(WIDTH_DOTS as i32 - 1));
    let (y0, y1) = ((miny.floor() as i32).max(0), (maxy.ceil() as i32).min(b.height as i32 - 1));
    if x1 < x0 || y1 < y0 {
        return;
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 - cx as f32, y as f32 - cy as f32);
            // Where this paper dot came from in the layer, turned the other way.
            let (sx, sy) = (dx * c + dy * s + lcx, -dx * s + dy * c + lcy);
            let (ix, iy) = (sx.round() as i32, sy.round() as i32);
            if ix >= 0 && iy >= 0 && (ix as usize) < WIDTH_DOTS && layer.get(ix as usize, iy as usize) {
                b.set(x as usize, y as usize);
            }
        }
    }
}

/// A tear-off line: dashes across the paper, closing on the contour of a small notch bitten out
/// of each edge. The notches are drawn as an outline, because this never erases what is already
/// printed.
pub fn perforation(b: &mut Bitmap, y: usize, dash: usize, gap: usize) {
    if b.height == 0 {
        return;
    }
    let y = y.min(b.height - 1) as i32;
    let notch = 8i32;
    // The dashes stop short of the notches.
    let from = (notch as usize + 2).min(WIDTH_DOTS / 2);
    draw::hline(b, y as usize, from, WIDTH_DOTS - 1 - from, 1, (dash, gap));
    for (edge, dir) in [(0i32, 1i32), (WIDTH_DOTS as i32 - 1, -1)] {
        for k in -notch..=notch {
            let off = ((notch * notch - k * k) as f32).sqrt() as i32;
            draw::put(b, edge + dir * off, y + k, true);
        }
    }
}

/// A corner folded up at the bottom right: the crease, the back of the paper showing as a
/// triangle of dots, and a faint line beside the crease for the shadow of the fold.
pub fn dog_ear(b: &mut Bitmap, size: usize) {
    if b.height == 0 {
        return;
    }
    let size = size.min(WIDTH_DOTS - 1).min(b.height - 1);
    if size < 6 {
        return;
    }
    let (ex, ey) = (WIDTH_DOTS as i32 - 1, b.height as i32 - 1);
    let (size, d) = (size as i32, size as i32 + 3);
    // The crease itself, from one edge of the paper to the other.
    draw::line(b, ex - size, ey, ex, ey - size, 2);
    // The folded corner, showing the back of the paper as a triangle of dots.
    for dy in 0..size {
        for dx in 0..(size - dy) {
            if (dx + dy) % 2 == 0 {
                draw::put(b, ex - dx, ey - dy, true);
            }
        }
    }
    // A broken line a little way in: the shadow where the fold lifts.
    for dx in 0..=d {
        if (dx / 3) % 2 != 0 {
            continue;
        }
        draw::put(b, ex - dx, ey - (d - dx), true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WIDTH_BYTES;

    fn ink(b: &Bitmap) -> u32 {
        b.data.iter().map(|x| x.count_ones()).sum()
    }

    /// Every dot `before` had is still there: the effect only added ink.
    fn keeps(before: &Bitmap, after: &Bitmap) -> bool {
        before.data.iter().zip(&after.data).all(|(a, b)| a & b == *a)
    }

    /// One closure per effect, so every test runs them all.
    fn all() -> Vec<(&'static str, fn(&mut Bitmap))> {
        vec![
            ("halftone", |b| halftone(b, 8, 2)),
            ("halftone_in", |b| halftone_in(b, 20, 30, 300, 150, 7, 2)),
            ("coffee_ring", |b| coffee_ring(b, 120, 120, 50, &mut Rng::new(5))),
            ("stamp", |b| stamp(b, 190, 100, 160, 64, "CHECKED", -8.0)),
            ("perforation", |b| perforation(b, 120, 6, 6)),
            ("dog_ear", |b| dog_ear(b, 40)),
        ]
    }

    /// A busy little ticket: dots everywhere, then a box.
    fn ticket() -> Bitmap {
        let mut b = Bitmap::blank(240);
        for y in 0..240 {
            for x in (0..WIDTH_DOTS).step_by(5) {
                b.set(x, y);
            }
        }
        draw::rect(&mut b, 10, 10, WIDTH_DOTS - 11, 229, 2);
        b
    }

    #[test]
    fn every_effect_adds_ink_and_draws_the_same_way_twice() {
        for (name, effect) in all() {
            let (mut a, mut b) = (Bitmap::blank(240), Bitmap::blank(240));
            effect(&mut a);
            assert!(ink(&a) > 0, "{name} drew nothing");
            assert_eq!(a.data.len(), 240 * WIDTH_BYTES, "{name} changed the size of the paper");
            effect(&mut b);
            assert_eq!(a, b, "{name} is not deterministic");
        }
    }

    #[test]
    fn no_effect_erases_ink() {
        let base = ticket();
        for (name, effect) in all() {
            let mut b = base.clone();
            effect(&mut b);
            assert!(keeps(&base, &b), "{name} erased ink");
            assert!(ink(&b) >= ink(&base), "{name} lost ink");
        }
    }

    #[test]
    fn the_effects_stay_on_the_paper_whatever_they_are_asked_for() {
        // A paper three rows long, positions off every edge, silly sizes and empty words:
        // nothing panics and nothing runs off the bitmap.
        let mut tiny = Bitmap::blank(3);
        halftone(&mut tiny, 0, 0);
        halftone_in(&mut tiny, 300, 2, 10, 1, 0, 0);
        coffee_ring(&mut tiny, -400, -400, 4, &mut Rng::new(1));
        coffee_ring(&mut tiny, 900, 1, 4000, &mut Rng::new(1));
        stamp(&mut tiny, -50, 900, 10, 4, "", -90.0);
        stamp(&mut tiny, 400, -3, 900, 500, "PAID", 37.0);
        perforation(&mut tiny, 999, 0, 0);
        perforation(&mut tiny, 1, 99, 99);
        dog_ear(&mut tiny, 9999);
        dog_ear(&mut tiny, 0);
        assert_eq!(tiny.data.len(), 3 * WIDTH_BYTES, "the paper is still the same paper");
        // And a normal paper with everything asked for outside it.
        let mut wide = Bitmap::blank(200);
        stamp(&mut wide, -100, -100, 200, 80, "DENIED", 45.0);
        stamp(&mut wide, 500, 300, 200, 80, "DENIED", -45.0);
        coffee_ring(&mut wide, 0, 0, 300, &mut Rng::new(2));
        perforation(&mut wide, 199, 6, 6);
        dog_ear(&mut wide, 60);
        assert!(ink(&wide) > 0);
    }

    #[test]
    fn the_stamp_turns_and_the_perforation_bites_both_edges() {
        let mut straight = Bitmap::blank(160);
        stamp(&mut straight, 190, 80, 170, 60, "PAID", 0.0);
        let mut turned = Bitmap::blank(160);
        stamp(&mut turned, 190, 80, 170, 60, "PAID", -15.0);
        assert_ne!(straight, turned, "the angle changes where the dots land");
        assert!(ink(&straight) > 100 && ink(&turned) > 100);
        let mut p = Bitmap::blank(60);
        perforation(&mut p, 30, 6, 6);
        // A notch contour on each edge, and the dashes in between.
        assert!(p.get(0, 22) && p.get(WIDTH_DOTS - 1, 38), "both edges are bitten");
        assert!(p.get(12, 30), "the dashes run across");
        assert!(!p.get(4, 30), "the dashes stop at the notch");
    }
}
