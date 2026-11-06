//! Fun official papers: boarding pass, movie ticket, fortune, diploma, horoscope, fridge note,
//! checklist, ransom note, name tag.

use super::{Ctx, Page, Theme, fit_line};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw::{self, Rng};
use crate::render::text::{self, Align, FontChoice, TextOpts};

type R = Result<Bitmap, String>;

/// One symbol or word, drawn alone and centred on the paper.
fn centred(s: &str, font: FontChoice, px: f32) -> Result<Bitmap, String> {
    text::text(s, TextOpts { px, font, align: Align::Center, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() })
}

/// Half-circle notches punched out of both edges at row `y`, where a ticket tears.
fn notches(b: &mut Bitmap, y: usize, r: i32) {
    draw::punch(b, 0, y as i32, r);
    draw::punch(b, WIDTH_DOTS as i32 - 1, y as i32, r);
}

pub fn boarding(c: &Ctx) -> R {
    let t = Theme::Boarding;
    let mut p = Page::new(20.0);
    p.band("BOARDING PASS", FontChoice::Bold, 32.0, Align::Center)?;
    p.gap(10);
    let opts = |align| TextOpts { px: 84.0, font: FontChoice::Impact, align, margin_x: 20.0, margin_y: 0.0, ..TextOpts::default() };
    let from = text::text(&c.field(t, "from").to_uppercase(), opts(Align::Left))?;
    let to = text::text(&c.field(t, "to").to_uppercase(), opts(Align::Right))?;
    let plane = centred("\u{2708}", FontChoice::Symbols, 54.0)?;
    let mut row = Bitmap::blank(from.height.max(to.height));
    draw::or_at(&mut row, &from, 0, 0);
    draw::or_at(&mut row, &to, 0, 0);
    let lift = (row.height.saturating_sub(plane.height)) as i32 / 2;
    draw::or_at(&mut row, &plane, 0, lift);
    p.push(row);
    p.gap(6);
    p.text("PASSENGER", FontChoice::Sans, 16.0, Align::Left)?;
    p.text(&c.field(t, "name").to_uppercase(), FontChoice::Bold, 30.0, Align::Left)?;
    p.gap(6);
    let mut rng = c.rng(7);
    let gate = format!("{}{}", (b'A' + rng.below(6) as u8) as char, 1 + rng.below(30));
    let flight = format!("TR {}", c.number(7, 4));
    p.leader("FLIGHT", &flight, FontChoice::Typewriter, 26.0)?;
    p.leader("GATE", &gate, FontChoice::Typewriter, 26.0)?;
    p.leader("SEAT", &c.field(t, "seat").to_uppercase(), FontChoice::Typewriter, 26.0)?;
    p.leader("BOARDING", &format!("{:02}:{:02}", 5 + rng.below(17), 5 * rng.below(12)), FontChoice::Typewriter, 26.0)?;
    p.gap(10);
    let tear = p.height() + 4;
    p.rule((7, 6), 2, 4);
    p.gap(6);
    p.barcode(&format!("{flight}{}", c.number(8, 8)), 60)?;
    p.gap(4);
    p.text(c.body.trim(), FontChoice::Sans, 20.0, Align::Center)?;
    p.gap(12);
    let mut b = p.done();
    {
        let bottom = b.height - 1;
        draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 2);
    }
    notches(&mut b, tear, 12);
    Ok(b)
}

