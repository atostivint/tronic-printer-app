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
/// Slow coloured curtains in the background, one palette's mood; a ticket's family only tints
/// them. The tint rides in the high bits of the slot's mark (see [`aurora`]), so the four data
/// floats keep their meaning from the plain palettes.
pub const AURORA: u32 = 18;

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
        _ => true,
    }
}

/// Builds the layer for one slot, or `None` for an unknown kind. `tint` recolours the aurora
/// (0 is the palette); `shape` is the slot's rect in points; `t` is Rune's clock.
fn layer(kind: u32, tint: u32, d: [f32; 4], shape: Shape, rect: [f32; 4], t: f32) -> Option<FxLayer> {
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
            // The palette's own colours, or the family tint (see `TINTS`): base, ridge, shadow.
            let p = crate::style::pal();
            let (a, b, c) = match TINTS.get(tint as usize).copied() {
                Some(t) if t.0 != 0 => (t.0, t.1, t.2),
                _ => (p.accent, p.accent2, p.panel),
            };
            Fx::aurora(AuroraParams {
                a: rgb(a),
                b: rgb(b),
                c: rgb(c),
                scale: d[0].max(60.0),
                speed: d[1].max(0.01),
                intensity: d[2].clamp(0.0, 0.8),
                feather: d[3].max(20.0),
            })
        }
        _ => return None,
    };
    Some(fx.on(shape).at(t))
}

/// The background weather a ticket's family adds to its palette: the palette's aurora, with the
/// family's own tint and bands. A plain ticket has no family and keeps the palette's own aurora,
/// so the window still looks like itself when nothing is dressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavour {
    Palette,
    Shop,
    Travel,
    OldWorld,
    Papers,
    Forms,
    Retro,
    Labels,
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
        Some(Family::Labels) => Flavour::Labels,
        Some(Family::Mine) => Flavour::Mine,
    }
}

// One aurora slot per flavour. A family only tints the curtains and nudges their width, speed and
// softness; the palette fallbacks are the numbers the app has always had.
//
// A tint index travels in the high bits of the slot's mark: `AURORA | tint << 8`. That keeps the
// four data floats meaning the same thing everywhere (band width in points, speed, intensity,
// feather), and the shader's own horizontal wander grows with the speed.

/// The palette's own aurora; its colours come from the palette, not from `TINTS`.
const AUR_PALETTE: u32 = 0;
/// Warm cream and amber.
const AUR_SHOP: u32 = 1;
/// Cyan and blue.
const AUR_TRAVEL: u32 = 2;
/// Ember orange.
const AUR_OLD_WORLD: u32 = 3;
/// White and pale grey.
const AUR_PAPERS: u32 = 4;
/// Pale blue.
const AUR_FORMS: u32 = 5;
/// Phosphor green.
const AUR_RETRO: u32 = 6;
/// Coral.
const AUR_MINE: u32 = 7;
/// Pale violet, for the label printer's tags.
const AUR_LABELS: u32 = 8;

/// Curtain colours (base, ridge, shadow) by tint index; index 0 is unused, the palette decides.
const TINTS: [(u32, u32, u32); 9] = [
    (0, 0, 0),
    (0xffd9a0, 0xd98a2b, 0x3a2a18),
    (0x22d3ee, 0x3b82f6, 0x0b2447),
    (0xff8a3d, 0xc2410c, 0x2a1108),
    (0xffffff, 0xd8dee9, 0x9aa7b8),
    (0x9ecbff, 0x6aa9e0, 0x2b4a6b),
    (0x3dff8b, 0x1faa55, 0x062d16),
    (0xff9b7a, 0xe8553d, 0x3a1410),
    (0xd8c2ff, 0x8b5cf6, 0x241a3a),
];

/// An aurora slot with a family tint, for the weather tables.
const fn aurora(tint: u32, d: [f32; 4]) -> (u32, [f32; 4]) {
    (AURORA | tint << 8, d)
}

/// The palette's own weather, picked when the ticket is plain.
const W_DISCOUNT: [(u32, [f32; 4]); 1] = [aurora(AUR_PALETTE, [260.0, 0.03, 0.16, 100.0])];
const W_NIGHT: [(u32, [f32; 4]); 1] = [aurora(AUR_PALETTE, [220.0, 0.04, 0.24, 90.0])];
const W_PRO: [(u32, [f32; 4]); 1] = [aurora(AUR_PALETTE, [340.0, 0.02, 0.10, 120.0])];
const W_CHRISTMAS: [(u32, [f32; 4]); 1] = [aurora(AUR_PALETTE, [240.0, 0.03, 0.20, 80.0])];
const W_HALLOWEEN: [(u32, [f32; 4]); 1] = [aurora(AUR_PALETTE, [180.0, 0.035, 0.24, 70.0])];
// The CRT palette: thin green curtains, a slow crawl rather than a scanline.
const W_TERMINAL: [(u32, [f32; 4]); 1] = [aurora(AUR_PALETTE, [110.0, 0.06, 0.16, 45.0])];
const W_MATRIX: [(u32, [f32; 4]); 1] = [aurora(AUR_PALETTE, [130.0, 0.05, 0.20, 60.0])];

