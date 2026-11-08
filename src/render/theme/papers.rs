//! Fun official papers: boarding pass, movie ticket, fortune, diploma, horoscope, fridge note,
//! checklist, ransom note, name tag.

use super::{Ctx, Page, Theme, date_of, fit_line};
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
    // The generated values keep their order, so a ticket with nothing typed keeps them.
    let gate = c.field_or(t, "gate", format!("{}{}", (b'A' + rng.below(6) as u8) as char, 1 + rng.below(30)));
    let flight = c.field_or(t, "flight", format!("TR {}", c.number(7, 4)));
    let class = c.field_or(t, "class", "ECONOMY");
    p.leader("FLIGHT", &flight, FontChoice::Typewriter, 26.0)?;
    p.leader("DATE", &date_of(c, t), FontChoice::Typewriter, 26.0)?;
    p.leader("GATE", &gate, FontChoice::Typewriter, 26.0)?;
    p.leader("SEAT", &c.field(t, "seat").to_uppercase(), FontChoice::Typewriter, 26.0)?;
    p.leader("CLASS", &class, FontChoice::Typewriter, 26.0)?;
    // The boarding time is made from the text, not a field: it stays a plain line, no box.
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

const COLOURS: [&str; 12] = ["teal", "amber", "violet", "coral", "green", "indigo", "gold", "pink", "beige", "microwave", "sock grey", "invisible"];
const MOODS: [&str; 10] = ["curious", "unstoppable", "sleepy", "generous", "mischievous", "calm", "hangry", "suspicious", "buffering", "snack-ready"];

pub fn fortune(c: &Ctx) -> R {
    let mut p = Page::new(24.0);
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
    let mut p = Page::new(26.0);
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

/// Signs nobody asked for: a name and a symbol the font has.
const DUMB_SIGNS: [(&str, &str); 15] = [
    ("Coffee", "\u{2615}"),
    ("Umbrella", "\u{2602}"),
    ("Landline", "\u{260E}"),
    ("Scissors", "\u{2702}"),
    ("Static Shock", "\u{26A1}"),
    ("Snowman", "\u{2603}"),
    ("Hot Soup", "\u{2668}"),
    ("Pirate", "\u{2620}"),
    ("Karaoke", "\u{266B}"),
    ("Anchor", "\u{2693}"),
    ("Radioactive", "\u{2622}"),
    ("Lost Luggage", "\u{2708}"),
    ("Sleepy Knight", "\u{265E}"),
    ("Lucky Clover", "\u{2618}"),
    ("Unread Mail", "\u{2709}"),
];

/// The sign's name and symbol: a zodiac sign or a dumb one by name, otherwise a sign of your own
/// (with the `symbol` you gave, or one picked from its name); nothing typed picks any sign.
fn sign_of(c: &Ctx) -> (String, String) {
    let typed = c.given("sign");
    let own_symbol = c.given("symbol");
    if typed.is_empty() {
        let k = c.dice_rng(41).below(SIGNS.len() + DUMB_SIGNS.len());
        return if k < SIGNS.len() { (SIGNS[k].0.to_string(), SIGNS[k].2.to_string()) } else { (DUMB_SIGNS[k - SIGNS.len()].0.to_string(), DUMB_SIGNS[k - SIGNS.len()].1.to_string()) };
    }
    if !own_symbol.is_empty() {
        return (typed, own_symbol);
    }
    let key = typed.to_lowercase().replace(['\u{e9}', '\u{e8}'], "e");
    if let Some(s) = SIGNS.iter().find(|(en, fr, _)| *en == key || *fr == key) {
        return (typed, s.2.to_string());
    }
    if let Some(d) = DUMB_SIGNS.iter().find(|(name, _)| name.to_lowercase() == key) {
        return (typed, d.1.to_string());
    }
    // A sign of your own gets a symbol from its name: the same name, the same symbol.
    let pick = draw::seed_of(&key) as usize % DUMB_SIGNS.len();
    (typed, DUMB_SIGNS[pick].1.to_string())
}

pub fn horoscope(c: &Ctx) -> R {
    let (sign, symbol) = sign_of(c);
    let mut rng = c.rng(13);
    let mut p = Page::new(28.0);
    p.gap(22);
    p.push(centred("\u{2605}  \u{2606}  \u{2605}  \u{2606}  \u{2605}", FontChoice::Symbols, 24.0)?);
    p.gap(4);
    p.push(centred(&symbol, FontChoice::Symbols, 96.0)?);
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
        _ => {
            draw::copy_cols(&mut tile, &glyph, x0, x1, pad as i32, pad as i32 - y0 as i32);
        }
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

/// Big lines, each in its own typeface: a poster assembled from whatever was in the drawer.
pub fn font_salad(c: &Ctx) -> R {
    let fonts = [FontChoice::Impact, FontChoice::Stencil, FontChoice::OldEnglish, FontChoice::Playbill, FontChoice::Marker, FontChoice::Chancery, FontChoice::Bold];
    let lines = c.lines();
    let mut p = Page::new(14.0);
    // An emptied text box leaves no line to set. Keep the card and its frame, on the room of one
    // blank line, instead of a page of no rows (the frame under it would have nothing to wrap).
    if lines.is_empty() {
        p.gap(96);
    }
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            p.gap(3);
        }
        p.push(fit_line(&line.trim().to_uppercase(), fonts[i % fonts.len()], 14.0, 150.0)?);
        if i % 2 == 1 {
            p.rule((2, 4), 2, 2);
        }
    }
    let mut b = p.done();
    let height = b.height;
    draw::rect(&mut b, 6, 6, WIDTH_DOTS - 7, height - 7, 2);
    draw::round_corners(&mut b, 6, 6, WIDTH_DOTS - 7, height - 7, 12);
    Ok(b)
}

