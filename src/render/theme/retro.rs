//! Techy & retro: terminal, dot-matrix report, label maker, telegram, punch card, big poster.

use super::{Ctx, Page, Theme, fit_line, today};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw;
use crate::render::text::{self, Align, FontChoice, TextOpts};

type R = Result<Bitmap, String>;

pub fn terminal(c: &Ctx) -> R {
    let t = Theme::Terminal;
    let font = FontChoice::Mono;
    let prompt = c.field(t, "prompt");
    let mut p = Page::new(14.0);
    p.band("  terminal", font, 20.0, Align::Left)?;
    p.gap(8);
    let lines: Vec<&str> = c.body.lines().filter(|l| !l.trim().is_empty()).collect();
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            p.text_any(&format!("{prompt} {line}"), font, 22.0)?;
        } else {
            p.text_any(line, font, 22.0)?;
        }
    }
    // The next prompt, with a block cursor.
    let last = text::text(&prompt, TextOpts { px: 22.0, font, margin_x: 14.0, margin_y: 0.0, break_anywhere: true, ..TextOpts::default() })?;
    let mut row = Bitmap::blank(last.height + 4);
    draw::or_at(&mut row, &last, 0, 0);
    let end = text::ink_span(&last).map(|(_, z)| z + 8).unwrap_or(30);
    draw::fill_rect(&mut row, end, 2, end + 11, last.height);
    p.push(row);
    p.gap(10);
    let mut b = p.done();
    {
        let bottom = b.height - 1;
        draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 3);
    }
    Ok(b)
}

pub fn dot_matrix(c: &Ctx) -> R {
    let t = Theme::DotMatrix;
    let font = FontChoice::Ocr;
    let mut p = Page::new(40.0);
    p.gap(12);
    p.text(&format!("*** {} ***", c.field(t, "title").to_uppercase()), font, 24.0, Align::Center)?;
    p.leader("PAGE 1 OF 1", &today(), font, 20.0)?;
    p.rule((2, 2), 3, 5);
    for (i, line) in c.lines().iter().enumerate() {
        p.text_any(&format!("{:03} {}", i + 1, line.to_uppercase()), font, 20.0)?;
        p.gap(3);
    }
    p.rule((2, 2), 3, 5);
    p.text("*** END OF REPORT ***", font, 20.0, Align::Center)?;
    p.gap(14);
    let mut b = p.done();
    draw::tractor_holes(&mut b, 44);
    let h = b.height - 1;
    draw::vline(&mut b, 26, 0, h, 1, (3, 5));
    draw::vline(&mut b, WIDTH_DOTS - 27, 0, h, 1, (3, 5));
    Ok(b)
}

pub fn label(c: &Ctx) -> R {
    let mut p = Page::new(0.0);
    p.gap(6);
    for line in c.lines().into_iter().take(10) {
        let fitted = fit_line(&line.to_uppercase(), FontChoice::Bold, 18.0, 44.0)?;
        let Some((x0, y0, x1, y1)) = draw::ink_box(&fitted) else { continue };
        let h = y1 - y0 + 1 + 26;
        let mut strip = Bitmap::blank(h);
        let (sx0, sx1) = (x0.saturating_sub(18), (x1 + 18).min(WIDTH_DOTS - 1));
        draw::fill_rect(&mut strip, sx0, 0, sx1, h - 1);
        draw::round_corners(&mut strip, sx0, 0, sx1, h - 1, 7);
        // The letters are knocked out of the black, like a Dymo strip.
        draw::xor_at(&mut strip, &fitted, 0, 13 - y0 as i32);
        p.push(strip);
        p.gap(8);
        let mut cut = Bitmap::blank(5);
        draw::hline(&mut cut, 2, 30, WIDTH_DOTS - 31, 1, (2, 6));
        p.push(cut);
        p.gap(8);
    }
    Ok(p.done())
}