pub fn movie(c: &Ctx) -> R {
    let t = Theme::Movie;
    let mut p = Page::new(26.0);
    p.gap(24);
    p.shout("ADMIT ONE", FontChoice::Playbill, 100.0)?;
    p.push(centred("\u{2605} \u{2605} \u{2605} \u{2605} \u{2605}", FontChoice::Symbols, 28.0)?);
    p.gap(8);
    p.shout(&c.body.trim().to_uppercase(), FontChoice::Impact, 58.0)?;
    p.gap(10);
    p.rule((0, 0), 2, 4);
    p.leader("ROOM", &c.field(t, "room"), FontChoice::Typewriter, 26.0)?;
    p.leader("SEAT", &c.field(t, "seat"), FontChoice::Typewriter, 26.0)?;
    p.leader("TIME", &c.field(t, "time"), FontChoice::Typewriter, 26.0)?;
    p.rule((0, 0), 2, 4);
    p.text(&format!("No. {}", c.number(9, 6)), FontChoice::Typewriter, 22.0, Align::Center)?;
    p.gap(10);
    let tear = p.height();
    p.rule((7, 6), 2, 3);
    p.gap(8);
    p.barcode(&c.number(10, 10), 44)?;
    p.gap(18);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::rect(&mut b, 8, 8, w - 8, h - 8, 3);
    draw::rect(&mut b, 14, 14, w - 14, h - 14, 1);
    notches(&mut b, tear + 4, 14);
    Ok(b)
}

const COLOURS: [&str; 8] = ["teal", "amber", "violet", "coral", "green", "indigo", "gold", "pink"];
const MOODS: [&str; 6] = ["curious", "unstoppable", "sleepy", "generous", "mischievous", "calm"];

pub fn fortune(c: &Ctx) -> R {
    let mut p = Page::new(30.0);
    p.gap(26);
    p.text("~ your fortune ~", FontChoice::Chancery, 26.0, Align::Center)?;
    p.gap(6);
    p.text(c.body.trim(), FontChoice::Typewriter, 28.0, Align::Center)?;
    p.gap(12);
    p.text("LUCKY NUMBERS", FontChoice::Sans, 16.0, Align::Center)?;
    let mut rng = c.rng(12);
    let mut nums: Vec<usize> = Vec::new();
    while nums.len() < 6 {
        let n = 1 + rng.below(49);
        if !nums.contains(&n) {
            nums.push(n);
        }
    }
    nums.sort_unstable();
    let list = nums.iter().map(|n| format!("{n:02}")).collect::<Vec<_>>().join("  ");
    p.text(&list, FontChoice::Bold, 28.0, Align::Center)?;
    p.gap(26);
    let mut b = p.done();
    draw::ornate_border(&mut b, 6);
    Ok(b)
}

pub fn diploma(c: &Ctx) -> R {
    let t = Theme::Diploma;
    let mut p = Page::new(36.0);
    p.gap(38);
    p.push(fit_line("CERTIFICATE", FontChoice::OldEnglish, 36.0, 52.0)?);
    p.push(fit_line(&c.field(t, "award"), FontChoice::Chancery, 36.0, 34.0)?);
    p.gap(6);
    p.flourish();
    p.gap(6);
    p.text("is proudly presented to", FontChoice::Serif, 22.0, Align::Center)?;
    p.gap(4);
    p.push(fit_line(&c.field(t, "name"), FontChoice::Chancery, 36.0, 64.0)?);
    p.gap(6);
    p.text(c.body.trim(), FontChoice::Serif, 26.0, Align::Center)?;
    p.gap(14);
    let mut sign = Bitmap::blank(34);
    draw::hline(&mut sign, 28, 90, WIDTH_DOTS - 91, 2, (0, 0));
    let name = text::text(&c.field(t, "signed"), TextOpts { px: 28.0, font: FontChoice::Chancery, align: Align::Center, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() })?;
    draw::or_at(&mut sign, &name, 0, -2);
    p.push(sign);
    p.gap(14);
    let mut seal = Bitmap::blank(80);
    draw::disc(&mut seal, WIDTH_DOTS as i32 / 2, 40, 34, None);
    for k in 0..14 {
        let a = k as f32 / 14.0 * std::f32::consts::TAU;
        draw::disc(&mut seal, WIDTH_DOTS as i32 / 2 + (32.0 * a.cos()) as i32, 40 + (32.0 * a.sin()) as i32, 5, None);
    }
    draw::disc(&mut seal, WIDTH_DOTS as i32 / 2, 40, 24, Some(2));
    let star = centred("\u{2605}", FontChoice::Symbols, 36.0)?;
    if let Some((x0, y0, x1, y1)) = draw::ink_box(&star) {
        draw::xor_at(&mut seal, &star, WIDTH_DOTS as i32 / 2 - ((x0 + x1) / 2) as i32, 40 - ((y0 + y1) / 2) as i32);
    }
    p.push(seal);
    p.gap(38);
    let mut b = p.done();
    draw::ornate_border(&mut b, 8);
    Ok(b)
}

