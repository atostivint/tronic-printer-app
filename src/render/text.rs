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
        }
    }

    fn files(self) -> &'static [&'static str] {
        match self {
            FontChoice::Sans => &["segoeui.ttf", "arial.ttf", "calibri.ttf", "tahoma.ttf"],
            FontChoice::Bold => &["segoeuib.ttf", "arialbd.ttf", "calibrib.ttf", "tahomabd.ttf"],
            FontChoice::Mono => &["consola.ttf", "cour.ttf", "lucon.ttf"],
            FontChoice::Serif => &["georgia.ttf", "times.ttf", "cambria.ttc"],
            FontChoice::Script => &["segoepr.ttf", "comic.ttf", "ink free.ttf", "inkfree.ttf"],
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
}

impl Default for TextOpts {
    fn default() -> Self {
        TextOpts { px: 32.0, font: FontChoice::Sans, align: Align::Left, invert: false, break_anywhere: false, line_gap: 0.0, margin_y: MARGIN }
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

/// Text wrapped to the paper width. An empty string is an error so the UI can say so.
pub fn text(text: &str, opts: TextOpts) -> Result<Bitmap, String> {
    if text.trim().is_empty() {
        return Err("nothing to print".into());
    }
    let font = font(opts.font)?;
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        max_width: Some(WIDTH_DOTS as f32 - 2.0 * MARGIN),
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
                let x = (glyph.x + MARGIN) as i64 + gx as i64;
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
