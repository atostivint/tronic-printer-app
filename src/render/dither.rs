//! Ways to turn gray levels into black or white dots.

use crate::protocol::{Bitmap, WIDTH_DOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dither {
    /// Hard cut at the middle gray: crisp for line art and text.
    Threshold,
    /// Error diffusion: smooth photos.
    FloydSteinberg,
    /// Lighter error diffusion with more contrast, the classic Macintosh look.
    Atkinson,
    /// A fixed 4×4 pattern: regular, no worms, good for gradients.
    Ordered,
}

impl Dither {
    pub const ALL: [Dither; 4] = [Dither::Threshold, Dither::FloydSteinberg, Dither::Atkinson, Dither::Ordered];

    pub fn label(self) -> &'static str {
        match self {
            Dither::Threshold => "Threshold",
            Dither::FloydSteinberg => "Floyd",
            Dither::Atkinson => "Atkinson",
            Dither::Ordered => "Ordered",
        }
    }
}

const BAYER4: [[f32; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];

/// `gray` holds `WIDTH_DOTS * height` values from 0 (black) to 255 (white).
pub fn apply(gray: &mut [f32], height: usize, method: Dither) -> Bitmap {
    let mut bitmap = Bitmap::blank(height);
    for y in 0..height {
        for x in 0..WIDTH_DOTS {
            let old = gray[y * WIDTH_DOTS + x];
            let (black, new) = match method {
                Dither::Ordered => {
                    let cut = (BAYER4[y % 4][x % 4] + 0.5) / 16.0 * 255.0;
                    (old < cut, 0.0)
                }
                _ => {
                    let b = old < 128.0;
                    (b, if b { 0.0 } else { 255.0 })
                }
            };
            if black {
                bitmap.set(x, y);
            }
            let err = old - new;
            let mut spread = |dx: i64, dy: i64, k: f32| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx >= 0 && (nx as usize) < WIDTH_DOTS && (ny as usize) < height {
                    gray[ny as usize * WIDTH_DOTS + nx as usize] += err * k;
                }
            };
            match method {
                Dither::FloydSteinberg => {
                    spread(1, 0, 7.0 / 16.0);
                    spread(-1, 1, 3.0 / 16.0);
                    spread(0, 1, 5.0 / 16.0);
                    spread(1, 1, 1.0 / 16.0);
                }
                Dither::Atkinson => {
                    for (dx, dy) in [(1, 0), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)] {
                        spread(dx, dy, 1.0 / 8.0);
                    }
                }
                Dither::Threshold | Dither::Ordered => {}
            }
        }
    }
    bitmap
}

#[cfg(test)]
mod tests {
    use super::*;

    fn black_ratio(method: Dither, level: f32) -> f32 {
        let height = 40;
        let mut gray = vec![level; WIDTH_DOTS * height];
        let bitmap = apply(&mut gray, height, method);
        let black = (0..height).flat_map(|y| (0..WIDTH_DOTS).map(move |x| (x, y))).filter(|&(x, y)| bitmap.get(x, y)).count();
        black as f32 / (WIDTH_DOTS * height) as f32
    }

    #[test]
    fn mid_gray_prints_about_half_the_dots_with_diffusion_and_ordered() {
        for m in [Dither::FloydSteinberg, Dither::Ordered] {
            let r = black_ratio(m, 128.0);
            assert!((0.4..0.6).contains(&r), "{m:?} ratio {r}");
        }
    }

    #[test]
    fn atkinson_drops_a_quarter_of_the_error_so_it_has_more_contrast_than_floyd() {
        // Highlights blow out to white and shadows sink to black.
        assert!(black_ratio(Dither::Atkinson, 200.0) <= black_ratio(Dither::FloydSteinberg, 200.0));
        assert!(black_ratio(Dither::Atkinson, 60.0) >= black_ratio(Dither::FloydSteinberg, 60.0));
    }

    #[test]
    fn threshold_is_all_or_nothing() {
        assert_eq!(black_ratio(Dither::Threshold, 100.0), 1.0);
        assert_eq!(black_ratio(Dither::Threshold, 200.0), 0.0);
    }
}
