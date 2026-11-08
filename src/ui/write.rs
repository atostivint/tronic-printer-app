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
            // The frame sits outside the part, so it never crosses what the ticket printed there:
            // grown past the glyphs, faint, and inert so it cannot take the press that starts writing.
            let (bx, by, bw, bh) = outline_rect(x, y, w, h, full);
            layer = layer.child(
                el().key(format!("outline-{}", r.key)).absolute(bx, by).size(bw, bh).radius(7.0).border(1.0, coral().alpha(0.18)).inert(),
            );
            // The click box keeps the part's own size, but draws nothing: what is written on is the
            // part itself, not a frame around it.
            layer = layer.child(el().key(format!("region-{}", r.key)).absolute(x, y).size(w, h).radius(4.0).on_press_tx(move |s: &mut App, tx| {
                seed_box(s, &key);
                s.edit_region(&key);
                tx.scroll_into_view(format!("region-{key}"));
            }));
        }
    }
    layer
}

/// The faint frame round a part that can be written on: `GROW` points outside the part on every
/// side, so it clears the glyphs, and kept on the paper (`full` is the sheet's height in points).
fn outline_rect(x: f32, y: f32, w: f32, h: f32, full: f32) -> (f32, f32, f32, f32) {
    const GROW: f32 = 3.0;
    let (x0, y0) = ((x - GROW).max(0.0), (y - GROW).max(0.0));
    let (x1, y1) = ((x + w + GROW).min(render::PREVIEW_WIDTH as f32), (y + h + GROW).min(full));
    (x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))
}

/// A box opening on a part starts from what the ticket printed there: a generated default, or a
/// hint the theme used as ink (a shop's name), is on the paper and must not turn grey when it is
/// written on. Rune copies the app's `TextEdit` into the field at every commit, so the seed has to
/// go into the state, at the press that opens the box; a field the theme drew nothing on (a blank
/// line) keeps its placeholder.
fn seed_box(s: &mut App, key: &str) {
    let Some(f) = s.theme.fields().iter().find(|f| f.key == key) else { return };
    let empty = s.theme_edits.get(f.key).is_none_or(|e| e.display().0.trim().is_empty());
    if !empty {
        return;
    }
    if let Some(value) = s.theme.drawn_value(f.key) {
        s.edit_for(f.key).replace_text(value);
    }
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
        .caret(ink, k.selection());
    // An empty box over printed ink shows that ink, not a grey hint: it is what the ticket says
    // there (the press that opens the box also seeds the text, see `seed_box`).
    input = match s.theme.drawn_value(key).filter(|_| edit.display().0.trim().is_empty()) {
        Some(value) => input.placeholder(value, ink),
        None => input.placeholder(field.map_or("Type something to print", |f| f.hint), k.c.text_faint),
    };
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
        chips = chips.child(Button::new(label.to_string()).variant(if on { ButtonVariant::Contrast } else { ButtonVariant::Ghost }).on_press(move |s: &mut App| {
            seed_box(s, &key);
            s.edit_region(&key);
        }).build(k));
    }
    let mut bar = col().gap(6.0).px(12.0).py(8.0).radius(14.0).fill(style::rgb(0xfaf6ec)).border(1.0, coral().alpha(0.6)).shadow(4.0, 16.0, Color::BLACK.alpha(0.4)).child(chips);
    if let Some(key) = unmapped.iter().map(|(k, _)| *k).find(|k| editing == Some(*k)) {
        let body = key == "body";
        bar = bar.child(col().w_full().px(8.0).py(4.0).radius(8.0).border(1.0, Color::BLACK.alpha(0.16)).min_h(if body { 56.0 } else { 34.0 }).child(edit_input(s, key, 18.0, body)));
    }
    Some(col().key("write-bar").w_full().items_center().child(bar.w(430.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Theme;

    #[test]
    fn the_writable_frame_clears_its_glyphs_and_stays_on_the_paper() {
        // Three points outside the part on each side: the frame never sits on the ink.
        assert_eq!(outline_rect(20.0, 40.0, 100.0, 30.0, 800.0), (17.0, 37.0, 106.0, 36.0));
        // A part on the paper's edge grows inward only, so the frame cannot leave the sheet.
        let (x, y, w, h) = outline_rect(0.0, 0.0, render::PREVIEW_WIDTH as f32, 800.0, 800.0);
        assert_eq!((x, y, w, h), (0.0, 0.0, render::PREVIEW_WIDTH as f32, 800.0));
        // The right and bottom edges stop on the paper too.
        let (_, _, w, h) = outline_rect(450.0, 780.0, 20.0, 30.0, 800.0);
        assert_eq!((w, h), (17.0, 23.0));
    }

    #[test]
    fn a_box_on_a_generated_default_starts_from_its_value() {
        let mut app = App::default();
        app.set_theme(Theme::Train);
        app.text = rune::TextEdit::new(Theme::Train.sample());
        app.refresh();
        // The e-ticket prints its generated class: the box over it starts from that ink.
        seed_box(&mut app, "class");
        assert_eq!(app.edit_for("class").display().0, "2de");
        // A hint the theme printed is a value too: the receipt's default shop name.
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        app.refresh();
        seed_box(&mut app, "store");
        assert_eq!(app.edit_for("store").display().0, "FRESH MART");
        // A blank line the theme drew nothing on keeps its placeholder.
        let mut app = App::default();
        app.set_theme(Theme::Complaint);
        app.text = rune::TextEdit::new(Theme::Complaint.sample());
        app.refresh();
        seed_box(&mut app, "dept");
        assert_eq!(app.theme_edits.get("dept").map(|e| e.display().0.clone()), Some(String::new()));
    }
}
