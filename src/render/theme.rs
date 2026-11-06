//! Writing themes: the same text, dressed as a shop receipt, a parchment scroll, a boarding pass or
//! a dot-matrix report. A theme is a font choice, a layout and a few 1-bit decorations (borders,
//! rules, stamps, barcodes, speckle). Some themes also take a few named fields (a store name, a
//! seat number); the body text is always the main content.
//!
//! Every theme returns an ordinary [`Bitmap`], so the preview, the CLI and the MCP server all go
//! through the same code as the printer.

mod papers;
mod retro;
mod shop;
mod world;

use std::collections::BTreeMap;

use super::draw::{self, Rng};
use super::text::{self, Align, FontChoice, TextOpts};
use super::{CodeKind, CodeOpts};
use crate::protocol::{Bitmap, WIDTH_DOTS};

/// Named extras a theme asks for, by key (`store`, `seat`…).
pub type Fields = BTreeMap<String, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Shop,
    OldWorld,
    Papers,
    Retro,
}

impl Family {
    pub const ALL: [Family; 4] = [Family::Shop, Family::OldWorld, Family::Papers, Family::Retro];

    pub fn label(self) -> &'static str {
        match self {
            Family::Shop => "Shop & diner",
            Family::OldWorld => "Parchment & old world",
            Family::Papers => "Fun official papers",
            Family::Retro => "Techy & retro",
        }
    }
}

/// What a theme asks the user for, besides the text.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    /// Shown greyed in the empty field, and used when the field is left empty.
    pub hint: &'static str,
}

const fn f(key: &'static str, label: &'static str, hint: &'static str) -> Field {
    Field { key, label, hint }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    #[default]
    Plain,
    // Shop & diner
    Receipt,
    Diner,
    Pharmacy,
    Coupon,
    Fine,
    // Parchment & old world
    Parchment,
    Decree,
    Wanted,
    Treasure,
    // Fun official papers
    Boarding,
    Movie,
    Fortune,
    Diploma,
    Horoscope,
    Fridge,
    Checklist,
    Ransom,
    NameTag,
    // Techy & retro
    Terminal,
    DotMatrix,
    Label,
    Telegram,
    PunchCard,
    Poster,
}

const NO_FIELDS: &[Field] = &[];

impl Theme {
    pub const ALL: [Theme; 25] = [
        Theme::Plain,
        Theme::Receipt,
        Theme::Diner,
        Theme::Pharmacy,
        Theme::Coupon,
        Theme::Fine,
        Theme::Parchment,
        Theme::Decree,
        Theme::Wanted,
        Theme::Treasure,
        Theme::Boarding,
        Theme::Movie,
        Theme::Fortune,
        Theme::Diploma,
        Theme::Horoscope,
        Theme::Fridge,
        Theme::Checklist,
        Theme::Ransom,
        Theme::NameTag,
        Theme::Terminal,
        Theme::DotMatrix,
        Theme::Label,
        Theme::Telegram,
        Theme::PunchCard,
        Theme::Poster,
    ];