const SIGNS: [(&str, &str, &str); 12] = [
    ("aries", "belier", "\u{2648}"),
    ("taurus", "taureau", "\u{2649}"),
    ("gemini", "gemeaux", "\u{264A}"),
    ("cancer", "cancer", "\u{264B}"),
    ("leo", "lion", "\u{264C}"),
    ("virgo", "vierge", "\u{264D}"),
    ("libra", "balance", "\u{264E}"),
    ("scorpio", "scorpion", "\u{264F}"),
    ("sagittarius", "sagittaire", "\u{2650}"),
    ("capricorn", "capricorne", "\u{2651}"),
    ("aquarius", "verseau", "\u{2652}"),
    ("pisces", "poissons", "\u{2653}"),
];

fn zodiac(name: &str) -> &'static str {
    let key: String = name.to_lowercase().replace(['\u{e9}', '\u{e8}'], "e");
    SIGNS.iter().find(|(en, fr, _)| *en == key || *fr == key).map(|s| s.2).unwrap_or("\u{2605}")
}

pub fn horoscope(c: &Ctx) -> R {
    let t = Theme::Horoscope;
    let sign = c.field(t, "sign");
    let mut rng = c.rng(13);
    let mut p = Page::new(28.0);
    p.gap(22);
    p.push(centred("\u{2605}  \u{2606}  \u{2605}  \u{2606}  \u{2605}", FontChoice::Symbols, 24.0)?);
    p.gap(4);
    p.push(centred(zodiac(&sign), FontChoice::Symbols, 96.0)?);
    p.push(fit_line(&sign.to_uppercase(), FontChoice::Impact, 28.0, 56.0)?);
    p.gap(6);
    p.flourish();
    p.gap(6);
    p.text_gap(c.body.trim(), FontChoice::Serif, 26.0, Align::Left, 3.0)?;
    p.gap(10);
    p.rule((3, 5), 2, 3);
    p.leader("Lucky number", &format!("{}", 1 + rng.below(99)), FontChoice::Sans, 24.0)?;
    p.leader("Lucky colour", COLOURS[rng.below(COLOURS.len())], FontChoice::Sans, 24.0)?;
    p.leader("Mood", MOODS[rng.below(MOODS.len())], FontChoice::Sans, 24.0)?;
    p.gap(24);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::hline(&mut b, 6, 6, w - 6, 2, (3, 5));
    draw::hline(&mut b, h - 7, 6, w - 6, 2, (3, 5));
    draw::vline(&mut b, 6, 6, h - 6, 2, (3, 5));
    draw::vline(&mut b, w - 7, 6, h - 6, 2, (3, 5));
    Ok(b)
}

/// "- milk", "[ ] milk", "[x] milk", "x milk": a tick box. `None` for an ordinary line.
fn item_of(line: &str) -> (Option<bool>, &str) {
    for (mark, done) in [("[x]", true), ("[X]", true), ("[ ]", false), ("[]", false), ("x ", true), ("X ", true), ("- ", false), ("* ", false)] {
        if let Some(rest) = line.strip_prefix(mark) {
            return (Some(done), rest.trim());
        }
    }
    (None, line)
}

/// One line with its box at the left; a ticked one is struck through.
fn item_line(text_s: &str, done: bool, font: FontChoice, px: f32) -> Result<Bitmap, String> {
    let body = text::text(text_s, TextOpts { px, font, margin_x: 64.0, margin_y: 2.0, break_anywhere: false, ..TextOpts::default() })?;
    let mut b = body.clone();
    let top = (px * 0.18) as usize + 2;
    let side = (px * 0.8) as usize;
    draw::rect(&mut b, 22, top, 22 + side, top + side, 3);
    if done {
        draw::line(&mut b, 26, top as i32 + 4, 18 + side as i32, (top + side) as i32 - 4, 4);
        draw::line(&mut b, 26, (top + side) as i32 - 4, 18 + side as i32, top as i32 + 4, 4);
        if let Some((x0, _, x1, _)) = draw::ink_box(&body) {
            draw::hline(&mut b, top + side / 2, x0, x1, 3, (0, 0));
        }
    }
    Ok(b)
}

