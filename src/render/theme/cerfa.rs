//! A fake CERFA: the French administrative form, folded down to 48 mm of paper.
//!
//! The joke is the paperwork itself: a heavy black band with a form number, a title, a reserved
//! box, strips of "NE RIEN ÉCRIRE DANS CETTE ZONE", numbered sections with rules, little square
//! boxes that are never ticked, dotted lines to write on and a footer in tiny print. Dense,
//! plausible, boring. The text is the motive of the request; the fields are the blanks of the
//! applicant. Everything is in French on purpose: an English pastiche of a CERFA would kill it.

use super::{Ctx, Page, fit_line, today};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw;
use crate::render::text::{self, Align, FontChoice, TextOpts};

type R = Result<Bitmap, String>;
type U = Result<(), String>;

const MX: f32 = 16.0;

/// The number in the black band, and the one in the footer: the same joke, twice, like a real form.
const NUMBER: &str = "CERFA N° 2042-BÊTE";
const FOOTER: &str = "Cerfa n° 2042-BÊTE — Sous-direction des papiers égarés";

fn opts(px: f32, font: FontChoice, align: Align, mx: f32) -> TextOpts {
    TextOpts { px, font, align, margin_x: mx, margin_y: 1.0, ..TextOpts::default() }
}

/// A date field left at "today" means today's date; a form that is left empty stays blank.
fn dated(s: String) -> String {
    if s.eq_ignore_ascii_case("today") { today() } else { s }
}

/// A ruled section heading: bold caps over a rule, as printed forms love them.
fn section(p: &mut Page, title: &str) -> U {
    p.gap(7);
    p.text(title, FontChoice::Bold, 17.0, Align::Left)?;
    p.rule((0, 0), 2, 2);
    p.gap(3);
    Ok(())
}

/// A dotted line to write on, `px` tall.
fn writing_line(p: &mut Page, px: f32) {
    let mut b = Bitmap::blank(px as usize + 7);
    let y = b.height - 3;
    draw::hline(&mut b, y, p.mx as usize, WIDTH_DOTS - 1 - p.mx as usize, 1, (1, 3));
    p.push(b);
}

/// A blank on the form: a small label, the value typed in the field `key` (never a hint), a
/// dotted line under it. An empty field is the same line, blank, so the form stays a form.
fn blank(p: &mut Page, c: &Ctx, key: &str, label: &str, date: bool) -> U {
    p.text(&label.to_uppercase(), FontChoice::Sans, 13.0, Align::Left)?;
    let value = if c.editing == key { String::new() } else if date { dated(c.given(key)) } else { c.given(key) };
    p.begin(key);
    // An empty line is as tall as a written one, so writing in it does not move the form.
    let row = if value.trim().is_empty() {
        Bitmap::blank(text::text("Ag", opts(22.0, FontChoice::Typewriter, Align::Left, p.mx))?.height)
    } else {
        text::text(value.trim(), opts(22.0, FontChoice::Typewriter, Align::Left, p.mx))?
    };
    let h = row.height;
    let mut b = Bitmap::blank(h + 7);
    draw::or_at(&mut b, &row, 0, 0);
    draw::hline(&mut b, h + 4, p.mx as usize, WIDTH_DOTS - 1 - p.mx as usize, 1, (1, 3));
    p.push(b);
    p.end();
    p.gap(3);
    Ok(())
}

/// A square box and its label, never ticked: the point of the form is that you decide.
fn check_row(label: &str, px: f32, mx: f32) -> R {
    let side = (px * 0.7) as usize;
    let x0 = mx as usize;
    let mut b = text::text(label, opts(px, FontChoice::Sans, Align::Left, mx + side as f32 + 10.0))?;
    draw::rect(&mut b, x0, 2, x0 + side, 2 + side, 2);
    Ok(b)
}

fn checks(p: &mut Page, labels: &[&str], px: f32) -> U {
    for label in labels {
        p.push(check_row(label, px, p.mx)?);
    }
    Ok(())
}

