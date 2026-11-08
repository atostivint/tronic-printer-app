//! GPU effects. The view places an *effect slot* (`slot(kind, data)`, an element that draws
//! nothing) where an effect should appear; this compositor turns every slot into one `rune-fx`
//! layer, drawn in painter order. The slot's mark says which effect, `effect_data` carries its few
//! live numbers, and the slot's rect and radius are the shape. The pattern follows Rune's
//! `examples/showreel`.
//!
//! The window only keeps redrawing while a slot is on screen: with effects off, or nothing to
//! show, the compositor reports it is not animating and the GPU rests.

use rune::compose::{Compositor, Frame};
use rune::render::{EffectPhase, EffectSite, KIND_EFFECT};
use rune::El;
use rune_fx::*;

use crate::app::App;
use crate::render::Family;
use crate::style::PaletteId;

pub const CHROME: u32 = 1;
pub const FIRE: u32 = 4;
pub const ELECTRIC: u32 = 5;
pub const GLOW: u32 = 7;
pub const SPARKLES: u32 = 8;
pub const SHOCK: u32 = 9;
pub const PULSE: u32 = 12;
pub const HAZE: u32 = 16;
pub const ARC: u32 = 17;
/// Slow coloured curtains in the background, one palette's mood.
pub const AURORA: u32 = 18;
/// Paper grain: static newsprint when `fps` is 0, a faint crawl when it is not.
pub const GRAIN: u32 = 19;
/// A band of light sweeping across the window now and then; keeps the eye moving.
pub const SHIMMER: u32 = 20;

/// Effect colours by index (the effect data carries the index).
pub const COLORS: [u32; 8] = [0xffffff, 0xff6a3d, 0xff3d8b, 0x3ddc97, 0xffc247, 0xff5c5c, 0x22d3ee, 0x7c5cff];

pub const WHITE: f32 = 0.0;
pub const PINK: f32 = 2.0;
pub const AMBER: f32 = 4.0;
pub const RED: f32 = 5.0;
pub const GREEN: f32 = 3.0;
pub const CORAL: f32 = 1.0;
pub const CYAN: f32 = 6.0;

/// The effect colour at index `i`; the accents, the green, amber and red are the palette's own.
fn color(i: f32) -> Rgb {
    let p = crate::style::pal();
    let at = (i.max(0.0) as usize).min(COLORS.len() - 1);
    rgb(match at {
        1 => p.accent,
        2 => p.accent2,
        3 => p.ok,
        4 => p.warn,
        5 => p.err,
        _ => COLORS[at],
    })
}

/// An effect slot: `kind` is one of the constants above, `data` its numbers.
pub fn slot(kind: u32, data: [f32; 4]) -> El {
    rune::effect(kind).effect_data(data)
}

/// Whether a slot changes by itself from frame to frame. A window with no such slot can rest.
fn moves(kind: u32, d: [f32; 4]) -> bool {
    match kind {
        CHROME => d[2] > 0.0,
        GLOW => d[0] as u32 == 2,
        GRAIN => d[1] > 0.0,
        _ => true,
    }
}

