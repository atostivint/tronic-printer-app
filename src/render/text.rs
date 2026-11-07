//! Text to bitmap, with the fonts Windows ships.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use fontdue::layout::{CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, WrapStyle};
use fontdue::{Font, FontSettings};

use crate::protocol::{Bitmap, WIDTH_DOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontChoice {
    Sans,
    Bold,
    Mono,
    Serif,
    Script,
    /// Fonts the themes use; each falls back to a plainer family when Windows lacks it.
    OldEnglish,
    Stencil,
    Impact,
    Playbill,
    Typewriter,
    Chancery,
    Papyrus,
    Ocr,
    Marker,
    /// Windows' symbol font: ✂ ✈ ★ ♈ …
    Symbols,
}

impl FontChoice {
    pub const ALL: [FontChoice; 5] = [FontChoice::Sans, FontChoice::Bold, FontChoice::Mono, FontChoice::Serif, FontChoice::Script];

    pub fn label(self) -> &'static str {
        match self {
            FontChoice::Sans => "Sans",
            FontChoice::Bold => "Bold",
            FontChoice::Mono => "Mono",
            FontChoice::Serif => "Serif",
            FontChoice::Script => "Script",
            FontChoice::OldEnglish => "Old English",
            FontChoice::Stencil => "Stencil",
            FontChoice::Impact => "Impact",
            FontChoice::Playbill => "Playbill",
            FontChoice::Typewriter => "Typewriter",
            FontChoice::Chancery => "Chancery",
            FontChoice::Papyrus => "Papyrus",
            FontChoice::Ocr => "OCR",
            FontChoice::Marker => "Marker",
            FontChoice::Symbols => "Symbols",
        }
    }

    fn files(self) -> &'static [&'static str] {
        match self {
            FontChoice::Sans => &["segoeui.ttf", "arial.ttf", "calibri.ttf", "tahoma.ttf"],
            FontChoice::Bold => &["segoeuib.ttf", "arialbd.ttf", "calibrib.ttf", "tahomabd.ttf"],
            FontChoice::Mono => &["consola.ttf", "cour.ttf", "lucon.ttf"],
            FontChoice::Serif => &["georgia.ttf", "times.ttf", "cambria.ttc"],
            FontChoice::Script => &["segoepr.ttf", "comic.ttf", "ink free.ttf", "inkfree.ttf"],
            FontChoice::OldEnglish => &["OLDENGL.TTF", "georgiab.ttf", "timesbd.ttf", "arialbd.ttf"],
            FontChoice::Stencil => &["STENCIL.TTF", "impact.ttf", "arialbd.ttf"],
            FontChoice::Impact => &["impact.ttf", "ariblk.ttf", "arialbd.ttf"],
            FontChoice::Playbill => &["PLAYBILL.TTF", "impact.ttf", "arialbd.ttf"],
            FontChoice::Typewriter => &["cour.ttf", "couri.ttf", "consola.ttf"],
            FontChoice::Chancery => &["LCALLIG.TTF", "MTCORSVA.TTF", "FRSCRIPT.TTF", "segoepr.ttf", "georgiai.ttf"],
            FontChoice::Papyrus => &["PAPYRUS.TTF", "GARA.TTF", "georgia.ttf", "times.ttf"],
            FontChoice::Ocr => &["OCRAEXT.TTF", "consola.ttf", "cour.ttf"],
            FontChoice::Marker => &["MAGNETOB.TTF", "segoepr.ttf", "comic.ttf", "arialbd.ttf"],
            FontChoice::Symbols => &["seguisym.ttf", "segoeui.ttf", "arial.ttf"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextOpts {
    /// Glyph height in dots (203 dpi: 32 dots is about 4 mm).
    pub px: f32,
    pub font: FontChoice,
    pub align: Align,
    /// White on black.
    pub invert: bool,
    /// Break anywhere instead of at spaces; for terminal output and fixed columns.
    pub break_anywhere: bool,
    /// Extra dots between lines.
    pub line_gap: f32,
    /// Blank dots above and below the text.
    pub margin_y: f32,
    /// Blank dots left and right of the text.
    pub margin_x: f32,
}

impl Default for TextOpts {
    fn default() -> Self {
        TextOpts { px: 32.0, font: FontChoice::Sans, align: Align::Left, invert: false, break_anywhere: false, line_gap: 0.0, margin_y: MARGIN, margin_x: MARGIN }
    }
}

const MARGIN: f32 = 8.0;

fn font(choice: FontChoice) -> Result<Arc<Font>, String> {
    static CACHE: OnceLock<Mutex<HashMap<FontChoice, Arc<Font>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(f) = cache.lock().unwrap().get(&choice) {
        return Ok(f.clone());
    }
    let dir = std::env::var("WINDIR").map(|w| format!(r"{w}\Fonts")).unwrap_or_else(|_| r"C:\Windows\Fonts".into());
    for file in choice.files() {
        if let Ok(bytes) = std::fs::read(format!(r"{dir}\{file}")) {
            if let Ok(f) = Font::from_bytes(bytes, FontSettings::default()) {
                let f = Arc::new(f);
                cache.lock().unwrap().insert(choice, f.clone());
                return Ok(f);
            }
        }
    }
    Err(format!("no {} font found in {dir}", choice.label()))
}

/// The columns between which a bitmap has ink, or `None` when it is blank.
pub fn ink_span(b: &Bitmap) -> Option<(usize, usize)> {
    let mut span: Option<(usize, usize)> = None;
    for y in 0..b.height {
        for x in 0..WIDTH_DOTS {
            if b.get(x, y) {
                span = Some(match span {
                    None => (x, x),
                    Some((a, z)) => (a.min(x), z.max(x)),
                });
            }
        }
    }
    span
}

thread_local! {
    /// The strings drawn since the last [`take_drawn`], while [`record_drawn`] is on: how a themed
    /// ticket finds which part of itself (a field, the text) each element shows.
    static DRAWN: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Start (or stop) remembering the strings that are drawn.
pub fn record_drawn(on: bool) {
    DRAWN.with(|d| *d.borrow_mut() = on.then(Vec::new));
}

/// The strings drawn since the last call.
pub fn take_drawn() -> Vec<String> {
    DRAWN.with(|d| d.borrow_mut().as_mut().map(std::mem::take).unwrap_or_default())
}

/// Text wrapped to the paper width. An empty string is an error so the UI can say so.
pub fn text(text: &str, opts: TextOpts) -> Result<Bitmap, String> {
    if text.trim().is_empty() {
        return Err("nothing to print".into());
    }
    DRAWN.with(|d| {
        if let Some(v) = d.borrow_mut().as_mut() {
            v.push(text.to_string());
        }
    });
    let font = font(opts.font)?;
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        max_width: Some((WIDTH_DOTS as f32 - 2.0 * opts.margin_x).max(8.0)),
        wrap_style: if opts.break_anywhere { WrapStyle::Letter } else { WrapStyle::Word },
        horizontal_align: match opts.align {
            Align::Left => HorizontalAlign::Left,
            Align::Center => HorizontalAlign::Center,
            Align::Right => HorizontalAlign::Right,
        },
        line_height: 1.0 + opts.line_gap / opts.px.max(1.0),
        ..LayoutSettings::default()
    });
    layout.append(&[&*font], &TextStyle::new(text, opts.px, 0));

    let height = layout.height().ceil() as usize + 2 * opts.margin_y as usize;
    let mut gray = vec![255u8; WIDTH_DOTS * height];
    for glyph in layout.glyphs() {
        if glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        let (metrics, coverage) = font.rasterize_config(glyph.key);
        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                let x = (glyph.x + opts.margin_x) as i64 + gx as i64;
                let y = (glyph.y + opts.margin_y) as i64 + gy as i64;
                if x < 0 || y < 0 || x >= WIDTH_DOTS as i64 || y >= height as i64 {
                    continue;
                }
                let cell = &mut gray[y as usize * WIDTH_DOTS + x as usize];
                *cell = (*cell).min(255 - coverage[gy * metrics.width + gx]);
            }
        }
    }
    let mut bitmap = Bitmap::blank(height);
    for y in 0..height {
        for x in 0..WIDTH_DOTS {
            if (gray[y * WIDTH_DOTS + x] < 128) != opts.invert {
                bitmap.set(x, y);
            }
        }
    }
    Ok(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_refused() {
        assert!(text("   ", TextOpts::default()).is_err());
    }

    #[test]
    fn text_prints_something_and_inverting_flips_the_paper() {
        let normal = text("Hello", TextOpts::default()).unwrap();
        let inverted = text("Hello", TextOpts { invert: true, ..TextOpts::default() }).unwrap();
        let ink = |b: &Bitmap| b.data.iter().map(|x| x.count_ones()).sum::<u32>();
        assert!(ink(&normal) > 0);
        assert!(ink(&inverted) > ink(&normal), "white on black covers more of the paper");
    }

    #[test]
    fn the_vertical_margin_is_adjustable_so_live_batches_can_join() {
        let loose = text("a", TextOpts::default()).unwrap();
        let tight = text("a", TextOpts { margin_y: 0.0, ..TextOpts::default() }).unwrap();
        assert_eq!(loose.height - tight.height, 16);
    }

    #[test]
    fn long_text_wraps_into_a_taller_ticket() {
        let short = text("one line", TextOpts::default()).unwrap();
        let long = text(&"word ".repeat(60), TextOpts::default()).unwrap();
        assert!(long.height > short.height * 3);
    }
}