    /// The word used on the command line and in the MCP tool: `receipt`, `name-tag`…
    pub fn id(self) -> &'static str {
        match self {
            Theme::Plain => "plain",
            Theme::Receipt => "receipt",
            Theme::Diner => "diner",
            Theme::Pharmacy => "pharmacy",
            Theme::Coupon => "coupon",
            Theme::Fine => "fine",
            Theme::Parchment => "parchment",
            Theme::Decree => "decree",
            Theme::Wanted => "wanted",
            Theme::Treasure => "treasure",
            Theme::Boarding => "boarding",
            Theme::Movie => "movie",
            Theme::Fortune => "fortune",
            Theme::Diploma => "diploma",
            Theme::Horoscope => "horoscope",
            Theme::Fridge => "fridge",
            Theme::Checklist => "checklist",
            Theme::Ransom => "ransom",
            Theme::NameTag => "name-tag",
            Theme::Terminal => "terminal",
            Theme::DotMatrix => "dot-matrix",
            Theme::Label => "label",
            Theme::Telegram => "telegram",
            Theme::PunchCard => "punch-card",
            Theme::Poster => "poster",
        }
    }

    pub fn from_id(name: &str) -> Option<Theme> {
        let want = name.trim().to_ascii_lowercase().replace([' ', '_'], "-");
        Theme::ALL.into_iter().find(|t| t.id() == want || t.id().replace('-', "") == want.replace('-', ""))
    }

    pub fn name(self) -> &'static str {
        match self {
            Theme::Plain => "Plain",
            Theme::Receipt => "Receipt",
            Theme::Diner => "Diner check",
            Theme::Pharmacy => "Pharmacy",
            Theme::Coupon => "Coupon",
            Theme::Fine => "Fine",
            Theme::Parchment => "Parchment",
            Theme::Decree => "Royal decree",
            Theme::Wanted => "Wanted",
            Theme::Treasure => "Treasure map",
            Theme::Boarding => "Boarding pass",
            Theme::Movie => "Movie ticket",
            Theme::Fortune => "Fortune",
            Theme::Diploma => "Diploma",
            Theme::Horoscope => "Horoscope",
            Theme::Fridge => "Fridge note",
            Theme::Checklist => "Checklist",
            Theme::Ransom => "Ransom note",
            Theme::NameTag => "Name tag",
            Theme::Terminal => "Terminal",
            Theme::DotMatrix => "Dot matrix",
            Theme::Label => "Label maker",
            Theme::Telegram => "Telegram",
            Theme::PunchCard => "Punch card",
            Theme::Poster => "Big poster",
        }
    }

    /// One line on what it does, for the picker and the MCP tool.
    pub fn tagline(self) -> &'static str {
        match self {
            Theme::Plain => "Your text, your font",
            Theme::Receipt => "A shop receipt: one item per line, with the price at the end",
            Theme::Diner => "A greasy-spoon guest check with a tip line",
            Theme::Pharmacy => "A prescription label with a barcode",
            Theme::Coupon => "A cut-out discount coupon",
            Theme::Fine => "An official penalty notice",
            Theme::Parchment => "An aged scroll with an ornate border and a wax seal",
            Theme::Decree => "A royal decree under a crown",
            Theme::Wanted => "A wanted poster with a reward",
            Theme::Treasure => "A treasure map with a dotted trail and an X",
            Theme::Boarding => "A boarding pass with a stub and barcode",
            Theme::Movie => "A cinema ticket with notches",
            Theme::Fortune => "A fortune-cookie slip with lucky numbers",
            Theme::Diploma => "A certificate with a seal",
            Theme::Horoscope => "Your sign and a reading",
            Theme::Fridge => "A handwritten note held by a magnet",
            Theme::Checklist => "A to-do list with boxes to tick",
            Theme::Ransom => "Every letter in a different font, cut from a magazine",
            Theme::NameTag => "HELLO my name is…",
            Theme::Terminal => "Command-line output with a prompt and a cursor",
            Theme::DotMatrix => "A continuous-paper report with sprocket holes",
            Theme::Label => "White-on-black embossed label strips",
            Theme::Telegram => "ALL CAPS and STOP",
            Theme::PunchCard => "Your text as holes on a punch card",
            Theme::Poster => "Every line as big as the paper allows",
        }
    }

    pub fn family(self) -> Option<Family> {
        Some(match self {
            Theme::Plain => return None,
            Theme::Receipt | Theme::Diner | Theme::Pharmacy | Theme::Coupon | Theme::Fine => Family::Shop,
            Theme::Parchment | Theme::Decree | Theme::Wanted | Theme::Treasure => Family::OldWorld,
            Theme::Boarding | Theme::Movie | Theme::Fortune | Theme::Diploma | Theme::Horoscope | Theme::Fridge | Theme::Checklist | Theme::Ransom | Theme::NameTag => Family::Papers,
            Theme::Terminal | Theme::DotMatrix | Theme::Label | Theme::Telegram | Theme::PunchCard | Theme::Poster => Family::Retro,
        })
    }

    /// The little form for this theme.
    pub fn fields(self) -> &'static [Field] {
        const RECEIPT: &[Field] = &[f("store", "Store", "FRESH MART"), f("address", "Address", "12 Rue du Pain"), f("date", "Date", "today")];
        const DINER: &[Field] = &[f("place", "Place", "Joe's Diner"), f("table", "Table", "12"), f("server", "Server", "Dolores")];
        const PHARMACY: &[Field] = &[f("patient", "Patient", "A. Person"), f("doctor", "Doctor", "Dr. Feelgood"), f("refills", "Refills", "0")];
        const COUPON: &[Field] = &[f("offer", "Offer", "-50%"), f("code", "Code", "SAVE-4U"), f("expires", "Expires", "never")];
        const FINE: &[Field] = &[f("offender", "Offender", "Citizen"), f("amount", "Amount", "42.00"), f("number", "Number", "000123")];
        const PARCHMENT: &[Field] = &[f("title", "Heading", "Hear ye!"), f("signed", "Seal letter", "")];
        const DECREE: &[Field] = &[f("title", "Title", "Royal Decree"), f("signed", "Signed", "The King")];
        const WANTED: &[Field] = &[f("name", "Name", "Bad Bart"), f("reward", "Reward", "$5,000")];
        const TREASURE: &[Field] = &[f("place", "Place", "Skull Island")];
        const BOARDING: &[Field] = &[f("from", "From", "PAR"), f("to", "To", "NYC"), f("name", "Passenger", "A. PERSON"), f("seat", "Seat", "12A")];
        const MOVIE: &[Field] = &[f("room", "Room", "Room 7"), f("seat", "Seat", "H12"), f("time", "Time", "20:15")];
        const DIPLOMA: &[Field] = &[f("name", "Name", "A. Person"), f("award", "Award", "Certificate of Awesomeness"), f("signed", "Signed", "The Management")];
        const HOROSCOPE: &[Field] = &[f("sign", "Sign", "Leo")];
        const CHECKLIST: &[Field] = &[f("title", "Title", "TO DO")];
        const TERMINAL: &[Field] = &[f("prompt", "Prompt", "user@printer:~$")];
        const DOTMATRIX: &[Field] = &[f("title", "Report", "DAILY REPORT")];
        const TELEGRAM: &[Field] = &[f("to", "To", "")];
        match self {
            Theme::Receipt => RECEIPT,
            Theme::Diner => DINER,
            Theme::Pharmacy => PHARMACY,
            Theme::Coupon => COUPON,
            Theme::Fine => FINE,
            Theme::Parchment => PARCHMENT,
            Theme::Decree => DECREE,
            Theme::Wanted => WANTED,
            Theme::Treasure => TREASURE,
            Theme::Boarding => BOARDING,
            Theme::Movie => MOVIE,
            Theme::Diploma => DIPLOMA,
            Theme::Horoscope => HOROSCOPE,
            Theme::Checklist => CHECKLIST,
            Theme::Terminal => TERMINAL,
            Theme::DotMatrix => DOTMATRIX,
            Theme::Telegram => TELEGRAM,
            _ => NO_FIELDS,
        }
    }

    /// Example text that shows the theme off; also what the preview uses when the box is empty.
    pub fn sample(self) -> &'static str {
        match self {
            Theme::Plain => "Hello from Windows!",
            Theme::Receipt => "Coffee 2.50\nCroissant 1.80\n2 x Orange juice 6.40\nBaguette 1.10\nBirthday cake 14.90",
            Theme::Diner => "Cheeseburger 9.50\nFries 3.00\nMilkshake 4.50\nApple pie 3.75",
            Theme::Pharmacy => "Take one tablet twice daily with food. Do not operate heavy machinery or argue on the internet.",
            Theme::Coupon => "On your next cup of coffee. One per customer, while the mugs last.",
            Theme::Fine => "Excessive enthusiasm in a quiet zone",
            Theme::Parchment => "Be it known to all who read this scroll that the coffee machine is, from this day, out of order. The realm shall endure.",
            Theme::Decree => "By order of the crown, every Friday shall henceforth be a half day, and all cakes shall be shared in equal parts.",
            Theme::Wanted => "Stole the last biscuit from the office kitchen. Last seen heading for the coffee machine. Armed with crumbs.",
            Theme::Treasure => "Ten paces from the old oak\nTurn left at the sleeping dragon\nDig where the shadow ends",
            Theme::Boarding => "Have a safe flight and don't forget your towel.",
            Theme::Movie => "THE PRINTER STRIKES BACK",
            Theme::Fortune => "A pleasant surprise is waiting for you in the paper tray.",
            Theme::Diploma => "For outstanding dedication to printing things that nobody asked for.",
            Theme::Horoscope => "Mercury is in a good mood. Expect a small win before lunch and an unread message you will enjoy. Avoid decisions about furniture.",
            Theme::Fridge => "Don't forget:\n- milk\n- call grandma\n- water the plant\nBack at 6, love you!",
            Theme::Checklist => "Buy milk\nCall the plumber\nx Print the tickets\nWater the plants",
            Theme::Ransom => "WE HAVE YOUR COFFEE\nLEAVE 5 EUROS\nIN THE JAR",
            Theme::NameTag => "Alex",
            Theme::Terminal => "cargo build --release\n   Compiling tronic-printer v0.1.0\n    Finished release in 42.0s",
            Theme::DotMatrix => "Printer connected\nPaper loaded\nBattery 87 percent\nAll systems nominal",
            Theme::Label => "KITCHEN\nSPARE KEYS\nDO NOT TOUCH",
            Theme::Telegram => "Arrived safely. Weather lovely. Send more coffee.",
            Theme::PunchCard => "HELLO WORLD 1964",
            Theme::Poster => "BIG\nNEWS\nTODAY",
        }
    }

    /// Themes that make something of an empty text box (a fortune, a boarding pass).
    pub fn body_optional(self) -> bool {
        matches!(self, Theme::Boarding | Theme::Fortune | Theme::Horoscope)
    }
}

