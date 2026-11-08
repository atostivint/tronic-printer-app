//! Silly forms: pads of mock paperwork (a message slip, a permission slip, an incident report, an
//! apology…) in the spirit of novelty stationery. Each one has blanks, boxes and a signature line.
//!
//! Nothing is filled in for you: boxes, meters and stars stay empty, and an empty field is a blank
//! line, so a form with no text at all prints as a clean sheet to write on by hand. The wording of
//! the boxes and the closing line can be typed, or shuffled from a list of jokes.

use super::{Ctx, Page, fit_line, today};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw;
use crate::render::text::{self, Align, FontChoice, TextOpts};

type R = Result<Bitmap, String>;
type U = Result<(), String>;

const MX: f32 = 18.0;

fn opts(px: f32, font: FontChoice, align: Align, mx: f32) -> TextOpts {
    TextOpts { px, font, align, margin_x: mx, margin_y: 1.0, ..TextOpts::default() }
}

/// A date field left at "today" means today's date; a form that is left empty stays blank.
fn dated(s: String) -> String {
    if s.eq_ignore_ascii_case("today") { today() } else { s }
}

/// A blank to fill in: a small label, the value in handwriting (if any), a line under it. The
/// value is what the user typed in the field `key`, drawn as it is typed, like any other value.
fn blank(p: &mut Page, c: &Ctx, key: &str, label: &str, date: bool, font: FontChoice, px: f32) -> U {
    p.text(&label.to_uppercase(), FontChoice::Sans, 15.0, Align::Left)?;
    let value = if date { dated(c.given(key)) } else { c.given(key) };
    p.begin(key);
    // An empty line is as tall as a written one, so writing in it does not move the form.
    let row = if value.trim().is_empty() { Bitmap::blank(text::text("Ag", opts(px, font, Align::Left, p.mx))?.height) } else { text::text(value.trim(), opts(px, font, Align::Left, p.mx))? };
    let h = row.height;
    let mut b = Bitmap::blank(h + 6);
    draw::or_at(&mut b, &row, 0, 0);
    draw::hline(&mut b, h + 3, p.mx as usize, WIDTH_DOTS - 1 - p.mx as usize, 2, (0, 0));
    p.push(b);
    p.end();
    p.gap(4);
    Ok(())
}

/// A centred name over a line (for "I hereby grant ____ permission").
fn name_line(p: &mut Page, c: &Ctx, key: &str, font: FontChoice, max_px: f32) -> U {
    let value = c.given(key);
    p.begin(key);
    let row = if value.trim().is_empty() { Bitmap::blank(40) } else { fit_line(value.trim(), font, 24.0, max_px)? };
    let h = row.height;
    let mut b = Bitmap::blank(h + 6);
    draw::or_at(&mut b, &row, 0, 0);
    draw::hline(&mut b, h + 3, 40, WIDTH_DOTS - 41, 2, (0, 0));
    p.push(b);
    p.end();
    Ok(())
}

/// A box with its label; a ticked one has a cross drawn in.
fn tick_row(label: &str, ticked: bool, px: f32, mx: f32) -> R {
    let side = (px * 0.8) as usize;
    let x0 = mx as usize;
    let mut b = text::text(label, opts(px, FontChoice::Sans, Align::Left, mx + side as f32 + 12.0))?;
    let top = (px * 0.18) as usize + 2;
    draw::rect(&mut b, x0, top, x0 + side, top + side, 3);
    if ticked {
        let (a, z) = (x0 as i32, (x0 + side) as i32);
        let (t, d) = (top as i32, (top + side) as i32);
        draw::line(&mut b, a - 3, t - 3, z + 3, d + 3, 4);
        draw::line(&mut b, a - 3, d + 3, z + 3, t - 3, 4);
    }
    Ok(b)
}

