//! Shop & diner: receipt, guest check, pharmacy label, coupon, fine.

use super::{Ctx, Page, Theme, effects, money, split_price, today};
use crate::render::draw;
use crate::render::text::{Align, FontChoice};
use crate::protocol::{Bitmap, WIDTH_DOTS};

type R = Result<Bitmap, String>;

/// The fine's letterhead, named so [`fine`] and its tests agree on where the header ends.
const FINE_TITLE: &str = "OFFICIAL NOTICE";
const FINE_TITLE_PX: f32 = 30.0;
const FINE_CHARGE: &str = "PENALTY CHARGE";
const FINE_CHARGE_PX: f32 = 60.0;

/// A dashed box around the whole ticket.
fn dashed_box(b: &mut Bitmap, inset: usize, dash: (usize, usize), t: usize) {
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::hline(b, inset, inset, w - inset, t, dash);
    draw::hline(b, h - inset, inset, w - inset, t, dash);
    draw::vline(b, inset, inset, h - inset, t, dash);
    draw::vline(b, w - inset, inset, h - inset, t, dash);
}

fn date_of(c: &Ctx, t: Theme) -> String {
    let d = c.field(t, "date");
    if d == "today" { today() } else { d }
}

pub fn receipt(c: &Ctx) -> R {
    let t = Theme::Receipt;
    let mono = FontChoice::Typewriter;
    let mut p = Page::new(14.0);
    p.gap(12);
    p.text(&c.field(t, "store").to_uppercase(), FontChoice::Bold, 40.0, Align::Center)?;
    p.text(&c.field(t, "address"), mono, 22.0, Align::Center)?;
    p.gap(4);
    p.rule((4, 4), 2, 6);
    p.leader(&date_of(c, t), &format!("N. {}", c.number(1, 4)), mono, 22.0)?;
    p.rule((4, 4), 2, 6);
    let (mut total, mut count) = (0i64, 0);
    for line in c.lines() {
        match split_price(line) {
            (name, Some(v)) => {
                total += v;
                count += 1;
                p.leader(&name, &money(v), mono, 24.0)?;
            }
            (note, None) => p.text(&note, mono, 22.0, Align::Left)?,
        }
        p.gap(2);
    }
    p.rule((0, 0), 2, 8);
    let total_top = p.height();
    p.leader("TOTAL", &money(total), FontChoice::Bold, 34.0)?;
    let total_bot = p.height();
    let cash = if total % 500 == 0 { total } else { (total / 500 + 1) * 500 };
    p.leader("CASH", &money(cash), mono, 24.0)?;
    p.leader("CHANGE", &money(cash - total), mono, 24.0)?;
    p.rule((4, 4), 2, 8);
    p.text(&format!("{count} ITEMS"), mono, 22.0, Align::Center)?;
    p.gap(8);
    p.barcode(&c.number(2, 12), 64)?;
    p.gap(4);
    p.text("THANK YOU!", FontChoice::Bold, 30.0, Align::Center)?;
    p.text("See you soon", mono, 22.0, Align::Center)?;
    p.gap(18);
    let mut b = p.done();
    draw::zigzag_bottom(&mut b, 6);
    // Stamped as paid, a little crooked, across the total line.
    effects::stamp(&mut b, WIDTH_DOTS as i32 / 2, ((total_top + total_bot) / 2) as i32, 156, 62, "PAID", -7.0);
    Ok(b)
}