/// "Arrived safely. Weather lovely." as a telegram wants it.
fn stop_words(s: &str) -> String {
    let mut out = String::new();
    for ch in s.to_uppercase().chars() {
        match ch {
            '.' | '!' | '?' => out.push_str(" STOP"),
            '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.ends_with("STOP") { out } else { format!("{out} STOP") }
}

pub fn telegram(c: &Ctx) -> R {
    let t = Theme::Telegram;
    let font = FontChoice::Typewriter;
    let mut p = Page::new(24.0);
    p.gap(26);
    p.shout("TELEGRAM", FontChoice::Stencil, 80.0)?;
    p.gap(4);
    p.rule((0, 0), 2, 3);
    p.rule((0, 0), 1, 3);
    p.leader("RECEIVED", &today(), font, 20.0)?;
    let to = c.field(t, "to");
    if !to.is_empty() {
        p.text(&format!("TO: {}", to.to_uppercase()), font, 24.0, Align::Left)?;
    }
    p.gap(8);
    p.text_gap(&stop_words(c.body), font, 26.0, Align::Left, 4.0)?;
    p.gap(12);
    p.text(&format!("NO. {}", c.number(16, 5)), font, 20.0, Align::Center)?;
    p.gap(26);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::rect(&mut b, 6, 6, w - 6, h - 6, 3);
    draw::rect(&mut b, 12, 12, w - 12, h - 12, 1);
    Ok(b)
}

/// Which of the twelve rows (12, 11, 0, 1…9, top to bottom) a character punches.
fn hollerith(ch: char) -> u16 {
    let digit = |d: u32| 1u16 << (2 + d);
    let (zone12, zone11, zone0) = (1u16, 1u16 << 1, 1u16 << 2);
    match ch.to_ascii_uppercase() {
        c @ '0'..='9' => digit(c as u32 - '0' as u32),
        c @ 'A'..='I' => zone12 | digit(c as u32 - 'A' as u32 + 1),
        c @ 'J'..='R' => zone11 | digit(c as u32 - 'J' as u32 + 1),
        c @ 'S'..='Z' => zone0 | digit(c as u32 - 'S' as u32 + 2),
        ' ' => 0,
        other => zone12 | zone11 | digit(other as u32 % 9 + 1),
    }
}

const CARD_COLS: usize = 44;

fn punch_row(chars: &[char]) -> Result<Bitmap, String> {
    let mut b = Bitmap::blank(134);
    let (x0, x1) = (16usize, 16 + CARD_COLS * 8 - 1);
    draw::rect(&mut b, x0, 2, x1, 131, 2);
    // The cut corner.
    for k in 0..10 {
        for x in x0..x0 + 9 - k {
            draw::clear(&mut b, x, 2 + k);
        }
    }
    draw::line(&mut b, x0 as i32, 12, x0 as i32 + 10, 2, 2);
    for (i, ch) in chars.iter().enumerate() {
        let colx = x0 + 2 + i * 8;
        if *ch != ' ' {
            let glyph = text::text(&ch.to_string(), TextOpts { px: 15.0, font: FontChoice::Ocr, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() })?;
            if let Some((gx0, _, gx1, _)) = draw::ink_box(&glyph) {
                draw::copy_cols(&mut b, &glyph, gx0, gx1, (colx + 3) as i32 - ((gx1 - gx0) / 2) as i32 - 2, 8);
            }
        }
        let rows = hollerith(*ch);
        for r in 0..12 {
            if rows & (1 << r) != 0 {
                draw::fill_rect(&mut b, colx + 1, 30 + r * 8, colx + 4, 30 + r * 8 + 5);
            }
        }
    }
    Ok(b)
}

pub fn punch_card(c: &Ctx) -> R {
    let mut p = Page::new(0.0);
    p.gap(8);
    let mut cards = 0;
    for line in c.lines() {
        let chars: Vec<char> = line.chars().collect();
        for chunk in chars.chunks(CARD_COLS) {
            if cards >= 12 {
                break;
            }
            p.push(punch_row(chunk)?);
            p.gap(10);
            cards += 1;
        }
    }
    Ok(p.done())
}

pub fn poster(c: &Ctx) -> R {
    let mut p = Page::new(8.0);
    p.gap(10);
    p.shout(c.body, FontChoice::Impact, 170.0)?;
    p.gap(10);
    Ok(p.done())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telegrams_say_stop() {
        assert_eq!(stop_words("Arrived safely. Weather lovely."), "ARRIVED SAFELY STOP WEATHER LOVELY STOP");
        assert_eq!(stop_words("no news"), "NO NEWS STOP");
    }

    #[test]
    fn holes_follow_the_punch_card_code() {
        assert_eq!(hollerith(' '), 0);
        assert_eq!(hollerith('0'), 1 << 2);
        assert_eq!(hollerith('9'), 1 << 11);
        assert_eq!(hollerith('A'), 1 | (1 << 3), "A is zone 12 and 1");
        assert_eq!(hollerith('J'), (1 << 1) | (1 << 3), "J is zone 11 and 1");
        assert_eq!(hollerith('S'), (1 << 2) | (1 << 4), "S is zone 0 and 2");
        assert_eq!(hollerith('a'), hollerith('A'));
    }

    #[test]
    fn a_long_line_runs_over_to_another_card() {
        let one = punch_card(&Ctx_for("HELLO")).unwrap();
        let two = punch_card(&Ctx_for(&"A".repeat(60))).unwrap();
        assert!(two.height > one.height + 100);
    }

    #[allow(non_snake_case)]
    fn Ctx_for(body: &str) -> Ctx<'_> {
        static EMPTY: std::sync::OnceLock<super::super::Fields> = std::sync::OnceLock::new();
        Ctx::new(body, EMPTY.get_or_init(Default::default))
    }
}