/// One group of boxes. `show` of the `pool` are printed: the first ones as they come, other ones
/// after "Shuffle". With `typed`, the user's own list wins: `a | b | x c` (an `x ` ticks a box).
fn boxes(p: &mut Page, c: &Ctx, salt: u64, pool: &[&str], show: usize, typed: bool, px: f32) -> U {
    let mut items: Vec<(String, bool)> = Vec::new();
    if typed {
        for part in c.given("boxes").split('|') {
            let part = part.trim();
            let (ticked, label) = match part.strip_prefix("[x]").or_else(|| part.strip_prefix("[X]")).or_else(|| part.strip_prefix("x ")).or_else(|| part.strip_prefix("X ")) {
                Some(rest) => (true, rest.trim()),
                None => (false, part),
            };
            if !label.is_empty() {
                items.push((label.to_string(), ticked));
            }
        }
        items.truncate(8);
    }
    if items.is_empty() {
        let mut picks: Vec<&str> = pool.to_vec();
        if c.dice != 0 {
            let mut rng = c.dice_rng(salt);
            for i in (1..picks.len()).rev() {
                picks.swap(i, rng.below(i + 1));
            }
        }
        items = picks.into_iter().take(show).map(|s| (s.to_string(), false)).collect();
    }
    for (label, ticked) in items {
        p.push(tick_row(&label, ticked, px, p.mx)?);
        p.gap(4);
    }
    Ok(())
}

/// The line at the foot of a form: typed by the user, or one of `pool` picked from the text (and
/// again after "Shuffle"); nothing when it is switched off (`footer` field set to `off`). The
/// line being typed is drawn like any other value; it stays a place to write while it is hidden,
/// so switching it off never takes away the way to switch it back on.
fn closing(p: &mut Page, c: &Ctx, pool: &[&str], font: FontChoice, px: f32) -> U {
    if !c.closing && c.editing != "footer" {
        return Ok(());
    }
    let typed = c.given("footer");
    if c.editing != "footer" && matches!(typed.to_lowercase().as_str(), "off" | "none" | "-") {
        return Ok(());
    }
    p.gap(4);
    p.begin("footer");
    let line = if typed.is_empty() { pool[c.dice_rng(99).below(pool.len())].to_string() } else { typed };
    p.text(&line, font, px, Align::Center)?;
    p.end();
    Ok(())
}

/// The signature line: the name in `key` (if it is a field of the form) over a line.
fn signature(p: &mut Page, c: &Ctx, key: Option<&str>, label: &str) -> U {
    let name = key.map_or_else(String::new, |k| c.given(k));
    let who = if name.trim().is_empty() { Bitmap::blank(34) } else { text::text(name.trim(), opts(30.0, FontChoice::Chancery, Align::Center, 0.0))? };
    let h = who.height;
    let mut b = Bitmap::blank(h + 6);
    draw::or_at(&mut b, &who, 0, 0);
    draw::hline(&mut b, h + 3, 40, WIDTH_DOTS - 41, 2, (0, 0));
    if let Some(k) = key {
        p.begin(k);
    }
    p.push(b);
    p.end();
    p.text(label, FontChoice::Sans, 15.0, Align::Center)
}

/// Circles in a row, the first `level` of them filled in, like a meter.
fn meter(level: usize, total: usize) -> Bitmap {
    let step = (WIDTH_DOTS as f32 - 2.0 * MX) / total as f32;
    let r = ((step / 2.0 - 4.0) as i32).min(15);
    let mut b = Bitmap::blank(2 * r as usize + 6);
    for i in 0..total {
        let cx = (MX + step * (i as f32 + 0.5)) as i32;
        draw::disc(&mut b, cx, r + 3, r, if i < level { None } else { Some(2) });
    }
    b
}

/// ★★★☆☆
fn stars(on: usize, total: usize, px: f32, align: Align, mx: f32) -> R {
    let s: Vec<&str> = (0..total).map(|i| if i < on { "\u{2605}" } else { "\u{2606}" }).collect();
    text::text(&s.join(" "), opts(px, FontChoice::Symbols, align, mx))
}

fn smiley(r: i32) -> Bitmap {
    let mut b = Bitmap::blank(2 * r as usize + 8);
    let (cx, cy) = (WIDTH_DOTS as i32 / 2, r + 4);
    draw::disc(&mut b, cx, cy, r, Some(3));
    for side in [-1, 1] {
        draw::disc(&mut b, cx + side * r / 3, cy - r / 4, r / 9 + 1, None);
    }
    for dx in -(r / 2)..=(r / 2) {
        let y = cy + r / 2 - dx * dx / r;
        for k in 0..3 {
            draw::put(&mut b, cx + dx, y + k, true);
        }
    }
    b
}

