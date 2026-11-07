//! The look: colour palettes (a discount supermarket by default, ink-violet night, and more),
//! thermal-paper cream, and a glow in the palette's accents.

use std::sync::OnceLock;

use rune::Color;
use rune_kit::{Kit, PaletteSeed};

/// `#rrggbb` to a colour.
pub const fn rgb(hex: u32) -> Color {
    Color { r: decode((hex >> 16) & 0xff), g: decode((hex >> 8) & 0xff), b: decode(hex & 0xff), a: 1.0 }
}

/// sRGB byte to linear light, without `powf` so this can stay a const fn.
const fn decode(v: u32) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { pow24((c + 0.055) / 1.055) }
}

/// `x^2.4` by `x^2 * x^0.4`, with `x^0.4` from a short Newton iteration; good to ~1e-4.
const fn pow24(x: f32) -> f32 {
    // x^0.4 = exp(0.4 ln x); ln and exp through series would be long, so use the fifth root
    // squared: x^0.4 = (x^(1/5))^2, the fifth root by Newton on y^5 = x.
    let mut y = 0.7;
    let mut i = 0;
    while i < 24 {
        let y2 = y * y;
        let y4 = y2 * y2;
        y = y - (y4 * y - x) / (5.0 * y4);
        i += 1;
    }
    x * x * y * y
}

/// The colour schemes the app can wear, chosen in a setting and applied at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PaletteId {
    /// A tongue-in-cheek discount supermarket: blue, yellow and red.
    #[default]
    Discount,
    /// Ink-violet night with a coral-to-pink glow (the look the app had at first).
    Night,
    /// Neutral greys and one calm blue, no glow.
    Professional,
    Christmas,
    Halloween,
    /// Amber phosphor on black.
    Terminal,
    Matrix,
}

impl PaletteId {
    pub const ALL: [PaletteId; 7] = [PaletteId::Discount, PaletteId::Night, PaletteId::Professional, PaletteId::Christmas, PaletteId::Halloween, PaletteId::Terminal, PaletteId::Matrix];

    /// The word kept in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            PaletteId::Discount => "discount",
            PaletteId::Night => "night",
            PaletteId::Professional => "professional",
            PaletteId::Christmas => "christmas",
            PaletteId::Halloween => "halloween",
            PaletteId::Terminal => "terminal",
            PaletteId::Matrix => "matrix",
        }
    }

    pub fn from_id(id: &str) -> Option<PaletteId> {
        PaletteId::ALL.into_iter().find(|p| p.id() == id.trim().to_ascii_lowercase())
    }

    pub fn palette(self) -> &'static Palette {
        &PALETTES[self as usize]
    }

    pub fn name(self) -> &'static str {
        self.palette().name
    }
}

/// One colour scheme, as `#rrggbb` numbers (so a test can measure their contrast).
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub id: PaletteId,
    pub name: &'static str,
    pub bg_top: u32,
    pub bg_bottom: u32,
    pub panel: u32,
    pub text: u32,
    pub text_dim: u32,
    /// The main accent: the Print button, the lit tab, highlights.
    pub accent: u32,
    /// The second accent: price-tag touches, the glow's other colour.
    pub accent2: u32,
    /// The two ends of the gradient on the main accent.
    pub grad_a: u32,
    pub grad_b: u32,
    /// Text on the main accent.
    pub on_accent: u32,
    pub ok: u32,
    pub warn: u32,
    pub err: u32,
    /// Whether the Print button glows and sparks (the quiet palettes do not).
    pub glow: bool,
}

