//! Parchment & old world: scroll, royal decree, wanted poster, treasure map.

use super::{Ctx, Page, Theme, fit_line};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw::{self, Rng};
use crate::render::text::{self, Align, FontChoice, TextOpts};

type R = Result<Bitmap, String>;

/// A letter in a dot-sized glyph bitmap, centred on (`cx`, `cy`) of `b` and XORed so that it comes
/// out white on whatever is black underneath.
fn stamp_letter(b: &mut Bitmap, letter: &str, font: FontChoice, px: f32, cx: i32, cy: i32) {
    let Ok(glyph) = text::text(letter, TextOpts { px, font, margin_x: 0.0, margin_y: 0.0, align: Align::Left, ..TextOpts::default() }) else { return };
    if let Some((x0, y0, x1, y1)) = draw::ink_box(&glyph) {
        let (w, h) = ((x1 - x0) as i32, (y1 - y0) as i32);
        draw::xor_at(b, &glyph, cx - w / 2 - x0 as i32, cy - h / 2 - y0 as i32);
    }
}

/// A wax seal, `r` dots across at its widest, with a letter pressed into it.
fn seal(letter: &str, r: i32) -> Bitmap {
    let size = (r * 2 + 16) as usize;
    let mut b = Bitmap::blank(size);
    let (cx, cy) = (WIDTH_DOTS as i32 / 2, size as i32 / 2);
    draw::disc(&mut b, cx, cy, r - 4, None);
    for k in 0..16 {
        let a = k as f32 / 16.0 * std::f32::consts::TAU;
        draw::disc(&mut b, cx + ((r - 5) as f32 * a.cos()) as i32, cy + ((r - 5) as f32 * a.sin()) as i32, 5, None);
    }
    // A thin ring pressed into the wax, then the letter.
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = dx * dx + dy * dy;
            if d2 < (r - 10) * (r - 10) && d2 > (r - 13) * (r - 13) {
                draw::put(&mut b, cx + dx, cy + dy, false);
            }
        }
    }
    stamp_letter(&mut b, letter, FontChoice::OldEnglish, r as f32 * 1.1, cx, cy);
    b
}

fn crown() -> Bitmap {
    let mut b = Bitmap::blank(56);
    let (cx, base) = (WIDTH_DOTS as i32 / 2, 46);
    let pts = [(-34, 0), (-34, -26), (-17, -10), (0, -34), (17, -10), (34, -26), (34, 0), (-34, 0)];
    for w in pts.windows(2) {
        draw::line(&mut b, cx + w[0].0, base + w[0].1, cx + w[1].0, base + w[1].1, 3);
    }
    draw::line(&mut b, cx - 34, base - 8, cx + 34, base - 8, 2);
    for (dx, dy) in [(-34, -26), (0, -34), (34, -26)] {
        draw::disc(&mut b, cx + dx, base + dy - 5, 5, None);
    }
    b
}

fn first_letter(s: &str) -> String {
    s.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "R".into())
}

fn age(b: &mut Bitmap, c: &Ctx, salt: u64) {
    let mut rng = c.rng(salt);
    let n = b.height / 5;
    draw::speckle(b, &mut rng, n, 5);
    draw::ragged_edges(b, &mut rng, 3);
}

pub fn parchment(c: &Ctx) -> R {
    let t = Theme::Parchment;
    let mut p = Page::new(26.0);
    p.gap(36);
    p.push(fit_line(&c.field(t, "title"), FontChoice::OldEnglish, 34.0, 52.0)?);
    p.gap(4);
    p.flourish();
    p.gap(8);
    p.text_gap(c.body.trim(), FontChoice::Papyrus, 30.0, Align::Left, 4.0)?;
    p.gap(8);
    p.flourish();
    p.gap(12);
    let signed = c.field(t, "signed");
    p.push(seal(&first_letter(if signed.is_empty() { c.body } else { &signed }), 38));
    p.gap(34);
    let mut b = p.done();
    draw::ornate_border(&mut b, 8);
    age(&mut b, c, 11);
    Ok(b)
}

pub fn decree(c: &Ctx) -> R {
    let t = Theme::Decree;
    let mut p = Page::new(30.0);
    p.gap(26);
    p.push(crown());
    p.gap(6);
    p.push(fit_line(&c.field(t, "title"), FontChoice::OldEnglish, 30.0, 54.0)?);
    p.gap(4);
    p.flourish();
    p.gap(8);
    p.text_gap(c.body.trim(), FontChoice::Serif, 28.0, Align::Left, 3.0)?;
    p.gap(14);
    p.text("Given under our hand and seal", FontChoice::Chancery, 26.0, Align::Center)?;
    p.gap(4);
    let signed = c.field(t, "signed");
    p.text(&signed, FontChoice::Chancery, 40.0, Align::Center)?;
    p.gap(8);
    p.push(seal(&first_letter(&signed), 36));
    p.gap(26);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::rect(&mut b, 6, 6, w - 6, h - 6, 4);
    draw::rect(&mut b, 14, 14, w - 14, h - 14, 1);
    let mut rng = c.rng(2);
    let n = b.height / 12;
    draw::speckle(&mut b, &mut rng, n, 6);
    Ok(b)
}