fn framed(p: Page, thick: usize) -> Bitmap {
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::rect(&mut b, 0, 0, w, h, thick);
    b
}

/// A dotted writing line `px` tall.
fn writing_line(p: &mut Page, px: f32) {
    let mut b = Bitmap::blank(px as usize + 6);
    let y = b.height - 3;
    draw::hline(&mut b, y, p.mx as usize, WIDTH_DOTS - 1 - p.mx as usize, 1, (2, 5));
    p.push(b);
}

/// The message: each line of the text in handwriting on a dotted line; with no text,
/// `blank_lines` empty lines to write on. A line is drawn as it is typed.
fn ruled(p: &mut Page, c: &Ctx, font: FontChoice, px: f32, align: Align, blank_lines: usize) -> U {
    let lines: Vec<&str> = c.body.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    p.begin("body");
    if lines.is_empty() {
        for _ in 0..blank_lines {
            writing_line(p, px + 6.0);
        }
    } else {
        for line in lines {
            let row = text::text(line, opts(px, font, align, p.mx))?;
            let h = row.height;
            let mut b = Bitmap::blank(h + 6);
            draw::or_at(&mut b, &row, 0, 0);
            draw::hline(&mut b, h + 3, p.mx as usize, WIDTH_DOTS - 1 - p.mx as usize, 1, (2, 5));
            p.push(b);
        }
    }
    p.end();
    Ok(())
}

/// The message in the middle of the paper (for "permission to ___").
fn centred(p: &mut Page, c: &Ctx, font: FontChoice, px: f32, blank_lines: usize) -> U {
    ruled(p, c, font, px, Align::Center, blank_lines)
}

pub fn while_out(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band("WHILE YOU WERE OUT", FontChoice::Bold, 30.0, Align::Center)?;
    p.gap(10);
    blank(&mut p, c, "to", "For", false, FontChoice::Marker, 30.0)?;
    blank(&mut p, c, "from", "From", false, FontChoice::Marker, 30.0)?;
    blank(&mut p, c, "time", "Time", true, FontChoice::Marker, 30.0)?;
    p.gap(4);
    boxes(&mut p, c, 1, &["Telephoned", "Came to see you", "Wants to see you", "Will call back", "Please call", "Was in a panic", "Left a strongly worded message", "Sent a carrier pigeon", "Is waiting outside", "Brought snacks", "Will haunt you"], 6, true, 24.0)?;
    p.gap(6);
    p.text("MESSAGE", FontChoice::Sans, 15.0, Align::Left)?;
    ruled(&mut p, c, FontChoice::Marker, 30.0, Align::Left, 3)?;
    p.gap(6);
    closing(&mut p, c, &["Taken by someone who was definitely listening", "Message may have improved in transit", "Please do not shoot the messenger", "Pinky promise it is accurate", "Not responsible for lost thoughts"], FontChoice::Sans, 16.0)?;
    p.gap(10);
    Ok(framed(p, 3))
}

pub fn permission(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band("PERMISSION SLIP", FontChoice::Bold, 32.0, Align::Center)?;
    p.gap(14);
    p.text("I hereby grant", FontChoice::Serif, 22.0, Align::Center)?;
    name_line(&mut p, c, "name", FontChoice::Marker, 60.0)?;
    p.text("permission to", FontChoice::Serif, 22.0, Align::Center)?;
    p.gap(4);
    centred(&mut p, c, FontChoice::Marker, 34.0, 2)?;
    p.gap(10);
    p.rule((0, 0), 2, 4);
    boxes(&mut p, c, 2, &["GRANTED", "DENIED", "ASK AGAIN AFTER COFFEE", "ASK YOUR OTHER PARENT", "ONLY IF NOBODY FINDS OUT", "DEPENDS ON THE WEATHER", "SLEEP ON IT", "DENIED BY THE CAT"], 3, true, 24.0)?;
    p.gap(8);
    closing(&mut p, c, &["Valid until revoked, forgotten or eaten.", "Void if wet, folded or ignored.", "Not valid on Mondays.", "Subject to change without notice.", "Terms and conditions: none."], FontChoice::Sans, 16.0)?;
    p.gap(14);
    signature(&mut p, c, Some("by"), "authorised by")?;
    p.gap(14);
    Ok(framed(p, 3))
}

