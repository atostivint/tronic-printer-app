//! Writing straight on the preview: the outlines, the boxes on the paper, and the bar under it.

use rune::tok::type_;
use rune::{Color, El, col, el, row, text};
use rune_kit::{Button, ButtonVariant};

use crate::app::App;
use crate::render;
use crate::style::{self, coral};

use super::print_key;


/// What lets you write straight on the preview: a faint outline round each part of the ticket
/// you can write on (not printed), a press on it starts typing there, in a text box right on the
/// paper, and a press on the paper elsewhere stops.
pub(crate) fn write_layer(s: &App, full: f32, turned: bool) -> El {
    let (ox, oy) = (render::PAPER_MARGIN as f32, render::TEETH as f32);
    let mut layer = col().absolute(0.0, 0.0).size(render::PREVIEW_WIDTH as f32, full);
    if s.editing.is_some() {
        layer = layer.on_press(|s: &mut App| s.stop_editing());
    }
    for r in &s.regions {
        let (x, y, w, h) = (ox + r.x0 as f32, oy + r.y0 as f32, (r.x1 - r.x0) as f32, (r.y1 - r.y0) as f32);
        if s.editing.as_deref() == Some(r.key.as_str()) {
            if turned {
                // The ticket is shown turned a quarter, so the box is made in the size it has on
                // screen (width and height swapped), centred on the part, and turned back upright.
                let (cx, cy) = (x + w / 2.0, y + h / 2.0);
                layer = layer.child(write_box(s, &r.key, cx - h / 2.0, cy - w / 2.0, h, w).rotate(std::f32::consts::FRAC_PI_2));
            } else {
                layer = layer.child(write_box(s, &r.key, x, y, w, h));
            }
        } else {
            let key = r.key.clone();
            layer = layer.child(el().key(format!("region-{}", r.key)).absolute(x, y).size(w, h).radius(4.0).border(1.0, coral().alpha(0.3)).on_press_tx(move |s: &mut App, tx| {
                s.edit_region(&key);
                tx.scroll_into_view(format!("region-{key}"));
            }));
        }
    }
    layer
}

/// The text box for a part of the ticket: dark ink on the cream of the paper, no frame. The text
/// is `multiline`; Enter ends a one-line field.
fn edit_input(s: &App, key: &str, size: f32, multiline: bool) -> El {
    let k = style::paper_kit();
    // The box this part writes into: its own for a field, the text itself for the body.
    let field = s.theme.fields().iter().find(|f| f.key == key);
    let edit = field.and_then(|f| s.theme_edits.get(f.key)).unwrap_or(&s.text);
    let mut style = k.ty.ui_compact;
    style.size = size;
    style.line_height = size + 8.0;
    let ink = k.c.text_base;
    let mut input = rune::text_input(edit, style)
        .on_input_key(print_key)
        .key("input")
        .w_full()
        .color(ink)
        .placeholder(field.map_or("Type something to print", |f| f.hint), k.c.text_faint)
        .caret(ink, k.selection());
    if !multiline {
        input = input.single_line().on_submit(|s: &mut App, _| s.stop_editing());
    }
    // Only keys the theme still has reach here (a region or a chip of the bar), so writing lands
    // in that field's box, or in the text for the body.
    let own = field.map_or("body", |f| f.key);
    input.on_edit(move |s: &mut App| s.region_edit_mut(own)).on_input_change(|s: &mut App, _| s.touch())
}

/// The box over the part of the ticket being written on. It is filled with the paper's cream, so
/// it covers what the ticket shows there (a silly form leaves that part blank itself).
fn write_box(s: &App, key: &str, x: f32, y: f32, w: f32, h: f32) -> El {
    let body = key == "body";
    let size = if body { 22.0 } else { (h * 0.62).clamp(15.0, 26.0) };
    el().key(format!("region-{key}")).absolute(x, y).size(w, h).radius(4.0).border(1.0, coral()).fill(style::rgb(0xfaf6ec)).child(edit_input(s, key, size, body))
}

/// For a part a theme draws in a way that cannot be read back (so it has no place on the paper):
/// a slim bar over the printer, under the ticket, with one chip for each such part and the text
/// box for the one picked. The ticket stays fully in view.
pub(crate) fn write_bar(s: &App) -> Option<El> {
    // The profile picture has its own picker in the Text panel; it is not a line to type on the paper.
    let unmapped: Vec<_> = s.unmapped().into_iter().filter(|(k, _)| *k != "avatar").collect();
    if unmapped.is_empty() {
        return None;
    }
    let k = style::paper_kit();
    let editing = s.editing.as_deref();
    let mut chips = row().gap(6.0).center_y().child(text("Write on".to_string()).font(type_::LABEL).color(style::rgb(0x6b6258)));
    for (key, label) in &unmapped {
        let key = key.to_string();
        let on = editing == Some(key.as_str());
        chips = chips.child(Button::new(label.to_string()).variant(if on { ButtonVariant::Contrast } else { ButtonVariant::Ghost }).on_press(move |s: &mut App| s.edit_region(&key)).build(k));
    }
    let mut bar = col().gap(6.0).px(12.0).py(8.0).radius(14.0).fill(style::rgb(0xfaf6ec)).border(1.0, coral().alpha(0.6)).shadow(4.0, 16.0, Color::BLACK.alpha(0.4)).child(chips);
    if let Some(key) = unmapped.iter().map(|(k, _)| *k).find(|k| editing == Some(*k)) {
        let body = key == "body";
        bar = bar.child(col().w_full().px(8.0).py(4.0).radius(8.0).border(1.0, Color::BLACK.alpha(0.16)).min_h(if body { 56.0 } else { 34.0 }).child(edit_input(s, key, 18.0, body)));
    }
    Some(col().key("write-bar").w_full().items_center().child(bar.w(430.0)))
}