/// Builds the layer for one slot, or `None` for an unknown kind. `shape` is the slot's rect in
/// points; `t` is Rune's clock.
fn layer(kind: u32, d: [f32; 4], shape: Shape, rect: [f32; 4], t: f32) -> Option<FxLayer> {
    let fx = match kind {
        CHROME => {
            let mut p = LiquidMetalParams::chrome();
            // d[2] is how fast the metal flows; 0 holds it still (and lets the window rest).
            p.flow *= d[2];
            if d[2] <= 0.0 {
                p.speed = 0.0;
            }
            Fx::fill(p)
        }
        FIRE => Fx::fire(FireParams {
            style: FireStyle::Top,
            height: d[0],
            intensity: d[1],
            smoke: d[3],
            embers: 24,
            scale: 26.0,
            ..FireParams::default()
        }),
        ELECTRIC => {
            let base = if d[2] > 0.5 { ElectricParams::charged() } else { ElectricParams::default() };
            Fx::electric(ElectricParams {
                intensity: d[0],
                amplitude: if d[1] > 0.0 { d[1] } else { base.amplitude },
                color: color(d[3]),
                ..base
            })
        }
        ARC => {
            // A bolt between two corners of the slot: `d[2]` 1 runs bottom-left to top-right, 2 runs
            // top-left to bottom-right. Used for lightning that leaps off the printer.
            let [x, y, w, h] = rect;
            let (a, b) = if d[2] as u32 == 2 { ([x, y], [x + w, y + h]) } else { ([x, y + h], [x + w, y]) };
            let base = ElectricParams::default();
            Fx::electric(ElectricParams {
                style: ElectricStyle::Arc { a, b },
                intensity: d[0],
                amplitude: if d[1] > 0.0 { d[1] } else { base.amplitude },
                color: color(d[3]),
                ..base
            })
        }
        GLOW => {
            let style = match d[0] as u32 {
                1 => GlowStyle::Neon,
                2 => GlowStyle::Aura,
                _ => GlowStyle::Soft,
            };
            Fx::glow(GlowParams { style, color: color(d[1]), intensity: d[2], radius: if d[3] > 0.0 { d[3] } else { 14.0 }, aura_speed: 0.35, ..Default::default() })
        }
        SPARKLES => Fx::sparkles(SparkleParams {
            burst: (d[0] >= 0.0).then_some(d[0]),
            count: d[1] as u32,
            size: d[2],
            color: color(d[3]),
            margin: 26.0,
            ..Default::default()
        }),
        SHOCK => {
            // The slot is a wide box around the thing pressed; the wave starts at its centre.
            let at = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
            Fx::distort(DistortParams::Shockwave { at, reach: 600.0, width: 70.0, amount: d[3], chroma: 0.5, t0: d[2], duration: 0.9 })
        }
        PULSE => Fx::pulse(PulseParams {
            style: PulseStyle::Radar,
            color: color(d[1]),
            reach: d[3],
            period: if d[2] > 0.0 { d[2] } else { 1.8 },
            // Discreet by default: one faint ring at a time.
            intensity: 0.4,
            count: 1,
            ..Default::default()
        }),
        HAZE => Fx::distort(DistortParams::HeatHaze { height: d[0], scale: 14.0, speed: 1.4, amount: d[1], whole_shape: d[2] > 0.5 }),
        AURORA => {
            let p = crate::style::pal();
            Fx::aurora(AuroraParams {
                a: rgb(p.accent),
                b: rgb(p.accent2),
                c: rgb(p.panel),
                scale: d[0].max(60.0),
                speed: d[1].max(0.01),
                intensity: d[2].clamp(0.0, 0.8),
                feather: d[3].max(20.0),
            })
        }
        GRAIN => Fx::grain(GrainParams { amount: d[0].clamp(0.0, 0.2), fps: d[1].max(0.0), feather: 0.0 }),
        SHIMMER => Fx::shimmer(ShimmerParams {
            color: color(d[0]),
            base: rgb(0x000000),
            base_alpha: 0.0,
            period: d[1].max(1.0),
            band: d[2].max(4.0),
            angle: d[3],
            intensity: 0.18,
            ..ShimmerParams::default()
        }),
        _ => return None,
    };
    Some(fx.on(shape).at(t))
}

/// The background weather a ticket's family adds to its palette. A plain ticket has no family and
/// keeps the palette's own weather, so the window still looks like itself when nothing is dressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavour {
    Palette,
    Shop,
    Travel,
    OldWorld,
    Papers,
    Forms,
    Retro,
    Mine,
}

/// Which flavour a ticket's family wears. `None` is a plain ticket: the palette decides.
pub fn flavour(family: Option<Family>) -> Flavour {
    match family {
        None => Flavour::Palette,
        Some(Family::Shop) => Flavour::Shop,
        Some(Family::Travel) => Flavour::Travel,
        Some(Family::OldWorld) => Flavour::OldWorld,
        Some(Family::Papers) => Flavour::Papers,
        Some(Family::Forms) => Flavour::Forms,
        Some(Family::Retro) => Flavour::Retro,
        Some(Family::Mine) => Flavour::Mine,
    }
}

// One slot per flavour, at most two. The numbers are deliberately timid: a hint of movement behind
// the panels, never a gradient. The palette fallbacks are the ones the app has always had.