pub fn incident(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band("INCIDENT REPORT", FontChoice::Bold, 32.0, Align::Center)?;
    p.text("Form 27-B/6", FontChoice::Typewriter, 16.0, Align::Right)?;
    p.gap(6);
    blank(&mut p, c, "by", "Reported by", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "place", "Where", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "date", "When", true, FontChoice::Marker, 28.0)?;
    p.gap(4);
    p.text("WHAT HAPPENED", FontChoice::Sans, 15.0, Align::Left)?;
    ruled(&mut p, c, FontChoice::Typewriter, 24.0, Align::Left, 3)?;
    p.gap(10);
    p.text("SEVERITY", FontChoice::Sans, 15.0, Align::Left)?;
    p.push(meter(0, 5));
    p.leader("harmless", "catastrophic", FontChoice::Sans, 16.0)?;
    p.gap(8);
    boxes(&mut p, c, 3, &["Coffee was involved", "A cat was involved", "There were witnesses", "It was my fault", "Nobody will admit it", "Fire was involved (small)", "A spreadsheet was harmed", "Somebody cried", "The Wi-Fi was blamed", "It was caught on camera"], 4, true, 22.0)?;
    p.gap(6);
    closing(&mut p, c, &["Filed under: nobody cares", "Filed under: not my problem", "Copies sent to nobody", "Case closed (by lack of interest)", "To be forgotten by Friday"], FontChoice::Typewriter, 16.0)?;
    p.gap(10);
    Ok(framed(p, 3))
}

pub fn passive(c: &Ctx) -> R {
    let blank_name = |s: String| if s.is_empty() { "__________".to_string() } else { s };
    let mut p = Page::new(22.0);
    p.gap(18);
    p.text(&format!("Hi {},", blank_name(c.given("to"))), FontChoice::Script, 38.0, Align::Left)?;
    p.gap(4);
    p.text("Just a friendly little reminder:", FontChoice::Script, 28.0, Align::Left)?;
    p.gap(8);
    ruled(&mut p, c, FontChoice::Script, 34.0, Align::Left, 3)?;
    p.gap(10);
    p.text("Thanks so much!!", FontChoice::Script, 30.0, Align::Left)?;
    p.text(&format!("- {}", blank_name(c.given("from"))), FontChoice::Script, 34.0, Align::Right)?;
    p.gap(10);
    p.push(smiley(24));
    p.gap(6);
    closing(&mut p, c, &["P.S. I'm not mad.", "P.S. No rush. (Rush.)", "P.S. We'll talk later.", "P.S. It's fine. Really.", "P.S. Just saying."], FontChoice::Script, 24.0)?;
    p.gap(18);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::hline(&mut b, 5, 5, w - 5, 2, (3, 5));
    draw::hline(&mut b, h - 6, 5, w - 5, 2, (3, 5));
    draw::vline(&mut b, 5, 5, h - 5, 2, (3, 5));
    draw::vline(&mut b, w - 6, 5, h - 5, 2, (3, 5));
    Ok(b)
}

pub fn pep(c: &Ctx) -> R {
    let name = c.given("name");
    let mut p = Page::new(24.0);
    p.band("PEP TALK", FontChoice::Impact, 44.0, Align::Center)?;
    p.gap(16);
    p.push(fit_line(&format!("HEY {}", if name.is_empty() { "YOU".to_string() } else { name.to_uppercase() }), FontChoice::Impact, 24.0, 70.0)?);
    p.gap(6);
    p.push(stars(5, 5, 40.0, Align::Center, 0.0)?);
    p.gap(10);
    p.begin("body");
    if c.body.trim().is_empty() {
        for _ in 0..3 {
            writing_line(&mut p, 34.0);
        }
    } else {
        p.text(c.body.trim(), FontChoice::Bold, 30.0, Align::Center)?;
    }
    p.end();
    p.gap(10);
    p.flourish();
    p.gap(6);
    closing(&mut p, c, &["Take as needed. No refills required.", "Side effects: confidence.", "Repeat every morning.", "Valid for 24 hours.", "Not a substitute for sleep."], FontChoice::Sans, 18.0)?;
    p.gap(24);
    let mut b = p.done();
    draw::ornate_border(&mut b, 6);
    Ok(b)
}