pub fn diner(c: &Ctx) -> R {
    let t = Theme::Diner;
    let mono = FontChoice::Typewriter;
    let mut p = Page::new(26.0);
    p.gap(26);
    p.text(&c.field(t, "place"), FontChoice::Script, 46.0, Align::Center)?;
    p.text("GUEST CHECK", FontChoice::Bold, 28.0, Align::Center)?;
    p.gap(4);
    p.rule((0, 0), 2, 4);
    let table_y0 = p.height();
    p.leader(&format!("Table {}", c.field(t, "table")), &format!("Server {}", c.field(t, "server")), mono, 22.0)?;
    let table_y1 = p.height();
    super::mark(super::Region { key: "table".into(), x0: 26, x1: WIDTH_DOTS - 26, y0: table_y0, y1: table_y1 });
    p.rule((0, 0), 2, 4);
    let mut total = 0i64;
    for line in c.lines() {
        match split_price(line) {
            (name, Some(v)) => {
                total += v;
                p.leader(&name, &money(v), mono, 26.0)?;
            }
            (note, None) => p.text(&note, mono, 24.0, Align::Left)?,
        }
        p.gap(3);
    }
    p.rule((4, 4), 2, 6);
    let tax = (total * 8 + 50) / 100;
    p.leader("Subtotal", &money(total), mono, 24.0)?;
    p.leader("Tax 8%", &money(tax), mono, 24.0)?;
    p.leader("TOTAL", &money(total + tax), FontChoice::Bold, 32.0)?;
    p.gap(12);
    p.text("Tip: ____________", mono, 26.0, Align::Left)?;
    p.gap(10);
    p.text("Thank you, come again!", FontChoice::Script, 32.0, Align::Center)?;
    p.gap(26);
    let mut b = p.done();
    {
        let bottom = b.height - 9;
        draw::rect(&mut b, 8, 8, WIDTH_DOTS - 9, bottom, 2);
    }
    {
        let bottom = b.height - 15;
        draw::rect(&mut b, 14, 14, WIDTH_DOTS - 15, bottom, 1);
    }
    // Cheap paper: a faint screen, and a cup that stood on the corner.
    effects::halftone(&mut b, 9, 1);
    let mut ring = c.rng(31);
    effects::coffee_ring(&mut b, WIDTH_DOTS as i32 - 74, 76, 44, &mut ring);
    Ok(b)
}

pub fn pharmacy(c: &Ctx) -> R {
    let t = Theme::Pharmacy;
    let mut p = Page::new(18.0);
    p.band("PHARMACY", FontChoice::Bold, 36.0, Align::Center)?;
    p.gap(8);
    p.text("Rx", FontChoice::OldEnglish, 60.0, Align::Left)?;
    p.text(&format!("Patient: {}", c.field(t, "patient")), FontChoice::Sans, 24.0, Align::Left)?;
    p.text(&format!("Prescriber: {}", c.field(t, "doctor")), FontChoice::Sans, 24.0, Align::Left)?;
    p.rule((0, 0), 2, 8);
    p.text(&c.body.trim().to_uppercase(), FontChoice::Bold, 28.0, Align::Left)?;
    p.rule((0, 0), 2, 8);
    p.leader("Refills", &c.field(t, "refills"), FontChoice::Sans, 24.0)?;
    p.leader("Rx number", &c.number(3, 7), FontChoice::Sans, 24.0)?;
    p.gap(8);
    p.barcode(&c.number(4, 10), 56)?;
    p.gap(4);
    p.text("Keep out of reach of children and cats.", FontChoice::Sans, 20.0, Align::Center)?;
    p.gap(12);
    let mut b = p.done();
    {
        let bottom = b.height - 1;
        draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 3);
    }
    Ok(b)
}

pub fn coupon(c: &Ctx) -> R {
    let t = Theme::Coupon;
    let mut p = Page::new(30.0);
    p.gap(30);
    p.text("COUPON", FontChoice::Stencil, 38.0, Align::Center)?;
    let offer_top = p.height();
    p.shout(&c.field(t, "offer"), FontChoice::Impact, 150.0)?;
    let offer_bot = p.height();
    p.gap(6);
    p.text(c.body.trim(), FontChoice::Bold, 26.0, Align::Center)?;
    p.gap(10);
    p.rule((6, 5), 2, 4);
    p.leader("CODE", &c.field(t, "code"), FontChoice::Typewriter, 26.0)?;
    p.leader("VALID UNTIL", &c.field(t, "expires"), FontChoice::Typewriter, 22.0)?;
    p.gap(8);
    p.barcode(&c.number(5, 10), 50)?;
    p.gap(30);
    let mut b = p.done();
    // A cheap screen behind the discount, so the offer reads like newsprint.
    effects::halftone_in(&mut b, 14, offer_top, WIDTH_DOTS - 15, offer_bot, 7, 2);
    dashed_box(&mut b, 8, (8, 6), 3);
    // The scissors sit on the dashed line, on a patch of white.
    let patch = crate::render::text::text("\u{2702}", crate::render::TextOpts { px: 30.0, font: FontChoice::Symbols, margin_x: 0.0, margin_y: 0.0, ..crate::render::TextOpts::default() });
    if let Ok(s) = patch {
        draw::clear_rect(&mut b, 22, 2, 70, 20);
        draw::or_at(&mut b, &s, 24, -4);
    }
    Ok(b)
}

