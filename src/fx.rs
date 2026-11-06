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

pub const CHROME: u32 = 1;
pub const FIRE: u32 = 4;
pub const ELECTRIC: u32 = 5;
pub const GLOW: u32 = 7;
pub const SPARKLES: u32 = 8;
pub const SHOCK: u32 = 9;
pub const PULSE: u32 = 12;
pub const HAZE: u32 = 16;

/// Effect colours by index (the effect data carries the index).
pub const COLORS: [u32; 8] = [0xffffff, 0xff6a3d, 0xff3d8b, 0x3ddc97, 0xffc247, 0xff5c5c, 0x22d3ee, 0x7c5cff];

pub const WHITE: f32 = 0.0;
pub const PINK: f32 = 2.0;
pub const AMBER: f32 = 4.0;
pub const RED: f32 = 5.0;

fn color(i: f32) -> Rgb {
    rgb(COLORS[(i.max(0.0) as usize).min(COLORS.len() - 1)])
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
            ..Default::default()
        }),
        HAZE => Fx::distort(DistortParams::HeatHaze { height: d[0], scale: 14.0, speed: 1.4, amount: d[1], whole_shape: d[2] > 0.5 }),
        _ => return None,
    };
    Some(fx.on(shape).at(t))
}

/// Draws the frame with Rune's renderer, painting `rune-fx` layers at every effect slot.
#[derive(Default)]
pub struct Gfx {
    fx: Option<FxRenderer>,
    /// A slot that moves on its own was drawn last frame, so the window must keep asking for
    /// frames. Still slots (a held chrome bar, a static glow) let the window rest.
    live: bool,
}

impl Compositor<App> for Gfx {
    fn animating(&self) -> bool {
        self.live
    }

    fn render(&mut self, f: &mut Frame<'_, App>) {
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
}