/// A profile picture, cut to a circle. A missing or unreadable file leaves a ring with initials,
/// so the theme still prints something sensible.
fn avatar(c: &Ctx) -> Bitmap {
    let side = 92usize;
    let r = side as i32 / 2;
    let given = c.given("avatar");
    let mut out = Bitmap::blank(side);
    let mut has_pic = false;
    if !given.is_empty()
        && let Ok(src) = crate::render::load_picture(std::path::Path::new(given.trim()))
    {
        let prepared = crate::render::prepare(&src, 0, false);
        let full = crate::render::tone(&prepared, crate::render::PictureOpts::default());
        let s = side.min(full.height).min(WIDTH_DOTS);
        if s > 8 {
            let (x0, y0) = ((WIDTH_DOTS - s) / 2, (full.height - s) / 2);
            for y in 0..s {
                for x in 0..s {
                    if full.get(x0 + x, y0 + y) {
                        out.set(x, y);
                    }
                }
            }
            has_pic = true;
        }
    }
    // Round it off; the ring is drawn after so the edge stays clean.
    for y in 0..side {
        for x in 0..side {
            if (x as i32 - r).pow(2) + (y as i32 - r).pow(2) > (r - 2).pow(2) {
                draw::put(&mut out, x as i32, y as i32, false);
            }
        }
    }
    draw::disc(&mut out, r, r, r - 1, Some(3));
    // No usable picture: two initials, in the middle of the ring.
    if !has_pic {
        let name = c.field(Theme::Tweet, "name");
        let initials: String = name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
        if !initials.is_empty()
            && let Ok(t) = centred(&initials, FontChoice::Bold, 38.0)
            && let Some((x0, y0, x1, y1)) = draw::ink_box(&t)
        {
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            draw::copy_cols(&mut out, &t, x0, x1, ((side - w) / 2) as i32, ((side - h) / 2) as i32 - y0 as i32);
        }
    }
    out
}