pub fn fine(c: &Ctx) -> R {
    let t = Theme::Fine;
    let mut p = Page::new(16.0);
    p.band(FINE_TITLE, FontChoice::Bold, FINE_TITLE_PX, Align::Center)?;
    p.gap(8);
    p.shout(FINE_CHARGE, FontChoice::Impact, FINE_CHARGE_PX)?;
    p.text(&format!("No. {}", c.field(t, "number")), FontChoice::Typewriter, 22.0, Align::Center)?;
    p.rule((0, 0), 2, 8);
    p.text("OFFENDER", FontChoice::Sans, 18.0, Align::Left)?;
    p.text(&c.field(t, "offender"), FontChoice::Bold, 32.0, Align::Left)?;
    p.gap(6);
    p.text("OFFENCE", FontChoice::Sans, 18.0, Align::Left)?;
    p.text(c.body.trim(), FontChoice::Typewriter, 26.0, Align::Left)?;
    p.gap(10);
    // The amount is drawn straight onto the page of the ticket: a Page of its own would publish
    // its region counted from its own top, which put the text box at the ticket's top left.
    let amount_top = p.height();
    p.begin("amount");
    p.gap(8);
    p.text("AMOUNT DUE", FontChoice::Sans, 18.0, Align::Center)?;
    p.text(&format!("{} EUR", c.field(t, "amount")), FontChoice::Impact, 56.0, Align::Center)?;
    p.gap(8);
    p.end();
    let amount_h = p.height() - amount_top;
    p.gap(10);
    p.band("PAY WITHIN 30 DAYS", FontChoice::Bold, 28.0, Align::Center)?;
    p.gap(8);
    p.barcode(&c.number(6, 12), 48)?;
    p.text("Appeals: shout into a pillow.", FontChoice::Sans, 20.0, Align::Center)?;
    p.gap(12);
    let mut b = p.done();
    {
        let bottom = b.height - 1;
        draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 3);
    }
    {
        // The box around the amount, the same dots the "amount" region covers.
        let (top, bottom) = (amount_top, amount_top + amount_h - 1);
        draw::rect(&mut b, 16, top, WIDTH_DOTS - 17, bottom, 3);
        draw::rect(&mut b, 23, top + 7, WIDTH_DOTS - 24, bottom - 7, 1);
    }
    // Unpaid, slapped on over the amount at an angle.
    effects::stamp(&mut b, WIDTH_DOTS as i32 / 2, (amount_top + amount_h / 2) as i32 + 12, 212, 70, "UNPAID", -9.0);
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::TextOpts;
    use crate::render::theme::{Fields, render_mapped};

    /// The bottom of the fine's letterhead: the band and the charge the amount must stay clear of.
    fn fine_header_bottom() -> usize {
        let mut head = Page::new(16.0);
        head.band(FINE_TITLE, FontChoice::Bold, FINE_TITLE_PX, Align::Center).unwrap();
        head.gap(8);
        head.shout(FINE_CHARGE, FontChoice::Impact, FINE_CHARGE_PX).unwrap();
        head.height()
    }

    #[test]
    fn the_fine_amount_region_covers_the_box_and_not_the_letterhead() {
        let (b, regions) = render_mapped(Theme::Fine, Theme::Fine.sample(), &Fields::new(), TextOpts::default()).unwrap();
        let amount = regions.iter().find(|r| r.key == "amount").expect("the amount has a region");
        assert!(amount.x0 < amount.x1 && amount.x1 <= WIDTH_DOTS && amount.y0 < amount.y1 && amount.y1 <= b.height, "the amount region is outside the ticket");
        // It used to start at the ticket's top left, over OFFICIAL NOTICE and PENALTY CHARGE.
        let header = fine_header_bottom();
        assert!(amount.y0 > header, "the amount region starts at {} but the letterhead ends at {}", amount.y0, header);
    }
}
