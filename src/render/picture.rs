//! Pictures: decode once, then rescale, adjust and dither as the settings change.

use std::path::Path;
use std::sync::Arc;

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use super::dither::{self, Dither};
use crate::protocol::{Bitmap, WIDTH_DOTS};

/// A decoded picture, cheap to clone.
#[derive(Clone)]
pub struct Source {
    pub name: String,
    /// Unique per decoded picture, so a cache can tell whether it still holds this one.
    pub id: u64,
    image: Arc<DynamicImage>,
}

fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Source {
    pub fn from_image(name: &str, image: DynamicImage) -> Source {
        Source { name: name.into(), id: next_id(), image: Arc::new(image) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PictureOpts {
    pub dither: Dither,
    pub invert: bool,
    /// -1.0 (darker) to 1.0 (lighter).
    pub brightness: f32,
    /// -1.0 (flatter) to 1.0 (punchier).
    pub contrast: f32,
    /// Quarter turns clockwise, 0 to 3.
    pub rotate: u8,
    pub flip_h: bool,
    /// Blank paper left above the picture, in millimetres (0 to [`MAX_SPACE_MM`]).
    pub space_before: u8,
}

/// Dots to the millimetre: 384 dots across 48 mm of paper.
pub const DOTS_PER_MM: usize = 8;
pub const MAX_SPACE_MM: u8 = 50;

impl Default for PictureOpts {
    fn default() -> Self {
        PictureOpts { dither: Dither::FloydSteinberg, invert: false, brightness: 0.0, contrast: 0.0, rotate: 0, flip_h: false, space_before: 0 }
    }
}

pub fn load_picture(path: &Path) -> Result<Source, String> {
    let image = image::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return Err("the picture is empty".into());
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Source { name, id: next_id(), image: Arc::new(image) })
}

/// A picture scaled to the paper width, flattened on white and turned to gray, before any tone
/// adjustment. This is the slow part (a high-quality resize), and it only depends on the picture,
/// its rotation and its flip, so it is made once and kept while brightness and contrast change.
pub struct Prepared {
    pub height: usize,
    luma: Vec<f32>,
}

impl Prepared {
    /// Which allocation this is, to tell in a test whether it was made again.
    #[cfg(test)]
    pub fn identity(&self) -> usize {
        self.luma.as_ptr() as usize
    }
}

/// The slow step: rotate, flip, scale to 384 dots, flatten, gray.
pub fn prepare(source: &Source, rotate: u8, flip_h: bool) -> Prepared {
    let mut img = (*source.image).clone();
    img = match rotate % 4 {
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => img,
    };
    if flip_h {
        img = img.fliph();
    }
    let (w, h) = img.dimensions();
    let new_h = ((u64::from(h) * WIDTH_DOTS as u64) / u64::from(w)).max(1) as u32;
    let scaled = img.resize_exact(WIDTH_DOTS as u32, new_h, FilterType::Lanczos3);
    let flat = flatten_on_white(&scaled);
    let luma = flat.pixels().map(|p| 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])).collect();
    Prepared { height: new_h as usize, luma }
}

/// The fast step: contrast, brightness, invert and dither. Cheap enough to run on every move of a
/// slider.
pub fn tone(prepared: &Prepared, opts: PictureOpts) -> Bitmap {
    let contrast = 1.0 + opts.contrast * 0.9;
    let shift = opts.brightness * 128.0;
    let mut gray: Vec<f32> = prepared
        .luma
        .iter()
        .map(|l| {
            let l = ((l - 128.0) * contrast + 128.0 + shift).clamp(0.0, 255.0);
            if opts.invert { 255.0 - l } else { l }
        })
        .collect();
    let bitmap = dither::apply(&mut gray, prepared.height, opts.dither);
    // Nothing is added at 0 mm: the picture is exactly what it was.
    match opts.space_before.min(MAX_SPACE_MM) as usize * DOTS_PER_MM {
        0 => bitmap,
        rows => super::stack(&[Bitmap::blank(rows), bitmap], 0),
    }
}

/// Scale to the paper width, flatten on white, adjust, dither.
pub fn picture(source: &Source, opts: PictureOpts) -> Bitmap {
    tone(&prepare(source, opts.rotate, opts.flip_h), opts)
}

fn flatten_on_white(img: &DynamicImage) -> RgbaImage {
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let a = f32::from(p[3]) / 255.0;
        for c in 0..3 {
            p[c] = (f32::from(p[c]) * a + 255.0 * (1.0 - a)).round() as u8;
        }
        *p = Rgba([p[0], p[1], p[2], 255]);
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(level: u8, w: u32, h: u32) -> Source {
        Source::from_image("t", DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba([level, level, level, 255]))))
    }

    #[test]
    fn white_space_before_is_blank_rows_on_top_and_nothing_at_zero() {
        let src = gray(0, 100, 50);
        let plain = picture(&src, PictureOpts::default());
        assert_eq!(picture(&src, PictureOpts { space_before: 0, ..PictureOpts::default() }), plain, "0 mm changes nothing");
        let spaced = picture(&src, PictureOpts { space_before: 5, ..PictureOpts::default() });
        assert_eq!(spaced.height, plain.height + 5 * DOTS_PER_MM);
        let blank = 5 * DOTS_PER_MM * crate::protocol::WIDTH_BYTES;
        assert!(spaced.data[..blank].iter().all(|b| *b == 0), "the top is blank paper");
        assert_eq!(&spaced.data[blank..], &plain.data[..], "the picture itself is untouched under it");
        let capped = picture(&src, PictureOpts { space_before: 200, ..PictureOpts::default() });
        assert_eq!(capped.height, plain.height + MAX_SPACE_MM as usize * DOTS_PER_MM, "more than 50 mm is cut to 50");
    }

    #[test]
    fn the_two_steps_together_equal_the_one_call() {
        let src = gray(100, 40, 30);
        let opts = PictureOpts { brightness: 0.2, contrast: 0.3, invert: true, ..PictureOpts::default() };
        assert_eq!(tone(&prepare(&src, opts.rotate, opts.flip_h), opts), picture(&src, opts));
    }

    #[test]
    fn a_picture_is_scaled_to_the_paper_width_keeping_its_shape() {
        let b = picture(&gray(0, 100, 50), PictureOpts::default());
        assert_eq!(b.height, WIDTH_DOTS / 2);
    }

    #[test]
    fn rotating_a_quarter_turn_swaps_the_shape() {
        let b = picture(&gray(0, 100, 50), PictureOpts { rotate: 1, ..PictureOpts::default() });
        assert_eq!(b.height, WIDTH_DOTS * 2);
    }

    #[test]
    fn brightness_lightens_and_invert_flips() {
        let ink = |b: &Bitmap| b.data.iter().map(|x| x.count_ones()).sum::<u32>();
        let src = gray(100, 64, 64);
        let normal = ink(&picture(&src, PictureOpts { dither: Dither::Threshold, ..PictureOpts::default() }));
        let bright = ink(&picture(&src, PictureOpts { dither: Dither::Threshold, brightness: 0.5, ..PictureOpts::default() }));
        let inverted = ink(&picture(&src, PictureOpts { dither: Dither::Threshold, invert: true, ..PictureOpts::default() }));
        assert!(normal > 0 && bright == 0 && inverted == 0);
    }

    #[test]
    fn a_transparent_picture_prints_as_white_paper() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0])));
        let b = picture(&Source::from_image("t", img), PictureOpts { dither: Dither::Threshold, ..PictureOpts::default() });
        assert!(b.data.iter().all(|x| *x == 0));
    }
}
