//! QR codes and barcodes.

use barcoders::sym::code39::Code39;
use barcoders::sym::code128::Code128;
use barcoders::sym::ean13::EAN13;
use qrcode::{Color, EcLevel as QrLevel, QrCode};

use super::text::{self, Align, TextOpts};
use crate::protocol::{Bitmap, WIDTH_DOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeKind {
    Qr,
    Code128,
    Code39,
    Ean13,
}

impl CodeKind {
    pub const ALL: [CodeKind; 4] = [CodeKind::Qr, CodeKind::Code128, CodeKind::Code39, CodeKind::Ean13];

    pub fn label(self) -> &'static str {
        match self {
            CodeKind::Qr => "QR",
            CodeKind::Code128 => "Code 128",
            CodeKind::Code39 => "Code 39",
            CodeKind::Ean13 => "EAN-13",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EcLevel {
    Low,
    Medium,
    Quartile,
    High,
}

impl EcLevel {
    pub const ALL: [EcLevel; 4] = [EcLevel::Low, EcLevel::Medium, EcLevel::Quartile, EcLevel::High];

    pub fn label(self) -> &'static str {
        match self {
            EcLevel::Low => "L",
            EcLevel::Medium => "M",
            EcLevel::Quartile => "Q",
            EcLevel::High => "H",
        }
    }

    fn qr(self) -> QrLevel {
        match self {
            EcLevel::Low => QrLevel::L,
            EcLevel::Medium => QrLevel::M,
            EcLevel::Quartile => QrLevel::Q,
            EcLevel::High => QrLevel::H,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeOpts {
    pub kind: CodeKind,
    pub ec: EcLevel,
    /// Print the encoded text under the code.
    pub caption: bool,
}

impl Default for CodeOpts {
    fn default() -> Self {
        CodeOpts { kind: CodeKind::Qr, ec: EcLevel::Medium, caption: true }
    }
}

const QUIET: usize = 24;

pub fn code(data: &str, opts: CodeOpts) -> Result<Bitmap, String> {
    if data.is_empty() {
        return Err("nothing to encode".into());
    }
    let body = match opts.kind {
        CodeKind::Qr => qr(data, opts.ec)?,
        CodeKind::Code128 => bars(&Code128::new(format!("\u{0181}{data}")).map_err(|e| format!("Code 128: {e}"))?.encode(), 3, 120),
        CodeKind::Code39 => bars(&Code39::new(data.to_uppercase()).map_err(|e| format!("Code 39: {e}"))?.encode(), 2, 120),
        CodeKind::Ean13 => bars(&EAN13::new(data).map_err(|e| format!("EAN-13 wants 12 or 13 digits: {e}"))?.encode(), 3, 120),
    };
    if !opts.caption {
        return Ok(body);
    }
    let label = text::text(data, TextOpts { px: 22.0, align: Align::Center, break_anywhere: true, ..TextOpts::default() })?;
    Ok(super::stack(&[body, label], 0))
}

fn qr(data: &str, ec: EcLevel) -> Result<Bitmap, String> {
    let code = QrCode::with_error_correction_level(data.as_bytes(), ec.qr()).map_err(|e| format!("QR: {e}"))?;
    let n = code.width();
    let scale = ((WIDTH_DOTS - 2 * QUIET) / n).max(1);
    let side = n * scale;
    let height = side + 2 * QUIET;
    let left = (WIDTH_DOTS.saturating_sub(side)) / 2;
    let mut out = Bitmap::blank(height);
    for (i, c) in code.to_colors().into_iter().enumerate() {
        if c == Color::Dark {
            let (mx, my) = (i % n, i / n);
            for dy in 0..scale {
                for dx in 0..scale {
                    out.set(left + mx * scale + dx, QUIET + my * scale + dy);
                }
            }
        }
    }
    Ok(out)
}

fn bars(modules: &[u8], module_w: usize, height: usize) -> Bitmap {
    let mut w = module_w;
    while modules.len() * w > WIDTH_DOTS - 2 * 8 && w > 1 {
        w -= 1;
    }
    let width = modules.len() * w;
    let left = WIDTH_DOTS.saturating_sub(width) / 2;
    let mut out = Bitmap::blank(height + 2 * QUIET / 2);
    for (i, m) in modules.iter().enumerate() {
        if *m == 1 {
            for dx in 0..w {
                for y in 0..height {
                    out.set(left + i * w + dx, QUIET / 2 + y);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_qr_code_is_square_inside_a_quiet_zone() {
        let b = code("https://example.com", CodeOpts { caption: false, ..CodeOpts::default() }).unwrap();
        assert!(b.height > 100 && b.height < WIDTH_DOTS);
        assert!(b.data.iter().any(|x| *x != 0));
        assert!(!b.get(0, 0) && !b.get(WIDTH_DOTS - 1, 0), "the quiet zone stays white");
    }

    #[test]
    fn barcodes_fit_the_paper_and_bad_input_is_explained() {
        let b = code("HELLO123", CodeOpts { kind: CodeKind::Code128, caption: false, ..CodeOpts::default() }).unwrap();
        assert!(b.data.iter().any(|x| *x != 0));
        assert!(code("12", CodeOpts { kind: CodeKind::Ean13, caption: false, ..CodeOpts::default() }).is_err());
        assert!(code("", CodeOpts::default()).is_err());
    }

    #[test]
    fn a_caption_makes_the_ticket_taller() {
        let plain = code("hi", CodeOpts { caption: false, ..CodeOpts::default() }).unwrap();
        let captioned = code("hi", CodeOpts::default()).unwrap();
        assert!(captioned.height > plain.height);
    }
}