/// Warm cream and amber, slower than the palette's curtains.
const W_SHOP: [(u32, [f32; 4]); 1] = [aurora(AUR_SHOP, [300.0, 0.02, 0.18, 110.0])];
/// Cyan and blue, drifting faster.
const W_TRAVEL: [(u32, [f32; 4]); 1] = [aurora(AUR_TRAVEL, [200.0, 0.07, 0.22, 80.0])];
/// Ember orange, very slow, wider bands.
const W_OLD_WORLD: [(u32, [f32; 4]); 1] = [aurora(AUR_OLD_WORLD, [360.0, 0.015, 0.20, 130.0])];
/// White and pale grey: wide, low contrast, the paper's own glow.
const W_PAPERS: [(u32, [f32; 4]); 1] = [aurora(AUR_PAPERS, [420.0, 0.02, 0.12, 150.0])];
/// Pale blue, medium.
const W_FORMS: [(u32, [f32; 4]); 1] = [aurora(AUR_FORMS, [260.0, 0.035, 0.16, 90.0])];
/// Phosphor green, thin bands crawling.
const W_RETRO: [(u32, [f32; 4]); 1] = [aurora(AUR_RETRO, [130.0, 0.09, 0.20, 55.0])];
/// Pale violet, wide and calm: a label has no roll pulling it along.
const W_LABELS: [(u32, [f32; 4]); 1] = [aurora(AUR_LABELS, [280.0, 0.025, 0.14, 120.0])];
/// A user's own theme: coral, medium.
const W_MINE: [(u32, [f32; 4]); 1] = [aurora(AUR_MINE, [240.0, 0.03, 0.18, 90.0])];

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
        Flavour::Labels => &W_LABELS,
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
        // Pink at a lower strength than a plain ticket's, which keeps the pair unique.
        Flavour::Labels => (PINK, 0.9),
        Flavour::Mine => (CORAL, 0.9),
    }
}

/// The slow layer behind everything: the palette gives the window its aurora, the ticket's family
/// only tints it. Plain tickets fall back to the palette, and every aurora moves, so the window
/// stays awake while one is on screen.
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
            let mark = marks.get(n).copied().unwrap_or(0);
            // The low byte is the effect kind; an aurora's tint rides above it (see `aurora`).
            let kind = mark & 0xff;
            let tint = mark >> 8;
            let k = i.params[3];
            let [x, y, w, h] = i.rect.map(|v| v / k);
            let r = i.radii.iter().fold(0.0f32, |a, v| a.max(v.abs())) / k;
            let Some(l) = layer(kind, tint, i.uv, Shape::rounded(x, y, w, h, r), [x, y, w, h], t) else { continue };
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
        assert!(layer(250, 0, [0.0; 4], Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0), [0.0, 0.0, 10.0, 10.0], 0.0).is_none());
        assert!(layer(FIRE, 0, [60.0, 1.0, 0.0, 0.5], Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0), [0.0, 0.0, 10.0, 10.0], 0.0).is_some());
    }

    #[test]
    fn every_palette_draws_a_moving_untinted_aurora() {
        let shape = Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0);
        for id in PaletteId::ALL {
            let _ = ambient(id, None);
            let (kind, data) = weather(flavour(None), id)[0];
            assert_eq!(kind, AURORA, "{id:?} does not show the palette's aurora");
            assert!(moves(kind, data), "{id:?} weather is dead still");
            assert!(
                layer(AURORA, AUR_PALETTE, data, shape, [0.0, 0.0, 10.0, 10.0], 0.0).is_some(),
                "{id:?} draws nothing"
            );
        }
        assert!(moves(AURORA, [100.0, 0.05, 0.3, 80.0]));
    }

    #[test]
    fn every_family_only_tints_the_aurora_and_a_plain_ticket_keeps_its_palette() {
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
        // Every family wears the same effect, recoloured: the scanner look is gone.
        let mut tints: Vec<u32> = Vec::new();
        for family in Family::ALL {
            let (kind, _) = weather(flavour(Some(family)), PaletteId::Discount)[0];
            assert_eq!(kind & 0xff, AURORA, "{family:?} is not an aurora");
            let tint = kind >> 8;
            assert_ne!(tint, AUR_PALETTE, "{family:?} keeps the palette's colours");
            assert!(!tints.contains(&tint), "{family:?} reuses another family's tint");
            tints.push(tint);
        }
        // The plain fallbacks are the palette's aurora, with the numbers it always had.
        let plain = |p| weather(flavour(None), p)[0];
        assert_eq!(plain(PaletteId::Discount), (AURORA, [260.0, 0.03, 0.16, 100.0]));
        assert_eq!(plain(PaletteId::Night), (AURORA, [220.0, 0.04, 0.24, 90.0]));
        assert_eq!(plain(PaletteId::Christmas), (AURORA, [240.0, 0.03, 0.20, 80.0]));
        assert_eq!(plain(PaletteId::Halloween), (AURORA, [180.0, 0.035, 0.24, 70.0]));
        assert_eq!(plain(PaletteId::Matrix), (AURORA, [130.0, 0.05, 0.20, 60.0]));
    }

    #[test]
    fn every_family_weather_draws_and_moves() {
        let shape = Shape::rounded(0.0, 0.0, 10.0, 10.0, 2.0);
        for family in Family::ALL {
            let slots = weather(flavour(Some(family)), PaletteId::Discount);
            assert_eq!(slots.len(), 1, "{family:?} should wear one aurora, not a stack");
            let (kind, data) = slots[0];
            assert_eq!(kind & 0xff, AURORA, "{family:?} draws something other than aurora");
            assert!(layer(kind & 0xff, kind >> 8, data, shape, [0.0, 0.0, 10.0, 10.0], 0.0).is_some(), "{family:?} draws nothing");
            assert!(moves(kind & 0xff, data), "{family:?} weather is dead still");
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
