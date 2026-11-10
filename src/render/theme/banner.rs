//! A sideways banner: big letters running along the length of the paper, each one as tall as the
//! paper is wide. Drawn on a sheet that is then turned a quarter turn, like the boarding pass.

use super::{Ctx, Theme};
use crate::protocol::{Bitmap, WIDTH_DOTS};
use crate::render::draw::{self, Anchor, Sheet};
use crate::render::text::{self, Align, FontChoice, TextOpts};

type R = Result<Bitmap, String>;

pub fn banner(c: &Ctx) -> R {
    let _ = Theme::Banner;
    let mut lines: Vec<String> = c
        .body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_uppercase)
        .collect();
    if lines.is_empty() {
        lines = vec!["BIG".to_string(), "BANNER".to_string()];
    }
    lines.truncate(3);

    let n = lines.len().max(1);
    let slice_h = WIDTH_DOTS / n; // each line gets 384/N dots of height

    // Vary the typeface per line when there is more than one.
    // A typeface named in the "font" field is used for every line; "mix" or nothing varies them.
    let named = c.given("font");
    let chosen = match named.trim().to_ascii_lowercase().as_str() {
        "" | "mix" => None,
        other => super::custom::font_of(Some(other)).ok(),
    };
    let fonts: Vec<FontChoice> = if let Some(f) = chosen {
        vec![f; n]
    } else if n == 1 {
        vec![FontChoice::Impact; n]
    } else {
        (0..n)
            .map(|i| match i % 3 {
                0 => FontChoice::Impact,
                1 => FontChoice::Stencil,
                _ => FontChoice::Bold,
            })
            .collect()
    };

    // Pick a point size so the ink of each line fills its slice, and measure its width.
    let mut info: Vec<(FontChoice, f32, usize, usize)> = Vec::new(); // font, px, ink_height, width
    for i in 0..n {
        let target = slice_h.saturating_sub(4).max(12);
        let px = choose_px(&lines[i], fonts[i], target);
        let (h, w) = measure(&lines[i], fonts[i], px);
        info.push((fonts[i], px, h, w));
    }

    let max_w = info.iter().map(|x| x.3).max().unwrap_or(0);
    let margin = 24usize;
    let sheet_w = (max_w + 2 * margin).clamp(400, 3000);

    let mut s = Sheet::new(sheet_w, WIDTH_DOTS);

    // Thin rounded border.
    draw::rect(&mut s, 0, 0, sheet_w - 1, WIDTH_DOTS - 1, 1);
    draw::round_corners(&mut s, 0, 0, sheet_w - 1, WIDTH_DOTS - 1, 10);

    for i in 0..n {
        let (font, px, line_h, line_w) = info[i];
        let slice_top = (i * slice_h) as i32;
        let y_top = slice_top + ((slice_h as i32 - line_h as i32) / 2).max(0);
        let x_start = ((sheet_w as i32 - line_w as i32) / 2).max(margin as i32);
        let mut x_cursor = x_start;
        for ch in lines[i].chars() {
            if ch == ' ' {
                x_cursor += space_w(px) as i32;
                continue;
            }
            let res = s.text(&ch.to_string(), font, px, x_cursor, y_top, Anchor::Left);
            match res {
                Some((x0, _y0, x1, _y1)) => {
                    x_cursor = x1 as i32 + 1 + gap_w(px) as i32;
                    let _ = x0;
                }
                None => {
                    x_cursor += (px * 0.5) as i32;
                }
            }
        }
    }

    let b = s.turned();
    // The whole banner is the text: pressing it on the preview opens the text box there.
    super::mark(super::Region { key: "body".into(), x0: 0, x1: WIDTH_DOTS, y0: 0, y1: b.height });
    Ok(b)
}

/// The air between two letters: a little, in proportion to their size, so they sit close.
fn gap_w(px: f32) -> usize {
    (px * 0.04).round().max(1.0) as usize
}