pub fn excuse(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band("EXCUSE NOTE", FontChoice::Bold, 32.0, Align::Center)?;
    p.gap(14);
    p.text("Please excuse", FontChoice::Serif, 22.0, Align::Center)?;
    name_line(&mut p, c, "who", FontChoice::Marker, 60.0)?;
    p.text("from", FontChoice::Serif, 22.0, Align::Center)?;
    p.gap(4);
    centred(&mut p, c, FontChoice::Marker, 34.0, 2)?;
    p.gap(10);
    p.rule((0, 0), 2, 4);
    p.text("BECAUSE", FontChoice::Sans, 15.0, Align::Left)?;
    boxes(&mut p, c, 5, &["My dog ate it", "Traffic was terrible", "Mercury is retrograde", "Adulting", "The Wi-Fi was down", "I just don't wanna", "A seagull stole it", "Autocorrect did it", "My cat sat on the keyboard", "Daylight saving", "The printer jammed", "I was abducted (briefly)"], 6, true, 22.0)?;
    p.gap(12);
    signature(&mut p, c, Some("by"), "signed")?;
    p.gap(6);
    closing(&mut p, c, &["Void where prohibited.", "Not valid in court.", "Forged signatures not accepted.", "No refunds on excuses.", "Valid once, like a good lie."], FontChoice::Sans, 15.0)?;
    p.gap(12);
    Ok(framed(p, 3))
}

pub fn complaint(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band("COMPLAINT FORM", FontChoice::Bold, 32.0, Align::Center)?;
    p.gap(10);
    blank(&mut p, c, "name", "Complainer", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "dept", "Sent to", false, FontChoice::Marker, 28.0)?;
    p.gap(4);
    p.text("NATURE OF COMPLAINT", FontChoice::Sans, 15.0, Align::Left)?;
    ruled(&mut p, c, FontChoice::Typewriter, 24.0, Align::Left, 3)?;
    p.gap(10);
    p.text("OUTRAGE LEVEL", FontChoice::Sans, 15.0, Align::Left)?;
    p.push(meter(0, 10));
    p.leader("mild", "furious", FontChoice::Sans, 16.0)?;
    p.gap(8);
    boxes(&mut p, c, 7, &["I want a refund", "I want the manager", "I want a hug", "I want it in writing", "I want an apology (sincere)", "I want my money AND a pony", "I want it to be Friday"], 3, true, 22.0)?;
    p.gap(6);
    p.rule((3, 5), 2, 3);
    closing(&mut p, c, &["Your complaint is important to us.", "Estimated reply: never.", "Please hold. Forever.", "Your call is not important to us.", "Thank you for your patience (we have none)."], FontChoice::Sans, 18.0)?;
    p.gap(6);
    p.barcode(&format!("CF{}", c.number(27, 8)), 44)?;
    p.gap(10);
    Ok(framed(p, 3))
}

pub fn apology(c: &Ctx) -> R {
    let blank_name = |s: String| if s.is_empty() { "________".to_string() } else { s };
    let mut p = Page::new(26.0);
    p.gap(26);
    p.push(fit_line("OFFICIAL APOLOGY", FontChoice::Playbill, 24.0, 48.0)?);
    p.gap(4);
    p.flourish();
    p.gap(8);
    p.text(&format!("Dear {},", blank_name(c.given("to"))), FontChoice::Serif, 24.0, Align::Left)?;
    p.text("I am sorry for:", FontChoice::Serif, 24.0, Align::Left)?;
    p.gap(4);
    ruled(&mut p, c, FontChoice::Marker, 32.0, Align::Left, 3)?;
    p.gap(12);
    p.text("SINCERITY", FontChoice::Sans, 15.0, Align::Left)?;
    boxes(&mut p, c, 8, &["Genuine", "Mostly genuine", "Forced", "Only because I was told to", "Sorry, not sorry", "Sorry I got caught", "Pending coffee"], 4, true, 22.0)?;
    p.gap(10);
    closing(&mut p, c, &["Valid for one (1) use.", "Not valid for repeat offences.", "Redeemable once, then it is on you.", "Handle with care.", "Please do not frame."], FontChoice::Sans, 16.0)?;
    p.gap(12);
    signature(&mut p, c, Some("from"), "signed")?;
    p.gap(26);
    let mut b = p.done();
    draw::ornate_border(&mut b, 8);
    Ok(b)
}

