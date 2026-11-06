//! The look: ink-violet night, thermal-paper cream, and a hot coral-to-pink glow.

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

pub const BG_TOP: Color = rgb(0x1a1228);
pub const BG_BOTTOM: Color = rgb(0x0b0911);
pub const PANEL: Color = rgb(0x171222);
pub const TEXT: Color = rgb(0xf4effa);
pub const TEXT_DIM: Color = rgb(0x9b92ad);
pub const CORAL: Color = rgb(0xff6a3d);
pub const PINK: Color = rgb(0xff3d8b);
pub const MINT: Color = rgb(0x3ddc97);
pub const AMBER: Color = rgb(0xffc247);
pub const RED: Color = rgb(0xff5c5c);

pub fn kit() -> &'static Kit {
    static K: OnceLock<Kit> = OnceLock::new();
    K.get_or_init(|| Kit::from_palette(PaletteSeed { bg: BG_BOTTOM, fg: TEXT, accent: CORAL }))
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
}