/// A post, on paper: avatar, name and handle, the text, a rule, and the numbers underneath.
pub fn tweet(c: &Ctx) -> R {
    let t = Theme::Tweet;
    let mut head = Bitmap::blank(104);
    draw::or_at(&mut head, &avatar(c), 6, 6);
    let name = text::text(&c.field(t, "name"), TextOpts { px: 26.0, font: FontChoice::Bold, align: Align::Left, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() }).unwrap_or_else(|_| Bitmap::blank(0));
    draw::copy_cols(&mut head, &name, 0, 250, 108, 12);
    let handle = text::text(&c.field(t, "handle"), TextOpts { px: 18.0, font: FontChoice::Sans, align: Align::Left, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() }).unwrap_or_else(|_| Bitmap::blank(0));
    draw::copy_cols(&mut head, &handle, 0, 250, 108, 48);
    if let Some((hx0, hy0, hx1, hy1)) = draw::ink_box(&handle) {
        super::mark(super::Region { key: "handle".into(), x0: 108 + hx0, x1: 108 + hx1 + 1, y0: 48 + hy0, y1: 48 + hy1 + 1 });
    }
    let when = text::text("today · now", TextOpts { px: 14.0, font: FontChoice::Sans, align: Align::Left, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() }).unwrap_or_else(|_| Bitmap::blank(0));
    draw::copy_cols(&mut head, &when, 0, 250, 108, 76);
    let mut p = Page::new(18.0);
    p.push(head);
    p.gap(8);
    p.text(c.body.trim(), FontChoice::Sans, 26.0, Align::Left)?;
    p.gap(10);
    p.rule((3, 4), 1, 3);
    p.text(&c.field(t, "stats"), FontChoice::Sans, 17.0, Align::Left)?;
    let footer = c.given("footer");
    if !footer.is_empty() {
        p.text(&footer, FontChoice::Sans, 14.0, Align::Left)?;
    }
    p.gap(14);
    let mut b = p.done();
    let height = b.height;
    draw::rect(&mut b, 4, 4, WIDTH_DOTS - 5, height - 5, 2);
    draw::round_corners(&mut b, 4, 4, WIDTH_DOTS - 5, height - 5, 14);
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_salad_gets_taller_with_lines_and_a_tweet_survives_a_missing_avatar() {
        let f = super::super::Fields::new();
        let six = font_salad(&Ctx::new("one\ntwo\nthree", &f)).unwrap();
        let one = font_salad(&Ctx::new("one", &f)).unwrap();
        assert!(six.height > one.height + 80, "each line takes its own big row");
        let named: super::super::Fields = [("name".to_string(), "Ada".to_string()), ("avatar".to_string(), r"C:\does\not\exist.png".to_string())].into_iter().collect();
        let tweet = tweet(&Ctx::new("hello paper", &named)).unwrap();
        assert!(tweet.height > 150 && tweet.data.iter().any(|x| *x != 0), "a bad avatar path still leaves a printable post");
    }

    #[test]
    fn list_markers_become_boxes() {
        assert_eq!(item_of("[x] done"), (Some(true), "done"));
        assert_eq!(item_of("- milk"), (Some(false), "milk"));
        assert_eq!(item_of("x tick"), (Some(true), "tick"));
        assert_eq!(item_of("plain line"), (None, "plain line"));
    }

    #[test]
    fn signs_are_found_in_english_and_french_and_dumb_and_custom_ones_get_a_symbol() {
        let sign = |typed: &str, symbol: &str| {
            let f: super::super::Fields = [("sign", typed), ("symbol", symbol)].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            sign_of(&Ctx::new("x", &f))
        };
        assert_eq!(sign("Leo", "").1, "\u{264C}");
        assert_eq!(sign("Bélier", "").1, "\u{2648}");
        assert_eq!(sign("coffee", "").1, "\u{2615}");
        assert_eq!(sign("Pancake", ""), sign("Pancake", ""), "the same name gets the same symbol");
        assert_eq!(sign("Pancake", "*").1, "*");
        let random = sign("", "");
        assert!(!random.0.is_empty() && !random.1.is_empty());
    }
}