/// The text and fields a theme is drawn from.
pub struct Ctx<'a> {
    pub body: &'a str,
    fields: &'a Fields,
    /// The same text always gets the same numbers, speckle and codes.
    pub seed: u64,
}

impl<'a> Ctx<'a> {
    pub fn new(body: &'a str, fields: &'a Fields) -> Ctx<'a> {
        // Empty fields count as missing, so they do not change the numbers and speckle.
        let given: Vec<&str> = fields.iter().map(|(_, v)| v.trim()).filter(|v| !v.is_empty()).collect();
        Ctx { body, fields, seed: draw::seed_of(&format!("{body}|{}", given.join("|"))) }
    }

    /// A field's value, or its default when left empty.
    pub fn field(&self, theme: Theme, key: &str) -> String {
        let given = self.fields.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());
        match given {
            Some(v) => v.to_string(),
            None => theme.fields().iter().find(|d| d.key == key).map(|d| d.hint.to_string()).unwrap_or_default(),
        }
    }

    /// Non-empty lines of the body.
    pub fn lines(&self) -> Vec<&str> {
        self.body.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
    }

    pub fn rng(&self, salt: u64) -> Rng {
        Rng::new(self.seed ^ salt.wrapping_mul(0x517C_C1B7_2722_0A95))
    }

    /// A number made from the text, `digits` long.
    pub fn number(&self, salt: u64, digits: usize) -> String {
        let mut rng = self.rng(salt);
        (0..digits).map(|_| char::from(b'0' + rng.below(10) as u8)).collect()
    }
}