pub fn iou(c: &Ctx) -> R {
    let mut p = Page::new(MX + 6.0);
    p.gap(22);
    p.push(fit_line("I.O.U.", FontChoice::Impact, 30.0, 120.0)?);
    p.gap(8);
    centred(&mut p, c, FontChoice::Marker, 36.0, 2)?;
    p.gap(12);
    blank(&mut p, c, "from", "Owed by", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "to", "Owed to", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "due", "Due", true, FontChoice::Marker, 28.0)?;
    p.gap(4);
    p.text("PAYABLE IN", FontChoice::Sans, 15.0, Align::Left)?;
    boxes(&mut p, c, 9, &["Cash", "Coffee", "Hugs", "Pizza", "Favours", "Tacos", "Back rubs", "Good vibes", "Dumplings"], 4, true, 22.0)?;
    p.gap(14);
    signature(&mut p, c, None, "debtor")?;
    p.gap(8);
    closing(&mut p, c, &["Witness: ____________________", "Cross my heart, hope to die.", "Legally binding in my head.", "Interest: 1 compliment per day.", "Late fee: one hug."], FontChoice::Typewriter, 18.0)?;
    p.gap(22);
    let mut b = p.done();
    let (w, h) = (WIDTH_DOTS - 1, b.height - 1);
    draw::rect(&mut b, 0, 0, w, h, 1);
    draw::hline(&mut b, 8, 8, w - 8, 2, (7, 5));
    draw::hline(&mut b, h - 9, 8, w - 8, 2, (7, 5));
    draw::vline(&mut b, 8, 8, h - 8, 2, (7, 5));
    draw::vline(&mut b, w - 9, 8, h - 8, 2, (7, 5));
    Ok(b)
}

/// For people whose model was "obviously" quantized overnight.
pub fn butthurt(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band("BUTTHURT REPORT", FontChoice::Bold, 32.0, Align::Center)?;
    p.text("Form Q-4 (lobotomy claim)", FontChoice::Typewriter, 14.0, Align::Right)?;
    p.gap(6);
    blank(&mut p, c, "user", "Complainer", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "provider", "Provider", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "model", "Model accused", false, FontChoice::Marker, 28.0)?;
    p.gap(4);
    p.text("WHAT IS WRONG", FontChoice::Sans, 15.0, Align::Left)?;
    ruled(&mut p, c, FontChoice::Typewriter, 24.0, Align::Left, 3)?;
    p.gap(10);
    p.text("I BELIEVE THAT", FontChoice::Sans, 15.0, Align::Left)?;
    boxes(&mut p, c, 10, &["It got dumber overnight", "It is quantized (no proof)", "It worked better last Tuesday", "They are nerfing it on purpose", "My prompt is fine, obviously", "It was smarter in March", "They swapped the model at 3am", "I can feel the INT4", "The vibes are off"], 5, true, 21.0)?;
    p.gap(8);
    p.text("EVIDENCE PROVIDED", FontChoice::Sans, 15.0, Align::Left)?;
    boxes(&mut p, c, 11, &["None", "Vibes", "One screenshot, cropped", "A Reddit thread", "My cousin agrees", "A tweet from 2023"], 3, false, 21.0)?;
    p.gap(8);
    p.text("BUTTHURT LEVEL", FontChoice::Sans, 15.0, Align::Left)?;
    p.push(meter(0, 10));
    p.leader("mild", "unsubscribing", FontChoice::Sans, 16.0)?;
    p.gap(8);
    p.rule((3, 5), 2, 3);
    closing(&mut p, c, &["Benchmarks were not consulted.", "Ticket closed: works on my end.", "Have you tried a better prompt?", "Your feelings have been logged.", "Status: skill issue (pending)."], FontChoice::Sans, 17.0)?;
    p.gap(6);
    p.barcode(&format!("Q4{}", c.number(32, 8)), 40)?;
    p.gap(10);
    Ok(framed(p, 3))
}

