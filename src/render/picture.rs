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

/// The window of a `w × h` picture that best fills `aspect` (width / height), centred: the
/// largest rectangle of that shape there is, in pixels. Answers `None` when the shape is not
/// usable (`aspect` not finite or ≤ 0, or a side at zero).
pub fn crop_window(w: u32, h: u32, aspect: f32) -> Option<(u32, u32, u32, u32)> {
    if !aspect.is_finite() || aspect <= 0.0 || w == 0 || h == 0 {
        return None;
    }
    // Keep the whole height when the picture is wider than the shape, the whole width when it
    // is taller: that is the largest rectangle of the shape that fits in the picture.
    let (new_w, new_h) = if w as f32 / h as f32 > aspect {
        ((h as f32 * aspect).floor() as u32, h)
    } else {
        (w, (w as f32 / aspect).floor() as u32)
    };
    // Rounding down can reach zero on a very thin shape, and a rounding slip must never push
    // the window past the picture.
    let new_w = new_w.clamp(1, w);
    let new_h = new_h.clamp(1, h);
    Some(((w - new_w) / 2, (h - new_h) / 2, new_w, new_h))
}

/// The picture cropped to `aspect`, untouched when the shape cannot be read or when the picture
/// already matches it.
pub fn cropped(img: &image::DynamicImage, aspect: f32) -> image::DynamicImage {
    let (w, h) = img.dimensions();
    let Some((x, y, cw, ch)) = crop_window(w, h, aspect) else {
        return img.clone();
    };
    if cw == w && ch == h {
        return img.clone();
    }
    image::imageops::crop_imm(img, x, y, cw, ch).to_image().into()
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

    #[test]
    fn a_wide_picture_is_cropped_on_the_sides_to_fill_a_tall_shape() {
        // Hand check: 100/50 = 2.0 > 7/15 ≈ 0.467, so the picture is wider than the shape.
        // Keep the whole height (50) and crop the sides to floor(50 * 7/15) = floor(23.33) = 23,
        // centred: x = (100 - 23) / 2 = 38.
        let (x, y, w, h) = crop_window(100, 50, 7.0 / 15.0).unwrap();
        assert_eq!(h, 50, "the whole height is kept");
        assert_eq!(w, 23, "floor(50 * 7/15)");
        assert_eq!(y, 0);
        assert_eq!(x, (100 - 23) / 2);
        assert!(x + w <= 100, "the window stays inside the picture");
    }

    #[test]
    fn a_tall_picture_is_cropped_on_the_top_and_bottom() {
        // Hand check: 50/100 = 0.5 > 7/15 ≈ 0.467, so this picture is wider than the shape,
        // not taller: the height is kept (100) and the sides are cropped to floor(100 * 7/15) = 46.
        // The window never exceeds the picture.
        let (x, y, w, h) = crop_window(50, 100, 7.0 / 15.0).unwrap();
        assert_eq!((x, y, w, h), (2, 0, 46, 100));
        assert!(x + w <= 50 && y + h <= 100, "the window stays inside the picture");
        // A picture that is genuinely taller than the shape is cropped top and bottom:
        // 40/100 = 0.4 < 0.467, keep the width (40), height floor(40 * 15/7) = floor(85.71) = 85,
        // centred: y = (100 - 85) / 2 = 7.
        let (x, y, w, h) = crop_window(40, 100, 7.0 / 15.0).unwrap();
        assert_eq!((x, y, w, h), (0, 7, 40, 85));
    }

    #[test]
    fn a_picture_that_already_has_the_shape_is_left_whole() {
        // 70/150 = 7/15 exactly: the window is the whole picture.
        let (x, y, w, h) = crop_window(70, 150, 7.0 / 15.0).unwrap();
        assert_eq!((x, y, w, h), (0, 0, 70, 150));
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(70, 150, Rgba([10, 20, 30, 255])));
        assert_eq!(cropped(&img, 7.0 / 15.0).dimensions(), (70, 150));
    }

    #[test]
    fn a_shape_that_makes_no_sense_is_refused() {
        assert_eq!(crop_window(100, 50, 0.0), None);
        assert_eq!(crop_window(100, 50, -2.0), None);
        assert_eq!(crop_window(100, 50, f32::NAN), None);
        assert_eq!(crop_window(0, 50, 7.0 / 15.0), None);
        assert_eq!(crop_window(100, 0, 7.0 / 15.0), None);
    }

    #[test]
    fn cropping_keeps_the_side_we_keep() {
        // 200x100, left half black, right half white, cropped to a square.
        let mut rgba = RgbaImage::from_pixel(200, 100, Rgba([255, 255, 255, 255]));
        for y in 0..100 {
            for x in 0..100 {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let img = DynamicImage::ImageRgba8(rgba);
        // Hand check: 200/100 = 2.0 > 1.0, keep the height (100), crop the sides to 100,
        // centred: x = (200 - 100) / 2 = 50.
        let (x, y, w, h) = crop_window(200, 100, 1.0).unwrap();
        assert_eq!((x, y, w, h), (50, 0, 100, 100));
        let out = cropped(&img, 1.0);
        assert_eq!(out.dimensions(), (100, 100));
        // The centre of the crop is the middle of the picture, which is white.
        assert_eq!(out.get_pixel(50, 50), Rgba([255, 255, 255, 255]));
    }
}
