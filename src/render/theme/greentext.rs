//! A thread on thermal paper: a grey post header, greentext lines with a quote bar, and the reply
//! links underneath. The paper has no green, so the joke is the layout, not a colour.

use super::{Ctx, Page, Region, Theme, today};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw;
use crate::render::text::{self, Align, FontChoice, TextOpts};

type R = Result<Bitmap, String>;

const MONO: FontChoice = FontChoice::Mono;

/// Text on its own, no margins: a piece to place on a row.
fn piece(s: &str, font: FontChoice, px: f32) -> Result<Bitmap, String> {
    text::text(s, TextOpts { px, font, align: Align::Left, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() })
}

fn width_of(b: &Bitmap) -> usize {
    draw::ink_box(b).map_or(0, |(x0, _, x1, _)| x1 - x0 + 1)
}

/// `s`, cut short until its ink fits in `room` dots: a long name must not run into the date.
fn fitting(s: &str, font: FontChoice, px: f32, room: usize) -> Result<Bitmap, String> {
    let mut cut = s.to_string();
    loop {
        match piece(&cut, font, px) {
            Ok(b) if width_of(&b) <= room || cut.chars().count() <= 1 => return Ok(b),
            Ok(_) => {
                cut.pop();
            }
            Err(_) => return Ok(Bitmap::blank(1)),
        }
    }
}

/// A grey patch: a 4x4 Bayer screen, `level` black dots out of 16 (3 is a light grey).
fn shade(b: &mut Bitmap, y0: usize, y1: usize, level: u8) {
    const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    for y in y0..=y1.min(b.height.saturating_sub(1)) {
        for x in 0..WIDTH_DOTS {
            if BAYER[y % 4][x % 4] < level {
                b.set(x, y);
            }
        }
    }
}

/// Thin a line's ink to half its dots: a reply is printed in a paler tone than the post it quotes.
fn paler(b: &mut Bitmap) {
    for y in 0..b.height {
        for x in 0..WIDTH_DOTS {
            if b.get(x, y) && (x + y) % 2 == 0 {
                draw::clear(b, x, y);
            }
        }
    }
}

/// Copy `src` onto `row` with its ink centred on `y`; returns the vertical offset used.
fn put(row: &mut Bitmap, src: &Bitmap, x: i32, y: i32) -> i32 {
    let dy = draw::ink_box(src).map_or(0, |(_, y0, _, y1)| y - ((y0 + y1) / 2) as i32);
    draw::or_at(row, src, x, dy);
    dy
}

/// Remember where `src` landed as the writable part `key`; `top` is the row's place on the ticket.
fn mark(key: &str, src: &Bitmap, x: i32, dy: i32, top: usize) {
    let Some((x0, y0, x1, y1)) = draw::ink_box(src) else { return };
    let x0 = (x + x0 as i32).max(0) as usize;
    let x1 = (x + x1 as i32 + 1).max(0) as usize;
    let y0 = (top as i32 + dy + y0 as i32).max(0) as usize;
    let y1 = (top as i32 + dy + y1 as i32 + 1).max(0) as usize;
    if x0 < x1 && y0 < y1 {
        super::mark(Region { key: key.into(), x0, x1, y0, y1 });
    }
}

/// The grey post header: the name in bold, the date, and the post number at the right edge.
fn header(name: &str, date: &str, no: &str, mx: f32, top: usize) -> Result<Bitmap, String> {
    let h = 38;
    let mut head = Bitmap::blank(h);
    // A light screen: at 19% the date stays readable in small type, and bold black still bites.
    shade(&mut head, 0, h - 1, 3);

    // The number hugs the right margin; the date and then the name take what is left of the band.
    let no_b = fitting(&format!("No.{no}"), MONO, 15.0, WIDTH_DOTS / 2)?;
    let (nx0, _, nx1, _) = draw::ink_box(&no_b).unwrap_or((0, 0, 0, 0));
    let no_x = WIDTH_DOTS as i32 - mx as i32 - (nx1 + 1) as i32;
    let date_b = fitting(date, MONO, 14.0, 120)?;
    let (dx0, _, dx1, _) = draw::ink_box(&date_b).unwrap_or((0, 0, 0, 0));
    let date_x = no_x + nx0 as i32 - 16 - (dx1 + 1) as i32;
    let room = (date_x + dx0 as i32 - 12 - mx as i32).max(48) as usize;
    let name_b = fitting(name, FontChoice::Bold, 22.0, room)?;
    let (mid, name_x) = ((h / 2) as i32, mx as i32);

    let dy_name = put(&mut head, &name_b, name_x, mid);
    let dy_date = put(&mut head, &date_b, date_x, mid + 1);
    let dy_no = put(&mut head, &no_b, no_x, mid + 1);
    mark("name", &name_b, name_x, dy_name, top);
    mark("date", &date_b, date_x, dy_date, top);
    mark("no", &no_b, no_x, dy_no, top);
    Ok(head)
}