pub const PALETTES: [Palette; 7] = [
    Palette { id: PaletteId::Discount, name: "Discount", bg_top: 0x0c2f6b, bg_bottom: 0x071b42, panel: 0x0a3a82, text: 0xffffff, text_dim: 0xb9cbea, accent: 0xfff000, accent2: 0xe10915, grad_a: 0xfff000, grad_b: 0xffc800, on_accent: 0x1a1a1a, ok: 0x35d07f, warn: 0xffc247, err: 0xff8a90, glow: true },
    Palette { id: PaletteId::Night, name: "Night", bg_top: 0x1a1228, bg_bottom: 0x0b0911, panel: 0x171222, text: 0xf4effa, text_dim: 0x9b92ad, accent: 0xff6a3d, accent2: 0xff3d8b, grad_a: 0xd0401a, grad_b: 0xd0286f, on_accent: 0xffffff, ok: 0x3ddc97, warn: 0xffc247, err: 0xff5c5c, glow: true },
    Palette { id: PaletteId::Professional, name: "Professional", bg_top: 0x20252b, bg_bottom: 0x15191d, panel: 0x1c2126, text: 0xe8eaed, text_dim: 0x9aa3ad, accent: 0x3470d0, accent2: 0x5f95e6, grad_a: 0x3470d0, grad_b: 0x2a5fb8, on_accent: 0xffffff, ok: 0x4cb782, warn: 0xd9a441, err: 0xe5686b, glow: false },
    Palette { id: PaletteId::Christmas, name: "Christmas", bg_top: 0x0f4027, bg_bottom: 0x082616, panel: 0x0c3320, text: 0xf7fbf7, text_dim: 0xb7d3c0, accent: 0xf4c430, accent2: 0xc8102e, grad_a: 0xf4c430, grad_b: 0xe0a800, on_accent: 0x2a1a00, ok: 0x7ee08f, warn: 0xf4c430, err: 0xff6b6b, glow: true },
    Palette { id: PaletteId::Halloween, name: "Halloween", bg_top: 0x1a1022, bg_bottom: 0x09060d, panel: 0x160f1e, text: 0xf3ece4, text_dim: 0xa99db3, accent: 0xff7a00, accent2: 0x8b3fd9, grad_a: 0xff8a1a, grad_b: 0xff6a00, on_accent: 0x1a0f00, ok: 0x7bdc65, warn: 0xffc247, err: 0xff5c5c, glow: true },
    Palette { id: PaletteId::Terminal, name: "Terminal", bg_top: 0x100e08, bg_bottom: 0x060503, panel: 0x0f0d07, text: 0xffb000, text_dim: 0xc08a10, accent: 0xffc233, accent2: 0xff7a00, grad_a: 0xffc233, grad_b: 0xffa500, on_accent: 0x000000, ok: 0xffb000, warn: 0xffd666, err: 0xff6a3d, glow: false },
    Palette { id: PaletteId::Matrix, name: "Matrix", bg_top: 0x03110a, bg_bottom: 0x000000, panel: 0x03140a, text: 0xb6ffc6, text_dim: 0x52b36d, accent: 0x00ff41, accent2: 0x00b32c, grad_a: 0x00ff41, grad_b: 0x00d636, on_accent: 0x001a06, ok: 0x00ff41, warn: 0xd6ff3d, err: 0xff5a5a, glow: false },
];

static CURRENT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Wear another palette: every view built after this uses it.
pub fn set_palette(id: PaletteId) {
    CURRENT.store(id as u8, std::sync::atomic::Ordering::Relaxed);
}

pub fn palette_id() -> PaletteId {
    PaletteId::ALL[(CURRENT.load(std::sync::atomic::Ordering::Relaxed) as usize).min(PaletteId::ALL.len() - 1)]
}

/// The palette being worn.
pub fn pal() -> &'static Palette {
    palette_id().palette()
}