/// Draw `body` as `theme`. `base` is what the Plain theme uses (font, size, alignment…); the others
/// bring their own.
pub fn render(theme: Theme, body: &str, fields: &Fields, base: TextOpts) -> Result<Bitmap, String> {
    if theme == Theme::Plain {
        return text::text(body, base);
    }
    let body = if body.trim().is_empty() && theme.body_optional() { theme.sample() } else { body };
    if body.trim().is_empty() {
        return Err("nothing to print".into());
    }
    let ctx = Ctx::new(body, fields);
    match theme {
        Theme::Plain => unreachable!(),
        Theme::Receipt => shop::receipt(&ctx),
        Theme::Diner => shop::diner(&ctx),
        Theme::Pharmacy => shop::pharmacy(&ctx),
        Theme::Coupon => shop::coupon(&ctx),
        Theme::Fine => shop::fine(&ctx),
        Theme::Parchment => world::parchment(&ctx),
        Theme::Decree => world::decree(&ctx),
        Theme::Wanted => world::wanted(&ctx),
        Theme::Treasure => world::treasure(&ctx),
        Theme::Boarding => papers::boarding(&ctx),
        Theme::Movie => papers::movie(&ctx),
        Theme::Fortune => papers::fortune(&ctx),
        Theme::Diploma => papers::diploma(&ctx),
        Theme::Horoscope => papers::horoscope(&ctx),
        Theme::Fridge => papers::fridge(&ctx),
        Theme::Checklist => papers::checklist(&ctx),
        Theme::Ransom => papers::ransom(&ctx),
        Theme::NameTag => papers::name_tag(&ctx),
        Theme::Terminal => retro::terminal(&ctx),
        Theme::DotMatrix => retro::dot_matrix(&ctx),
        Theme::Label => retro::label(&ctx),
        Theme::Telegram => retro::telegram(&ctx),
        Theme::PunchCard => retro::punch_card(&ctx),
        Theme::Poster => retro::poster(&ctx),
    }
}

