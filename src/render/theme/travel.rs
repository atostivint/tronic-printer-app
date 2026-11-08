//! Travel tickets: a boarding pass lying sideways like the real thing, the old SNCF-style card
//! ticket, and the new e-ticket with its QR code.

use super::{Ctx, Page, Theme, today};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw::{self, Anchor, Sheet};
use crate::render::text::{Align, FontChoice, TextOpts};
use crate::render::{self, CodeKind, CodeOpts};

type R = Result<Bitmap, String>;

fn date_of(c: &Ctx, t: Theme) -> String {
    let d = c.field(t, "date");
    if d == "today" { today() } else { d }
}

fn clock(minutes: usize) -> String {
    format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}

/// A departure and an arrival time, made from the text.
fn times(c: &Ctx, salt: u64) -> (String, String) {
    let mut rng = c.rng(salt);
    let dep = 5 * 60 + 5 * rng.below(170);
    (clock(dep), clock(dep + 70 + 5 * rng.below(60)))
}

fn shorten(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn label(s: &mut Sheet, text: &str, x: i32, y: i32) {
    s.text(text, FontChoice::Sans, 15.0, x, y, Anchor::Left);
}

/// The boarding pass, drawn 780 dots long and 384 across, then turned to print down the paper.
pub fn flight(c: &Ctx) -> R {
    let t = Theme::Flight;
    let (w, h, cut) = (780usize, 384usize, 560usize);
    let mut s = Sheet::new(w, h);
    let (from, to) = (c.field(t, "from").to_uppercase(), c.field(t, "to").to_uppercase());
    let name = c.field(t, "name").to_uppercase();
    let mut rng = c.rng(21);
    let flight = format!("TR {}", c.number(21, 4));
    let gate = format!("{}{}", (b'A' + rng.below(6) as u8) as char, 1 + rng.below(30));
    let boarding = clock(5 * 60 + 5 * rng.below(170));
    let seat = c.field(t, "seat").to_uppercase();
    let date = today();

    draw::rect(&mut s, 0, 0, w - 1, h - 1, 3);
    // The header bands.
    draw::fill_rect(&mut s, 0, 0, cut - 1, 62);
    s.knockout("AIR TRONIC", FontChoice::Bold, 38.0, 24, 14, Anchor::Left);
    s.knockout("BOARDING PASS", FontChoice::Sans, 24.0, cut as i32 - 24, 20, Anchor::Right);
    draw::fill_rect(&mut s, cut + 1, 0, w - 1, 62);
    s.knockout("BOARDING", FontChoice::Bold, 26.0, (cut + (w - cut) / 2) as i32, 18, Anchor::Centre);
    // The route.
    if let Some((x0, y0, x1, y1)) = s.text(&from, FontChoice::Impact, 84.0, 24, 80, Anchor::Left) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "from".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    if let Some((x0, y0, x1, y1)) = s.text(&to, FontChoice::Impact, 84.0, cut as i32 - 24, 80, Anchor::Right) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "to".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    s.text("\u{2708}", FontChoice::Symbols, 60.0, cut as i32 / 2, 92, Anchor::Centre);
    // The details, two rows of label and value.
    let name_px = if name.chars().count() > 14 { 22.0 } else { 30.0 };
    label(&mut s, "PASSENGER", 24, 186);
    if let Some((x0, y0, x1, y1)) = s.text(&name, FontChoice::Bold, name_px, 24, 206, Anchor::Left) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "name".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    label(&mut s, "FLIGHT", 240, 186);
    s.text(&flight, FontChoice::Bold, 28.0, 240, 206, Anchor::Left);
    label(&mut s, "DATE", 380, 186);
    s.text(&date, FontChoice::Bold, 28.0, 380, 206, Anchor::Left);
    for (x, title, value) in [(24, "GATE", gate.as_str()), (130, "BOARDING", boarding.as_str()), (260, "SEAT", seat.as_str()), (380, "CLASS", "ECONOMY")] {
        label(&mut s, title, x, 258);
        if let Some((x0, y0, x1, y1)) = s.text(value, FontChoice::Bold, 28.0, x, 278, Anchor::Left) {
            if title == "SEAT" {
                let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
                super::mark(super::Region { key: "seat".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
            }
        }
    }
    draw::hline(&mut s, 330, 24, cut - 24, 1, (3, 4));
    s.text(&shorten(c.body.trim(), 24), FontChoice::Sans, 20.0, 24, 346, Anchor::Left);
    if let Ok(modules) = render::code128_modules(&format!("{flight}{}", c.number(22, 8))) {
        let width = modules.len();
        s.bars(&modules, 1, cut - 24 - width, 340, 30);
    }
    // The stub.
    let sx = cut as i32 + 20;
    label(&mut s, "PASSENGER", sx, 76);
    if let Some((x0, y0, x1, y1)) = s.text(&shorten(&name, 15), FontChoice::Bold, 22.0, sx, 96, Anchor::Left) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "name".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    s.text(&format!("{from}  >  {to}"), FontChoice::Bold, 28.0, sx, 138, Anchor::Left);
    label(&mut s, "FLIGHT", sx, 190);
    s.text(&flight, FontChoice::Bold, 26.0, sx, 210, Anchor::Left);
    label(&mut s, "SEAT", sx, 252);
    if let Some((x0, y0, x1, y1)) = s.text(&seat, FontChoice::Impact, 60.0, sx, 270, Anchor::Left) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "seat".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    if let Ok(modules) = render::code128_modules(&format!("{flight}{seat}")) {
        s.bars(&modules, 1, sx as usize, 342, 28);
    }
    // The tear line, with a notch at each end.
    draw::vline(&mut s, cut, 0, h - 1, 2, (6, 6));
    draw::punch(&mut s, cut as i32, 0, 9);
    draw::punch(&mut s, cut as i32, h as i32 - 1, 9);
    draw::round_corners(&mut s, 0, 0, w - 1, h - 1, 14);
    Ok(s.turned())
}

/// The classic card ticket: black header, boxed details, a magnetic strip along the bottom.
pub fn train_old(c: &Ctx) -> R {
    let t = Theme::TrainOld;
    let (w, h) = (700usize, 384usize);
    let mut s = Sheet::new(w, h);
    let (from, to) = (c.field(t, "from").to_uppercase(), c.field(t, "to").to_uppercase());
    let name = c.body.trim().to_uppercase();
    let mut rng = c.rng(23);
    let (dep, arr) = times(c, 23);
    // The generated values keep their order, so a ticket with nothing typed stays what it was.
    let price = c.field_or(t, "price", format!("{},{:02} EUR", 19 + rng.below(110), rng.below(100)));
    let train = c.field_or(t, "train", (6000 + rng.below(3999)).to_string());
    let operator = c.field_or(t, "operator", "SNCF").to_uppercase();
    let class = c.field_or(t, "class", "2");

    draw::rect(&mut s, 0, 0, w - 1, h - 1, 3);
    draw::fill_rect(&mut s, 0, 0, w - 1, 64);
    if let Some((x0, y0, x1, y1)) = s.knockout(&shorten(&operator, 12), FontChoice::Bold, 46.0, 24, 10, Anchor::Left) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "operator".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    s.knockout("BILLET  ALLER", FontChoice::Sans, 24.0, w as i32 - 24, 22, Anchor::Right);
    let city_px = |s: &str| if s.chars().count() <= 12 { 54.0 } else if s.chars().count() <= 18 { 40.0 } else { 31.0 };
    label(&mut s, "DE", 24, 84);
    if let Some((x0, y0, x1, y1)) = s.text(&from, FontChoice::Impact, city_px(&from), 24, 104, Anchor::Left) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "from".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    label(&mut s, "A", 24, 178);
    if let Some((x0, y0, x1, y1)) = s.text(&to, FontChoice::Impact, city_px(&to), 24, 198, Anchor::Left) {
        let (fx0, fy0, fx1, fy1) = s.turned_box(x0, y0, x1, y1);
        super::mark(super::Region { key: "to".into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
    }
    label(&mut s, "VOYAGEUR", 24, 268);
    s.text(&shorten(&name, 22), FontChoice::Bold, 28.0, 24, 288, Anchor::Left);
    // The boxed details on the right. Each one is a box the user can write in.
    let cells = [
        ("TRAIN", train.clone(), Some("train")),
        ("DATE", date_of(c, t), Some("date")),
        ("DEPART-ARRIVEE", format!("{dep}-{arr}"), None),
        ("VOITURE / PLACE", c.field(t, "seat"), Some("seat")),
        ("PRIX", price, Some("price")),
        ("CLASSE", class, Some("class")),
    ];
    for (i, (title, value, key)) in cells.iter().enumerate() {
        let (x, y) = (410 + (i % 2) * 140, 84 + (i / 2) * 60);
        draw::rect(&mut s, x, y, x + 134, y + 54, 1);
        label(&mut s, title, x as i32 + 6, y as i32 + 5);
        s.text(&shorten(value, 12), FontChoice::Bold, 21.0, x as i32 + 6, y as i32 + 24, Anchor::Left);
        // The whole value line is the place to write on, not just the ink: "2" alone would be a
        // tiny target.
        if let Some(k) = *key {
            let (fx0, fy0, fx1, fy1) = s.turned_box(x + 4, y + 20, x + 130, y + 52);
            super::mark(super::Region { key: k.into(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
        }
    }
    if let Ok(modules) = render::code128_modules(&format!("{train}{}", c.number(24, 9))) {
        s.bars(&modules, 2, 410, 276, 40);
    }
    // The magnetic strip.
    draw::fill_rect(&mut s, 0, 326, w - 1, 356);
    s.text("BILLET NON ECHANGEABLE APRES DEPART", FontChoice::Sans, 15.0, 24, 364, Anchor::Left);
    draw::round_corners(&mut s, 0, 0, w - 1, h - 1, 12);
    Ok(s.turned())
}

/// The e-ticket: times, stations, the details and a QR code to show on board.
pub fn train(c: &Ctx) -> R {
    let t = Theme::Train;
    let (from, to) = (c.field(t, "from").to_uppercase(), c.field(t, "to").to_uppercase());
    let name = c.body.trim().to_uppercase();
    let (dep, arr) = times(c, 25);
    let mut rng = c.rng(25);
    // Same order of draws as before the fields existed: an empty field draws today's ticket.
    let price = c.field_or(t, "price", format!("{},{:02} EUR", 19 + rng.below(110), rng.below(100)));
    let reference: String = (0..6).map(|_| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"[rng.below(32)] as char).collect();
    let train = c.field_or(t, "train", (6000 + rng.below(3999)).to_string());
    let operator = c.field_or(t, "operator", "TGV");
    let class = c.field_or(t, "class", "2de");
    let seat = c.field(t, "seat");

    let mut p = Page::new(18.0);
    p.band("E-BILLET", FontChoice::Bold, 34.0, Align::Center)?;
    p.gap(8);
    p.text(&format!("Aller  -  {}", date_of(c, t)), FontChoice::Sans, 22.0, Align::Center)?;
    p.gap(6);
    let opts = |align| TextOpts { px: 56.0, font: FontChoice::Impact, align, margin_x: 18.0, margin_y: 0.0, ..TextOpts::default() };
    let left = render::text(&dep, opts(Align::Left))?;
    let right = render::text(&arr, opts(Align::Right))?;
    let arrow = render::text("\u{2192}", TextOpts { px: 44.0, font: FontChoice::Symbols, align: Align::Center, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() })?;
    let mut row = Bitmap::blank(left.height.max(right.height));
    draw::or_at(&mut row, &left, 0, 0);
    draw::or_at(&mut row, &right, 0, 0);
    let lift = row.height.saturating_sub(arrow.height) as i32 / 2;
    draw::or_at(&mut row, &arrow, 0, lift);
    p.push(row);
    p.text(&from, FontChoice::Bold, 26.0, Align::Left)?;
    p.text(&to, FontChoice::Bold, 26.0, Align::Right)?;
    p.gap(4);
    p.rule((0, 0), 2, 6);
    p.begin("operator");
    p.leader("Operateur", &shorten(&operator, 18), FontChoice::Sans, 24.0)?;
    p.end();
    p.begin("train");
    p.leader("Train", &shorten(&train, 12), FontChoice::Sans, 24.0)?;
    p.end();
    p.begin("seat");
    p.leader("Voiture / Place", &seat, FontChoice::Sans, 24.0)?;
    p.end();
    p.begin("class");
    p.leader("Classe", &shorten(&class, 8), FontChoice::Sans, 24.0)?;
    p.end();
    p.leader("Passager", &shorten(&name, 18), FontChoice::Sans, 24.0)?;
    p.begin("price");
    p.leader("Prix", &shorten(&price, 14), FontChoice::Bold, 26.0)?;
    p.end();
    p.rule((0, 0), 2, 6);
    // The tear-off line above the part to show on board.
    let tear_y = p.height();
    p.gap(10);
    let data = format!("TRONIC-TRAIN|{reference}|{from}>{to}|{dep}|{seat}|{name}");
    p.push(render::code(&data, CodeOpts { kind: CodeKind::Qr, caption: false, ..CodeOpts::default() })?);
    p.text(&format!("Reference  {reference}"), FontChoice::Typewriter, 28.0, Align::Center)?;
    p.gap(4);
    p.text("A presenter a bord, sur telephone ou imprime.", FontChoice::Sans, 18.0, Align::Center)?;
    p.gap(14);
    let mut b = p.done();
    let bottom = b.height - 1;
    draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 3);
    super::effects::perforation(&mut b, tear_y, 6, 5);
    super::effects::dog_ear(&mut b, 34);
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The strings a theme actually draws, so a test can say a value reached the paper.
    fn drawn(theme: Theme, body: &str, fields: &super::super::Fields) -> (Bitmap, Vec<String>) {
        crate::render::text::record_drawn(true);
        let b = super::super::render(theme, body, fields, TextOpts::default()).unwrap();
        let strings = crate::render::text::take_drawn();
        crate::render::text::record_drawn(false);
        (b, strings)
    }

    #[test]
    fn the_e_ticket_has_a_tear_line_and_a_folded_corner_and_stays_repeatable() {
        let f = super::super::Fields::new();
        let a = train(&Ctx::new("Alex Dupont", &f)).unwrap();
        assert_eq!(a, train(&Ctx::new("Alex Dupont", &f)).unwrap());
        // The folded corner puts dots on the bottom-right of the ticket.
        let (x, y) = (WIDTH_DOTS - 4, a.height - 4);
        assert!(a.get(x, y) || a.get(x - 1, y - 1) || a.get(x - 2, y), "the dog-ear is drawn");
    }

    #[test]
    fn a_blank_train_field_keeps_the_generated_ticket() {
        let blank: super::super::Fields =
            [("operator", "   "), ("train", ""), ("price", " "), ("class", "")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        for theme in [Theme::TrainOld, Theme::Train] {
            let idle = super::super::render(theme, "Alex Dupont", &super::super::Fields::new(), TextOpts::default()).unwrap();
            let blanked = super::super::render(theme, "Alex Dupont", &blank, TextOpts::default()).unwrap();
            assert_eq!(idle, blanked, "{}: a blank field changes the generated ticket", theme.id());
        }
    }

    #[test]
    fn the_train_brand_number_price_and_class_are_drawn_as_typed() {
        let pick = [("operator", "TER"), ("train", "8251"), ("price", "9,90 EUR"), ("class", "1re")];
        for theme in [Theme::TrainOld, Theme::Train] {
            let (idle, _) = drawn(theme, "Alex Dupont", &super::super::Fields::new());
            for (key, value) in pick {
                let mut one = super::super::Fields::new();
                one.insert(key.into(), value.into());
                let (mine, _) = drawn(theme, "Alex Dupont", &one);
                assert_ne!(idle, mine, "{} ignores {key}", theme.id());
                assert_eq!(idle.height, mine.height, "{} grows when {key} is typed", theme.id());
            }
        }
        // The card ticket draws its text as strings, so the typed words can be read back there.
        // (A Page keeps them: it can put a box on each part by itself, and takes them as it goes.)
        let mut typed = super::super::Fields::new();
        for (k, v) in pick {
            typed.insert(k.into(), v.into());
        }
        let (_, strings) = drawn(Theme::TrainOld, "Alex Dupont", &typed);
        for value in ["TER", "8251", "9,90 EUR", "1re"] {
            assert!(strings.iter().any(|s| s.contains(value)), "train-old never draws {value}: {strings:?}");
        }
    }

    #[test]
    fn every_train_field_has_its_own_place_on_the_paper() {
        for theme in [Theme::TrainOld, Theme::Train] {
            let (b, regions) = super::super::render_mapped(theme, "Alex Dupont", &super::super::Fields::new(), TextOpts::default()).unwrap();
            for key in ["from", "to", "date", "seat", "operator", "train", "price", "class"] {
                assert!(regions.iter().any(|r| r.key == key), "{}: {key} has no box on the paper", theme.id());
            }
            for r in &regions {
                assert!(r.x1 <= WIDTH_DOTS && r.y1 <= b.height, "{}: {} is outside the ticket", theme.id(), r.key);
            }
            for (i, a) in regions.iter().enumerate() {
                for b in &regions[i + 1..] {
                    let apart = a.x1 <= b.x0 || b.x1 <= a.x0 || a.y1 <= b.y0 || b.y1 <= a.y0;
                    assert!(apart, "{}: the boxes for {} and {} sit on top of each other", theme.id(), a.key, b.key);
                }
            }
        }
    }

    #[test]
    fn a_long_train_field_keeps_the_layout() {
        let mut long = super::super::Fields::new();
        for (k, v) in [("operator", "OPERATEUR FERROVIAIRE TRES LONG"), ("train", "123456789012345678"), ("price", "123456789012345 EUR"), ("class", "premiere classe")] {
            long.insert(k.into(), v.into());
        }
        for theme in [Theme::TrainOld, Theme::Train] {
            let (idle, _) = drawn(theme, "Alex Dupont", &super::super::Fields::new());
            let (mine, _) = drawn(theme, "Alex Dupont", &long);
            assert_eq!(idle.height, mine.height, "{} grows when a field is long", theme.id());
        }
    }
}