pub fn checklist(c: &Ctx) -> R {
    let t = Theme::Checklist;
    let mut p = Page::new(16.0);
    p.band(&c.field(t, "title").to_uppercase(), FontChoice::Bold, 34.0, Align::Center)?;
    p.gap(10);
    for line in c.lines() {
        let (mark, text_s) = item_of(line);
        p.push(item_line(text_s, mark == Some(true), FontChoice::Sans, 30.0)?);
        p.gap(6);
    }
    p.gap(10);
    let mut b = p.done();
    {
        let bottom = b.height - 1;
        draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 3);
    }
    Ok(b)
}

pub fn fridge(c: &Ctx) -> R {
    let mut rng = c.rng(14);
    let mut p = Page::new(40.0);
    p.gap(64);
    for line in c.lines() {
        let (mark, text_s) = item_of(line);
        let row = match mark {
            Some(done) => item_line(text_s, done, FontChoice::Script, 34.0)?,
            None => text::text(text_s, TextOpts { px: 34.0, font: FontChoice::Script, margin_x: 34.0, margin_y: 2.0, ..TextOpts::default() })?,
        };
        // Handwriting drifts: every line sits a little off.
        let dx = rng.below(11) as i32 - 5;
        let mut shifted = Bitmap::blank(row.height + 8);
        draw::or_at(&mut shifted, &row, dx, 0);
        draw::hline(&mut shifted, row.height + 4, 30, WIDTH_DOTS - 31, 1, (2, 6));
        p.push(shifted);
    }
    p.gap(26);
    let mut b = p.done();
    // The magnet that holds it to the fridge.
    let cx = WIDTH_DOTS as i32 / 2;
    draw::disc(&mut b, cx, 32, 26, None);
    draw::punch(&mut b, cx, 32, 17);
    draw::disc(&mut b, cx, 32, 12, None);
    draw::punch(&mut b, cx - 5, 27, 3);
    draw::zigzag_bottom(&mut b, 8);
    Ok(b)
}

const RANSOM_FONTS: [FontChoice; 9] = [
    FontChoice::Serif,
    FontChoice::Bold,
    FontChoice::Typewriter,
    FontChoice::Impact,
    FontChoice::Script,
    FontChoice::OldEnglish,
    FontChoice::Playbill,
    FontChoice::Sans,
    FontChoice::Stencil,
];

/// One letter cut out of a magazine: its own font, size and background.
fn ransom_tile(ch: char, rng: &mut Rng) -> Option<Bitmap> {
    let font = RANSOM_FONTS[rng.below(RANSOM_FONTS.len())];
    let px = 30.0 + rng.below(22) as f32;
    let glyph = text::text(&ch.to_string(), TextOpts { px, font, margin_x: 4.0, margin_y: 0.0, ..TextOpts::default() }).ok()?;
    let (x0, y0, x1, y1) = draw::ink_box(&glyph)?;
    let (w, h, pad) = (x1 - x0 + 1, y1 - y0 + 1, 5usize);
    let mut tile = Bitmap::blank(h + 2 * pad);
    let (tw, th) = (w + 2 * pad, h + 2 * pad);
    match rng.below(4) {
        0 => {
            draw::fill_rect(&mut tile, 0, 0, tw - 1, th - 1);
            draw::xor_at(&mut tile, &glyph, pad as i32 - x0 as i32, pad as i32 - y0 as i32);
        }
        1 => {
            draw::copy_cols(&mut tile, &glyph, x0, x1, pad as i32, pad as i32 - y0 as i32);
            draw::rect(&mut tile, 0, 0, tw - 1, th - 1, 2);
        }
        2 => {
            draw::copy_cols(&mut tile, &glyph, x0, x1, pad as i32, pad as i32 - y0 as i32);
            draw::hline(&mut tile, th - 2, 0, tw - 1, 2, (0, 0));
        }
        _ => draw::copy_cols(&mut tile, &glyph, x0, x1, pad as i32, pad as i32 - y0 as i32),
    }
    // Only the tile's own width counts: the rest of the row is blank.
    for y in 0..tile.height {
        for x in tw..WIDTH_DOTS {
            draw::clear(&mut tile, x, y);
        }
    }
    Some(tile)
}