// ---- the toolkit the themes share ----------------------------------------------------------

/// A ticket built from the top down: lines of text and pictures stacked, then finished.
pub struct Page {
    parts: Vec<Bitmap>,
    /// Blank dots kept free left and right of the text.
    pub mx: f32,
}

type R = Result<(), String>;

impl Page {
    pub fn new(mx: f32) -> Page {
        Page { parts: Vec::new(), mx }
    }

    pub fn gap(&mut self, rows: usize) {
        if rows > 0 {
            self.parts.push(Bitmap::blank(rows));
        }
    }

    pub fn push(&mut self, b: Bitmap) {
        self.parts.push(b);
    }

    pub fn height(&self) -> usize {
        self.parts.iter().map(|p| p.height).sum()
    }

    fn opts(&self, font: FontChoice, px: f32, align: Align) -> TextOpts {
        TextOpts { px, font, align, invert: false, break_anywhere: false, line_gap: 0.0, margin_y: 0.0, margin_x: self.mx }
    }

    /// Wrapped text; a blank string is just a gap.
    pub fn text(&mut self, s: &str, font: FontChoice, px: f32, align: Align) -> R {
        if s.trim().is_empty() {
            self.gap((px * 0.6) as usize);
            return Ok(());
        }
        let b = text::text(s, self.opts(font, px, align))?;
        self.parts.push(b);
        Ok(())
    }

    /// Wrapped at any character, for columns that must stay put (terminal output, listings).
    pub fn text_any(&mut self, s: &str, font: FontChoice, px: f32) -> R {
        let mut o = self.opts(font, px, Align::Left);
        o.break_anywhere = true;
        self.parts.push(text::text(s, o)?);
        Ok(())
    }

    /// Like [`Page::text`], tightening the gap between lines by `squeeze` dots.
    pub fn text_gap(&mut self, s: &str, font: FontChoice, px: f32, align: Align, line_gap: f32) -> R {
        let mut o = self.opts(font, px, align);
        o.line_gap = line_gap;
        self.parts.push(text::text(s, o)?);
        Ok(())
    }

    /// White on a full-width black band.
    pub fn band(&mut self, s: &str, font: FontChoice, px: f32, align: Align) -> R {
        let mut o = self.opts(font, px, align);
        o.invert = true;
        o.margin_y = 5.0;
        self.parts.push(text::text(s, o)?);
        Ok(())
    }