/// The box the administration writes nothing in, with a number that means nothing.
fn reserved(c: &Ctx) -> R {
    let mut b = Bitmap::blank(70);
    let (x0, x1) = (MX as usize, WIDTH_DOTS - 1 - MX as usize);
    draw::rect(&mut b, x0, 0, x1, 69, 2);
    let label = text::text("CADRE RÉSERVÉ À L'ADMINISTRATION", opts(13.0, FontChoice::Sans, Align::Left, MX + 8.0))?;
    draw::or_at(&mut b, &label, 0, 6);
    let number = text::text(&format!("N° {}", c.number(31, 7)), opts(15.0, FontChoice::Mono, Align::Left, MX + 8.0))?;
    draw::or_at(&mut b, &number, 0, 24);
    draw::hline(&mut b, 49, x0 + 8, x1 - 8, 1, (1, 3));
    draw::hline(&mut b, 61, x0 + 8, x1 - 8, 1, (1, 3));
    Ok(b)
}

/// The motive, on ruled lines; a long one carries on to a second page of the same strip.
fn motive(p: &mut Page, c: &Ctx) -> U {
    const PAGE_ONE: usize = 8;
    let lines = c.lines();
    p.begin("body");
    if c.editing == "body" {
        for _ in 0..3.max(lines.len() + 1) {
            writing_line(p, 27.0);
        }
    } else if lines.is_empty() {
        for _ in 0..3 {
            writing_line(p, 27.0);
        }
    } else {
        for (i, line) in lines.iter().enumerate() {
            // A declaration that does not fit in the frame gets a continuation, like the real thing.
            if i == PAGE_ONE && lines.len() > PAGE_ONE + 1 {
                p.gap(6);
                p.band("SUITE DU CADRE 3 — PAGE 2/2", FontChoice::Bold, 14.0, Align::Center)?;
                p.gap(4);
            }
            let row = text::text(line, opts(21.0, FontChoice::Typewriter, Align::Left, p.mx))?;
            let h = row.height;
            let mut b = Bitmap::blank(h + 7);
            draw::or_at(&mut b, &row, 0, 0);
            draw::hline(&mut b, h + 4, p.mx as usize, WIDTH_DOTS - 1 - p.mx as usize, 1, (1, 3));
            p.push(b);
        }
    }
    p.end();
    Ok(())
}