/// The palette's own weather, picked when the ticket is plain.
const W_DISCOUNT: [(u32, [f32; 4]); 1] = [(AURORA, [260.0, 0.03, 0.16, 100.0])];
const W_NIGHT: [(u32, [f32; 4]); 1] = [(AURORA, [220.0, 0.04, 0.24, 90.0])];
const W_PRO: [(u32, [f32; 4]); 1] = [(GRAIN, [0.012, 0.0, 0.0, 0.0])];
const W_CHRISTMAS: [(u32, [f32; 4]); 1] = [(AURORA, [240.0, 0.03, 0.20, 80.0])];
const W_HALLOWEEN: [(u32, [f32; 4]); 1] = [(AURORA, [180.0, 0.035, 0.24, 70.0])];
// A faint crawl, like a CRT that is not quite asleep.
const W_TERMINAL: [(u32, [f32; 4]); 1] = [(GRAIN, [0.026, 5.0, 0.0, 0.0])];
const W_MATRIX: [(u32, [f32; 4]); 1] = [(AURORA, [130.0, 0.05, 0.20, 60.0])];

/// Printed-paper grain, the ink speckle drifting slowly.
const W_SHOP: [(u32, [f32; 4]); 1] = [(GRAIN, [0.018, 1.5, 0.0, 0.0])];
/// Thin wind streaks: a narrow band that skims across, like a departure board.
const W_TRAVEL: [(u32, [f32; 4]); 1] = [(SHIMMER, [CYAN, 4.5, 2.0, 0.9])];
/// Parchment texture plus slow warm embers.
const W_OLD_WORLD: [(u32, [f32; 4]); 2] =
    [(GRAIN, [0.015, 0.0, 0.0, 0.0]), (SHIMMER, [AMBER, 9.0, 4.0, 0.15])];
/// Photocopier static and a broad paper-curl reflection.
const W_PAPERS: [(u32, [f32; 4]); 2] =
    [(GRAIN, [0.028, 0.0, 0.0, 0.0]), (SHIMMER, [WHITE, 11.0, 46.0, 0.05])];
/// A blue-ink sheen that comes back often enough to feel like a blot.
const W_FORMS: [(u32, [f32; 4]); 1] = [(SHIMMER, [CYAN, 10.0, 8.0, -0.12])];
/// CRT scanlines: crawling grain under one thin green band.
const W_RETRO: [(u32, [f32; 4]); 2] =
    [(GRAIN, [0.03, 4.0, 0.0, 0.0]), (SHIMMER, [GREEN, 8.0, 2.0, 0.0])];
/// A user's own theme: quiet, but its own paper.
const W_MINE: [(u32, [f32; 4]); 1] = [(SHIMMER, [CORAL, 13.0, 10.0, 0.2])];

/// The slots making a flavour's weather. Kept as slices of constants so no frame allocates.
fn weather(f: Flavour, palette: PaletteId) -> &'static [(u32, [f32; 4])] {
    match f {
        Flavour::Palette => match palette {
            PaletteId::Discount => &W_DISCOUNT,
            PaletteId::Night => &W_NIGHT,
            PaletteId::Professional => &W_PRO,
            PaletteId::Christmas => &W_CHRISTMAS,
            PaletteId::Halloween => &W_HALLOWEEN,
            PaletteId::Terminal => &W_TERMINAL,
            PaletteId::Matrix => &W_MATRIX,
        },
        Flavour::Shop => &W_SHOP,
        Flavour::Travel => &W_TRAVEL,
        Flavour::OldWorld => &W_OLD_WORLD,
        Flavour::Papers => &W_PAPERS,
        Flavour::Forms => &W_FORMS,
        Flavour::Retro => &W_RETRO,
        Flavour::Mine => &W_MINE,
    }
}

/// The colour index and the strength a family gives the Print halo and its lightning: embers for
/// the old world, phosphor green for the techy themes, the palette's pink for a plain ticket.
pub fn press_style(family: Option<Family>) -> (f32, f32) {
    match flavour(family) {
        Flavour::Palette => (PINK, 1.0),
        Flavour::Shop => (CORAL, 1.0),
        Flavour::Travel => (CYAN, 0.9),
        Flavour::OldWorld => (AMBER, 0.85),
        Flavour::Papers => (WHITE, 0.8),
        Flavour::Forms => (CYAN, 0.75),
        Flavour::Retro => (GREEN, 1.0),
        Flavour::Mine => (CORAL, 0.9),
    }
}

/// The slow layer behind everything: the palette gives the window its weather, the ticket's family
/// gives it a flavour. Plain tickets fall back to the palette, and only lively slots keep the
/// window awake, so it can still rest.
pub fn ambient(palette: PaletteId, family: Option<Family>) -> El {
    let slots = weather(flavour(family), palette);
    let mut body = rune::col().w_full().h_full();
    for (kind, data) in slots {
        body = body.child(slot(*kind, *data).absolute(0.0, 0.0).w_full().h_full().effect_bleed(0.0));
    }
    body
}