fn mugshot() -> Bitmap {
    let mut b = Bitmap::blank(214);
    let (x0, x1, y0, y1) = (80usize, 304usize, 6usize, 207usize);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if (x + y) % 6 == 0 {
                b.set(x, y);
            }
        }
    }
    let (cx, cy) = (192i32, 88i32);
    // Shoulders, cut off by the frame at the bottom.
    for y in 130..=y1 as i32 {
        for x in x0 as i32..=x1 as i32 {
            let (nx, ny) = ((x - cx) as f32 / 96.0, (y - 205) as f32 / 74.0);
            if nx * nx + ny * ny <= 1.0 {
                b.set(x as usize, y as usize);
            }
        }
    }
    draw::punch(&mut b, cx, cy, 48);
    draw::disc(&mut b, cx, cy, 40, None);
    draw::rect(&mut b, x0, y0, x1, y1, 4);
    b
}

pub fn wanted(c: &Ctx) -> R {
    let t = Theme::Wanted;
    let mut p = Page::new(24.0);
    p.gap(26);
    p.shout("WANTED", FontChoice::Playbill, 130.0)?;
    p.text("DEAD OR ALIVE", FontChoice::Stencil, 30.0, Align::Center)?;
    p.gap(8);
    p.push(mugshot());
    p.gap(8);
    p.push(fit_line(&c.field(t, "name").to_uppercase(), FontChoice::Impact, 30.0, 64.0)?);
    p.gap(4);
    p.text(c.body.trim(), FontChoice::Typewriter, 24.0, Align::Center)?;
    p.gap(10);
    p.text("REWARD", FontChoice::Stencil, 32.0, Align::Center)?;
    p.push(fit_line(&c.field(t, "reward"), FontChoice::Impact, 30.0, 96.0)?);
    p.gap(26);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::rect(&mut b, 4, 4, w - 4, h - 4, 5);
    draw::rect(&mut b, 14, 14, w - 14, h - 14, 1);
    let mut rng = c.rng(3);
    let n = b.height / 8;
    draw::speckle(&mut b, &mut rng, n, 4);
    Ok(b)
}

/// The dotted trail, the mountains and the compass on a dashed-edge map.
fn map(rng: &mut Rng) -> Bitmap {
    let mut b = Bitmap::blank(250);
    draw::rect(&mut b, 22, 4, 361, 245, 2);
    // Mountains.
    for _ in 0..6 {
        let (x, y) = (70 + rng.below(240) as i32, 100 + rng.below(110) as i32);
        draw::line(&mut b, x - 14, y, x, y - 18, 2);
        draw::line(&mut b, x, y - 18, x + 14, y, 2);
        draw::line(&mut b, x - 3, y - 11, x + 4, y - 7, 1);
    }
    // The trail: a dotted wave from the lower left to the X.
    let (mut prev, steps) = ((0i32, 0i32), 90);
    for i in 0..=steps {
        let s = i as f32 / steps as f32;
        let x = 62.0 + 240.0 * s + 14.0 * (s * 14.0).sin();
        let y = 220.0 - 150.0 * s + 22.0 * (s * 9.0).cos();
        let (x, y) = (x as i32, y as i32);
        if i % 2 == 0 {
            draw::disc(&mut b, x, y, 2, None);
        }
        prev = (x, y);
    }
    draw::line(&mut b, prev.0 - 12, prev.1 - 12, prev.0 + 12, prev.1 + 12, 5);
    draw::line(&mut b, prev.0 - 12, prev.1 + 12, prev.0 + 12, prev.1 - 12, 5);
    // The compass rose.
    let (cx, cy) = (74, 56);
    draw::disc(&mut b, cx, cy, 24, Some(2));
    draw::line(&mut b, cx, cy - 34, cx, cy + 34, 2);
    draw::line(&mut b, cx - 34, cy, cx + 34, cy, 2);
    draw::line(&mut b, cx - 12, cy - 12, cx + 12, cy + 12, 1);
    draw::line(&mut b, cx - 12, cy + 12, cx + 12, cy - 12, 1);
    draw::disc(&mut b, cx, cy, 5, None);
    stamp_letter_black(&mut b, "N", cx, cy - 46);
    b
}

fn stamp_letter_black(b: &mut Bitmap, letter: &str, cx: i32, cy: i32) {
    if let Ok(g) = text::text(letter, TextOpts { px: 22.0, font: FontChoice::Serif, margin_x: 0.0, margin_y: 0.0, align: Align::Left, ..TextOpts::default() }) {
        if let Some((x0, y0, x1, y1)) = draw::ink_box(&g) {
            draw::or_at(b, &g, cx - ((x1 - x0) / 2) as i32 - x0 as i32, cy - ((y1 - y0) / 2) as i32 - y0 as i32);
        }
    }
}

pub fn treasure(c: &Ctx) -> R {
    let t = Theme::Treasure;
    let mut rng = c.rng(4);
    let mut p = Page::new(26.0);
    p.gap(24);
    p.push(fit_line(&c.field(t, "place"), FontChoice::Chancery, 34.0, 58.0)?);
    p.gap(6);
    p.push(map(&mut rng));
    p.gap(10);
    for (i, line) in c.lines().iter().enumerate() {
        p.text(&format!("{}. {line}", i + 1), FontChoice::Papyrus, 30.0, Align::Left)?;
        p.gap(3);
    }
    p.gap(22);
    let mut b = p.done();
    draw::ornate_border(&mut b, 8);
    age(&mut b, c, 5);
    Ok(b)
}