    /// Text each line of which is scaled to fill the width (as large as `max_px`).
    pub fn shout(&mut self, s: &str, font: FontChoice, max_px: f32) -> R {
        for line in s.lines().map(str::trim).filter(|l| !l.is_empty()) {
            self.parts.push(fit_line(line, font, self.mx, max_px)?);
        }
        Ok(())
    }

    /// `left` ........ `right`
    pub fn leader(&mut self, left: &str, right: &str, font: FontChoice, px: f32) -> R {
        self.parts.push(leader_line(left, right, font, px, self.mx)?);
        Ok(())
    }

    /// A horizontal rule between the margins.
    pub fn rule(&mut self, dash: (usize, usize), thick: usize, space: usize) {
        let mut b = Bitmap::blank(thick + 2 * space);
        draw::hline(&mut b, space, self.mx as usize, WIDTH_DOTS - 1 - self.mx as usize, thick, dash);
        self.parts.push(b);
    }

    /// A rule with a diamond in the middle.
    pub fn flourish(&mut self) {
        let mut b = Bitmap::blank(22);
        let (mx, mid) = (self.mx as usize + 10, WIDTH_DOTS as i32 / 2);
        draw::hline(&mut b, 10, mx, mid as usize - 14, 2, (0, 0));
        draw::hline(&mut b, 10, mid as usize + 14, WIDTH_DOTS - mx, 2, (0, 0));
        for k in 0..9i32 {
            let w = 9 - k;
            for dx in -w..=w {
                draw::put(&mut b, mid + dx, 10 + k - 4, true);
                draw::put(&mut b, mid + dx, 10 - k + 4, true);
            }
        }
        self.parts.push(b);
    }

    /// A barcode of `data`, `rows` tall.
    pub fn barcode(&mut self, data: &str, rows: usize) -> R {
        let code = super::code(data, CodeOpts { kind: CodeKind::Code128, caption: false, ..CodeOpts::default() })?;
        self.parts.push(draw::slice(&code, 12, (12 + rows).min(code.height)));
        Ok(())
    }

    pub fn done(self) -> Bitmap {
        super::stack(&self.parts, 0)
    }
}