pub fn ransom(c: &Ctx) -> R {
    let mut rng = c.rng(15);
    let mut rows: Vec<Bitmap> = vec![Bitmap::blank(14)];
    for line in c.body.lines().map(str::trim).filter(|l| !l.is_empty()) {
        // Tiles of one row wait here until the row is full.
        let mut tiles: Vec<(usize, Bitmap, i32)> = Vec::new();
        let mut x = 12usize;
        let flush = |tiles: &mut Vec<(usize, Bitmap, i32)>, rows: &mut Vec<Bitmap>| {
            if tiles.is_empty() {
                return;
            }
            let tallest = tiles.iter().map(|(_, t, dy)| t.height + *dy as usize).max().unwrap_or(0);
            let mut row = Bitmap::blank(tallest + 8);
            for (tx, tile, dy) in tiles.drain(..) {
                let width = draw::ink_box(&tile).map(|b| b.2 + 1).unwrap_or(0);
                draw::copy_cols(&mut row, &tile, 0, width.saturating_sub(1), tx as i32, dy);
            }
            rows.push(row);
        };
        for word in line.split_whitespace() {
            // A word is cut out whole, then placed on this row or the next.
            let cut: Vec<Bitmap> = word.chars().filter_map(|ch| ransom_tile(ch, &mut rng)).collect();
            let widths: Vec<usize> = cut.iter().map(|t| draw::ink_box(t).map(|b| b.2 + 1).unwrap_or(0)).collect();
            let total: usize = widths.iter().map(|w| w + 3).sum();
            if x + total > WIDTH_DOTS - 10 && x > 12 {
                flush(&mut tiles, &mut rows);
                x = 12;
            }
            for (tile, width) in cut.into_iter().zip(widths) {
                if x + width > WIDTH_DOTS - 10 {
                    flush(&mut tiles, &mut rows);
                    x = 12;
                }
                let dy = rng.below(8) as i32;
                tiles.push((x, tile, dy));
                x += width + 3 + rng.below(3);
            }
            x += 14;
        }
        flush(&mut tiles, &mut rows);
        rows.push(Bitmap::blank(8));
    }
    if rows.len() < 3 {
        return Err("nothing to print".into());
    }
    Ok(super::super::stack(&rows, 0))
}

pub fn name_tag(c: &Ctx) -> R {
    let lines = c.lines();
    let mut p = Page::new(18.0);
    p.band("HELLO", FontChoice::Impact, 92.0, Align::Center)?;
    p.band("my name is", FontChoice::Bold, 30.0, Align::Center)?;
    p.gap(14);
    p.push(fit_line(lines.first().copied().unwrap_or(""), FontChoice::Marker, 24.0, 90.0)?);
    for extra in lines.iter().skip(1) {
        p.text(extra, FontChoice::Sans, 26.0, Align::Center)?;
    }
    p.gap(24);
    let mut b = p.done();
    {
        let bottom = b.height - 1;
        draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 5);
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_markers_become_boxes() {
        assert_eq!(item_of("[x] done"), (Some(true), "done"));
        assert_eq!(item_of("- milk"), (Some(false), "milk"));
        assert_eq!(item_of("x tick"), (Some(true), "tick"));
        assert_eq!(item_of("plain line"), (None, "plain line"));
    }

    #[test]
    fn signs_are_found_in_english_and_french() {
        assert_eq!(zodiac("Leo"), "\u{264C}");
        assert_eq!(zodiac("Bélier"), "\u{2648}");
        assert_eq!(zodiac("Poissons"), "\u{2653}");
        assert_eq!(zodiac("Dragon"), "\u{2605}");
    }
}