// The colours of the palette being worn, by the names the view has always used.
pub fn bg_top() -> Color {
    rgb(pal().bg_top)
}
pub fn bg_bottom() -> Color {
    rgb(pal().bg_bottom)
}
pub fn panel() -> Color {
    rgb(pal().panel)
}
/// Main text.
pub fn ink() -> Color {
    rgb(pal().text)
}
pub fn dim() -> Color {
    rgb(pal().text_dim)
}
/// The main accent.
pub fn coral() -> Color {
    rgb(pal().accent)
}
/// The second accent.
pub fn pink() -> Color {
    rgb(pal().accent2)
}
pub fn grad_a() -> Color {
    rgb(pal().grad_a)
}
pub fn grad_b() -> Color {
    rgb(pal().grad_b)
}
pub fn on_accent() -> Color {
    rgb(pal().on_accent)
}
pub fn mint() -> Color {
    rgb(pal().ok)
}
pub fn amber() -> Color {
    rgb(pal().warn)
}
pub fn red() -> Color {
    rgb(pal().err)
}

pub fn kit() -> &'static Kit {
    static K: [OnceLock<Kit>; 7] = [const { OnceLock::new() }; 7];
    let p = pal();
    K[p.id as usize].get_or_init(|| Kit::from_palette(PaletteSeed { bg: rgb(p.bg_bottom), fg: rgb(p.text), accent: rgb(p.accent) }))
}

/// The same kit on the cream of the paper, so text typed on the preview is dark ink, in every palette.
pub fn paper_kit() -> &'static Kit {
    static K: [OnceLock<Kit>; 7] = [const { OnceLock::new() }; 7];
    let p = pal();
    K[p.id as usize].get_or_init(|| Kit::from_palette(PaletteSeed { bg: rgb(0xfaf6ec), fg: rgb(0x1b1620), accent: rgb(p.accent) }))
}

/// WCAG contrast ratio between two `#rrggbb` colours (1 to 21).
pub fn contrast(a: u32, b: u32) -> f32 {
    fn lum(hex: u32) -> f32 {
        let c = rgb(hex);
        0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
    }
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colour_helper_matches_the_srgb_curve() {
        let white = rgb(0xffffff);
        assert!((white.r - 1.0).abs() < 1e-3 && (white.b - 1.0).abs() < 1e-3);
        let black = rgb(0x000000);
        assert!(black.r == 0.0);
        let mid = rgb(0x808080);
        assert!((mid.r - 0.2158).abs() < 2e-3, "sRGB 128 is about 0.2158 linear, got {}", mid.r);
    }

    #[test]
    fn the_default_is_discount_and_ids_round_trip() {
        assert_eq!(PaletteId::default(), PaletteId::Discount);
        for id in PaletteId::ALL {
            assert_eq!(PaletteId::from_id(id.id()), Some(id));
            assert_eq!(id.palette().id, id, "the table is in the order of the enum");
        }
        assert_eq!(PaletteId::from_id(" Matrix "), Some(PaletteId::Matrix));
        assert_eq!(PaletteId::from_id("blue"), None);
    }

    #[test]
    fn every_palette_is_distinct_and_readable() {
        for p in PALETTES {
            assert_ne!(p.bg_bottom, p.accent, "{}: the accent shows on the background", p.name);
            for (what, fg, bg) in [
                ("text on the top", p.text, p.bg_top),
                ("text on the bottom", p.text, p.bg_bottom),
                ("text on a panel", p.text, p.panel),
                ("dim text on a panel", p.text_dim, p.panel),
                ("dim text on the bottom", p.text_dim, p.bg_bottom),
                ("text on the gradient start", p.on_accent, p.grad_a),
                ("text on the gradient end", p.on_accent, p.grad_b),
                ("errors on a panel", p.err, p.panel),
            ] {
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "{}: {what} is {c:.1}:1", p.name);
            }
            assert!(contrast(p.accent, p.bg_bottom) >= 3.0, "{}: the accent stands out", p.name);
        }
        for (i, a) in PALETTES.iter().enumerate() {
            for b in &PALETTES[i + 1..] {
                assert!(a.bg_bottom != b.bg_bottom || a.accent != b.accent, "{} and {} are the same", a.name, b.name);
            }
        }
    }
}