/// One line of text at the largest size (up to `max_px`) that fits between the margins.
pub fn fit_line(s: &str, font: FontChoice, mx: f32, max_px: f32) -> Result<Bitmap, String> {
    let room = WIDTH_DOTS as f32 - 2.0 * mx;
    let mut lo = 12.0f32;
    let mut hi = max_px;
    let render = |px: f32| -> Result<Bitmap, String> {
        text::text(s, TextOpts { px, font, align: Align::Center, margin_x: mx, margin_y: 0.0, break_anywhere: true, ..TextOpts::default() })
    };
    // The widest single line wins: shrink until the ink fits on one line.
    for _ in 0..7 {
        let mid = (lo + hi) / 2.0;
        let b = render(mid)?;
        let one_line = b.height as f32 <= mid * 1.7;
        let wide = text::ink_span(&b).map(|(a, z)| (z - a + 1) as f32).unwrap_or(0.0);
        if one_line && wide <= room {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    render(lo)
}

/// `left` and `right` on one line joined by dots.
pub fn leader_line(left: &str, right: &str, font: FontChoice, px: f32, mx: f32) -> Result<Bitmap, String> {
    let one = |s: &str, align: Align| -> Result<Bitmap, String> {
        if s.trim().is_empty() {
            return Ok(Bitmap::blank(1));
        }
        text::text(s, TextOpts { px, font, align, margin_x: mx, margin_y: 0.0, break_anywhere: true, ..TextOpts::default() })
    };
    let right_b = one(right, Align::Right)?;
    let right_from = text::ink_span(&right_b).map(|(a, _)| a).unwrap_or(WIDTH_DOTS - mx as usize);
    // A long name is cut short so that it never runs into the price.
    let mut name: String = left.trim().to_string();
    let mut left_b = one(&name, Align::Left)?;
    while left_b.height as f32 > px * 1.7 || text::ink_span(&left_b).is_some_and(|(_, z)| z + 18 > right_from) {
        if name.chars().count() <= 3 {
            break;
        }
        name.pop();
        left_b = one(&name, Align::Left)?;
    }
    let height = left_b.height.max(right_b.height).max(1);
    let mut out = Bitmap::blank(height);
    draw::or_at(&mut out, &left_b, 0, 0);
    draw::or_at(&mut out, &right_b, 0, 0);
    let left_to = text::ink_span(&left_b).map(|(_, z)| z).unwrap_or(mx as usize);
    let y = height.saturating_sub((px * 0.22) as usize + 1);
    let mut x = left_to + 8;
    while x + 6 < right_from {
        out.set(x, y);
        out.set(x + 1, y);
        x += 6;
    }
    Ok(out)
}

/// Today's date as `DD/MM/YYYY`, from the system clock (UTC; a ticket does not need more).
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = civil(secs as i64 / 86_400);
    format!("{d:02}/{m:02}/{y}")
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The price at the end of a line: `Coffee 2.50` is ("Coffee", Some(250 cents)).
pub fn split_price(line: &str) -> (String, Option<i64>) {
    let line = line.trim().trim_end_matches(['€', '$', '£']).trim_end();
    let Some((head, last)) = line.rsplit_once(char::is_whitespace) else { return (line.to_string(), None) };
    let cleaned = last.trim_start_matches(['€', '$', '£']).trim_end_matches(['€', '$', '£']).replace(',', ".");
    match cleaned.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 && v < 1e7 => (head.trim().to_string(), Some((v * 100.0).round() as i64)),
        _ => (line.to_string(), None),
    }
}

pub fn money(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(b: &Bitmap) -> u32 {
        b.data.iter().map(|x| x.count_ones()).sum()
    }

    fn draw_theme(theme: Theme, body: &str) -> Bitmap {
        render(theme, body, &Fields::new(), TextOpts::default()).unwrap_or_else(|e| panic!("{}: {e}", theme.id()))
    }

    #[test]
    fn every_theme_has_a_name_an_id_a_family_and_a_sample_that_draws() {
        let mut ids = std::collections::HashSet::new();
        for theme in Theme::ALL {
            assert!(ids.insert(theme.id()), "ids are unique: {}", theme.id());
            assert!(!theme.name().is_empty() && !theme.tagline().is_empty());
            assert_eq!(Theme::from_id(theme.id()), Some(theme));
            assert_eq!(theme.family().is_none(), theme == Theme::Plain);
            assert!(theme.fields().len() <= MAX_FIELDS, "{} has too many fields", theme.id());
            let b = draw_theme(theme, theme.sample());
            assert!(b.height > 40 && ink(&b) > 200, "{} drew almost nothing ({} rows)", theme.id(), b.height);
            assert!(b.height < 3000, "{} is {} rows, too long for its sample", theme.id(), b.height);
        }
        assert_eq!(Theme::from_id("Name Tag"), Some(Theme::NameTag));
        assert_eq!(Theme::from_id("dotmatrix"), Some(Theme::DotMatrix));
        assert_eq!(Theme::from_id("nonsense"), None);
    }

    #[test]
    fn the_client_list_of_themes_matches_the_real_ones() {
        let listed: Vec<&str> = crate::jobspec::THEMES.iter().map(|t| t.0).collect();
        assert_eq!(listed, Theme::ALL.map(Theme::id));
        for (theme, (_, keys, _)) in Theme::ALL.iter().zip(crate::jobspec::THEMES) {
            let real: Vec<&str> = theme.fields().iter().map(|f| f.key).collect();
            assert_eq!(real.join(","), *keys, "{}", theme.id());
        }
    }

    #[test]
    fn the_same_text_gives_the_same_ticket() {
        for theme in [Theme::Parchment, Theme::Ransom, Theme::Receipt, Theme::Fridge] {
            assert_eq!(draw_theme(theme, theme.sample()), draw_theme(theme, theme.sample()), "{} is not repeatable", theme.id());
        }
    }

    #[test]
    fn an_empty_text_is_refused_unless_the_theme_can_make_something() {
        for theme in Theme::ALL {
            let r = render(theme, "  ", &Fields::new(), TextOpts::default());
            assert_eq!(r.is_ok(), theme.body_optional(), "{}", theme.id());
        }
    }

    #[test]
    fn fields_change_the_ticket_and_empty_fields_use_the_hint() {
        let mut named = Fields::new();
        named.insert("store".into(), "THE BIG SHOP".into());
        let a = render(Theme::Receipt, "Tea 1.00", &named, TextOpts::default()).unwrap();
        let b = draw_theme(Theme::Receipt, "Tea 1.00");
        assert_ne!(a, b);
        let blank = Fields::from([("store".to_string(), "   ".to_string())]);
        assert_eq!(render(Theme::Receipt, "Tea 1.00", &blank, TextOpts::default()).unwrap(), b, "blank means default");
    }

    #[test]
    #[ignore]
    fn print_meta_table() {
        for t in Theme::ALL {
            let keys: Vec<&str> = t.fields().iter().map(|f| f.key).collect();
            println!("    (\"{}\", \"{}\", \"{}\"),", t.id(), keys.join(","), t.tagline().replace('"', "'"));
        }
    }

    /// `THEME_SHEETS=dir cargo test sheets -- --ignored` writes one PNG per theme, to look at.
    #[test]
    #[ignore]
    fn sheets() {
        let Ok(dir) = std::env::var("THEME_SHEETS") else { return };
        std::fs::create_dir_all(&dir).unwrap();
        for theme in Theme::ALL {
            let b = draw_theme(theme, theme.sample());
            let img = image::GrayImage::from_fn(WIDTH_DOTS as u32, b.height as u32, |x, y| image::Luma([if b.get(x as usize, y as usize) { 20 } else { 244 }]));
            img.save(format!("{dir}/{}.png", theme.id())).unwrap();
        }
    }

    #[test]
    fn plain_is_just_the_text() {
        let base = TextOpts::default();
        assert_eq!(render(Theme::Plain, "Hi", &Fields::new(), base).unwrap(), text::text("Hi", base).unwrap());
    }

    #[test]
    fn prices_are_found_at_the_end_of_a_line() {
        assert_eq!(split_price("Coffee 2.50"), ("Coffee".into(), Some(250)));
        assert_eq!(split_price("2 x Juice 6,40 €"), ("2 x Juice".into(), Some(640)));
        assert_eq!(split_price("Just a note"), ("Just a note".into(), None));
        assert_eq!(split_price("Soup"), ("Soup".into(), None));
        assert_eq!(money(1480), "14.80");
        assert_eq!(money(5), "0.05");
    }

    #[test]
    fn the_calendar_maths_is_right() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_723), (2024, 1, 1));
        assert_eq!(civil(20_508), (2026, 2, 24));
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn leader_lines_keep_the_price_clear_of_a_long_name() {
        let b = leader_line(&"Extremely long product name ".repeat(4), "12.99", FontChoice::Mono, 24.0, 10.0).unwrap();
        let (_, right) = text::ink_span(&b).unwrap();
        assert!(right < WIDTH_DOTS - 4);
        assert!(b.height < 50, "stays on one line");
    }

    #[test]
    fn shouted_lines_fill_the_paper() {
        let b = fit_line("HELLO", FontChoice::Impact, 8.0, 200.0).unwrap();
        let (a, z) = text::ink_span(&b).unwrap();
        assert!(z - a > 300, "wide: {}", z - a);
        assert!(z - a < WIDTH_DOTS - 12);
    }
}

/// The most fields any theme asks for (the app keeps this many text boxes).
pub const MAX_FIELDS: usize = 4;