/// Draws the frame with Rune's renderer, painting `rune-fx` layers at every effect slot.
#[derive(Default)]
pub struct Gfx {
    fx: Option<FxRenderer>,
    /// A slot that moves on its own was drawn last frame, so the window must keep asking for
    /// frames. Still slots (a held chrome bar, a static glow) let the window rest.
    live: bool,
    bars_set: bool,
}

impl Compositor<App> for Gfx {
    fn animating(&self) -> bool {
        self.live
    }

    fn render(&mut self, f: &mut Frame<'_, App>) {
        if !self.bars_set {
            // Scroll bars stay visible instead of fading in only while scrolling (Windows' default
            // here), so you can see that the gallery scrolls.
            f.rt.set_system_scroll_bars(rune::ScrollBars::Always);
            self.bars_set = true;
        }
        if f.rt.state.drag.is_some() {
            // A gallery picture is being dragged: tell the app where the pointer is and whether it
            // is over the preview (where the printer is), so letting go can print it.
            let pointer = f.rt.pointer();
            let over = match (pointer, f.rt.presented("stage")) {
                (Some((x, y)), Some(([sx, sy, sw, sh], _))) => x >= sx && x <= sx + sw && y >= sy && y <= sy + sh,
                _ => false,
            };
            f.rt.state.pointer = pointer;
            f.rt.state.over_stage = over;
        }
        let (device, queue, size) = (f.device, f.queue, f.size);
        let t = f.rt.t as f32;
        let marks = &f.rt.marks;
        let mut at: Vec<u32> = Vec::new();
        let mut layers: Vec<FxLayer> = Vec::new();
        let mut placements: Vec<FxSlot> = Vec::new();
        let mut fire = false;
        let mut animating = false;
        for (n, i) in f.rt.instances.iter().enumerate() {
            if i.params[1] != KIND_EFFECT {
                continue;
            }
            let kind = marks.get(n).copied().unwrap_or(0) & 0xff;
            let k = i.params[3];
            let [x, y, w, h] = i.rect.map(|v| v / k);
            let r = i.radii.iter().fold(0.0f32, |a, v| a.max(v.abs())) / k;
            let Some(l) = layer(kind, i.uv, Shape::rounded(x, y, w, h, r), [x, y, w, h], t) else { continue };
            fire |= kind == FIRE;
            animating |= moves(kind, i.uv);
            at.push(n as u32);
            layers.push(l);
            placements.push(FxSlot::default().scale(k).clip(i.clip, i.fade));
        }
        self.live = animating;
        let fx = self.fx.get_or_insert_with(|| {
            let mut fx = FxRenderer::new(device, f.format, FxOptions { dither: true });
            fx.set_fire_sim(device, true);
            fx
        });
        fx.set_srgb_blend(f.srgb_blend());
        fx.add_format(device, wgpu::TextureFormat::Rgba16Float);
        fx.prepare_with_slots(device, queue, &FxFrame { viewport: [size.0, size.1], scale: 1.0, layers: &layers, backdrop: None, sources: &[] }, &placements);
        if fire {
            fx.simulate(queue, f.encoder, None);
        }
        let clear = f.rt.clear_color();
        f.ui.prepare(device, queue, &mut f.rt.text.atlas, &f.rt.instances, size);
        f.ui.draw_with_backdrop(f.encoder, f.target, clear, &mut |phase, s: &EffectSite<'_>| {
            let Some(k) = at.iter().position(|n| *n == s.index) else { return };
            match phase {
                EffectPhase::Backdrop(enc) => {
                    if let Some(bd) = s.backdrop {
                        fx.blur_backdrop(device, queue, enc, bd, k);
                    }
                }
                EffectPhase::Draw(pass) => fx.draw_layer_in(pass, s.format, s.origin, k),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_chrome_bar_lets_the_window_rest_but_flames_and_plasma_do_not() {
        assert!(!moves(CHROME, [0.0, 0.0, 0.0, 0.0]));
        assert!(moves(CHROME, [0.0, 0.0, 0.6, 0.0]));
        assert!(!moves(GLOW, [1.0, PINK, 0.5, 20.0]), "a neon glow is still");
        assert!(moves(GLOW, [2.0, PINK, 0.5, 20.0]), "an aura breathes");
        assert!(moves(FIRE, [100.0, 1.0, 0.0, 0.5]) && moves(ELECTRIC, [1.0, 0.0, 1.0, PINK]));
    }

    #[test]
    fn every_colour_index_resolves_and_unknown_kinds_draw_nothing() {
        for i in 0..COLORS.len() {
            let _ = color(i as f32);
        }
        let _ = color(99.0);
        assert!(layer(250, [0.0; 4], Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0), [0.0, 0.0, 10.0, 10.0], 0.0).is_none());
        assert!(layer(FIRE, [60.0, 1.0, 0.0, 0.5], Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0), [0.0, 0.0, 10.0, 10.0], 0.0).is_some());
    }

    #[test]
    fn each_palette_has_a_weather_and_only_the_lively_ones_keep_the_window_awake() {
        for id in PaletteId::ALL {
            let _ = ambient(id, None);
        }
        assert!(!moves(GRAIN, [0.02, 0.0, 0.0, 0.0]), "static newsprint lets the window rest");
        assert!(moves(GRAIN, [0.02, 5.0, 0.0, 0.0]), "a crawling grain does not");
        assert!(moves(AURORA, [100.0, 0.05, 0.3, 80.0]) && moves(SHIMMER, [0.0; 4]));
        let shape = Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0);
        assert!(layer(AURORA, [100.0, 0.05, 0.3, 80.0], shape, [0.0, 0.0, 10.0, 10.0], 0.0).is_some());
        assert!(layer(GRAIN, [0.02, 0.0, 0.0, 0.0], shape, [0.0, 0.0, 10.0, 10.0], 0.0).is_some());
        assert!(layer(SHIMMER, [1.0, 2.0, 20.0, 0.3], shape, [0.0, 0.0, 10.0, 10.0], 0.0).is_some());
    }

