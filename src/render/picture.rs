//! Pictures: decode once, then rescale, adjust and dither as the settings change.

use std::path::Path;
use std::sync::Arc;

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use super::dither::{self, Dither};
use super::label::{rotate_cw, LABEL_PAGE_H, LABEL_PAGE_W};
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

    /// The decoded image, for callers that need to inspect or crop it (e.g. a label editor).
    pub fn image(&self) -> &DynamicImage {
        &self.image
    }

    /// The image size in pixels.
    pub fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }
}

/// A crop window at a given aspect ratio, positioned inside the picture. `x` and `y` are
/// normalised offsets in `[0, 1]`: `0.0` places the window at the left/top edge, `1.0` at the
/// right/bottom edge, and `0.5` centres it (the default when no offset is given).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crop {
    /// Width divided by height of the crop window (e.g. `30.0 / 14.0` for a label, which is read
    /// sideways).
    pub aspect: f32,
    /// Horizontal position of the window, normalised to `[0, 1]` (0 = left, 1 = right).
    pub x: f32,
    /// Vertical position of the window, normalised to `[0, 1]` (0 = top, 1 = bottom).
    pub y: f32,
}

impl Crop {
    /// A centred crop at the given aspect ratio.
    pub fn centered(aspect: f32) -> Self {
        Crop { aspect, x: 0.5, y: 0.5 }
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
    /// Optional crop window. `None` keeps the full picture (pocket behaviour, unchanged).
    pub crop: Option<Crop>,
}

/// Dots to the millimetre: 384 dots across 48 mm of paper.
pub const DOTS_PER_MM: usize = 8;
pub const MAX_SPACE_MM: u8 = 50;

/// Label canvas as it goes to the head: 112 dots across, 240 down (14 × 30 mm). The label is read
/// lying down, so its content is designed landscape, on [`LABEL_PAGE_W`] × [`LABEL_PAGE_H`], and
/// turned a quarter turn clockwise on its way out.
pub const LABEL_WIDTH: usize = 112;
pub const LABEL_HEIGHT: usize = 240;
/// Corner radius of the printable label area, in dots.
pub const LABEL_CORNER: usize = 10;
/// Shape (width / height) of the label as it is read, for the crop window of a picture.
const LABEL_ASPECT: f32 = LABEL_PAGE_W as f32 / LABEL_PAGE_H as f32;

impl Default for PictureOpts {
    fn default() -> Self {
        PictureOpts {
            dither: Dither::FloydSteinberg,
            invert: false,
            brightness: 0.0,
            contrast: 0.0,
            rotate: 0,
            flip_h: false,
            space_before: 0,
            crop: None,
        }
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
/// its orientation and its crop, so it is made once and kept while brightness and contrast change.
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

/// The slow step: rotate, flip, optionally crop, scale to 384 dots, flatten, gray. No crop is
/// applied (`None`), which keeps the pocket behaviour unchanged.
pub fn prepare(source: &Source, rotate: u8, flip_h: bool) -> Prepared {
    prepare_with(source, rotate, flip_h, None)
}

/// The picture turned as the settings ask, or `None` when it is already in that orientation: a
/// caller then borrows the decoded image instead of copying a full-size picture for nothing.
fn oriented(source: &Source, rotate: u8, flip_h: bool) -> Option<DynamicImage> {
    let turns = rotate % 4;
    if turns == 0 && !flip_h {
        return None;
    }
    let img = match turns {
        1 => source.image().rotate90(),
        2 => source.image().rotate180(),
        3 => source.image().rotate270(),
        _ => source.image().clone(),
    };
    Some(if flip_h { img.fliph() } else { img })
}

/// The slow step with an optional crop window applied after rotation/flip and before scaling.
pub fn prepare_with(source: &Source, rotate: u8, flip_h: bool, crop: Option<Crop>) -> Prepared {
    let turned = oriented(source, rotate, flip_h);
    let img = turned.as_ref().unwrap_or_else(|| source.image());
    let cropped = crop.map(|c| cropped_at(img, c.aspect, c.x, c.y));
    let img = cropped.as_ref().unwrap_or(img);
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

/// Scale to the paper width, flatten on white, adjust, dither. Applies `opts.crop` when set.
pub fn picture(source: &Source, opts: PictureOpts) -> Bitmap {
    tone(&prepare_with(source, opts.rotate, opts.flip_h, opts.crop), opts)
}

/// Thermal heads bleed: dots print wider than drawn, so a label comes out darker than the
/// preview. Lighten the mids with a gamma before dithering; `ink_bias` lifts the whites too.
pub const LABEL_GAMMA: f32 = 0.85;
pub const LABEL_INK_BIAS: f32 = 12.0;

/// The picture framed for one label: cropped to the 30:14 landscape window (`opts.crop` places it,
/// centred when absent), scaled to the design page, tones applied, dithered, masked to the rounded
/// label corners, then turned a quarter turn clockwise into the 112 × 240 dots that print.
pub fn label_picture(source: &Source, opts: PictureOpts) -> Bitmap {
    label_picture_biased(source, opts, LABEL_GAMMA, LABEL_INK_BIAS)
}

/// The slow step of a label: rotate, flip, crop to the label's landscape window (`crop` places it,
/// centred when absent), scale to the 240 × 112 design page, flatten on white and turn to gray.
/// Kept between slider moves, so dragging the crop window only redoes the window and the resize.
pub fn label_prepare(source: &Source, rotate: u8, flip_h: bool, crop: Option<Crop>) -> Prepared {
    let turned = oriented(source, rotate, flip_h);
    let img = turned.as_ref().unwrap_or_else(|| source.image());
    // The window is the label's own shape, not `crop.aspect`: the tag is read sideways, and a
    // caller that still carries the old 14/30 shape must not frame a portrait strip on it. The
    // crop only says where the window sits.
    let (ox, oy) = crop.map_or((0.5, 0.5), |c| (c.x, c.y));
    let cropped = cropped_at(img, LABEL_ASPECT, ox, oy);
    // Triangle, not Lanczos3: on a 240×112 target the finer filter is not visible on 1-bit paper
    // and costs most of the frame while the crop window is dragged.
    let scaled = cropped.resize_exact(LABEL_PAGE_W as u32, LABEL_PAGE_H as u32, FilterType::Triangle);
    let flat = flatten_on_white(&scaled);
    let luma = flat.pixels().map(|p| 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])).collect();
    Prepared { height: LABEL_PAGE_H, luma }
}

/// The fast step of a label: brightness, contrast, thermal gamma, ink bias, invert and dither on
/// the landscape design page, then the rounded corners and the quarter turn clockwise into the
/// 112 × 240 dots the head prints. Cheap enough to run on every move of a slider or of the crop
/// window.
pub fn label_tone(prepared: &Prepared, opts: PictureOpts, gamma: f32, ink_bias: f32) -> Bitmap {
    let contrast = 1.0 + opts.contrast * 0.9;
    let shift = opts.brightness * 128.0;
    let mut gray: Vec<f32> = prepared
        .luma
        .iter()
        .map(|l| {
            let l = ((l - 128.0) * contrast + 128.0 + shift).clamp(0.0, 255.0);
            let g = (l / 255.0).clamp(0.0, 1.0);
            let l = (255.0 * g.powf(gamma) + ink_bias).clamp(0.0, 255.0);
            if opts.invert { 255.0 - l } else { l }
        })
        .collect();
    let mut bitmap = dither::apply_width(&mut gray, LABEL_PAGE_W, prepared.height, opts.dither);
    // The paper is cut square to the tag as it is read, so the corners are rounded on the design
    // page; the turn that follows keeps them in the corners of the printed bitmap.
    mask_label_corners(&mut bitmap);
    rotate_cw(&bitmap)
}

/// Like [`label_picture`], with thermal gamma and ink gain compensation applied before dithering.
pub fn label_picture_biased(source: &Source, opts: PictureOpts, gamma: f32, ink_bias: f32) -> Bitmap {
    label_tone(&label_prepare(source, opts.rotate, opts.flip_h, opts.crop), opts, gamma, ink_bias)
}

fn mask_label_corners(bitmap: &mut Bitmap) {
    let w = bitmap.width;
    let h = bitmap.height;
    let r = LABEL_CORNER;
    let wb = bitmap.width_bytes();
    for y in 0..h {
        for x in 0..w {
            let in_corner = (x < r && y < r)
                || (x >= w.saturating_sub(r) && y < r)
                || (x < r && y >= h.saturating_sub(r))
                || (x >= w.saturating_sub(r) && y >= h.saturating_sub(r));
            if !in_corner {
                continue;
            }
            let (cx, cy) = if x < r && y < r {
                (r as f32 - 0.5, r as f32 - 0.5)
            } else if x >= w - r && y < r {
                ((w - r) as f32 - 0.5, r as f32 - 0.5)
            } else if x < r && y >= h - r {
                (r as f32 - 0.5, (h - r) as f32 - 0.5)
            } else {
                ((w - r) as f32 - 0.5, (h - r) as f32 - 0.5)
            };
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy > (r as f32) * (r as f32) {
                bitmap.data[y * wb + x / 8] &= !(0x80 >> (x % 8));
            }
        }
    }
}

/// The window of a `w × h` picture that best fills `aspect` (width / height), positioned by the
/// normalised offsets `ox, oy` in `[0, 1]` (`0.0` = left/top edge, `1.0` = right/bottom edge,
/// `0.5` = centred): the largest rectangle of that shape there is, in pixels. Answers `None`
/// when the shape is not usable (`aspect` not finite or ≤ 0, or a side at zero).
pub fn crop_window_at(w: u32, h: u32, aspect: f32, ox: f32, oy: f32) -> Option<(u32, u32, u32, u32)> {
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
    let ox = if ox.is_finite() { ox.clamp(0.0, 1.0) } else { 0.5 };
    let oy = if oy.is_finite() { oy.clamp(0.0, 1.0) } else { 0.5 };
    let x = ((w - new_w) as f32 * ox).round() as i32;
    let y = ((h - new_h) as f32 * oy).round() as i32;
    let x = (x.max(0) as u32).min(w - new_w);
    let y = (y.max(0) as u32).min(h - new_h);
    // Preserve the historical centred position (integer half) so existing behaviour stays identical.
    let x = if (ox - 0.5).abs() < f32::EPSILON { (w - new_w) / 2 } else { x };
    let y = if (oy - 0.5).abs() < f32::EPSILON { (h - new_h) / 2 } else { y };
    Some((x, y, new_w, new_h))
}

/// The centred window of a `w × h` picture that best fills `aspect` (width / height): the largest
/// rectangle of that shape there is, in pixels. Answers `None` when the shape is not usable.
pub fn crop_window(w: u32, h: u32, aspect: f32) -> Option<(u32, u32, u32, u32)> {
    crop_window_at(w, h, aspect, 0.5, 0.5)
}

/// The picture cropped to `aspect` at the given normalised offsets, untouched when the shape cannot
/// be read or when the picture already matches it.
pub fn cropped_at(img: &DynamicImage, aspect: f32, ox: f32, oy: f32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let centered = (ox - 0.5).abs() < f32::EPSILON && (oy - 0.5).abs() < f32::EPSILON;
    let window = if centered { crop_window(w, h, aspect) } else { crop_window_at(w, h, aspect, ox, oy) };
    let Some((x, y, cw, ch)) = window else {
        return img.clone();
    };
    if cw == w && ch == h {
        return img.clone();
    }
    image::imageops::crop_imm(img, x, y, cw, ch).to_image().into()
}

/// The picture cropped to `aspect`, centred, untouched when the shape cannot be read or when the
/// picture already matches it.
pub fn cropped(img: &DynamicImage, aspect: f32) -> DynamicImage {
    cropped_at(img, aspect, 0.5, 0.5)
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

    #[test]
    fn crop_window_at_moves_the_window_by_normalised_offset() {
        // 100x100, aspect 14/30 (label): wider than the shape, keep height 100,
        // width floor(100 * 14/30) = 46, horizontal travel 54.
        let aspect = Crop::centered(14.0 / 30.0).aspect;
        let (x0, y0, w, h) = crop_window_at(100, 100, aspect, 0.0, 0.0).unwrap();
        assert_eq!((x0, y0, w, h), (0, 0, 46, 100));
        let (x1, y1, _, _) = crop_window_at(100, 100, aspect, 1.0, 1.0).unwrap();
        assert_eq!((x1, y1), (54, 0), "max offset reaches the right edge, no vertical travel");
        let (xc, yc, _, _) = crop_window_at(100, 100, aspect, 0.5, 0.5).unwrap();
        assert_eq!((xc, yc), (27, 0), "0.5 stays centred, not stuck at one side");
        // A tall picture crops vertically: 40x100, keep width 40, height floor(40 * 30/14)=85, travel 15.
        let (x, yt, _, ht) = crop_window_at(40, 100, aspect, 0.0, 0.0).unwrap();
        assert_eq!((x, yt, ht), (0, 0, 85));
        let (_, yb, _, _) = crop_window_at(40, 100, aspect, 0.0, 1.0).unwrap();
        assert_eq!(yb, 15, "vertical offset reaches the bottom edge");
    }

    #[test]
    fn cropped_at_ratio_14_30_selects_the_positioned_region() {
        // 120x80 image: left third black, middle third mid, right third white.
        let mut rgba = RgbaImage::from_pixel(120, 80, Rgba([128, 128, 128, 255]));
        for y in 0..80 {
            for x in 0..40 {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
            for x in 80..120 {
                rgba.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        let img = DynamicImage::ImageRgba8(rgba);
        let aspect = 14.0 / 30.0;
        // 120/80=1.5 wider than shape, keep height 80, width floor(80*14/30)=37, travel 83.
        let left = cropped_at(&img, aspect, 0.0, 0.5);
        assert_eq!(left.dimensions(), (37, 80));
        assert_eq!(left.get_pixel(18, 40), Rgba([0, 0, 0, 255]), "left offset samples the black side");
        let right = cropped_at(&img, aspect, 1.0, 0.5);
        assert_eq!(right.dimensions(), (37, 80));
        assert_eq!(right.get_pixel(18, 40), Rgba([255, 255, 255, 255]), "right offset samples the white side");
    }

    #[test]
    fn no_crop_keeps_picture_behaviour_unchanged() {
        let src = gray(100, 40, 30);
        let base = PictureOpts { brightness: 0.1, contrast: 0.2, ..PictureOpts::default() };
        assert_eq!(base.crop, None, "default asks for no crop");
        let plain = picture(&src, base);
        assert_eq!(picture(&src, PictureOpts { crop: None, ..base }), plain);
        assert_eq!(tone(&prepare(&src, base.rotate, base.flip_h), base), plain);
        assert_eq!(tone(&prepare_with(&src, base.rotate, base.flip_h, None), base), plain);
        let untouched = PictureOpts::default();
        assert_eq!(picture(&src, untouched), tone(&prepare(&src, 0, false), untouched));
    }

    #[test]
    fn source_exposes_the_decoded_image() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(12, 34, Rgba([1, 2, 3, 255])));
        let src = Source::from_image("x", img);
        assert_eq!(src.dimensions(), (12, 34));
        assert_eq!(src.image().dimensions(), (12, 34));
    }

    #[test]
    fn a_label_picture_is_112_by_240_when_printed() {
        // Designed landscape, printed down the 112-dot head: the bitmap handed over is the tag's
        // 112 dots across and 240 down, whichever way its content was laid out.
        let src = gray(80, 200, 100);
        let b = label_picture(&src, PictureOpts::default());
        assert_eq!((b.width, b.height), (LABEL_WIDTH, LABEL_HEIGHT));
        assert_eq!(b.data.len(), LABEL_WIDTH / 8 * LABEL_HEIGHT);
    }

    #[test]
    fn a_wide_picture_fills_the_landscape_label() {
        // 300 × 80 is wider than the label's 30:14 shape, so the window keeps the whole height and
        // crops the sides: the picture arrives whole down the tag, not squeezed into a strip.
        let (x, y, w, h) = crop_window(300, 80, LABEL_ASPECT).unwrap();
        assert_eq!((y, h), (0, 80), "the whole height of the picture is used");
        assert_eq!(w, (80.0 * LABEL_ASPECT).floor() as u32);
        assert_eq!(x, (300 - w) / 2, "centred");
        // Black top half, white bottom half: the picture fills the design page's height, so the
        // print has ink on one half of its width and none on the other. The turn puts the top of
        // the design on the right of the print.
        let mut rgba = RgbaImage::from_pixel(300, 80, Rgba([255, 255, 255, 255]));
        for y in 0..40 {
            for x in 0..300 {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let src = Source::from_image("tb", DynamicImage::ImageRgba8(rgba));
        let b = label_picture(&src, PictureOpts { dither: Dither::Threshold, ..PictureOpts::default() });
        assert!(b.get(LABEL_WIDTH - 5, LABEL_HEIGHT / 2), "the black part of the picture is on the label");
        assert!(!b.get(5, LABEL_HEIGHT / 2), "and the white part stays paper: nothing was cut off");
    }

    #[test]
    fn label_prepare_is_landscape() {
        // The slow step works on the 30 × 14 mm page: 240 luma values a row, 112 rows, so the
        // picture is framed the way the tag is read. The turn happens once, at the very end.
        let src = gray(80, 200, 100);
        let prepared = label_prepare(&src, 0, false, None);
        assert_eq!(prepared.height, LABEL_PAGE_H);
        assert_eq!(prepared.luma.len(), LABEL_PAGE_W * LABEL_PAGE_H);
        assert_eq!(prepared.luma.len() / prepared.height, LABEL_PAGE_W, "240 dots a row");
        assert!(LABEL_PAGE_W > prepared.height, "wider than tall: landscape");
        // The printed bitmap is the same page turned, so it is portrait even though the design is
        // not: this is the whole point of the turn.
        let b = label_picture(&src, PictureOpts::default());
        assert_eq!((b.width, b.height), (prepared.height, LABEL_PAGE_W), "printed 112 × 240");
    }

    #[test]
    fn the_corners_of_a_label_are_masked() {
        let src = gray(0, 200, 200); // black image => ink in center
        let b = label_picture(&src, PictureOpts { dither: Dither::Threshold, ..PictureOpts::default() });
        // extreme corners must be white (masked out)
        assert!(!b.get(0, 0));
        assert!(!b.get(LABEL_WIDTH - 1, 0));
        assert!(!b.get(0, LABEL_HEIGHT - 1));
        assert!(!b.get(LABEL_WIDTH - 1, LABEL_HEIGHT - 1));
        // central zone may have ink
        let ink = (60..LABEL_HEIGHT - 60)
            .flat_map(|y| (20..LABEL_WIDTH - 20).map(move |x| (x, y)))
            .any(|(x, y)| b.get(x, y));
        assert!(ink, "the centre should keep ink");
    }

    #[test]
    fn a_label_crop_offset_moves_the_window() {
        // The window is landscape, so on a square picture it travels up and down: the top of the
        // picture and its bottom are two different labels.
        let mut rgba = RgbaImage::from_pixel(200, 200, Rgba([255, 255, 255, 255]));
        for y in 0..100 {
            for x in 0..200 {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let src = Source::from_image("tb", DynamicImage::ImageRgba8(rgba));
        let at = |y: f32| {
            label_picture(
                &src,
                PictureOpts {
                    dither: Dither::Threshold,
                    crop: Some(Crop { aspect: LABEL_ASPECT, x: 0.5, y }),
                    ..PictureOpts::default()
                },
            )
        };
        let (top, bottom) = (at(0.0), at(1.0));
        assert_ne!(top.data, bottom.data, "different crop offsets must select different regions");
        let ink = |b: &Bitmap| b.data.iter().map(|x| x.count_ones()).sum::<u32>();
        assert!(ink(&top) > 0, "the top of the window is the black half");
        assert_eq!(ink(&bottom), 0, "the bottom of the window is the white half");
    }

    #[test]
    fn a_positive_bias_and_a_gamma_below_one_lighten_the_label() {
        let src = gray(120, 200, 200);
        let opts = PictureOpts { dither: Dither::Threshold, ..PictureOpts::default() };
        let ink = |b: &Bitmap| b.data.iter().map(|x| x.count_ones()).sum::<u32>();
        let dark = ink(&label_picture_biased(&src, opts, 1.0, 0.0));
        let light = ink(&label_picture_biased(&src, opts, 0.85, 12.0));
        assert!(light < dark, "gamma 0.85 and bias 12 should print fewer black dots than gamma 1.0 and bias 0: {light} vs {dark}");
    }

    #[test]
    fn label_picture_uses_the_default_ink_bias() {
        let src = gray(120, 200, 200);
        let opts = PictureOpts { dither: Dither::FloydSteinberg, brightness: 0.1, contrast: 0.2, ..PictureOpts::default() };
        assert_eq!(label_picture(&src, opts), label_picture_biased(&src, opts, LABEL_GAMMA, LABEL_INK_BIAS));
    }

    #[test]
    fn label_prepare_then_tone_equals_label_picture() {
        // Turning, flipping and a moved window: the two steps must add up to the one-call render,
        // so the cache in `App` can serve either path.
        let src = gray(120, 200, 150);
        let opts = PictureOpts {
            dither: Dither::Atkinson,
            brightness: 0.15,
            contrast: -0.25,
            invert: true,
            rotate: 1,
            flip_h: true,
            crop: Some(Crop { aspect: LABEL_ASPECT, x: 0.3, y: 0.7 }),
            ..PictureOpts::default()
        };
        let prepared = label_prepare(&src, opts.rotate, opts.flip_h, opts.crop);
        assert_eq!(prepared.height, LABEL_PAGE_H);
        assert_eq!(prepared.luma.len(), LABEL_PAGE_W * LABEL_PAGE_H);
        assert_eq!(label_tone(&prepared, opts, LABEL_GAMMA, LABEL_INK_BIAS), label_picture(&src, opts));
        assert_eq!(label_picture(&src, opts), label_picture_biased(&src, opts, LABEL_GAMMA, LABEL_INK_BIAS));
    }

    #[test]
    fn label_prepare_without_turning_borrows_the_picture() {
        // No turn and no flip borrows the decoded picture instead of copying it; a full turn (4)
        // is no turn either, and the two prepared labels must hold the same luma.
        let src = gray(70, 90, 120);
        let crop = Some(Crop::centered(LABEL_ASPECT));
        let borrowed = label_prepare(&src, 0, false, crop);
        let full_turn = label_prepare(&src, 4, false, crop);
        assert_eq!(borrowed.height, LABEL_PAGE_H);
        assert_eq!(borrowed.luma, full_turn.luma, "four quarter turns is the same picture");
        // The source is untouched: preparing twice gives the same label.
        assert_eq!(label_prepare(&src, 0, false, crop).luma, borrowed.luma);
    }
}