pub fn cerfa(c: &Ctx) -> R {
    let mut p = Page::new(MX);
    p.band(NUMBER, FontChoice::Bold, 26.0, Align::Center)?;
    p.text("MINISTÈRE DES PAPIERS ÉGARÉS", FontChoice::Sans, 13.0, Align::Center)?;
    p.rule((0, 0), 3, 4);
    p.push(fit_line("DEMANDE D'AGRÉMENT", FontChoice::Bold, MX, 38.0)?);
    p.push(fit_line("POUR UNE CRÉATURE DE COMPAGNIE", FontChoice::Bold, MX, 26.0)?);
    p.gap(6);
    p.leader("RÉFÉRENCE", &format!("2042-BÊTE/{}", c.number(32, 4)), FontChoice::Mono, 15.0)?;
    p.gap(3);
    p.band("NE RIEN ÉCRIRE DANS CETTE ZONE", FontChoice::Sans, 15.0, Align::Center)?;
    p.gap(5);
    p.push(reserved(c)?);
    p.gap(3);

    section(&mut p, "1. IDENTITÉ DU DÉCLARANT")?;
    blank(&mut p, c, "name", "Nom et prénom", false)?;
    blank(&mut p, c, "place", "Commune de résidence", false)?;

    section(&mut p, "2. CRÉATURE CONCERNÉE")?;
    blank(&mut p, c, "creature", "Nom usuel de la créature", false)?;
    checks(&mut p, &["Mammifère", "Reptile", "Objet domestique", "Non identifiée"], 20.0)?;

    section(&mut p, "3. MOTIF DE LA DEMANDE")?;
    motive(&mut p, c)?;

    section(&mut p, "4. ENGAGEMENTS DU DÉCLARANT")?;
    checks(&mut p, &["Je m'engage à la nourrir", "Je ne la revendrai pas", "Je préviens le voisin"], 20.0)?;

    section(&mut p, "5. DATE ET SIGNATURE")?;
    blank(&mut p, c, "date", "Date de la demande", true)?;
    p.gap(2);
    writing_line(&mut p, 30.0);
    p.text("Signature du déclarant", FontChoice::Sans, 13.0, Align::Right)?;

    p.gap(8);
    p.band("NE RIEN ÉCRIRE DANS CETTE ZONE", FontChoice::Sans, 15.0, Align::Center)?;
    p.gap(4);
    p.text(FOOTER, FontChoice::Mono, 14.0, Align::Center)?;
    p.text("Formulaire à conserver trente ans, même si la créature part avant.", FontChoice::Mono, 14.0, Align::Center)?;
    p.gap(6);
    p.barcode(&format!("2042{}", c.number(33, 8)), 34)?;
    p.gap(6);

    let mut b = p.done();
    {
        let bottom = b.height - 1;
        draw::rect(&mut b, 0, 0, WIDTH_DOTS - 1, bottom, 3);
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::TextOpts;
    use crate::render::theme::{Fields, Theme, render, render_mapped};

    fn print(body: &str, fields: &[(&str, &str)]) -> Bitmap {
        let f: Fields = fields.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        render(Theme::Cerfa, body, &f, TextOpts::default()).unwrap_or_else(|e| panic!("cerfa: {e}"))
    }

    #[test]
    fn the_sample_draws_a_dense_repeatable_form() {
        let b = print(Theme::Cerfa.sample(), &[]);
        assert!(b.height > 600, "the form should be dense: {} rows", b.height);
        assert!(b.height < 3000, "the sample stays one ticket: {} rows", b.height);
        assert_eq!(b, print(Theme::Cerfa.sample(), &[]), "the same text gives the same form");
    }

    #[test]
    fn every_declared_field_and_the_body_have_a_place_on_the_paper() {
        let (b, regions) = render_mapped(Theme::Cerfa, Theme::Cerfa.sample(), &Fields::new(), TextOpts::default()).unwrap();
        let keys: Vec<&str> = regions.iter().map(|r| r.key.as_str()).collect();
        for field in Theme::Cerfa.fields() {
            assert!(keys.contains(&field.key), "{} has no region on the paper: {keys:?}", field.key);
        }
        assert!(keys.contains(&"body"), "the motive is writable: {keys:?}");
        assert!(regions.iter().all(|r| r.x0 < r.x1 && r.x1 <= WIDTH_DOTS && r.y0 < r.y1 && r.y1 <= b.height));
    }

    #[test]
    fn empty_fields_leave_lines_to_write_on_not_the_hints() {
        let blank = print("", &[]);
        let spaced = print("", &[("name", "   "), ("creature", "  "), ("place", " "), ("date", "  ")]);
        assert_eq!(blank, spaced, "a blank field is blank, not its hint");
        assert_eq!(blank, print("", &[]), "a blank form is repeatable");
        // Nothing is ticked and no example name is inked: the sheet is for writing on.
        assert_ne!(print("", &[("name", "Alex")]), blank);
        // A typed date saying "today" means today's date, as on the other forms.
        let dated = print("", &[("date", "today")]);
        assert_ne!(dated, blank);
        assert!(Theme::Cerfa.fields().iter().all(|f| f.key != "boxes"), "no tick box is a field of this form");
    }

    #[test]
    fn a_long_motive_runs_over_onto_a_second_page() {
        let short = print("Une phrase.", &[]);
        let long = print(&"La créature refuse encore de rendre la télécommande.\n".repeat(12), &[]);
        assert!(long.height > short.height + 200, "the motive flows down the paper: {} vs {}", long.height, short.height);
        assert!(long.height > 900, "the continuation is a real second page: {} rows", long.height);
    }

    #[test]
    fn the_sample_is_a_dry_french_form() {
        let s = Theme::Cerfa.sample();
        assert!(s.contains("créature") && s.contains("canapé"), "French, and about the form's subject: {s}");
        assert!(!s.contains('!'), "no exclamation marks: the form is boring on purpose");
        assert_eq!(NUMBER, "CERFA N° 2042-BÊTE");
        assert_eq!(FOOTER, "Cerfa n° 2042-BÊTE — Sous-direction des papiers égarés");
    }

    /// `THEME_SHEETS=dir cargo test cerfa_sheets -- --ignored` writes the continuation, to look at.
    #[test]
    #[ignore]
    fn cerfa_sheets() {
        let Ok(dir) = std::env::var("THEME_SHEETS") else { return };
        std::fs::create_dir_all(&dir).unwrap();
        let long = "La créature refuse encore de rendre la télécommande.\n".repeat(12);
        let b = print(&long, &[]);
        let img = image::GrayImage::from_fn(WIDTH_DOTS as u32, b.height as u32, |x, y| image::Luma([if b.get(x as usize, y as usize) { 20 } else { 244 }]));
        img.save(format!("{dir}/cerfa-page2.png")).unwrap();
    }
}