    #[test]
    fn every_family_has_a_distinct_weather_and_a_plain_ticket_keeps_its_palette() {
        assert_eq!(flavour(None), Flavour::Palette);
        let worn: Vec<Flavour> = Family::ALL.iter().map(|f| flavour(Some(*f))).collect();
        for (i, a) in worn.iter().enumerate() {
            assert_ne!(*a, Flavour::Palette, "{a:?} must not borrow the palette's weather");
            for b in &worn[i + 1..] {
                assert_ne!(a, b, "two families share the same flavour");
            }
            for (j, other) in worn.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    weather(*a, PaletteId::Discount),
                    weather(*other, PaletteId::Discount),
                    "{:?} and {:?} draw the same weather",
                    Family::ALL[i],
                    Family::ALL[j]
                );
            }
        }
        // The plain fallback is the palette's old weather, still still or moving as it always was.
        let plain = |p| weather(flavour(None), p);
        assert_eq!(plain(PaletteId::Discount)[0].0, AURORA);
        assert_eq!(plain(PaletteId::Professional)[0].0, GRAIN);
        assert!(!moves(plain(PaletteId::Professional)[0].0, plain(PaletteId::Professional)[0].1));
        assert!(moves(plain(PaletteId::Terminal)[0].0, plain(PaletteId::Terminal)[0].1));
    }

    #[test]
    fn every_family_weather_draws_and_moves() {
        let shape = Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0);
        for family in Family::ALL {
            let slots = weather(flavour(Some(family)), PaletteId::Discount);
            assert!(!slots.is_empty(), "{family:?} has no weather");
            for (kind, data) in slots {
                assert!(layer(*kind, *data, shape, [0.0, 0.0, 10.0, 10.0], 0.0).is_some(), "{family:?} draws nothing");
            }
            assert!(slots.iter().any(|(kind, data)| moves(*kind, *data)), "{family:?} weather is dead still");
        }
    }

    #[test]
    fn each_family_tints_the_print_feedback_differently() {
        assert_eq!(press_style(None), (PINK, 1.0));
        let tints: Vec<(f32, f32)> = Family::ALL.iter().map(|f| press_style(Some(*f))).collect();
        for (i, a) in tints.iter().enumerate() {
            for b in &tints[i + 1..] {
                assert_ne!(a, b, "two families give the Print button the same lightning");
            }
        }
        assert_eq!(press_style(Some(Family::OldWorld)), (AMBER, 0.85));
        assert_eq!(press_style(Some(Family::Retro)), (GREEN, 1.0));
    }
}