/// The links under the post, and the board it was posted on.
fn footer(board: &str, mx: f32, top: usize) -> Result<Bitmap, String> {
    let h = 22;
    let mut row = Bitmap::blank(h);
    let mid = (h / 2) as i32;
    let links = fitting("Reply   Quote   Report", MONO, 12.0, WIDTH_DOTS / 2)?;
    put(&mut row, &links, mx as i32, mid);
    let board_b = fitting(board, FontChoice::Bold, 14.0, 110)?;
    let (_, _, bx1, _) = draw::ink_box(&board_b).unwrap_or((0, 0, 0, 0));
    let x = WIDTH_DOTS as i32 - mx as i32 - (bx1 + 1) as i32;
    let dy = put(&mut row, &board_b, x, mid);
    mark("board", &board_b, x, dy, top);
    Ok(row)
}

/// (`has a quote bar`, `replies to another post`) for one line of the body.
fn quote_of(line: &str) -> (bool, bool) {
    let line = line.trim_start();
    if line.starts_with(">>") { (true, true) } else { (line.starts_with('>'), false) }
}

pub fn greentext(c: &Ctx) -> R {
    let t = Theme::Greentext;
    let mx = 14.0;
    let mut p = Page::new(mx);

    let name = c.field(t, "name");
    let typed_date = c.field(t, "date");
    // The field's `today` hint is the day it is printed, as on the receipts.
    let date = if typed_date == "today" { today() } else { typed_date };
    // An empty number gets one from the text, so the same post always carries the same number.
    let typed_no = c.given("no");
    let no = if typed_no.is_empty() {
        let digits = c.number(23, 8);
        let trimmed = digits.trim_start_matches('0');
        if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
    } else {
        typed_no
    };

    let top = p.height();
    let head = header(&name, &date, &no, mx, top)?;
    // The header's parts are placed by hand, so the page must not read them again as one block.
    text::take_drawn();
    p.push(head);
    p.gap(6);
    p.rule((0, 0), 1, 2);
    p.gap(6);

    for line in c.body.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            p.gap(8);
            continue;
        }
        let (bar, reply) = quote_of(line);
        let indent = if bar { 18.0 } else { 0.0 };
        let mut row = text::text(line.trim_start(), TextOpts { px: 21.0, font: MONO, align: Align::Left, margin_x: mx + indent, margin_y: 1.0, ..TextOpts::default() })?;
        // A reply is paler than the post, and its bar is dashed to say so.
        if reply {
            paler(&mut row);
        }
        if bar {
            let x = mx as usize + if reply { 5 } else { 4 };
            let end = row.height.saturating_sub(3);
            draw::vline(&mut row, x, 2, end, 2, if reply { (2, 2) } else { (0, 0) });
        }
        p.push(row);
    }

    p.gap(8);
    p.rule((2, 4), 1, 2);
    p.gap(2);
    let top = p.height();
    let foot = footer(&c.field(t, "board"), mx, top)?;
    text::take_drawn();
    p.push(foot);
    p.gap(12);
    Ok(p.done())
}

#[cfg(test)]
mod tests {
    use super::super::Fields;
    use super::*;

    #[test]
    fn a_line_is_a_quote_a_reply_or_neither() {
        assert_eq!(quote_of(">be me"), (true, false));
        assert_eq!(quote_of("  >>48219631"), (true, true));
        assert_eq!(quote_of("plain text"), (false, false));
        assert_eq!(quote_of("a > b"), (false, false));
    }

    #[test]
    fn the_post_number_comes_from_the_text_unless_it_is_typed() {
        let none = Fields::new();
        let a = greentext(&Ctx::new(">be me", &none)).unwrap();
        assert_eq!(a, greentext(&Ctx::new(">be me", &none)).unwrap(), "the same post gets the same number");
        assert_ne!(a, greentext(&Ctx::new(">be you", &none)).unwrap(), "another post gets another number");
        let typed = Fields::from([("no".to_string(), "42".to_string())]);
        assert_ne!(a, greentext(&Ctx::new(">be me", &typed)).unwrap(), "a typed number is used");
    }
}