/// "Cooking 4" is ("Cooking", Some(4)); a line without a rating is ("…", None).
fn rating_of(line: &str) -> (&str, Option<usize>) {
    match line.rsplit_once(char::is_whitespace) {
        Some((head, last)) => match last.parse::<usize>() {
            Ok(n) if (0..=5).contains(&n) => (head.trim(), Some(n)),
            _ => (line, None),
        },
        None => (line, None),
    }
}

fn rated_row(label: &str, n: usize) -> R {
    let short: String = label.chars().take(16).collect();
    let left = text::text(&short, opts(25.0, FontChoice::Sans, Align::Left, MX))?;
    let right = stars(n, 5, 21.0, Align::Right, MX)?;
    let mut b = Bitmap::blank(left.height.max(right.height));
    draw::or_at(&mut b, &left, 0, 0);
    draw::or_at(&mut b, &right, 0, 0);
    Ok(b)
}

pub fn review(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band("PERFORMANCE REVIEW", FontChoice::Bold, 28.0, Align::Center)?;
    p.gap(10);
    blank(&mut p, c, "name", "Employee", false, FontChoice::Marker, 28.0)?;
    blank(&mut p, c, "period", "Period", false, FontChoice::Marker, 28.0)?;
    p.gap(6);
    // Ratings are only the ones written ("Cooking 4"); a line without one has empty stars.
    let defaults = ["Punctuality", "Teamwork", "Charm", "Dishes", "Snack stealing"];
    let lines: Vec<&str> = if c.lines().is_empty() { defaults.to_vec() } else { c.lines() };
    let (mut total, mut rated) = (0, 0);
    for line in lines {
        let (label, rating) = rating_of(line);
        p.push(rated_row(label, rating.unwrap_or(0))?);
        p.gap(6);
        if let Some(n) = rating {
            total += n;
            rated += 1;
        }
    }
    let average = if rated == 0 { 0 } else { (total as f32 / rated as f32).round() as usize };
    p.rule((0, 0), 2, 5);
    p.text("OVERALL", FontChoice::Sans, 15.0, Align::Center)?;
    p.push(stars(average, 5, 40.0, Align::Center, 0.0)?);
    p.gap(8);
    boxes(&mut p, c, 12, &["Promote", "Keep an eye on", "Give more coffee", "Give a raise (lol)", "Move to the quiet desk", "Send on holiday", "Ask Susan"], 3, true, 22.0)?;
    p.gap(10);
    signature(&mut p, c, None, "manager")?;
    closing(&mut p, c, &["Raise denied pending coffee.", "This review will self-destruct.", "Results may vary by Monday.", "Reviewed by someone who skimmed.", "Next review: when we remember."], FontChoice::Sans, 16.0)?;
    p.gap(12);
    Ok(framed(p, 3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Theme;
    use crate::render::theme::{Fields, render};

    #[test]
    fn ratings_are_read_from_the_end_of_a_line() {
        assert_eq!(rating_of("Cooking 4"), ("Cooking", Some(4)));
        assert_eq!(rating_of("Snack stealing 5"), ("Snack stealing", Some(5)));
        assert_eq!(rating_of("Punctuality"), ("Punctuality", None));
        assert_eq!(rating_of("Charm 9"), ("Charm 9", None));
    }

    fn forms() -> impl Iterator<Item = Theme> {
        Theme::BUILTIN.into_iter().filter(|t| t.is_form())
    }

    fn print(theme: Theme, body: &str, fields: &[(&str, &str)]) -> Bitmap {
        let f: Fields = fields.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        render(theme, body, &f, TextOpts::default()).unwrap_or_else(|e| panic!("{}: {e}", theme.id()))
    }

    #[test]
    fn every_form_prints_blank_without_any_text() {
        for t in forms() {
            let b = print(t, "", &[]);
            assert!(b.height > 200, "{} is {} rows", t.id(), b.height);
        }
    }

    #[test]
    fn nothing_is_ticked_for_you_but_a_typed_list_can_tick() {
        let plain = print(Theme::Excuse, "", &[]);
        let ticked = print(Theme::Excuse, "", &[("boxes", "x Traffic | My dog ate it")]);
        assert_ne!(plain, ticked, "a typed list replaces the boxes");
        // The same blank form is always the same sheet.
        assert_eq!(plain, print(Theme::Excuse, "", &[]));
    }

    #[test]
    fn shuffle_changes_the_wording_and_each_press_gives_another_one() {
        let base = print(Theme::Incident, "", &[]);
        let one = print(Theme::Incident, "", &[("_shuffle", "1")]);
        let two = print(Theme::Incident, "", &[("_shuffle", "2")]);
        assert_ne!(base, one);
        assert_ne!(one, two);
    }

    #[test]
    fn the_closing_line_is_typed_random_or_off() {
        let on = print(Theme::Permission, "", &[]);
        let off = print(Theme::Permission, "", &[("footer", "off")]);
        let off2 = print(Theme::Permission, "", &[("_closing", "off")]);
        let typed = print(Theme::Permission, "", &[("footer", "Void in Utah")]);
        assert!(off.height < on.height, "off drops the line");
        assert_eq!(off, off2, "the switch and the word agree");
        assert_ne!(typed, on);
        assert_ne!(typed, off);
    }

    #[test]
    fn the_part_being_typed_is_drawn_live_and_the_rest_of_the_ticket_does_not_move() {
        let fields: Fields = [("name".to_string(), "Alex".to_string())].into();
        let (plain, regions) = crate::render::theme::render_mapped(Theme::Complaint, "My coffee is cold", &fields, TextOpts::default()).unwrap();
        let keys: Vec<&str> = regions.iter().map(|r| r.key.as_str()).collect();
        assert!(keys.contains(&"name") && keys.contains(&"dept") && keys.contains(&"body") && keys.contains(&"footer"), "{keys:?}");
        assert!(regions.iter().all(|r| r.y0 < r.y1 && r.y1 <= plain.height && r.x0 < r.x1 && r.x1 <= WIDTH_DOTS));
        // Being written on draws nothing different: the theme inks the part like any other value,
        // so the sheet is the one it would be with the name already typed.
        let mut typing = fields.clone();
        typing.insert("_editing".into(), "name".into());
        let (during, again) = crate::render::theme::render_mapped(Theme::Complaint, "My coffee is cold", &typing, TextOpts::default()).unwrap();
        assert_eq!(plain, during, "the part being typed is drawn, not blanked");
        // Every letter lands on the paper as it is typed, in the theme's own typeface.
        let mut longer = typing.clone();
        longer.insert("name".into(), "Alexandra".into());
        let (drawn, _) = crate::render::theme::render_mapped(Theme::Complaint, "My coffee is cold", &longer, TextOpts::default()).unwrap();
        assert_ne!(plain, drawn, "what is typed shows on the ticket");
        // And the rest of the ticket has not moved for it.
        let top = |k: &str, r: &[crate::render::Region]| r.iter().find(|x| x.key == k).map(|x| x.y0);
        assert_eq!(top("body", &regions), top("body", &again), "typing a name does not shift the message");
        assert_eq!(top("footer", &regions), top("footer", &again), "nor the closing line");
        // Another ticket is read as it is drawn; a sideways one cannot be, and has none.
        let (_, receipt) = crate::render::theme::render_mapped(Theme::Receipt, "Tea 1.00", &Fields::new(), TextOpts::default()).unwrap();
        assert!(receipt.iter().any(|r| r.key == "store") && receipt.iter().any(|r| r.key == "body"));
        let (_, flight) = crate::render::theme::render_mapped(Theme::Flight, "", &Fields::new(), TextOpts::default()).unwrap();
        assert!(!flight.is_empty());
    }
}