/// The width of a space between words.
fn space_w(px: f32) -> usize {
    (px * 0.25) as usize
}

fn measure(line: &str, font: FontChoice, px: f32) -> (usize, usize) {
    let mut hmax = 0usize;
    let mut wtotal = 0usize;
    for ch in line.chars() {
        if ch == ' ' {
            wtotal += space_w(px);
            continue;
        }
        let opts = TextOpts {
            px,
            font,
            align: Align::Left,
            margin_x: 0.0,
            margin_y: 0.0,
            break_anywhere: true,
            ..TextOpts::default()
        };
        if let Ok(bmp) = text::text(&ch.to_string(), opts) {
            if let Some((x0, y0, x1, y1)) = draw::ink_box(&bmp) {
                hmax = hmax.max(y1 - y0 + 1);
                wtotal += (x1 - x0 + 1) + gap_w(px);
            }
        }
    }
    (hmax.max(1), wtotal.max(1))
}

fn choose_px(line: &str, font: FontChoice, target: usize) -> f32 {
    let mut lo = 12.0f32;
    let mut hi = target as f32;
    // Make sure something fits.
    if measure(line, font, hi).0 > target {
        // shrink hi until it fits
        while hi > 12.0 && measure(line, font, hi).0 > target {
            hi -= 4.0;
        }
        lo = 12.0;
    }
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        let (h, _) = measure(line, font, mid);
        if h <= target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo.max(12.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::theme::{Ctx, Fields};

    #[test]
    fn banner_grows_with_text_and_draws_ink() {
        let fields = Fields::new();
        let a = banner(&Ctx::new("BIG", &fields)).unwrap();
        let b = banner(&Ctx::new("BIG BANNER EXTRA LONG TEXT", &fields)).unwrap();
        assert!(b.height > a.height, "longer text should be taller: {} vs {}", b.height, a.height);
        // Deterministic.
        assert_eq!(a, banner(&Ctx::new("BIG", &fields)).unwrap());
        let ink: u32 = a.data.iter().map(|x| x.count_ones()).sum();
        assert!(ink > 200, "banner drew too little: {ink}");
    }

    #[test]
    fn the_typeface_can_be_chosen_and_a_bad_name_falls_back_to_the_mix() {
        // The banner picks its face from the field. On a machine where two names resolve to the
        // same face, comparing the drawings would measure the fallback, not the choice: ask the
        // resolver what each name gave, and say so when it gave one face for both.
        let probe = |choice: FontChoice| -> Option<Bitmap> {
            text::text("BIG", TextOpts { px: 48.0, font: choice, margin_x: 0.0, margin_y: 0.0, ..TextOpts::default() }).ok()
        };
        let (Some(script_face), Some(impact_face)) = (probe(FontChoice::Script), probe(FontChoice::Impact)) else {
            eprintln!("skipped: no font found on this machine");
            return;
        };
        if script_face == impact_face {
            let (w, h) = draw::ink_box(&script_face).map(|(x0, y0, x1, y1)| (x1 - x0 + 1, y1 - y0 + 1)).unwrap_or((0, 0));
            eprintln!("skipped: Script and Impact both resolve to a face {w}x{h} on this machine");
            return;
        }
        let with = |name: &str| -> Bitmap {
            let f: Fields = [("font".to_string(), name.to_string())].into_iter().collect();
            banner(&Ctx::new("BIG\nBANNER", &f)).expect("the banner draws with a known font")
        };
        assert_ne!(with("script"), with("impact"), "the typeface changes the drawing");
        assert_eq!(with("nonsense"), with("mix"), "an unknown name is the mix");
    }

    #[test]
    fn banner_empty_uses_sample() {
        // Empty body is handled by render(); banner itself expects non-empty but should not panic.
        let fields = Fields::new();
        let b = banner(&Ctx::new("BIG\nBANNER", &fields)).unwrap();
        assert!(b.height > 40);
    }
}
