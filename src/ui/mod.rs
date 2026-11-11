//! The window: a top bar, the stage (the printer and its ticket), and the drawer of galleries.

mod ghost;
mod write;

pub(crate) use ghost::*;
pub(crate) use write::*;

use rune::tok::{motion, type_};
use rune::{Color, Cx, Drag, El, FileDialogOptions, Phase, ScrollSpec, SurfaceFit, Tx, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Kit, Progress, ScrollBar, SegmentedControl, Switch, TextInput};

use crate::app::{App, DrawerSpace, Link, Mode};
use crate::fx;
use crate::jobs::clamp_copies;
use crate::protocol;
use crate::render::{self, Align, CodeKind, Dither, EcLevel, Family, FontChoice, Theme};
use crate::style::{self, amber, coral, dim, ink, mint, panel, pink, red};
use crate::update::{self, Update};
use crate::version;
use crate::voice;

/// Width of the side columns (the galleries on the left, the printer/settings on the right), in
/// points. The stage takes what is left (all of it when closed).
const SIDE_PANEL_W: f32 = 300.0;
/// Size of the Print key on the printer's case, and width of the four mode tabs in the top bar.
const PRINT_W: f32 = 140.0;
const PRINT_H: f32 = 42.0;
const TABS_W: f32 = 252.0;
/// The drawer's rhythm, in points: its own padding, the gap between two sections, the gap between
/// two rows of a section, and the room left under the last card so it is not glued to the edge.
const PAD: f32 = 16.0;
const SECTION_GAP: f32 = 16.0;
const ROW_GAP: f32 = 10.0;
const SCROLL_FOOT: f32 = 28.0;
/// The width a card had in the old drawer, kept for the layout tests that still measure against it.
#[cfg(test)]
const DRAWER: f32 = 360.0;
/// The width a control may draw on inside a card: the old drawer's padding and the card's, twice.
/// Only the layout tests measure against it now that the controls sit on the strip.
#[cfg(test)]
const COL_W: f32 = DRAWER - 4.0 * PAD;
/// The gap between two settings of the strip under the paper.
const STRIP_GAP: f32 = 16.0;
/// The build stamp is a footnote, smaller than any label.
const STAMP_FONT: rune::TextStyle = rune::TextStyle::new(10.5, 500.0);

pub fn kit() -> &'static Kit {
    style::kit()
}

/// A small, quiet, upper-case label naming a section or a setting.
fn caption(title: &str) -> El {
    text(title.to_uppercase()).font(type_::BODY).color(dim())
}

fn field(title: &str, body: El) -> El {
    col().gap(8.0).child(caption(title)).child(body)
}

/// A rounded panel that groups controls.
fn card(body: El) -> El {
    col().w_full().padding(PAD).radius(18.0).fill(panel()).border(1.0, Color::WHITE.alpha(0.06)).child(body)
}

/// One setting on one line: its name at the left, its control at the right. The label takes the
/// room the control leaves and wraps inside it rather than running under the control or being cut.
fn row_label(title: &str, tint: Color, control: El) -> El {
    row()
        .w_full()
        .gap(ROW_GAP)
        .center_y()
        .child(text(title.to_string()).font(type_::BODY).color(tint).wrap().grow(1.0).min_w(0.0))
        .child(control)
}

/// A switch and its name. The kit draws the switch's own label in its small faint style, glued to
/// the track, which read as a stray caption; hidden, it stays for the screen reader and the row
/// draws the name whole in the body face.
fn switch_row(title: &str, checked: bool, toggle: fn(&mut App)) -> El {
    row_label(title, ink(), Switch::new(checked).label(title).hide_label().on_toggle(toggle).build(kit()))
}

/// A segmented choice that takes only the room its labels need, so several of them fit on one
/// line of the settings strip. It may have nothing picked (the value lies between its choices).
fn choice_fit(labels: &[&str], selected: Option<usize>, pick: fn(&mut App, usize)) -> El {
    SegmentedControl::new(labels, selected).fit().on_pick(move |s: &mut App, i, _| pick(s, i)).build(kit())
}

/// One setting of the strip: its name in the small label face, its control right beside it.
fn strip_item(title: &str, control: El) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(text(title.to_string()).font(type_::LABEL).color(dim()))
        .child(control)
}

/// A switch and its name on one line of the strip, without the row growing to the stage's width.
fn strip_switch(title: &str, checked: bool, toggle: fn(&mut App)) -> El {
    strip_item(title, Switch::new(checked).label(title).hide_label().on_toggle(toggle).build(kit()))
}

/// Size of a theme's picture in the gallery, in points.
const THEME_W: f32 = 92.0;
const THEME_H: f32 = 118.0;
/// Size of a picture in the Image gallery: landscape, so a photo is not squashed.
const PIC_W: f32 = 118.0;
const PIC_H: f32 = 92.0;

/// The name under a tile: small, over two lines at most. A file name has no spaces to break on,
/// so it wraps anywhere rather than losing its end to an ellipsis on a single line.
const TILE_NAME: rune::TextStyle = rune::TextStyle::new(11.0, 500.0).lh(14.0);

/// What both pickers show for one choice: its picture in a box (lit coral when picked), a star in
/// the corner to favorite it, and its name underneath. `picture` is the box itself, already set up
/// to be clicked and dragged; the star sits beside it, not inside, so pressing it never starts a drag.
fn pick_tile(w: f32, h: f32, key: &str, name: &str, picked: bool, fav: bool, picture: El, on_star: impl Fn(&mut App) + 'static) -> El {
    let star = col()
        .key(format!("{key}-star"))
        .role("button")
        .label(if fav { format!("Remove {name} from favorites") } else { format!("Add {name} to favorites") })
        .focusable(true)
        .absolute(w - 24.0, 4.0)
        .size(20.0, 20.0)
        .radius(10.0)
        .center()
        .fill(Color::BLACK.alpha(if fav { 0.6 } else { 0.4 }))
        .on_press(on_star)
        .child(text(if fav { "★" } else { "☆" }.to_string()).font(type_::LABEL).color(if fav { amber() } else { Color::WHITE.alpha(0.7) }));
    col()
        .gap(4.0)
        .w(w)
        .child(col().w(w).h(h).child(picture).child(star))
        .child(text(name.to_string()).font(TILE_NAME).color(if picked { ink() } else { dim() }).wrap_anywhere().max_lines(2).w_full())
}

/// Whether `family` has a theme in `themes` (the gallery's list for one printer).
fn family_has_themes(themes: &[Theme], family: Family) -> bool {
    themes.iter().any(|t| t.family() == Some(family))
}

/// Whether the gallery draws `family`'s header at all: a group with nothing on this printer is
/// left out whole instead of naming itself over an empty row. "My themes" is the folder's own
/// panel and stays even empty — but only on the roll, since a custom theme is a pocket ticket.
fn family_is_shown(themes: &[Theme], family: Family, target: render::Target) -> bool {
    family_has_themes(themes, family) || (family == Family::Mine && target == render::Target::Pocket)
}

/// A gallery of the writing themes: a small picture of each ticket, grouped, as plain content in
/// the drawer. Click one to use it; pull it toward the printer to print your text in that
/// theme; star the ones you like and they gather in a Favorites group at the top.
fn theme_gallery(s: &App) -> El {
    let label_target = s.on_a_label_printer();
    let target = if label_target { render::Target::Label } else { render::Target::Pocket };
    let available = render::themes_for(target);
    if available.is_empty() && label_target {
        return col()
            .w_full()
            .gap(8.0)
            .child(caption("Themes"))
            .child(text("No label themes yet. Use Image for now.".to_string()).font(type_::LABEL).color(dim()).wrap());
    }
    // `scope` keeps the keys apart when a starred theme shows up twice (in Favorites and its family).
    let tile = |theme: Theme, scope: &str| {
        let picked = s.theme == theme;
        let dragging = s.drag.as_ref().is_some_and(|d| d.theme == Some(theme) && d.travel >= crate::app::DRAG_MIN);
        let key = format!("{scope}theme-{}", theme.id());
        let picture = el()
            .key(key.clone())
            .role("button")
            .label(theme.name().to_string())
            .focusable(true)
            .size(THEME_W, THEME_H)
            .radius(10.0)
            .fill(Color::BLACK.alpha(0.35))
            .border(if picked { 2.0 } else { 1.0 }, if picked { coral() } else { Color::WHITE.alpha(0.08) })
            .opacity(if dragging { 0.35 } else { 1.0 })
            .on_drag(move |s: &mut App, d: &mut Drag, _: &mut Tx| match d.phase {
                Phase::Down => s.theme_drag_start(theme),
                Phase::Move => s.drag_move(d.dx, d.dy),
                Phase::Up => s.drag_end(d.cancelled()),
            });
        let picture = match s.theme_thumbs.get(&theme) {
            Some(image) => picture.pixels_fit(image.pixels(), SurfaceFit::Fit),
            None => picture,
        };
        pick_tile(THEME_W, THEME_H, &key, theme.name(), picked, s.is_fav_theme(theme), picture, move |s: &mut App| s.toggle_fav_theme(theme))
    };
    let mut grid = col().w_full().gap(12.0);
    let favorites: Vec<Theme> = s.favorite_themes().into_iter().filter(|t| t.target() == target).collect();
    if !favorites.is_empty() {
        let mut tiles = row().flex_wrap().gap(10.0);
        for theme in &favorites {
            tiles = tiles.child(tile(*theme, "fav-"));
        }
        grid = grid.child(text("★  Favorites".to_string()).font(type_::LABEL).color(amber())).child(tiles);
    }
    // The plain ticket leads the first group instead of taking a row of its own.
    let mut plain_left = available.contains(&Theme::Plain);
    for family in Family::ALL {
        // A family with nothing for this printer keeps no header: a label printer was being shown
        // every empty pocket group, and the roll printer a "Labels" one.
        if !family_is_shown(&available, family, target) {
            continue;
        }
        let themes: Vec<Theme> = available.iter().copied().filter(|t| t.family() == Some(family)).collect();
        let mut tiles = row().flex_wrap().gap(10.0);
        let mut label = family.label().to_string();
        if plain_left {
            tiles = tiles.child(tile(Theme::Plain, ""));
            label = format!("Plain  ·  {label}");
            plain_left = false;
        }
        for theme in &themes {
            tiles = tiles.child(tile(*theme, ""));
        }
        if family == Family::Mine {
            // The user's own themes: files in a folder, added and removed without recompiling.
            let header = row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(text(label).font(type_::LABEL).color(dim()))
                .child(row().grow(1.0))
                .child(Button::new("Open folder").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.open_themes_folder()).build(kit()))
                .child(Button::new("Reload").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.reload_themes()).build(kit()));
            grid = grid.child(header);
            if themes.is_empty() {
                grid = grid.child(text("Drop a .json theme file in the folder to add your own; delete it to remove it.".to_string()).font(type_::LABEL).color(dim()).wrap());
            } else {
                grid = grid.child(tiles);
            }
            for problem in s.theme_report.errors.iter().take(3) {
                grid = grid.child(text(problem.clone()).font(type_::LABEL).color(coral()).wrap());
            }
            continue;
        }
        grid = grid.child(text(label).font(type_::LABEL).color(dim())).child(tiles);
    }
    // Plain content, no window of its own: the drawer scrolls it with the rest of its column, and
    // two nested scroll bars would fight for the wheel.
    col().w_full().gap(8.0).child(caption("Themes")).child(grid)
}

/// The profile picture of a theme that shows one (the tweet): a file, not a line on the paper, so
/// it keeps a picker of its own in the settings strip.
fn avatar_pick(s: &App) -> Option<El> {
    let label = s.theme.fields().iter().find(|f| f.key == "avatar")?.label;
    let path = s.theme_edits.get("avatar").map(|e| e.display().0.trim().to_string()).unwrap_or_default();
    let name = path.rsplit(['\\', '/']).next().unwrap_or("").to_string();
    let choose = if path.is_empty() { "Choose…" } else { "Change…" };
    Some(
        strip_item(
            label,
            row()
                .gap(8.0)
                .center_y()
                .child(
                    Button::new(choose)
                        .on_press_tx(move |_s: &mut App, tx: &mut Tx| {
                            let dialog = FileDialogOptions { title: "Choose a profile picture".into(), ..Default::default() }
                                .filter("Pictures", &["png", "jpg", "jpeg", "bmp", "gif", "webp"]);
                            tx.open_file(dialog, move |s: &mut App, result, _| {
                                // Errors are dropped here, so a dialog already showing (Rune's
                                // "already open") is a no-op already, and a picker that cannot
                                // open leaves the avatar as it was.
                                if let Ok(Some(path)) = result {
                                    s.edit_for("avatar").replace_text(path.to_string_lossy().to_string());
                                    s.touch();
                                }
                            });
                        })
                        .build(kit()),
                )
                .child(
                    Button::new("Clear")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut App| {
                            s.edit_for("avatar").replace_text("");
                            s.touch();
                        })
                        .build(kit()),
                )
                .child(
                    text(if name.is_empty() { "a ring with the initials".to_string() } else { name })
                        .font(type_::LABEL)
                        .color(dim())
                        .max_w(120.0)
                        .truncate(),
                ),
        ),
    )
}

/// What the panel still holds of a theme's little form: the switches that change how a silly form
/// is drawn (Shuffle, Blank form, the closing line). They are not text on the ticket, so they have
/// no place on the paper. Every field itself is written on the ticket now (the bar catches the few
/// parts a theme cannot say where they are).
fn form_switches(s: &App) -> Option<El> {
    let mut out = col().gap(ROW_GAP);
    let mut any = false;
    if s.theme.shuffles() {
        let mut buttons = row().gap(8.0).center_y().child(Button::new("Shuffle").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.shuffle_form()).build(kit()));
        if s.theme.is_form() {
            buttons = buttons.child(Button::new("Blank form").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.blank_form()).build(kit()));
        }
        out = out.child(buttons);
        any = true;
    }
    if s.theme.is_form() {
        out = out.child(switch_row("Closing line", s.closing_line, |s: &mut App| {
            s.closing_line = !s.closing_line;
            s.touch();
        }));
        any = true;
    }
    any.then_some(out)
}

/// The Text settings, on the strip under the paper. Font, size, alignment and white on black
/// only dress a plain sheet; a themed ticket draws its own type, so its line is a reminder of
/// what the theme is for instead. The profile picture a theme asks for (the tweet) rides along.
fn text_settings(s: &App) -> El {
    let o = s.text_opts;
    let mut out = row().w_full().gap(STRIP_GAP).center_y().flex_wrap();
    if s.theme == Theme::Plain {
        let font_ix = FontChoice::ALL.iter().position(|f| *f == o.font).unwrap_or(0);
        let size_ix = [22.0, 32.0, 48.0, 72.0].iter().position(|p| *p == o.px).unwrap_or(1);
        let align_ix = [Align::Left, Align::Center, Align::Right].iter().position(|a| *a == o.align).unwrap_or(0);
        out = out
            .child(strip_item(
                "Font",
                choice_fit(&FontChoice::ALL.map(FontChoice::label), Some(font_ix), |s, i| {
                    s.text_opts.font = FontChoice::ALL[i];
                    s.touch();
                }),
            ))
            .child(strip_item(
                "Size",
                choice_fit(&["S", "M", "L", "XL"], Some(size_ix), |s, i| {
                    s.text_opts.px = [22.0, 32.0, 48.0, 72.0][i];
                    s.touch();
                }),
            ))
            .child(strip_item(
                "Align",
                choice_fit(&["Left", "Center", "Right"], Some(align_ix), |s, i| {
                    s.text_opts.align = [Align::Left, Align::Center, Align::Right][i];
                    s.touch();
                }),
            ))
            .child(strip_switch("White on black", o.invert, |s: &mut App| {
                s.text_opts.invert = !s.text_opts.invert;
                s.touch();
            }));
    } else {
        out = out.child(text(s.theme.tagline().to_string()).font(type_::LABEL).color(dim()).truncate());
    }
    if let Some(pick) = avatar_pick(s) {
        out = out.child(pick);
    }
    out
}

/// Thumbnails per row in the gallery, and their size in points.
const THUMB: f32 = 80.0;

/// Whether a file dialog error only means one was already showing. Rune allows a single dialog
/// and answers a second request with this error instead of queueing it, so a slow native dialog
/// (or a second click on the button) is not a failure: it deserves no banner.
fn is_busy_dialog(message: &str) -> bool {
    message.contains("already open")
}

/// A folder of pictures to pick from, and the switch that prints new arrivals by itself.
fn gallery_section(s: &App) -> El {
    let g = &s.gallery;
    let folder = g.folder_name().unwrap_or_else(|| "No folder yet".into());
    let head = row()
        .gap(8.0)
        .center_y()
        .child(text(folder).font(type_::BODY).color(if g.dir.is_some() { ink() } else { dim() }).grow(1.0).min_w(0.0).truncate())
        .child(
            Button::new("File…")
                .on_press_tx(|_: &mut App, tx: &mut Tx| {
                    let options = FileDialogOptions { title: "Choose a picture".into(), ..Default::default() }
                        .filter("Pictures", &["png", "jpg", "jpeg", "bmp", "gif", "webp"]);
                    tx.open_file(options, |s: &mut App, result, _| match result {
                        Ok(Some(path)) => s.choose_picture(path),
                        Ok(None) => {}
                        Err(e) => {
                            let why = e.to_string();
                            if !is_busy_dialog(&why) {
                                s.error = Some(why);
                            }
                        }
                    });
                })
                .build(kit()),
        )
        .child(
            Button::new(if g.dir.is_some() { "Folder…" } else { "Choose folder…" })
                .on_press_tx(|_: &mut App, tx: &mut Tx| {
                    let options = FileDialogOptions { title: "Choose your picture folder".into(), ..Default::default() };
                    tx.open_folder(options, |s: &mut App, result, _| match result {
                        Ok(Some(dir)) => s.set_gallery_dir(dir),
                        Ok(None) => {}
                        Err(e) => {
                            let why = e.to_string();
                            if !is_busy_dialog(&why) {
                                s.error = Some(why);
                            }
                        }
                    });
                })
                .build(kit()),
        );
    let mut body = col().w_full().gap(ROW_GAP).child(head);
    if g.dir.is_none() {
        body = body.child(
            col().w_full().h(84.0).center().radius(12.0).border_dash(6.0, 5.0).border(1.0, Color::WHITE.alpha(0.16)).gap(2.0)
                .child(text("Point me at your meme folder".to_string()).font(type_::BODY).color(ink()))
                .child(text("or drop a picture on the window to preview it".to_string()).font(type_::LABEL).color(dim())),
        );
    } else if g.entries.is_empty() {
        body = body.child(text("No pictures in this folder yet.".to_string()).font(type_::LABEL).color(dim()));
    } else {
        // The same tiles as the theme picker: picture, star, name; starred pictures gather in a
        // Favorites group above the rest.
        let tile = |e: &crate::gallery::Entry, scope: &str| {
            let picked = g.selected.as_ref() == Some(&e.path);
            let path = e.path.clone();
            let key = format!("{scope}thumb-{}", e.name);
            let picture = el()
                .key(key.clone())
                .role("button")
                .label(e.name.clone())
                .focusable(true)
                .size(PIC_W, PIC_H)
                .radius(10.0)
                .fill(Color::BLACK.alpha(0.35))
                .border(if picked { 2.0 } else { 1.0 }, if picked { coral() } else { Color::WHITE.alpha(0.08) })
                .opacity(if s.drag.as_ref().is_some_and(|d| d.path == e.path && d.travel >= crate::app::DRAG_MIN) { 0.35 } else { 1.0 })
                // A short press picks the picture; pulling it toward the printer prints it.
                .on_drag(move |s: &mut App, d: &mut Drag, _: &mut Tx| match d.phase {
                    Phase::Down => s.drag_start(path.clone()),
                    Phase::Move => s.drag_move(d.dx, d.dy),
                    Phase::Up => s.drag_end(d.cancelled()),
                });
            let picture = match g.thumbs.get(&e.path) {
                Some(image) => picture.pixels_fit(image.pixels(), SurfaceFit::Fit),
                None => picture,
            };
            let path = e.path.clone();
            pick_tile(PIC_W, PIC_H, &key, &e.name, picked, g.is_favorite(&e.path), picture, move |s: &mut App| s.toggle_fav_picture(path.clone()))
        };
        let favorites = g.favorite_entries();
        let recent = g.recent_entries();
        let mut grid = col().w_full().gap(14.0);
        if !favorites.is_empty() {
            let mut tiles = row().flex_wrap().gap(12.0);
            for e in &favorites {
                tiles = tiles.child(tile(e, "fav-"));
            }
            grid = grid.child(text("★  Favorites".to_string()).font(type_::LABEL).color(amber())).child(tiles);
        }
        if !recent.is_empty() {
            let mut tiles = row().flex_wrap().gap(12.0);
            for e in &recent {
                tiles = tiles.child(tile(e, ""));
            }
            if !favorites.is_empty() {
                grid = grid.child(text("Newest".to_string()).font(type_::LABEL).color(dim()));
            }
            grid = grid.child(tiles);
        }
        // Plain content, no window of its own: the drawer scrolls the gallery with the settings.
        body = body.child(grid);
        let more = g.entries.len().saturating_sub(favorites.len() + recent.len());
        if more > 0 {
            body = body.child(text(format!("+ {more} older pictures in the folder, newest shown first")).font(type_::LABEL).color(dim()));
        }
    }
    if g.dir.is_some() {
        let waiting = g.queue.len();
        let note = match (g.auto, waiting, g.auto_printed) {
            (false, _, _) => "Pictures you add to the folder are left alone.".to_string(),
            (true, 0, 0) => "Watching. Drop a picture in the folder and it prints.".to_string(),
            (true, 0, n) => format!("Watching. {n} printed so far."),
            (true, w, _) => format!("{w} waiting for the printer…"),
        };
        body = body
            .child(switch_row("Print new pictures automatically", g.auto, |s: &mut App| s.set_auto_print(!s.gallery.auto)))
            .child(text(note).font(type_::LABEL).color(if g.auto { coral() } else { dim() }).truncate());
    }
    col().w_full().gap(8.0).child(caption("Gallery")).child(body)
}

/// A horizontal slider from -1 to +1, shown as a percentage. Drag it, click on the track, or use
/// the arrow keys when it has focus. `w` is the whole track, knob included.
fn slider_track(key: &'static str, title: &str, value: f32, set: fn(&mut App, f32), w: f32) -> El {
    const H: f32 = 22.0;
    const KNOB: f32 = 16.0;
    const BAR: f32 = 4.0;
    let travel = w - KNOB;
    let u = ((value + 1.0) / 2.0).clamp(0.0, 1.0);
    let at = move |x: f32| (((x - KNOB / 2.0) / travel).clamp(0.0, 1.0) * 2.0 - 1.0).clamp(-1.0, 1.0);
    let snap = |v: f32| (v * 100.0).round() / 100.0;
    el()
        .key(key)
        .role("slider")
        .label(title.to_string())
        .focusable(true)
        .size(w, H)
        .motion(motion::SNAPPY)
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| {
            match d.phase {
                Phase::Down | Phase::Move => {
                    set(s, snap(at(d.x)));
                    tx.instant();
                }
                Phase::Up => {}
            }
        })
        .on_step(move |s: &mut App, dir: i32, _: &mut Tx| set(s, snap((value + dir as f32 * 0.05).clamp(-1.0, 1.0))))
        .child(el().key("track").size(w, BAR).absolute(0.0, (H - BAR) / 2.0).radius(BAR / 2.0).fill(Color::WHITE.alpha(0.12)))
        .child(el().key("fill").size(KNOB / 2.0 + u * travel, BAR).absolute(0.0, (H - BAR) / 2.0).radius(BAR / 2.0).gradient(coral(), pink()))
        .child(el().key("knob").size(KNOB, KNOB).absolute(u * travel, (H - KNOB) / 2.0).radius(KNOB / 2.0).fill(Color::WHITE).shadow(1.0, 4.0, Color::BLACK.alpha(0.5)))
}

/// A slider on one line of the strip: its name, the track, and the value it holds.
fn slider_inline(key: &'static str, title: &str, value: f32, set: fn(&mut App, f32)) -> El {
    const W: f32 = 80.0;
    strip_item(
        title,
        row()
            .gap(8.0)
            .center_y()
            .child(slider_track(key, title, value, set, W))
            .child(text(format!("{:+.0}%", value * 100.0)).font(type_::LABEL).color(dim()).w(32.0).truncate()),
    )
}

/// The three looks most pictures want, so the dithering names stay out of the way.
const LOOKS: [(&str, Dither); 4] = [("Photo", Dither::FloydSteinberg), ("Meme", Dither::Atkinson), ("Line art", Dither::Threshold), ("Soft", Dither::Ordered)];

/// The Image settings, on the strip under the paper. The look, the two tones sliders, the white
/// space before the picture, and the four ways to turn it.
///
/// A label printer has no paper to space before the picture, and its 14:30 window is always
/// applied by the render: those two controls would only lie there, so they stay off its strip.
fn image_settings(s: &App) -> El {
    let o = s.pic_opts;
    let on_label = s.on_a_label_printer();
    let look_ix = LOOKS.iter().position(|(_, d)| *d == o.dither).unwrap_or(0);
    let crop_on = o.crop.is_some();
    let (cx, cy) = o.crop.map(|c| (c.x, c.y)).unwrap_or((0.5, 0.5));
    let mut out = row()
        .w_full()
        .gap(STRIP_GAP)
        .center_y()
        .flex_wrap()
        .child(strip_item(
            "Look",
            choice_fit(&LOOKS.map(|(n, _)| n), Some(look_ix), |s, i| {
                s.pic_opts.dither = LOOKS[i].1;
                s.touch();
            }),
        ))
        .child(slider_inline("brightness", "Brightness", o.brightness, |s, v| {
            s.pic_opts.brightness = v;
            s.touch();
        }))
        .child(slider_inline("contrast", "Contrast", o.contrast, |s, v| {
            s.pic_opts.contrast = v;
            s.touch();
        }));
    if !on_label {
        out = out.child(strip_item(
            "Space",
            row()
                .gap(8.0)
                .center_y()
                .child(Button::new("−").on_press(|s: &mut App| {
                    s.pic_opts.space_before = s.pic_opts.space_before.saturating_sub(1);
                    s.touch();
                }).build(kit()))
                .child(text(format!("{} mm", o.space_before)).font(type_::LABEL).color(ink()).w(50.0).truncate())
                .child(Button::new("+").on_press(|s: &mut App| {
                    s.pic_opts.space_before = (s.pic_opts.space_before + 1).min(render::MAX_SPACE_MM);
                    s.touch();
                }).build(kit()))
                .child(choice_fit(&["0", "5", "10", "20"], [0u8, 5, 10, 20].iter().position(|v| *v == o.space_before), |s, i| {
                    s.pic_opts.space_before = [0u8, 5, 10, 20][i];
                    s.touch();
                })),
        ));
    }
    out = out.child(
        row()
            .gap(8.0)
            .center_y()
            .child(Button::new("Rotate").on_press(|s: &mut App| {
                s.pic_opts.rotate = (s.pic_opts.rotate + 1) % 4;
                s.touch();
            }).build(kit()))
            .child(Button::new("Flip").on_press(|s: &mut App| {
                s.pic_opts.flip_h = !s.pic_opts.flip_h;
                s.touch();
            }).build(kit()))
            .child(Button::new("Reset").variant(ButtonVariant::Ghost).on_press(|s: &mut App| {
                s.pic_opts = Default::default();
                s.touch();
            }).build(kit()))
            .child(strip_switch("Invert", o.invert, |s: &mut App| {
                s.pic_opts.invert = !s.pic_opts.invert;
                s.touch();
            })),
    );
    // Label crop (DP-L13, 30×14): the pocket switch that turns the window on. A label render
    // always crops to that shape (`label_prepare`), so the switch has nothing to toggle there.
    if !on_label {
        out = out.child(strip_switch("Label 30×14", crop_on, |s: &mut App| {
            if s.pic_opts.crop.is_some() {
                s.pic_opts.crop = None;
            } else {
                s.pic_opts.crop = Some(render::Crop::centered(30.0 / 14.0));
            }
            s.touch();
        }));
    }
    if crop_on {
        out = out.child(slider_inline("crop_x", "Crop X", cx * 2.0 - 1.0, |s, v| {
            let prev = s.pic_opts.crop.unwrap_or_else(|| render::Crop::centered(30.0 / 14.0));
            s.pic_opts.crop = Some(render::Crop {
                aspect: prev.aspect,
                x: ((v + 1.0) / 2.0).clamp(0.0, 1.0),
                y: prev.y,
            });
            s.touch();
        }));
        out = out.child(slider_inline("crop_y", "Crop Y", cy * 2.0 - 1.0, |s, v| {
            let prev = s.pic_opts.crop.unwrap_or_else(|| render::Crop::centered(30.0 / 14.0));
            s.pic_opts.crop = Some(render::Crop {
                aspect: prev.aspect,
                x: prev.x,
                y: ((v + 1.0) / 2.0).clamp(0.0, 1.0),
            });
            s.touch();
        }));
    }
    if s.on_a_label_printer() && s.mode == Mode::Image {
        out = out.child(strip_item(
            "Place",
            text("Drag the picture on the paper to place it").font(type_::LABEL).color(dim()).wrap(),
        ));
    }
    out
}

/// The Codes settings, on the strip under the paper. The code itself is typed on its sheet.
fn code_settings(s: &App) -> El {
    let o = s.code_opts;
    let kind_ix = CodeKind::ALL.iter().position(|k| *k == o.kind).unwrap_or(0);
    let ec_ix = EcLevel::ALL.iter().position(|e| *e == o.ec).unwrap_or(1);
    let mut out = row()
        .w_full()
        .gap(STRIP_GAP)
        .center_y()
        .flex_wrap()
        .child(strip_item(
            "Type",
            choice_fit(&CodeKind::ALL.map(CodeKind::label), Some(kind_ix), |s, i| {
                s.code_opts.kind = CodeKind::ALL[i];
                s.touch();
            }),
        ));
    if o.kind == CodeKind::Qr {
        out = out.child(strip_item(
            "Correction",
            choice_fit(&EcLevel::ALL.map(EcLevel::label), Some(ec_ix), |s, i| {
                s.code_opts.ec = EcLevel::ALL[i];
                s.touch();
            }),
        ));
    }
    out.child(strip_switch("Text underneath", o.caption, |s: &mut App| {
        s.code_opts.caption = !s.code_opts.caption;
        s.touch();
    }))
}

/// The Terminal settings, on the strip under the paper: how big its lines print, and whether a
/// live command prints as it runs.
fn terminal_settings(s: &App) -> El {
    row()
        .w_full()
        .gap(STRIP_GAP)
        .center_y()
        .flex_wrap()
        .child(strip_item(
            "Text size",
            choice_fit(&["Small", "Medium", "Large"], Some(s.term.size), |s, i| {
                s.term.size = i;
                s.touch();
            }),
        ))
        .child(strip_switch("Print live output", s.term.auto, |s: &mut App| {
            s.term.auto = !s.term.auto;
            s.touch();
        }))
}

/// The settings of the mode, on one line under the paper (two when the window is narrow): they
/// belong to the page, not to the drawer of galleries. Built in the same widgets and handlers the
/// drawer used, only laid out beside their names instead of in a card.
fn settings_strip(s: &App) -> El {
    match s.mode {
        Mode::Text => text_settings(s),
        Mode::Image => image_settings(s),
        Mode::Codes => code_settings(s),
        Mode::Terminal => terminal_settings(s),
    }
}

/// How to let an AI assistant print: copy the command for Claude Code, or the JSON for Claude
/// Desktop, Cursor and the others. The paths are those of the program that is running.
fn mcp_section(s: &App) -> El {
    let listening = crate::mcp_http::LISTENING.load(std::sync::atomic::Ordering::Relaxed);
    let (state, state_color) = if listening { ("Listening", dim()) } else { ("Port busy: pick another", coral()) };
    let exe = crate::mcp_setup::mcp_exe();
    let (code_cmd, json) = (crate::mcp_setup::claude_code_command(&exe), crate::mcp_setup::json_config(&exe));
    let (http_url, http_cmd, http_json) = (crate::mcp_setup::http_url(), crate::mcp_setup::claude_code_http_command(), crate::mcp_setup::json_http_config());
    let copy = |label: &'static str, what: String, note: &'static str| {
        Button::new(label)
            .on_press_tx(move |s: &mut App, tx: &mut Tx| {
                tx.copy(what.clone());
                s.status = note.into();
                s.touch();
            })
            .build(kit())
    };
    field(
        "From an AI assistant (MCP)",
        col()
            .gap(8.0)
            .child(text("Let Claude or another assistant print: tools to print text, QR codes, pictures and check the printer.".to_string()).font(type_::LABEL).color(dim()).wrap())
            .child(text(format!("While the app is open it is an MCP server at {http_url} (this computer only).")).font(type_::LABEL).color(dim()).wrap())
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(text("Port".to_string()).font(type_::LABEL).color(dim()))
                    .child(
                        TextInput::new(&s.mcp_port)
                            .placeholder("7391")
                            .width(None)
                            .on_key(print_key)
                            .on_edit(|s: &mut App| &mut s.mcp_port)
                            .build(kit())
                            .w(110.0),
                    )
                    .child(Button::new("Apply").on_press(|s: &mut App| s.apply_mcp_port()).build(kit())),
            )
            // The state line used to end the row above and run past the card: it gets a line of
            // its own, whole ("Port busy: pick another" was cut to "pick a").
            .child(text(state.to_string()).font(type_::LABEL).color(state_color).wrap())
            .child(
                row()
                    .gap(8.0)
                    .child(copy("Copy URL", http_url, "MCP URL copied: paste it in any app that takes an MCP server URL"))
                    .child(copy("Copy command", http_cmd, "Claude Code command copied: paste it in a terminal"))
                    .child(copy("Copy JSON", http_json, "JSON config copied: paste it in your assistant's MCP settings")),
            )
            .child(text("Or run it through the helper program (stdio):".to_string()).font(type_::LABEL).color(dim()))
            .child(
                row()
                    .gap(8.0)
                    .child(copy("Copy stdio command", code_cmd, "Claude Code command copied: paste it in a terminal"))
                    .child(copy("Copy stdio JSON", json, "JSON config copied: paste it in your assistant's MCP settings")),
            )
            .child(text("Claude Desktop, Cursor and others take the JSON. Run tronic-mcp --config for the full steps.".to_string()).font(type_::LABEL).color(dim()).wrap()),
    )
}

/// What the Terminal drawer still holds: how to feed it from a command line and from an assistant,
/// and what the live session is doing. The size and the live switch moved to the strip.
fn terminal_info(s: &App) -> El {
    let usage = |line: &str| {
        row().px(10.0).h(30.0).center_y().radius(8.0).fill(Color::BLACK.alpha(0.35)).child(text(line.to_string()).font(type_::BODY).color(coral()))
    };
    let state = if s.term.live { "Live session connected" } else { "No live session" };
    col()
        .gap(SECTION_GAP)
        .child(field(
            "From a terminal",
            col()
                .gap(4.0)
                .child(usage("dir | tronic-print"))
                .child(usage("tronic-print -- git log --oneline -5"))
                .child(usage("tronic-print --live -- cargo build"))
                .child(text("Keep the app open and connected.".to_string()).font(type_::LABEL).color(dim())),
        ))
        .child(mcp_section(s))
        .child(row_label(
            &format!("{state}  ·  {} lines, {} not printed", s.term.total(), s.term.unprinted()),
            dim(),
            Button::new("Clear").variant(ButtonVariant::Ghost).on_press(|s: &mut App| {
                s.term.clear();
                s.touch();
            }).build(kit()),
        ))
}

/// The big glowing button. Disabled, it fades into the panel; ready, it crackles with plasma, and
/// pressing it sends a shockwave through the button. The ticket's family picks the colour of the
/// halo and the lightning.
/// How far the Print button's glow reaches past its edge, in points. Small on purpose: on the case
/// it may not spill onto the ticket above, and beside the case it may not reach the settings strip.
const HALO: f32 = 4.0;

/// The family of the ticket on the paper, when a ticket is what is being printed. The other tabs
/// keep the palette's weather: their theme is not on screen.
fn ticket_family(s: &App) -> Option<Family> {
    if s.mode == Mode::Text { s.theme.family() } else { None }
}

/// The connection pill: the battery gauge, its percentage and the four signal bars.
const PILL_W: f32 = 158.0;
const PILL_H: f32 = 28.0;

/// Where the pair sat on the case before it went to the stage's foot: the pill's left edge (the old
/// `PILL_X`) and the key's top (the old `KEY_Y`), from `a902df5`. The two travel together now, so
/// these are the travelling row's top-left corner: the pair lands where it always did.
const KEY_X: f32 = 58.0;
const KEY_Y: f32 = 146.0;
/// The gap between the pill and the key inside that row.
const CONTROLS_GAP: f32 = 10.0;
/// Below this scale the case is too small to wear the pair legibly (a long ticket shrinks it):
/// they stand beside it, at full size, instead.
const KEY_ON_CASE_MIN: f32 = 0.85;
/// How far the pair stands from the case when it is beside it.
const KEY_BESIDE_GAP: f32 = 12.0;

/// The travelling row's own width at full size.
fn controls_w() -> f32 {
    PILL_W + CONTROLS_GAP + PRINT_W
}

/// Where the controls go in the stage: the top-left corner of their row, and the scale to draw it
/// at. On the case at `KEY_X`/`KEY_Y`, scaled with it, while the case can wear them; otherwise
/// beside it at full size on its left — and never outside the stage. `None` until the stage has
/// been laid out once. A label printer has its controls in the flow under its turned case
/// (`label_controls`), so this is the pocket stage's own placement.
fn controls_place(stage_w: Option<f32>, drop_h: Option<f32>, k: f32, turned: bool) -> Option<(f32, f32, f32)> {
    let (stage_w, drop_h) = (stage_w?, drop_h?);
    let w = stage_w - 2.0 * STAGE_PAD;
    let case_left = w / 2.0 - MACHINE_W * k / 2.0;
    let case_top = drop_h - MACHINE_H * k;
    let (x, y, scale) = if !turned && k >= KEY_ON_CASE_MIN {
        (case_left + KEY_X * k, case_top + KEY_Y * k, k)
    } else {
        // Beside the case: full size, the middle of its front, its foot clear of the stage's edge.
        // A turned ticket has its case on the right (the group turns anticlockwise), so the left
        // is free for them there too.
        let x = case_left - KEY_BESIDE_GAP - controls_w();
        let y = drop_h - (MACHINE_H - SLIT_FRONT) * k / 2.0 - PRINT_H / 2.0;
        (x, y, 1.0)
    };
    // Never outside the stage, whatever the window: the pair stays whole and reachable.
    let max_x = (w - controls_w() * scale - 4.0).max(4.0);
    let max_y = (drop_h - PRINT_H * scale - 4.0).max(4.0);
    Some((x.clamp(4.0, max_x), y.clamp(4.0, max_y), scale))
}

/// The Print key. On the case its disabled face is a light grey with dark lettering (the palette's
/// dim is made for dark panels and vanishes on the white shell); beside the case, on the stage's
/// dark floor, it keeps the faint light face.
fn hero_button(s: &App, label: String, on_case: bool) -> El {
    // A label printer takes a picture, and one of its own label themes typed on the Text tab:
    // the key stays dark for the pocket themes and for the other tabs.
    let label_blocked = label_online(s)
        && !((s.mode == Mode::Image && s.source.is_some()) || (s.mode == Mode::Text && s.theme.target() == render::Target::Label));
    let enabled = s.can_print() && !label_blocked;
    let help = if label_blocked {
        voice::label_pictures_only().to_string()
    } else {
        format!("Print  ·  {}", App::print_shortcut_label())
    };
    let (tint, power) = fx::press_style(ticket_family(s));
    let glyph = if enabled {
        style::on_accent()
    } else if on_case {
        Color::BLACK.alpha(0.35)
    } else {
        dim().alpha(0.7)
    };
    let b = row()
        .key("print")
        .role("button")
        .label(label.clone())
        .focusable(true)
        .w(PRINT_W)
        .h(PRINT_H)
        // The pill's cap is half its height: keep the glyphs that far from either end so a long
        // label, cut by `.truncate()`, never runs under the curve.
        .px(18.0)
        .gap(8.0)
        .center()
        .radius(PRINT_H / 2.0)
        // The label grows to the key's inner width, then centres its glyphs in that box. Without
        // `.align` a grown text draws at the start of its box, which left the label against the cap.
        .child(print_glyph(glyph))
        .child(
            text(label)
                .font(type_::TITLE)
                .color(if enabled { style::on_accent() } else if on_case { Color::BLACK.alpha(0.42) } else { dim() })
                .align(rune::Align::Center)
                .truncate()
                .grow(1.0)
                .min_w(0.0),
        )
        .help(help);
    let b = if enabled {
        b.gradient(style::grad_a(), style::grad_b()).shadow(6.0, 22.0, pink().alpha(if style::pal().glow { 0.45 } else { 0.0 })).on_press(|s: &mut App| {
            s.print();
            // The ripple is drawn from `press_at` on this same frame: ask for the redraw now.
            s.touch();
        })
    } else if on_case {
        b.fill(Color::BLACK.alpha(0.10)).border(1.0, Color::BLACK.alpha(0.14))
    } else {
        b.fill(Color::WHITE.alpha(0.10)).border(1.0, Color::WHITE.alpha(0.18))
    };
    if !s.fx_on {
        return b;
    }
    // Slots overlay the key: a soft neon halo when it is ready, thin lightning while it is
    // pressed or printing, and a short shockwave just after it was pressed. Every effect stays
    // inside the key's box and its few points of bleed, over the case.
    let mut wrap = col().w(PRINT_W).h(PRINT_H).child(b);
    let pressed = s.press_at.is_some_and(|t| s.clock - t < 0.6);
    if style::pal().glow {
        if enabled {
            wrap = wrap.child(fx::slot(fx::GLOW, [0.45, tint, 0.4, 8.0]).absolute(0.0, 0.0).w_full().h(PRINT_H).radius(PRINT_H / 2.0).effect_bleed(HALO));
        }
        // The lightning is the job's own feedback, so it also runs while the key is disabled by
        // a job in flight. Intensity and colour come from the ticket's family.
        if pressed || s.printing.is_busy() {
            let strength = if pressed { 1.2 } else { 0.7 } * power;
            wrap = wrap.child(fx::slot(fx::ELECTRIC, [strength, 1.5, 0.0, tint]).absolute(0.0, 0.0).w_full().h(PRINT_H).radius(PRINT_H / 2.0).effect_bleed(HALO));
        }
    }
    if let Some(t0) = s.press_at.filter(|t| style::pal().glow && s.clock - t < 1.0) {
        // A ripple that stays on the key and its few points of bleed: it used to cross the
        // whole left panel.
        wrap = wrap.child(fx::slot(fx::SHOCK, [0.0, 0.0, t0 as f32, 5.0]).effect_backdrop(true).absolute(0.0, 0.0).w_full().h(PRINT_H).radius(PRINT_H / 2.0).effect_bleed(6.0));
    }
    wrap
}

/// The little printer mark on the key: a sheet leaving a body, with the tray under it. Drawn with
/// blocks rather than a font glyph, which no platform is sure to have.
fn print_glyph(color: Color) -> El {
    el().size(16.0, 15.0)
        .child(el().absolute(4.0, 0.0).size(8.0, 5.0).radius(1.0).fill(color))
        .child(el().absolute(0.0, 4.0).size(16.0, 8.0).radius(2.0).fill(color))
        .child(el().absolute(3.0, 11.0).size(10.0, 4.0).radius(1.0).fill(color.alpha(0.75)))
}

/// Ctrl+P / Cmd+P prints, also while a text box has the focus and would otherwise keep the key.
fn print_key(s: &mut App, e: &rune::KeyEvent, _: &mut Tx) -> bool {
    s.key_shortcut(e)
}

/// True when the Print space offers a copies count. One print is one label, so the count would
/// only lie on a label printer; the pocket printer still prints a series.
fn print_space_shows_copies(label: bool) -> bool {
    !label
}

/// Darkness, the number of copies and the blank paper below the ticket: the whole Print space of
/// the drawer. The density fits its three words whole instead of a fixed share of the drawer that
/// cut "Medium" to "Medi…".
///
/// The density and the copies share a line only while they fit in the card: together they ask for
/// more than a card gives, so the row wraps rather than pushing the "+" out of the panel. On a
/// label printer there are no copies to count, and the line holds the density alone.
fn print_options(s: &App) -> El {
    let k = kit();
    let density = SegmentedControl::new(&["Light", "Medium", "Dark"], Some(s.density)).fit().on_pick(|s: &mut App, i, _| {
        s.density = i;
        s.touch();
    }).build(k);
    let copies = row()
        .gap(6.0)
        .center_y()
        .child(Button::new("−").on_press(|s: &mut App| {
            s.copies = clamp_copies(s.copies.saturating_sub(1));
            s.touch();
        }).build(k))
        .child(text(format!("×{}", s.copies)).font(type_::BODY).color(ink()).w(26.0))
        .child(Button::new("+").on_press(|s: &mut App| {
            s.copies = clamp_copies(s.copies + 1);
            s.touch();
        }).build(k));
    let mut line = row().w_full().gap(ROW_GAP).center_y().flex_wrap().child(density);
    if print_space_shows_copies(s.on_a_label_printer()) {
        line = line.child(copies);
    }
    let mut body = col().w_full().gap(ROW_GAP).child(caption("Print")).child(line);
    // A label is cut at its edge, so there is no paper to leave below it: the row is the pocket
    // printer's alone.
    if !s.on_a_label_printer() {
        body = body.child(row_label("Space after", dim(), space_after_control(s)));
    }
    card(body)
}

/// Blank paper below the pocket ticket, 0 to [`render::MAX_SPACE_MM`], in 5 mm steps: enough to
/// tear the sheet off without cutting into the next one.
fn space_after_control(s: &App) -> El {
    let k = kit();
    row()
        .gap(6.0)
        .center_y()
        .child(Button::new("−").on_press(|s: &mut App| {
            s.set_space_after(s.space_after.saturating_sub(5));
        }).build(k))
        .child(text(format!("{} mm", s.space_after)).font(type_::LABEL).color(ink()).w(50.0).truncate())
        .child(Button::new("+").on_press(|s: &mut App| {
            s.set_space_after(s.space_after.saturating_add(5));
        }).build(k))
}

/// A joke headline when we have one, the real reason always underneath. A fresh error shakes and
/// throws a red zap.
fn error_banner(s: &App, reason: String) -> El {
    let fresh = s.fx_on && s.error_at.is_some_and(|t| s.clock - t < 0.9);
    let mut body = col().gap(2.0);
    if let Some(headline) = voice::error_headline(&reason) {
        body = body.child(text(headline.to_string()).font(type_::BODY).color(red()));
    }
    body = body.child(text(reason).font(type_::LABEL).color(red().alpha(0.85)).wrap());
    let mut banner = col().w_full().padding(10.0).radius(12.0).fill(red().alpha(0.14)).border(1.0, red().alpha(0.35)).child(body);
    if fresh {
        let since = (s.clock - s.error_at.unwrap_or(0.0)) as f32;
        // A quick shake that settles: a decaying wobble.
        banner = banner
            .rotate(0.018 * (since * 48.0).sin() * (1.0 - since / 0.9).max(0.0))
            .child(fx::slot(fx::ELECTRIC, [1.0, 0.0, 0.0, fx::RED]).absolute(0.0, 0.0).w_full().h_full().radius(12.0).effect_bleed(14.0));
    }
    banner
}

/// The battery as a gauge: a rounded body with a cap on its right, filled from the left by the
/// charge — mint above 30 %, amber down to 15 %, red below. Only the outline when the printer has
/// not said.
fn battery_gauge(battery: Option<u8>) -> El {
    const W: f32 = 22.0;
    const H: f32 = 11.0;
    let color = match battery {
        Some(b) if b > 30 => mint(),
        Some(b) if b >= 15 => amber(),
        Some(_) => red(),
        None => dim(),
    };
    let mut body = el().size(W, H).radius(3.0).border(1.0, color.alpha(if battery.is_some() { 0.9 } else { 0.45 }));
    if let Some(b) = battery {
        // Inside the 1 pt border: 18 pt of room for the charge to fill.
        let fill = (18.0 * b.min(100) as f32 / 100.0).max(1.0);
        body = body.child(el().absolute(2.0, 2.0).size(fill, H - 4.0).radius(1.5).fill(color));
    }
    row()
        .gap(1.5)
        .center_y()
        .child(body)
        .child(el().size(2.0, 4.5).radius(1.0).fill(color.alpha(if battery.is_some() { 0.8 } else { 0.3 })))
}

/// How many bars of signal `rssi` is worth, out of four: good from −55 dBm, fair from −70, weak
/// from −82, one bar below that, none when the printer has not said.
fn signal_bars(rssi: Option<i16>) -> u8 {
    match rssi {
        None => 0,
        Some(r) if r >= -55 => 4,
        Some(r) if r >= -70 => 3,
        Some(r) if r >= -82 => 2,
        Some(_) => 1,
    }
}

/// Four bars of reception, growing to the right; the one not earned stay drawn, dimmed, so the
/// gauge always has its scale. The dBm itself stays in the pill's tooltip.
fn signal(rssi: Option<i16>) -> El {
    let lit = signal_bars(rssi);
    let hue = match lit {
        4 | 3 => mint(),
        2 => amber(),
        1 => red(),
        _ => dim(),
    };
    let mut r = row().gap(1.5).center_y().h(12.0);
    for i in 0..4u8 {
        let h = 4.0 + 2.4 * i as f32;
        r = r.child(el().size(2.5, h).radius(1.0).fill(if i < lit { hue } else { Color::WHITE.alpha(0.18) }));
    }
    r
}

/// True when the connected device is the label printer: the app can see it and talk to it, but
/// pocket tickets cannot be printed on it. One source of truth: the same check the print path
/// uses (`on_a_label_printer`), plus the link being actually up.
fn label_online(s: &App) -> bool {
    matches!(s.link, Link::Online(_)) && s.on_a_label_printer()
}

/// The connection pill: the battery gauge, its percentage and the signal's bars, or the link's
/// state when there is nothing to show yet. Pressing it looks for the printer, or lets it go. The
/// tooltip keeps what the bar's chip used to say: model, firmware and signal.
fn status_pill(s: &App) -> El {
    let label = match &s.link {
        Link::Offline => "Offline".to_string(),
        Link::Connecting => "Connecting…".to_string(),
        // The label printer's battery is never read: a dash would read like a dead machine.
        Link::Online(_) if label_online(s) => "label".to_string(),
        Link::Online(_) => match s.battery {
            Some(b) => format!("{b}%"),
            None => "—".to_string(),
        },
    };
    let help = match &s.link {
        Link::Offline => "Press to look for the printer".to_string(),
        Link::Connecting => "Looking for the printer".to_string(),
        // A label printer shows its device line too, so the user keeps the model, firmware and
        // signal even though this app cannot print on it.
        Link::Online(_) if label_online(s) => match device_line(s) {
            Some(line) => format!("{line}\n{}", voice::label_unprintable()),
            None => voice::label_unprintable().to_string(),
        },
        Link::Online(_) => device_line(s).unwrap_or_else(|| "Press to disconnect".into()),
    };
    row()
        .key("status-pill")
        .role("button")
        .label(format!("Printer: {label}"))
        .focusable(true)
        .w(PILL_W)
        .h(PILL_H)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(PILL_H / 2.0)
        .fill(Color::BLACK.alpha(0.55))
        .border(1.0, Color::WHITE.alpha(0.2))
        .child(battery_gauge(if matches!(s.link, Link::Online(_)) { s.battery } else { None }))
        .child(text(label).font(type_::LABEL).color(ink()).truncate().grow(1.0).min_w(0.0))
        .child(signal(s.rssi))
        .on_press(|s: &mut App| {
            match s.link.clone() {
                Link::Offline => s.connect(),
                Link::Online(_) if !s.printing.is_busy() => s.disconnect(),
                _ => {}
            }
            s.touch();
        })
        .help(help)
}

/// The machine's controls, as one row: the connection pill and then the Print key. The row is
/// out of the flow, placed over the stage by `controls_place`: on the case while it is big enough
/// to wear it, beside the case at full size once a long ticket has shrunk it. The same key
/// covers both places, so Rune glides the pair from one to the other instead of jumping; a label
/// printer's controls are built by `label_controls` instead, under its turned case.
fn print_controls(s: &App, stage_w: Option<f32>, drop_h: Option<f32>, k: f32, turned: bool) -> Option<El> {
    let (x, y, scale) = controls_place(stage_w, drop_h, k, turned)?;
    let on_case = !turned && k >= KEY_ON_CASE_MIN;
    let label = if s.printing.is_busy() { "Printing…" } else { "Print" };
    let mut r = row()
        .key("stage-controls")
        .absolute(x, y)
        .h(PRINT_H)
        .gap(CONTROLS_GAP)
        .center_y()
        .motion(motion::GLIDE)
        .child(status_pill(s))
        .child(hero_button(s, label.to_string(), on_case));
    if scale < 0.999 {
        // Scaled with the case, its top-left stays where `controls_place` put it.
        r = r.scale(scale).origin(0.0, 0.0);
    }
    Some(r)
}

/// The machine's controls on a label stage: the connection pill and then the Print key, upright
/// and in the flow under the turned case, where nothing turns them.
fn label_controls(s: &App) -> El {
    let label = if s.printing.is_busy() { "Printing…" } else { "Print" };
    row()
        .key("stage-controls")
        .h(PRINT_H)
        .gap(CONTROLS_GAP)
        .center_y()
        .child(status_pill(s))
        .child(hero_button(s, label.to_string(), false))
}

/// "Version 0.2.0 is out": open the release page to get the installer, or dismiss.
fn update_banner(update: &Update) -> El {
    let url = update.url.clone();
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .padding(10.0)
        .radius(12.0)
        .fill(mint().alpha(0.12))
        .border(1.0, mint().alpha(0.35))
        .child(text(format!("Version {} is available", update.version)).font(type_::LABEL).color(mint()).grow(1.0).min_w(0.0).truncate())
        .child(Button::new("Get it").variant(ButtonVariant::Contrast).on_press(move |_: &mut App| update::open(&url)).build(kit()))
        .child(Button::new("Later").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.update = None).build(kit()))
}

/// The taglines are capped at 40 characters (`voice`), about 280 pt at LABEL size: the brand keeps
/// that much, plus the mark, so the line under the wordmark is never cut by its neighbours.
const BRAND_MIN: f32 = 300.0;

/// The mark and the wordmark, with what the printer is doing under it.
fn brand(s: &App) -> El {
    let mark = match &s.logo {
        Some(image) => el().key("logo").size(40.0, 40.0).radius(10.0).pixels(image.pixels()),
        None => el().size(40.0, 40.0).radius(10.0).gradient(coral(), pink()),
    }
    // Five taps on the mark: the ghost says boo.
    .on_press(|s: &mut App| s.logo_tap());
    // The wordmark keeps a dry line under it, coming up out of the slot like paper out of the
    // printer every minute. The box under TRONIC keeps the line's full height, so the wordmark
    // never moves; the line inside it grows from the bottom and is clipped, so the letters appear
    // from below. When something is actually happening (connecting, printing, an error, a fresh
    // result), the line says that instead, whole: the quip can wait.
    let fresh_done = s.printing.done_at().is_some_and(|t| s.clock - t < 6.0);
    let busy = s.printing.is_busy() || matches!(s.link, Link::Connecting) || s.error.is_some() || fresh_done;
    let (line, rise) = if busy { (s.status.clone(), 1.0) } else { (s.tagline().to_string(), s.tagline_reveal()) };
    let line_h = type_::LABEL.line_px();
    let slot = col().h(line_h).clip_rounded(0.0).justify_end().child(
        col()
            .h(line_h * rise)
            .clip_rounded(0.0)
            .justify_end()
            .child(text(line).font(type_::LABEL).color(dim()).truncate()),
    );
    row().grow(1.0).min_w(BRAND_MIN).gap(10.0).center_y().child(mark).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .child(text("TRONIC".to_string()).font(type_::TITLE).color(coral()))
            .child(slot),
    )
}

/// "Mini Pocket Printer_BLE · pocket printer · A2Y · V1.06LY · -51 dBm" once the printer has
/// answered: which device it is first, then what it is, then what it said (the signal arrives
/// with the connection, the model and firmware with their queries).
fn device_line(s: &App) -> Option<String> {
    let mut parts = Vec::new();
    if let Link::Online(name) = &s.link {
        parts.push(name.clone());
        // The kind says what the machine is, so two Tronic devices never look alike.
        if let Some(kind) = protocol::kind_of(name) {
            parts.push(voice::device_kind(kind).to_string());
        }
    }
    if let Some(m) = &s.info.model {
        parts.push(m.clone());
    }
    if let Some(f) = &s.info.firmware {
        parts.push(f.clone());
    }
    if let Some(rssi) = s.rssi {
        parts.push(format!("{rssi} dBm"));
    }
    // The paper query's reply format is undocumented, so the raw hex is not user-facing; show
    // nothing until it can be read as OK/empty.
    if parts.is_empty() { None } else { Some(parts.join(" · ")) }
}

/// The printer the user chose, or an invitation to choose one. Clicking opens the drawer on its
/// Printer space, where the list lives.
fn device_picker(s: &App) -> El {
    let label = match &s.preferred {
        Some(name) => name.clone(),
        None => "Choose a printer".to_string(),
    };
    Button::new(format!("Device: {label}"))
        .variant(ButtonVariant::Ghost)
        .help("Choose the printer")
        .on_press(|s: &mut App| {
            s.drawer_open = true;
            s.drawer_space = DrawerSpace::Printer;
            s.touch();
        })
        .build(kit())
}

/// Short label for a printer kind, shown under the device name.
fn device_kind_short(kind: protocol::Kind) -> &'static str {
    match kind {
        protocol::Kind::Label => "label",
        protocol::Kind::Pocket => "pocket",
    }
}

/// The small meta line under a discovered printer: kind and signal.
fn printer_meta(kind: protocol::Kind, rssi: Option<i16>) -> String {
    match rssi {
        Some(r) => format!("{} · {r} dBm", device_kind_short(kind)),
        None => device_kind_short(kind).to_string(),
    }
}

/// The list of printers in the drawer's Printer space: scan, then pick a device. There is no
/// "Automatic" row: the app only ever connects to the device named here.
fn printers_panel(s: &App) -> El {
    let scan_label = if s.scanning { "Scanning…" } else { "Scan" };
    let mut scan = Button::new(scan_label).on_press(|s: &mut App| s.scan_printers());
    if s.scanning {
        scan = scan.disabled(true);
    }
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(caption("Printers"))
        .child(row().grow(1.0))
        .child(scan.build(kit()));

    let connected_name = match &s.link {
        Link::Online(name) => Some(name.as_str()),
        _ => None,
    };
    let status = col()
        .w_full()
        .gap(2.0)
        .child(
            text(match connected_name {
                Some(name) => format!("Connected: {name}"),
                None => "No printer connected".to_string(),
            })
            .font(type_::LABEL)
            .color(if connected_name.is_some() { ink() } else { dim() })
            .wrap(),
        )
        .child(
            text(match &s.preferred {
                Some(name) => format!("Choice: {name}"),
                None => "No printer chosen yet".to_string(),
            })
            .font(type_::LABEL)
            .color(dim())
            .wrap(),
        );

    let mut list = col().w_full().gap(ROW_GAP).child(status);

    if s.discovered.is_empty() {
        let note = if s.scanning { "Scanning…" } else { "No printer found yet. Press Scan." };
        list = list.child(text(note.to_string()).font(type_::LABEL).color(dim()).wrap());
    } else {
        for d in &s.discovered {
            let connected = connected_name == Some(d.name.as_str());
            let chosen = s.preferred.as_deref() == Some(&d.name);
            let name = d.name.clone();
            let kind = d.kind;
            let right: El = if connected {
                text("Connected".to_string()).font(type_::LABEL).color(mint())
            } else if chosen {
                text("In use".to_string()).font(type_::LABEL).color(ink())
            } else {
                Button::new("Use")
                    .variant(ButtonVariant::Ghost)
                    .on_press(move |s: &mut App| s.prefer(&name, kind))
                    .build(kit())
            };
            list = list.child(
                row()
                    .w_full()
                    .gap(ROW_GAP)
                    .center_y()
                    .child(
                        col()
                            .gap(2.0)
                            .grow(1.0)
                            .min_w(0.0)
                            .child(text(d.name.clone()).font(type_::BODY).color(if connected || chosen { ink() } else { dim() }).wrap())
                            .child(text(printer_meta(d.kind, d.rssi)).font(type_::LABEL).color(dim())),
                    )
                    .child(right),
            );
        }
    }

    card(col().w_full().gap(ROW_GAP).child(head).child(list))
}

/// One tab. The lit pill is drawn separately and slides between tabs, so this only colours text.
fn tab(label: &'static str, selected: bool, mode: Mode) -> El {
    row()
        .key(format!("tab-{label}"))
        .role("tab")
        .label(label)
        .focusable(true)
        .grow(1.0)
        .h(32.0)
        .center()
        .radius(16.0)
        .on_press(move |s: &mut App| s.switch_mode(mode))
        .child(text(label.to_string()).font(type_::LABEL).color(if selected { style::on_accent() } else { dim() }))
}

fn tabs(s: &App) -> El {
    const GAP: f32 = 4.0;
    let each = (TABS_W - 2.0 * GAP - (Mode::ALL.len() as f32 - 1.0) * GAP) / Mode::ALL.len() as f32;
    let at = Mode::ALL.iter().position(|m| *m == s.mode).unwrap_or(0) as f32;
    // The lit pill springs from tab to tab.
    let pill = el()
        .key("tab-pill")
        .absolute(GAP + at * (each + GAP), GAP)
        .size(each, 32.0)
        .radius(16.0)
        .gradient(style::grad_a(), style::grad_b())
        .shadow(4.0, 16.0, pink().alpha(if style::pal().glow { 0.35 } else { 0.0 }))
        .motion(motion::SNAPPY);
    let mut r = row().w(TABS_W).gap(GAP).padding(GAP).radius(20.0).fill(panel()).border(1.0, Color::WHITE.alpha(0.06)).child(pill);
    for m in Mode::ALL {
        r = r.child(tab(m.label(), m == s.mode, m));
    }
    r
}

/// The one row above the stage: the mark and what the printer is doing, the four modes, the
/// ticket's length and fit, FX and the drawer switch. The connection pill and Print live on the
/// printer's case in the stage, and the palettes in the drawer, so the bar stays a bar.
fn top_bar(s: &App, drop_h: Option<f32>, stage_w: Option<f32>) -> El {
    let fit = stage_fit(s, drop_h, stage_w).0;
    let cm = s.composed.as_ref().map(|b| format!("{:.1} cm", render::length_cm(b))).unwrap_or_else(|_| "— cm".to_string());
    let size = text(format!("{cm} · fit {:.0}%", fit * 100.0)).font(type_::LABEL).color(if fit < 0.999 { coral() } else { dim() }).w(112.0).truncate();
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(brand(s))
        .child(tabs(s))
        .child(size)
        .child(device_picker(s))
        .child(Switch::new(s.fx_on).label("FX").on_toggle(|s: &mut App| s.set_effects(!s.fx_on)).build(kit()))
        .child(drawer_button(s))
}

/// The name of the drawer, on its button and in its header. It does not follow the mode: a button
/// that says "Pictures" in Image mode is how the themes went missing. The header says what this
/// mode's drawer holds.
const DRAWER_LABEL: &str = "Options";

/// The one button that opens the drawer, and closes it again. The four mode tabs never open it.
fn drawer_button(s: &App) -> El {
    Button::new(DRAWER_LABEL)
        // Open, the same control turns into a filled face: that is the pressed state, so the
        // button never renames itself to "Close" and never moves from under the pointer.
        .variant(if s.drawer_open { ButtonVariant::Contrast } else { ButtonVariant::Ghost })
        .expanded(s.drawer_open)
        .on_press(|s: &mut App| s.toggle_drawer())
        .build(kit())
}

/// The left zone of the open scene: the mode's galleries, its extras and the colour row. No head
/// and no space selector — the scene now shows its three zones at once.
fn themes_panel(s: &App) -> El {
    let k = kit();
    let body = match s.mode {
        Mode::Text => {
            let mut body = col().w_full().gap(SECTION_GAP).child(theme_gallery(s));
            body = body.child(row_label(
                "Out of ideas?",
                dim(),
                Button::new("Surprise me").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.surprise_me()).build(kit()),
            ));
            if let Some(switches) = form_switches(s) {
                body = body.child(card(switches));
            }
            // The text itself is typed on the paper. The type choices and the rest of the
            // settings live on the strip under the ticket.
            body
        }
        Mode::Image => col().w_full().gap(SECTION_GAP).child(gallery_section(s)),
        Mode::Codes => col().w_full().gap(SECTION_GAP).child(text("The code is typed on its sheet.".to_string()).font(type_::LABEL).color(dim()).wrap()),
        Mode::Terminal => col().w_full().gap(SECTION_GAP).child(terminal_info(s)),
    };
    // The colour row scrolls with the galleries: pinned to the panel's edge it was cut at the
    // bottom, under a long column of thumbnails.
    let content = col()
        .w_full()
        .gap(SECTION_GAP)
        .child(body)
        .child(field("Colour", palette_chips(s)))
        .pb(SCROLL_FOOT);
    col()
        .key("themes-panel")
        .w(SIDE_PANEL_W)
        .h_full()
        .padding(PAD)
        .radius(20.0)
        .fill(style::bg_top())
        .border(1.0, Color::WHITE.alpha(0.08))
        .child(col().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(content).scroll_bar(k))
}

/// The right zone of the open scene: the printers to choose from, then density, copies and paper,
/// one under the other. No scroll: two short cards do not mount up to a column worth scrolling,
/// and a scrolling child of a full-height column settles at its foot instead of its top.
fn right_panels(s: &App) -> El {
    col()
        .key("right-panels")
        .w(SIDE_PANEL_W)
        .h_full()
        .padding(PAD)
        .gap(SECTION_GAP)
        .justify_start()
        .child(printers_panel(s))
        .child(print_options(s))
}

/// Size of the printer picture (`assets/printer.svg`) and where its parts sit in it. The live parts
/// (chrome lip, flames, lamp, lightning) are placed with these numbers, so keep them in step with
/// the picture.
const MACHINE_W: f32 = 600.0;
const MACHINE_H: f32 = 290.0;
/// The smallest the printer is drawn when a long ticket leaves it little room.
const MACHINE_MIN: f32 = 0.5;
/// The stage's padding round the preview (`stage-frame`), which the machine shares with a ghost.
const STAGE_PAD: f32 = 18.0;

/// A label printer gets a scene of its own (`label_stage`): the case turned, the tag coming out of
/// its slot. A pocket printer keeps its ticket standing out of the slit above the case, so the two
/// layouts are exclusive.
fn label_stage_is_side_by_side(label: bool) -> bool {
    label
}

/// Whether the label stage builds the machine's controls itself, in the flow under its turned case
/// (`label_controls`). The pair is built exactly once: a label stage draws it in the flow and the
/// stage overlay (`print_controls`) is skipped, or the stage is a pocket one and the overlay is the
/// pair. A label printer with no sheet yet is drawn as the pocket layout — no label stage is built
/// at all — so the overlay is then its only pair and must stay.
fn label_stage_draws_controls(label: bool, has_paper: bool) -> bool {
    label_stage_is_side_by_side(label) && has_paper
}

/// Radians the label printer's case is turned, clockwise. Its picture is 600 wide by 290 tall with
/// the slot running along the top (`SLIT_Y`), so a quarter turn stands the case up — visually
/// `LABEL_CASE_W` by `LABEL_CASE_H` — with the slot on its right flank, where the tag comes out.
/// Rune reads a positive angle as clockwise.
fn label_printer_turn() -> f32 {
    std::f32::consts::FRAC_PI_2
}

/// The turned case as it looks on the desk: `MACHINE_H` across and `MACHINE_W` tall.
const LABEL_CASE_W: f32 = MACHINE_H;
const LABEL_CASE_H: f32 = MACHINE_W;

/// Where the case's own 600 x 290 box goes inside the `LABEL_CASE_W` x `LABEL_CASE_H` it looks
/// like: a turn only draws turned (the layout box keeps its size), so the box is pushed out by
/// half the difference to put its middle on the middle of the scene.
fn label_case_origin() -> (f32, f32) {
    ((LABEL_CASE_W - MACHINE_W) / 2.0, (LABEL_CASE_H - MACHINE_H) / 2.0)
}

/// Where the tag's left edge sits from the case's left: the slot's front lip is `SLIT_FRONT` in
/// from the top of the picture, and the turn brings that edge in from the case's right edge by the
/// same amount. The tag starts there and covers the lip's band, so it reads as coming out of the
/// slot — as a pocket ticket covers the band above its slit.
fn label_tag_left() -> f32 {
    LABEL_CASE_W - SLIT_FRONT
}

/// The scene's width: the tag leaves the slot and runs on past the case's right edge.
fn label_pair_w(tag_w: f32) -> f32 {
    label_tag_left() + tag_w
}

/// The gap between the pair and the controls under it, in points.
const LABEL_CONTROLS_GAP: f32 = 12.0;

/// How much the pair is shrunk so that it and the controls under it fit the stage's height: full
/// size on the 700 pt stage the app's window gives, `MACHINE_MIN` at the smallest, like the
/// pocket case.
fn label_scene_k(drop_h: Option<f32>) -> f32 {
    drop_h.map_or(1.0, |h| ((h - PRINT_H - LABEL_CONTROLS_GAP - 8.0) / LABEL_CASE_H).clamp(MACHINE_MIN, 1.0))
}

/// The paper slit: left edge, width, top, and the front lip where the paper enters.
const SLIT_X: f32 = 64.0;
const SLIT_W: f32 = 472.0;
const SLIT_Y: f32 = 78.0;
const SLIT_FRONT: f32 = 86.0;

/// The label case's slot, in the same 600 x 290 box (`label-printer.svg`): short — one label,
/// centred — where the pocket case's runs nearly the whole top.
const LABEL_SLIT_X: f32 = 220.0;
const LABEL_SLIT_W: f32 = 160.0;

/// Where the paper leaves the case, as left edge and width: the two pictures give their slot
/// different lengths, so the band of paper in it, the bare stub and the chrome have to follow the
/// case that is drawn or they spill past its slot. The lip and the top are shared.
fn slit_x_w(label: bool) -> (f32, f32) {
    if label { (LABEL_SLIT_X, LABEL_SLIT_W) } else { (SLIT_X, SLIT_W) }
}

/// The printer as it looks on the desk: a soft white cube with the paper leaving a slit across its
/// top, one lamp low on the front (green when connected, amber while working or connecting, red
/// when off) and a USB-C port on the side. Its controls (the connection pill and the Print key)
/// are drawn over the stage by `print_controls`: on this front while the case is big, beside it
/// once a long ticket has shrunk it. The ticket stands above it and runs into the slit.
fn printer_body(s: &App, armed: bool) -> El {
    let light = match &s.link {
        Link::Online(_) if s.printing.is_busy() => amber(),
        Link::Online(_) => mint(),
        Link::Connecting => amber(),
        Link::Offline => red(),
    };
    let online = matches!(s.link, Link::Online(_));
    let has_paper = matches!((&s.preview, &s.composed), (Some(_), Ok(_)));
    let label = s.on_a_label_printer();
    let mut m = col().w(MACHINE_W).h(MACHINE_H);
    let art = if label { &s.printer_art_label } else { &s.printer_art };
    if let Some(art) = art {
        m = m.child(el().absolute(0.0, 0.0).size(MACHINE_W, MACHINE_H).pixels_fit(art.pixels(), SurfaceFit::Fit));
    }
    // With nothing to print only a short stub of paper shows in the slit (it springs up when a
    // picture is dragged over). A ticket is drawn by the stage, over the machine, down to the slit.
    let tail = if has_paper {
        0.0
    } else if armed {
        60.0
    } else if online {
        24.0
    } else {
        0.0
    };
    if tail > 0.0 {
        let (slit_x, slit_w) = slit_x_w(label);
        let w = slit_w - 8.0;
        // The ticket above fades to a darker, shaded paper at its bottom edge (as if going back
        // into the slot), so a ticket's tail picks that up and darkens on towards the slit. A bare
        // stub is fresh paper: bright, shading only a little. The outer strips are the margins the
        // print head cannot reach, a touch darker, as in the ticket.
        let shade = |k: f32| style::rgb(((250.0 * k) as u32) << 16 | ((246.0 * k) as u32) << 8 | (236.0 * k) as u32);
        let (top, bottom) = if armed { (style::rgb(0xffd9c9), style::rgb(0xe8a98f)) } else { (shade(1.0), shade(0.72)) };
        let rim = |x: f32| el().absolute(x, 0.0).size(40.0, tail).fill(Color::BLACK.alpha(0.06));
        m = m.child(
            el()
                .absolute(slit_x + 4.0, SLIT_FRONT - tail)
                .size(w, tail)
                .gradient(top, bottom)
                .shadow(0.0, 8.0, Color::BLACK.alpha(0.28))
                .child(rim(0.0))
                .child(rim(w - 40.0)),
        );
    }
    // No controls are built here: the pair is drawn over the stage (`print_controls`), so a long
    // ticket that scales the case can hand it to the side at full size. Only the lamp stays as the
    // case's live part.
    let lamp = if label {
        let mut ring = el()
            .absolute(262.0, 186.0)
            .size(76.0, 76.0)
            .radius(38.0)
            .border(3.0, light)
            .shadow(0.0, 16.0, light.alpha(0.85));
        if s.fx_on && matches!(s.link, Link::Connecting) {
            // A slow radar ring around the button while connecting.
            ring = ring.child(
                fx::slot(fx::PULSE, [0.0, fx::AMBER, 2.4, 22.0])
                    .absolute(-10.0, -10.0)
                    .size(96.0, 96.0)
                    .radius(48.0)
                    .effect_bleed(22.0),
            );
        }
        ring
    } else {
        let mut bar = el()
            .absolute(278.0, 200.0)
            .size(44.0, 12.0)
            .radius(6.0)
            .fill(light)
            .shadow(0.0, 16.0, light.alpha(0.85));
        if s.fx_on && matches!(s.link, Link::Connecting) {
            // A small, slow radar ring around the light only.
            bar = bar.child(
                fx::slot(fx::PULSE, [0.0, fx::AMBER, 2.4, 22.0])
                    .absolute(-4.0, -4.0)
                    .size(52.0, 20.0)
                    .radius(10.0)
                    .effect_bleed(22.0),
            );
        }
        bar
    };
    m.child(lamp)
}

/// What happens on top of the printer and the ticket: the chrome lip of the slit, flames and heat
/// haze, lightning. Drawn above the ticket so the flames lick the paper rather than hide behind it.
fn printer_fx(s: &App, _armed: bool) -> El {
    let mut m = col().w(MACHINE_W).h(MACHINE_H);
    let (slit_x, slit_w) = slit_x_w(s.on_a_label_printer());
    if s.fx_on {
        let (flame, smoke) = (s.flame(), s.smoke());
        // The slit's front lip is liquid chrome; the metal only flows while the printer works.
        let flow = if s.printing.is_busy() || flame > 0.0 { 0.6 } else { 0.0 };
        m = m.child(fx::slot(fx::CHROME, [0.0, 0.0, flow, 0.0]).absolute(slit_x, SLIT_FRONT - 1.0).size(slit_w, 3.0).radius(1.5));
        if flame > 0.0 || smoke > 0.0 {
            // Flames while it prints; once the job is done they die down and only smoke is left,
            // drifting up off the paper for a few seconds. The fire keeps feeding its flames
            // whatever the intensity, so "going out" means a short, dim stub that fades away with
            // the smoke (intensity scales the smoke too, so it must stay up while smoke is thick),
            // and a heavy smoke rate so the cooling stub leaves plenty behind.
            let (height, intensity, rate) = if flame > 0.0 {
                (110.0 + 120.0 * flame, 0.55 + flame, smoke)
            } else {
                (45.0, 0.55 * smoke.min(1.0), 2.0)
            };
            m = m.child(fx::slot(fx::FIRE, [height, intensity, 0.0, rate]).absolute(slit_x, SLIT_Y).size(slit_w, 8.0).radius(4.0).effect_bleed(220.0));
        }
        if flame > 0.0 {
            m = m.child(fx::slot(fx::HAZE, [170.0, 1.3 * flame, 0.0, 0.0]).effect_backdrop(true).absolute(0.0, -120.0).size(MACHINE_W, 196.0));
        }
    }
    m
}

/// A machine-sized layer pinned to the bottom centre of the stage, drawn at `k` times its size
/// (about the middle of its bottom edge, so it stays on the floor) when the ticket is long.
fn at_machine(inner: El, k: f32) -> El {
    let inner = if k < 0.999 { inner.scale(k).origin(0.5, 1.0) } else { inner };
    col().items_center().pin(Some(0.0), None, Some(0.0), Some(0.0)).child(inner)
}

/// The label printer's stage: the case turned a quarter turn clockwise (`label_printer_turn`) so
/// the slot that runs along the top of its picture faces right, the tag coming out of that slot,
/// and the machine's controls upright under the pair. A turn only draws turned, so the case keeps
/// its 600 x 290 layout box: the pair sets up the `LABEL_CASE_W` x `LABEL_CASE_H` the case looks
/// like and pushes the box out into it (`label_case_origin`).
fn label_stage(s: &App, armed: bool, sheet: El, k: f32) -> El {
    let (case_x, case_y) = label_case_origin();
    let case = printer_body(s, armed).absolute(case_x, case_y).rotate(label_printer_turn());
    // The tag's left edge on the slot's lip and its middle at the middle of the slot's length:
    // the label case's slot takes one label and is centred on the picture, so the tag is centred
    // on the case's height.
    let tag_h = s.ticket_height();
    let tag = sheet.absolute(label_tag_left(), (LABEL_CASE_H - tag_h) / 2.0);
    let (pair_w, pair_h) = (label_pair_w(preview_w(s)), LABEL_CASE_H);
    let pair = col().size(pair_w, pair_h).child(case).child(tag);
    // A short stage shrinks the pair whole, its box with it so the controls follow; the controls
    // themselves never shrink or turn, they stay legible under the case.
    let pair = if k < 0.999 {
        col().size(pair_w * k, pair_h * k).child(pair.scale(k).origin(0.0, 0.0))
    } else {
        pair
    };
    col()
        .key("stage-group")
        .gap(LABEL_CONTROLS_GAP)
        .items_start()
        .child(pair)
        // Under the case rather than under the pair: the controls belong to the machine.
        .child(col().w(LABEL_CASE_W * k).items_center().child(label_controls(s)))
}

/// A small row of round chips, one for each colour palette (its background with its accent in the
/// middle); the one being worn is outlined. A press wears it at once.
fn palette_chips(s: &App) -> El {
    let mut r = row().gap(6.0).center_y();
    for id in style::PaletteId::ALL {
        let p = id.palette();
        let on = s.palette == id;
        let dot = col().size(10.0, 10.0).radius(5.0).fill(style::rgb(p.accent));
        let chip = col()
            .key(format!("palette-{}", id.id()))
            .size(22.0, 22.0)
            .radius(11.0)
            .center()
            .fill(style::rgb(p.bg_top))
            .border(if on { 2.0 } else { 1.0 }, if on { ink() } else { Color::WHITE.alpha(0.25) })
            .help(p.name)
            .child(dot)
            .on_press(move |s: &mut App| s.set_palette(id));
        r = r.child(chip);
    }
    r
}

/// The width of the preview sheet, in points: the composed bitmap's real width plus its paper
/// margins. A label is a wide landscape sheet (the tag lying down, 30 × 14 mm); a pocket ticket
/// keeps the usual 400 pt.
fn preview_w(s: &App) -> f32 {
    s.composed
        .as_ref()
        .ok()
        .map(|b| render::preview_width(b) as f32)
        .unwrap_or(render::PREVIEW_WIDTH as f32)
}

/// How a ticket is scaled to fit the stage whole, and (for a turned ticket) the scale and height of
/// the printer-plus-ticket group. Shared by the stage and by the top bar's `fit NN%`, so the two
/// never disagree.
fn stage_fit(s: &App, drop_h: Option<f32>, stage_w: Option<f32>) -> (f32, f32, f32) {
    // A label printer stands beside its label (`label_stage_is_side_by_side`), which is short and
    // wide: nothing above it, nothing to turn, so the sheet is shown whole at its real size.
    if label_stage_is_side_by_side(s.on_a_label_printer()) {
        return (1.0, 1.0, stage_w.map_or(MACHINE_W * 2.0, |w| w - 36.0));
    }
    let has_paper = matches!((&s.preview, &s.composed), (Some(_), Ok(_)));
    let turned = has_paper && s.preview_turned();
    // A long ticket is scaled down to fit whole on the stage, never below half size; while a part
    // is being typed on it goes back to full size, and the sheet's window keeps the size it had
    // when the box opened (`App::ticket_height`), so typing cannot move the machine.
    let ticket_h = s.ticket_height();
    // The drop zone's own height, not the frame's: the frame also holds the write bar and the gaps
    // under the printer, so taking it would overestimate the room and clip a long ticket. The 8 pt
    // keep its top edge clear of the frame's rounded border.
    let room = drop_h.map(|h| h - 8.0).unwrap_or(f32::MAX);
    let printer_min = (MACHINE_H - SLIT_FRONT) * MACHINE_MIN;
    // Turned, the printer and its ticket lie on their side as one piece: the printer's width (600)
    // becomes the height it takes, so the whole group is scaled to fit that; its length is then
    // the stage's width divided by that scale, and the ticket gets what the printer leaves.
    let group_k = if turned && room.is_finite() { (room / MACHINE_W).clamp(0.3, 1.0) } else { 1.0 };
    let frame_h = stage_w.map_or(MACHINE_W * 2.0, |w| (w - 36.0) / group_k);
    let fit = if turned {
        ((frame_h - (MACHINE_H - SLIT_FRONT) - 24.0) / ticket_h.max(1.0)).clamp(0.25, 1.0)
    } else if has_paper && room.is_finite() && s.editing.is_none() {
        ((room - printer_min).max(160.0) / ticket_h.max(1.0)).clamp(0.45, 1.0)
    } else {
        1.0
    };
    (fit, group_k, frame_h)
}

/// How much of the ticket stands out of the slit while a job runs, in paper points: `progress` of
/// the ticket itself, with a stub of 24 pt so the sheet starts with a visible edge, and never more
/// than the ticket (a short ticket is simply out whole, not a band taller than the paper).
fn rise_height(full: f32, progress: f32) -> f32 {
    (full * progress.clamp(0.02, 1.0)).max(24.0).min(full)
}

fn paper(s: &App, drop_h: Option<f32>, stage_w: Option<f32>, gaze: (f32, f32)) -> El {
    let has_paper = matches!((&s.preview, &s.composed), (Some(_), Ok(_)));
    let turned = has_paper && s.preview_turned();
    // A long ticket is scaled down to fit whole on the stage, never below half size; while a part
    // is being typed on it goes back to full size, and the sheet's window keeps the size it had
    // when the box opened (`App::ticket_height`), so typing cannot move the machine.
    let ticket_h = s.ticket_height();
    let (fit, group_k, frame_h) = stage_fit(s, drop_h, stage_w);
    // How much room the printer and its ticket have, to shrink the printer when the ticket is long.
    let room = drop_h.map(|h| h - 8.0).unwrap_or(f32::MAX);
    // On a wide stage with no ticket the printer talks from beside its case, so the space above the
    // slit stays free for the ticket that will come; a narrow stage keeps the bubble over the slit.
    let beside = s.preview.is_none() && stage_w.is_some_and(|w| w - 2.0 * STAGE_PAD >= GHOST_SIDE_MIN_W);
    let w_px = preview_w(s);
    let sheet = match (&s.preview, &s.composed) {
        (Some(image), Ok(bitmap)) => {
            let full = render::preview_height(bitmap) as f32;
            let mut paper = el().key("paper").w(w_px).pixels(image.pixels());
            if s.mode == Mode::Image && s.on_a_label_printer() && !s.printing.is_busy() {
                let (shown_w, shown_h) = (w_px * fit, full * fit);
                paper = paper.on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| {
                    if d.phase == Phase::Move {
                        let c = s
                            .pic_opts
                            .crop
                            .get_or_insert(render::Crop::centered(30.0 / 14.0));
                        crop_drag(c, d.dx, d.dy, shown_w, shown_h);
                        s.touch();
                        tx.instant();
                    }
                });
            }
            if s.fx_on && s.printing.done_at().is_some_and(|t| s.clock - t < 2.2) {
                let t0 = s.printing.done_at().unwrap_or(0.0) as f32;
                paper = paper.child(fx::slot(fx::SPARKLES, [t0, 16.0, 9.0, fx::WHITE]).absolute(0.0, 0.0).size(w_px, full).effect_bleed(30.0));
            }
            if !s.printing.is_busy() && s.mode == Mode::Text {
                paper = paper.child(write_layer(s, full, turned));
            }
            // An empty Codes sheet is the code box: pressing it writes the code text on the paper,
            // like the panel's own text area (the box stays up while the code is typed).
            if !s.printing.is_busy()
                && s.mode == Mode::Codes
                && (s.editing.as_deref() == Some("_code") || s.regions.iter().any(|r| r.key == "_code"))
            {
                paper = paper.child(code_write_layer(s, full));
            }
            let inner = if s.printing.is_busy() && s.fx_on {
                // The ticket is pushed out of the printer as the job goes: only the band that has left
                // the slit shows. That band is the foot of the ticket, its bottom edge on the slit, so
                // the sheet reads as fed out of the machine instead of as a window sliding over it.
                // The paper is taller than its window; `justify_end` pins its foot to the bottom and
                // the part still inside the printer overflows above, where the clip cuts it.
                let shown = rise_height(full, s.printing.progress());
                col().w(w_px).h(shown).clip_rounded(0.0).justify_end().child(paper)
            } else {
                paper
            };
            if fit < 0.999 {
                // The layout box shrinks with the picture, so the scroll window stops guessing.
                col().w(w_px * fit).h(full * fit).clip_rounded(0.0).child(inner.scale(fit).origin(0.0, 0.0))
            } else {
                inner
            }
        }
        _ if beside => col(),
        _ => ghost_paper(s, gaze),
    };
    // A picture is being pulled out of the gallery: the preview lights up as the place to drop it.
    let dragging = s.drag.as_ref().is_some_and(|d| d.travel >= crate::app::DRAG_MIN);
    let armed = dragging && s.over_stage;
    // Where a dragged picture is let go to print: the ticket and the printer, not the gallery.
    // Three layers, painted in this order: the printer's body at the bottom, the ticket standing
    // out of its slit (pinned so its bottom edge sits on the slit, over the body), and the effects
    // on top of both. A bubble for the empty state stays above the printer instead.
    // A label printer has a scene of its own: the case turned so its slot faces right, the tag
    // coming out of it (`label_stage`). With nothing to put there it keeps the pocket layout. The
    // pocket scene is untouched: the printer and its ticket are one group, so a sideways theme can
    // turn all of it, and the turn (and the change of scale) is animated by Rune because the group
    // keeps its key.
    let side_by_side = label_stage_draws_controls(s.on_a_label_printer(), has_paper);
    let shown_ticket_h = ticket_h * fit;
    let k = if turned || side_by_side {
        1.0
    } else if has_paper && room.is_finite() {
        ((room - shown_ticket_h) / (MACHINE_H - SLIT_FRONT)).clamp(MACHINE_MIN, 1.0)
    } else {
        1.0
    };
    let ticket_floor = if has_paper { (MACHINE_H - SLIT_FRONT) * k } else { MACHINE_H - 14.0 };
    let group = if side_by_side {
        label_stage(s, armed, sheet, label_scene_k(drop_h))
    } else {
        let pocket = col()
            .key("stage-group")
            .child(at_machine(printer_body(s, armed), k))
            .child(
                col()
                    .items_center()
                    .justify_end()
                    .pin(Some(0.0), Some(0.0), Some(0.0), Some(ticket_floor))
                    // While a part is written on, the sheet must not stick to the bottom of its window:
                    // a line that grows would push the box up from under the caret. It scrolls instead.
                    .child(col().items_center().child(sheet).scroll(ScrollSpec { stick_bottom: s.editing.is_none() }).shrink().min_h(0.0)),
            )
            .child(at_machine(if k < 0.999 { col() } else { printer_fx(s, armed) }, k));
        if turned {
            pocket.w(MACHINE_W).h(frame_h).scale(group_k).rotate(-std::f32::consts::FRAC_PI_2)
        } else {
            pocket.w_full().h_full().scale(1.0).rotate(0.0)
        }
    };
    let mut drop_zone = col()
        .key("stage")
        .w_full()
        .grow(1.0)
        .min_h(MACHINE_H * MACHINE_MIN)
        .items_center()
        .justify_center()
        .child(group);
    if beside {
        drop_zone = drop_zone.child(ghost_beside(s, gaze));
    }
    // With a ticket on the paper the ghost floats round it: above the printer and the ticket in z,
    // but always outside the ticket's rectangle, so no ink is covered. A turned ticket lies across
    // the stage and leaves it no room; a label's sheet is not centred on its case, so the round of
    // spots does not describe where it stands; a stage too narrow for every spot simply goes without.
    if has_paper && !turned && !side_by_side && !armed {
        if let (Some(sw), Some(dh)) = (stage_w, drop_h) {
            let (w, h) = (sw - 2.0 * STAGE_PAD, dh);
            let tw = preview_w(s) * fit;
            let shown = shown_ticket_h.min((dh - ticket_floor).max(0.0));
            let ticket = ((w - tw) / 2.0, dh - ticket_floor - shown, tw, shown);
            let case_right = w / 2.0 + MACHINE_W * k / 2.0;
            let (line, typing) = s.ghost_line_shown();
            // The card takes the room the stage leaves beside the paper, up to its widest: a
            // narrow window gets a taller, narrower bubble rather than a clipped one.
            let max_text = ((w - tw) / 2.0 - GHOST_GAP - GHOST_EDGE - 2.0 * GHOST_PAD_X - 1.0).min(GHOST_TEXT_W);
            let talk = Talk::new(&line, typing, max_text);
            let stack = (talk.w.max(GHOST_BODY_W), talk.h + GHOST_TAIL_GAP + GHOST_BODY_H);
            // A place the user dragged it to wins over the round of spots; the next drift clears it.
            let place = match s.ghost_at {
                Some(at) => GhostPlace::Whole(inside_stage(at, (w, h), stack)),
                None => ghost_place(s.ghost_spot, (w, h), stack, ticket, case_right),
            };
            match place {
                GhostPlace::Whole((x, y)) => {
                    drop_zone = drop_zone.child(ghost_flying(s, gaze, Some(&talk), (x, y), (w, h), stack).pin(Some(x), Some(y), None, None));
                }
                GhostPlace::Face((x, y)) => {
                    let face = (GHOST_BODY_W, GHOST_BODY_H);
                    drop_zone = drop_zone.child(ghost_flying(s, gaze, None, (x, y), (w, h), face).pin(Some(x), Some(y), None, None));
                }
                GhostPlace::Hidden => {}
            }
        }
    }
    // The pill and the key travel as one row: on the case while it can wear them, beside it once a
    // long ticket has shrunk it. The row keeps its key across both places, so Rune glides the pair
    // from one to the other instead of jumping. A label stage builds its controls in the flow under
    // the turned case (`label_controls`), where they never turn, so the overlay is not added there:
    // doing so would draw the pill and the Print key twice on the same stage.
    if !side_by_side {
        if let Some(controls) = print_controls(s, stage_w, drop_h, k, turned) {
            drop_zone = drop_zone.child(controls);
        }
    }
    if armed {
        // A picture lands on the preview; a theme tile dressed the text and prints it.
        let what = if s.drag.as_ref().is_some_and(|d| d.theme.is_some()) { "Let go to print" } else { "Let go to preview" };
        drop_zone = drop_zone.child(row().absolute(0.0, 0.0).w_full().h(48.0).center().child(text(what.to_string()).font(type_::TITLE).color(coral())));
    }
    let write_bar_el = if has_paper && !s.printing.is_busy() { write_bar(s) } else { None };
    let mut stage = col()
        .key("stage-frame")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .radius(24.0)
        .gradient(if armed { coral().alpha(0.16) } else { Color::WHITE.alpha(0.05) }, if armed { pink().alpha(0.10) } else { Color::WHITE.alpha(0.015) })
        .border(if armed { 2.0 } else { 1.0 }, if armed { coral() } else if dragging { coral().alpha(0.45) } else { Color::WHITE.alpha(0.07) })
        .padding(18.0)
        .gap(12.0)
        .items_center()
        .child(drop_zone);
    if let Some(bar) = write_bar_el {
        stage = stage.child(bar);
    }
    // The mode's settings, at the very bottom of the stage: under the paper and under the write
    // bar, so neither the "Write on" picker nor its box is ever covered.
    stage = stage.child(settings_strip(s));
    // The printing states live under the stage now that the big Print button is a top-bar pill.
    let mut out = col().w_full().grow(1.0).min_h(0.0).gap(10.0).child(stage);
    if s.printing.is_busy() {
        out = out.child(Progress::new(Some(s.printing.progress())).build(kit()));
    }
    if let Some(e) = s.error.clone() {
        out = out.child(error_banner(s, e));
    }
    out
}

/// Move the crop window by dragging the picture on the paper: the window moves the opposite way
/// (pulling the picture pulls what is shown with it). The result is clamped to [0, 1].
fn crop_drag(c: &mut render::Crop, dx: f32, dy: f32, w: f32, h: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    c.x = (c.x - dx / w).clamp(0.0, 1.0);
    c.y = (c.y - dy / h).clamp(0.0, 1.0);
}

/// The empty Codes sheet is one big box for the code: a press starts the code text on the paper,
/// and the box stays up while the first characters turn the blank sheet into the real code.
fn code_write_layer(s: &App, full: f32) -> El {
    let w_px = preview_w(s);
    let mut layer = col().absolute(0.0, 0.0).size(w_px, full);
    if s.editing.is_some() {
        layer = layer.on_press(|s: &mut App| s.stop_editing());
    }
    let (x, y) = (render::PAPER_MARGIN as f32, render::TEETH as f32);
    let (w, h) = ((crate::protocol::WIDTH_DOTS - 2 * render::PAPER_MARGIN) as f32, full - render::TEETH as f32);
    if s.editing.as_deref() == Some("_code") {
        let k = style::paper_kit();
        let mut font = k.ty.ui_compact;
        font.size = 22.0;
        font.line_height = 30.0;
        let ink = k.c.text_base;
        let input = rune::text_input(&s.code_text, font)
            .on_input_key(print_key)
            .key("input")
            .w_full()
            .color(ink)
            .placeholder("A link, some text, or digits for EAN-13", k.c.text_faint)
            .caret(ink, k.selection())
            .on_edit(|s: &mut App| &mut s.code_text)
            .on_input_change(|s: &mut App, _| s.touch());
        layer = layer.child(el().key("region-code").absolute(x, y).size(w, h).radius(4.0).border(1.0, coral()).fill(style::rgb(0xfaf6ec)).child(input));
    } else {
        layer = layer.child(el().key("region-code").absolute(x, y).size(w, h).radius(4.0).border(1.0, coral().alpha(0.3)).on_press(|s: &mut App| s.edit_region("_code")));
    }
    layer
}

/// The picture being dragged out of the gallery, following the pointer. It does not take presses,
/// so the drag it belongs to carries on.
fn drag_ghost(s: &App) -> Option<El> {
    let (d, (x, y)) = (s.drag.as_ref()?, s.pointer?);
    if d.travel < crate::app::DRAG_MIN {
        return None;
    }
    let tile = el().key("drag-ghost").size(THUMB, THUMB).radius(12.0).fill(Color::BLACK.alpha(0.5)).opacity(0.94).rotate(0.07).shadow(10.0, 26.0, Color::BLACK.alpha(0.6));
    let thumb = match d.theme {
        Some(theme) => s.theme_thumbs.get(&theme),
        None => s.gallery.thumbs.get(&d.path),
    };
    let tile = match thumb {
        Some(image) => tile.pixels_fit(image.pixels(), SurfaceFit::Fit),
        None => tile,
    };
    Some(tile.absolute(x - THUMB / 2.0, y - THUMB / 2.0))
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    // The height the printer and its ticket actually get: the drop zone (`stage`), not the stage
    // frame, which also holds the write bar and the gaps. In coarse 8 pt steps: a few points more
    // or less do not rebuild the view.
    let drop_h = cx.layout_with("stage", |r| (r[3] / 8.0).round() as i32).map(|q| q as f32 * 8.0);
    let stage_w = cx.layout_with("stage-frame", |r| (r[2] / 8.0).round() as i32).map(|q| q as f32 * 8.0);
    // Where the ghost's eyes should look: the pointer, from the face's centre (the rect is the
    // previous frame's, which is fine for a glance).
    let face = cx.layout_with("ghost-face", |r| (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5));
    let gaze = match (face, s.pointer_at) {
        (Some((fx, fy)), Some((px, py))) => (((px - fx) / 220.0).clamp(-1.0, 1.0), ((py - fy) / 160.0).clamp(-1.0, 1.0)),
        _ => (0.0, 0.0),
    };
    // One bar on top, then the three zones of the open scene — galleries, the stage, printer and
    // print settings — or the stage alone when the panels are shut.
    let mut body = col()
        .w_full()
        .h_full()
        .padding(24.0)
        .gap(14.0)
        .gradient(style::bg_top(), style::bg_bottom());
    // Each palette has its own weather, behind every panel; a ticket's family dresses it while the
    // Text tab shows that ticket. Effects off means a flat, still window.
    if s.fx_on {
        body = body.child(fx::ambient(s.palette, ticket_family(s)).absolute(0.0, 0.0).w_full().h_full());
    }
    body = body.child(top_bar(s, drop_h, stage_w));
    if let Some(update) = &s.update {
        body = body.child(update_banner(update));
    }
    let mut middle = row().w_full().grow(1.0).min_h(0.0).gap(16.0);
    if s.drawer_open {
        middle = middle.child(themes_panel(s));
    }
    middle = middle.child(paper(s, drop_h, stage_w, gaze));
    if s.drawer_open {
        middle = middle.child(right_panels(s));
    }
    body = body
        .child(middle)
        // Pictures dropped on the window from another app: shown here, ready to print.
        .on_drop_files(|s: &mut App, files, _| s.drop_files(files))
        .on_pointer_move(|s: &mut App, p| s.pointer_at = Some((p.x, p.y)));
    if let Some(ghost) = drag_ghost(s) {
        body = body.child(ghost);
    }
    // Which build this is, bottom-right over the background and clear of the ticket. Pinned (not
    // laid out in the column), so resizing the window does not move it, and plain text with no
    // press handler, so clicks go through to whatever is under it.
    body = body.child(
        text(version::short())
            .font(STAMP_FONT)
            .color(dim().alpha(0.6))
            .pin(None, None, Some(18.0), Some(10.0))
            .help(version::long()),
    );
    rune_kit::root(body, Vec::<El>::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_dialog_request_is_not_an_error_worth_a_banner() {
        // Rune's own wording for a request made while a dialog is showing.
        assert!(is_busy_dialog("a file dialog is already open"));
        // The other refusals a picker can answer with stay visible.
        assert!(!is_busy_dialog("cannot open a file dialog while the window is hidden"));
        assert!(!is_busy_dialog("native file dialogs are unsupported on web"));
        assert!(!is_busy_dialog("no portal"));
        assert!(!is_busy_dialog(""));
    }

    #[test]
    fn the_side_panels_are_the_same_width() {
        // The galleries and the printer/settings column frame the stage: one width for both, so the
        // paper stays centred and a switch between tabs does not shift it.
        assert_eq!(SIDE_PANEL_W, 300.0);
    }

    #[test]
    fn the_signal_gauge_counts_its_bars_from_the_dbm() {
        // Nothing heard yet: no bar at all, and the gauge shows its scale dimmed.
        assert_eq!(signal_bars(None), 0);
        // Every tier, and both sides of each boundary.
        assert_eq!(signal_bars(Some(-40)), 4, "a strong link is four bars");
        assert_eq!(signal_bars(Some(-55)), 4, "−55 dBm is still good");
        assert_eq!(signal_bars(Some(-56)), 3, "just under good is fair");
        assert_eq!(signal_bars(Some(-70)), 3, "−70 dBm is still fair");
        assert_eq!(signal_bars(Some(-71)), 2, "just under fair is weak");
        assert_eq!(signal_bars(Some(-82)), 2, "−82 dBm is still weak");
        assert_eq!(signal_bars(Some(-83)), 1, "just under weak is one bar");
        assert_eq!(signal_bars(Some(-100)), 1, "a faint link keeps its bar");
    }

    #[test]
    fn the_controls_sit_on_the_big_case_and_beside_the_small_one() {
        let (stage, dh) = (Some(1140.0), Some(700.0));
        let w = 1140.0 - 2.0 * STAGE_PAD;
        // A full-size case wears the pair at the pill's old corner.
        let (x, y, scale) = controls_place(stage, dh, 1.0, false).expect("a laid-out stage has a place");
        let case = (w / 2.0 - MACHINE_W / 2.0, 700.0 - MACHINE_H);
        assert!((scale - 1.0).abs() < 1e-6, "full size on a full case: {scale}");
        assert!((x - (case.0 + KEY_X)).abs() < 1e-3, "not on the case's left front: {x}");
        assert!((y - (case.1 + KEY_Y)).abs() < 1e-3, "not on the case's front: {y}");
        // Half a case is too small: the pair stands beside it, at full size, on its left.
        let (x, y, scale) = controls_place(stage, dh, 0.5, false).expect("a laid-out stage has a place");
        let case_left = w / 2.0 - MACHINE_W * 0.5 / 2.0;
        assert!((scale - 1.0).abs() < 1e-6, "shrunk beside the case: {scale}");
        assert!(x + controls_w() <= case_left, "the pair runs into the case: {x}");
        assert!(y > 0.0 && y + PRINT_H < 700.0, "the pair is off the stage: {y}");
        // The threshold is the case's own: 0.85 still wears them, 0.84 does not.
        assert!(controls_place(stage, dh, 0.85, false).is_some_and(|(.., s)| s < 0.999));
        assert!(controls_place(stage, dh, 0.84, false).is_some_and(|(.., s)| (s - 1.0).abs() < 1e-6));
        // Whatever the window and the ticket, the pair stays whole inside the stage.
        for stage_w in [700.0, 900.0, 1140.0, 1400.0] {
            for drop_h in [200.0, 320.0, 500.0, 800.0] {
                for k in [0.25, 0.5, 0.84, 0.85, 0.95, 1.0] {
                    for turned in [false, true] {
                        let (x, y, scale) = controls_place(Some(stage_w), Some(drop_h), k, turned).expect("a place");
                        let w = stage_w - 2.0 * STAGE_PAD;
                        assert!(x >= 0.0 && y >= 0.0, "{stage_w}x{drop_h} k{k}: off the top-left ({x}, {y})");
                        assert!(x + controls_w() * scale <= w + 1e-3, "{stage_w}x{drop_h} k{k}: past the right edge");
                        assert!(y + PRINT_H * scale <= drop_h + 1e-3, "{stage_w}x{drop_h} k{k}: past the bottom");
                    }
                }
            }
        }
        // Before the first layout there is no place to put them: the row is not built yet.
        assert_eq!(controls_place(None, dh, 1.0, false), None);
        assert_eq!(controls_place(stage, None, 1.0, false), None);
    }

    #[test]
    fn a_label_has_a_short_slit() {
        // The label case takes one label in its slot, so its slot is 160 wide and centred in the
        // picture where the pocket case's runs nearly the whole top: the band of paper, the bare
        // stub and the chrome all take their x and width from this, or they spill past the case.
        assert_eq!(slit_x_w(true), (220.0, 160.0), "the label case's short, centred slot");
        assert_eq!(slit_x_w(false), (64.0, 472.0), "the pocket case's long slit");
        assert!(slit_x_w(true).1 < slit_x_w(false).1, "a label's slot is the shorter of the two");
        // The lip, and so the tag's left edge, does not move: the slot's short end falls on the
        // same edge of the turned case as the long slot's did.
        assert!((label_tag_left() - (LABEL_CASE_W - SLIT_FRONT)).abs() < 1e-6, "the tag no longer starts on the lip");
    }

    #[test]
    fn a_label_printer_sits_beside_the_label() {
        // The tag a label printer takes: a landscape sheet 252 pt across and 120 tall.
        let mut s = App::default();
        s.preferred_kind = Some(protocol::Kind::Label);
        s.composed = Ok(protocol::Bitmap::blank_at(112, 240));
        assert!(s.on_a_label_printer(), "the chosen printer is the label one");
        assert!(label_stage_is_side_by_side(s.on_a_label_printer()), "a label stands beside its case");
        let (sheet_w, sheet_h) = (preview_w(&s), s.ticket_height());
        assert_eq!((sheet_w, sheet_h), (252.0, 120.0), "the tag is the landscape sheet the render makes");
        // The case is turned a quarter turn: its 600 x 290 box looks 290 across by 600 tall, and the
        // box is pushed out into that footprint by half the difference on each axis, so the two
        // middles meet.
        assert_eq!((LABEL_CASE_W, LABEL_CASE_H), (MACHINE_H, MACHINE_W), "a quarter turn swaps the sides");
        let (case_x, case_y) = label_case_origin();
        assert!((case_x + MACHINE_W / 2.0 - LABEL_CASE_W / 2.0).abs() < 1e-6, "the box is not centred across: {case_x}");
        assert!((case_y + MACHINE_H / 2.0 - LABEL_CASE_H / 2.0).abs() < 1e-6, "the box is not centred down: {case_y}");
        // The tag leaves the slot's lip on the turned case's right flank and runs on past it, so the
        // pair is wider than the case alone.
        let tag_left = label_tag_left();
        assert!((tag_left - (LABEL_CASE_W - SLIT_FRONT)).abs() < 1e-6, "the tag does not start on the lip: {tag_left}");
        assert!(tag_left > LABEL_CASE_W / 2.0, "the tag starts on the case's right half: {tag_left}");
        assert!(label_pair_w(sheet_w) > LABEL_CASE_W, "the tag never leaves the case: {}", label_pair_w(sheet_w));
        assert!(sheet_h < MACHINE_H, "a tag is shorter than the case");
        // The pair and the controls under it fit the stage's height: full size on the app's 700 pt
        // stage, the pair shrunk whole on a short one — the controls themselves never shrink.
        assert!((label_scene_k(Some(700.0)) - 1.0).abs() < 1e-6, "a full stage shows the label whole");
        for drop_h in [700.0, 560.0, 460.0, 400.0] {
            let k = label_scene_k(Some(drop_h));
            assert!((MACHINE_MIN..=1.0).contains(&k), "{drop_h}: k {k} out of range");
            let used = LABEL_CASE_H * k + LABEL_CONTROLS_GAP + PRINT_H + 8.0;
            assert!(used <= drop_h + 1e-3, "{drop_h}: pair, gap and controls take {used}");
        }
        // The pair is narrower than a full stage: it stands there whole, nothing clipped.
        let w = 1140.0 - 2.0 * STAGE_PAD;
        assert!(label_pair_w(sheet_w) <= w, "the pair runs off the stage: {}", label_pair_w(sheet_w));
        assert!(controls_w() <= label_pair_w(sheet_w), "the controls are wider than the pair: {}", controls_w());
    }

    #[test]
    fn an_empty_family_header_is_not_drawn() {
        let pocket = render::themes_for(render::Target::Pocket);
        let label = render::themes_for(render::Target::Label);
        // Every pocket family has something on the roll, and nothing a label printer can print.
        for family in [Family::Shop, Family::Travel, Family::OldWorld, Family::Papers, Family::Forms, Family::Retro] {
            assert!(family_has_themes(&pocket, family), "{} lost its themes on the roll", family.label());
            assert!(!family_is_shown(&label, family, render::Target::Label), "{} would name itself over an empty row on a label printer", family.label());
        }
        // And the other way round: "Labels" has no place on the roll.
        assert!(family_has_themes(&label, Family::Labels), "Labels lost its label themes");
        assert!(family_is_shown(&label, Family::Labels, render::Target::Label), "Labels is not drawn on a label printer");
        assert!(!family_has_themes(&pocket, Family::Labels), "a label theme leaked into the roll's list");
        assert!(!family_is_shown(&pocket, Family::Labels, render::Target::Pocket), "Labels would name itself over an empty row on the roll");
        // "My themes" is the folder's panel: drawn on the roll even with no file of the user's,
        // and left out on a label printer, where a custom pocket theme could never appear.
        assert!(family_is_shown(&[], Family::Mine, render::Target::Pocket), "the roll loses its folder buttons");
        assert!(!family_is_shown(&label, Family::Mine, render::Target::Label), "the label printer shows an empty folder panel");
        // Nothing empty, in either mode: every drawn header names a group with a theme under it.
        for (target, themes) in [(render::Target::Pocket, &pocket), (render::Target::Label, &label)] {
            for family in Family::ALL {
                assert!(
                    !family_is_shown(themes, family, target) || family_has_themes(themes, family) || family == Family::Mine,
                    "{target:?}: {} is drawn with nothing under it",
                    family.label()
                );
            }
        }
    }

    #[test]
    fn the_label_stage_draws_its_controls_once() {
        // The pair (the connection pill and the Print key) is built once: `label_stage` puts it in
        // the flow under the turned case (`label_controls`), and `paper` skips the overlay there.
        // A label stage with its sheet is the case that owns the pair.
        assert!(label_stage_draws_controls(true, true), "a label stage with a sheet owns the pair");
        // A pocket stage, with or without paper, keeps the overlay that travels over the stage.
        assert!(!label_stage_draws_controls(false, true), "a pocket ticket keeps the overlay");
        assert!(!label_stage_draws_controls(false, false), "an empty pocket stage keeps the overlay");
        // Before the first sheet a label printer is drawn as the pocket layout (no label stage is
        // built), so the overlay is still its only pair: dropping it would leave the stage with no
        // pill and no key.
        assert!(!label_stage_draws_controls(true, false), "an empty label stage still needs the overlay");
    }

    #[test]
    fn a_pocket_ticket_still_stands_above_the_printer() {
        // A pocket ticket is the 384-dot sheet, taller than the band the case has under its slit:
        // it stands out of the slit, above the case, as it always has.
        let mut s = App::default();
        s.composed = Ok(protocol::Bitmap::blank_at(protocol::WIDTH_DOTS, 240));
        assert!(!s.on_a_label_printer(), "the pocket printer keeps the pocket layout");
        assert!(!label_stage_is_side_by_side(s.on_a_label_printer()), "a pocket ticket stands above the case");
        assert_eq!(preview_w(&s), render::PREVIEW_WIDTH as f32, "a pocket preview keeps its usual width");
        let ticket_h = s.ticket_height();
        assert!(ticket_h > MACHINE_H - SLIT_FRONT, "a pocket ticket stands above its case: {ticket_h}");
        // A pocket stage has no shift to carry (`controls_place` owns the pocket layout only), so
        // the pair lands where it always has: on the front of the centred case while it can wear it ...
        let (stage, dh) = (Some(1140.0), Some(700.0));
        let w = 1140.0 - 2.0 * STAGE_PAD;
        let case_left = w / 2.0 - MACHINE_W / 2.0;
        let (x, y, scale) = controls_place(stage, dh, 1.0, false).expect("a laid-out stage has a place");
        assert!((scale - 1.0).abs() < 1e-6, "full size on a full case: {scale}");
        assert!((x - (case_left + KEY_X)).abs() < 1e-3, "not on the centred case's front: {x}");
        assert!((y - (700.0 - MACHINE_H + KEY_Y)).abs() < 1e-3, "not on the case's front: {y}");
        // ... and beside its left, at full size, once a long ticket has shrunk it.
        let (x, y, scale) = controls_place(stage, dh, 0.5, false).expect("a laid-out stage has a place");
        let small_case = w / 2.0 - MACHINE_W * 0.5 / 2.0;
        assert!((scale - 1.0).abs() < 1e-6, "shrunk beside the case: {scale}");
        assert!(x + controls_w() <= small_case, "the pair runs into the case: {x}");
        assert!(y > 0.0 && y + PRINT_H < 700.0, "the pair is off the stage: {y}");
    }

    /// The ticket of a stage with a drawer closed at 1180 pt: 384 wide, standing nearly the whole
    /// height, its bottom on the slit.
    fn ticket(w: f32, h: f32) -> (f32, f32, f32, f32) {
        let tw = render::PREVIEW_WIDTH as f32;
        ((w - tw) / 2.0, 90.0, tw, h - 300.0)
    }

    /// The column the ghost takes for `line`: its card and the face under it.
    fn stack(line: &str) -> (f32, f32) {
        let talk = Talk::new(line, false, GHOST_TEXT_W);
        (talk.w.max(GHOST_BODY_W), talk.h + GHOST_TAIL_GAP + GHOST_BODY_H)
    }

    #[test]
    fn the_comic_bubble_is_sized_to_its_talk_and_never_cuts_it() {
        for line in ["Blank page energy.", "I'd print a receipt for your thoughts.", "Drop a photo here. I'll make it crunchy."] {
            let talk = Talk::new(line, false, GHOST_TEXT_W);
            let (tw, th) = rune::text::measure(&talk.text, &[], type_::LABEL, Some(GHOST_TEXT_W));
            assert!(talk.w >= tw + 2.0 * GHOST_PAD_X, "{line:?}: the card is narrower than its line");
            assert!(talk.h >= th + 2.0 * GHOST_PAD_Y, "{line:?}: the card is shorter than its talk");
            assert!(talk.w <= GHOST_TEXT_W + 2.0 * GHOST_PAD_X + 1.0, "{line:?}: wider than the wrap width");
            let line_h = rune::text::measure("Hg", &[], type_::LABEL, None).1;
            assert!(th <= line_h * GHOST_TEXT_LINES as f32 + 1.0, "{line:?}: more than three lines");
        }
    }

    #[test]
    fn a_narrower_window_gets_a_narrower_card_that_still_holds_its_words() {
        // The 900 pt window leaves about 93 pt of text beside a full ticket: the card folds the
        // line onto more lines instead of running past the stage.
        let line = "Bluetooth is playing hide and seek.";
        let talk = Talk::new(line, false, 93.0);
        assert!(talk.w <= 93.0 + 2.0 * GHOST_PAD_X + 1.0, "the card is wider than the room: {}", talk.w);
        assert!(talk.text.contains("seek"), "the end of the line was lost: {:?}", talk.text);
        let (tw, th) = rune::text::measure(&talk.text, &[], type_::LABEL, Some(93.0));
        assert!(talk.w >= tw + 2.0 * GHOST_PAD_X && talk.h >= th + 2.0 * GHOST_PAD_Y, "the talk is clipped");
        assert!(talk.h > Talk::new(line, false, GHOST_TEXT_W).h, "the narrow card is not taller");
    }

    #[test]
    fn a_line_too_long_for_the_bubble_stops_at_three_lines_with_an_ellipsis() {
        let line = "This is a line no small comic bubble could ever hope to hold whole, not even in three lines of its talk, so it ends";
        let talk = Talk::new(line, false, GHOST_TEXT_W);
        assert!(talk.text.ends_with('…'), "the line ran on: {:?}", talk.text);
        assert!(talk.text.len() < line.len(), "nothing was cut");
        let line_h = rune::text::measure("Hg", &[], type_::LABEL, None).1;
        let (_, th) = rune::text::measure(&talk.text, &[], type_::LABEL, Some(GHOST_TEXT_W));
        assert!(th <= line_h * GHOST_TEXT_LINES as f32 + 1.0, "the cut still takes four lines");
    }

    #[test]
    fn the_floating_ghost_never_covers_the_ticket() {
        let (w, h) = (1096.0, 700.0);
        let t = ticket(w, h);
        let stack = stack("Blank page energy.");
        for spot in 0..crate::app::GHOST_SPOTS {
            let GhostPlace::Whole((x, y)) = ghost_place(spot, (w, h), stack, t, w / 2.0 + MACHINE_W / 2.0) else {
                panic!("spot {spot}: a wide stage holds the whole ghost");
            };
            assert!(!covers((x, y), stack, t), "spot {spot}: the ghost sits on the paper at ({x}, {y})");
            assert!(x >= GHOST_EDGE && y >= GHOST_EDGE, "spot {spot}: off the stage at ({x}, {y})");
            assert!(x + stack.0 <= w - GHOST_EDGE && y + stack.1 <= h - GHOST_EDGE, "spot {spot}: the column runs off the stage");
        }
    }

    #[test]
    fn a_narrow_stage_refuses_every_spot_rather_than_draw_half_a_ghost() {
        // The stage of the drawer-open window on a long ticket: 527 pt inside, the ticket runs the
        // whole height and leaves 71 pt on either side, so neither the bubble nor the face fits.
        let (w, h) = (527.0, 700.0);
        let t = (71.0, 60.0, 384.0, 620.0);
        let case_right = w / 2.0 + MACHINE_W * 0.62 / 2.0;
        for spot in 0..crate::app::GHOST_SPOTS {
            assert_eq!(ghost_place(spot, (w, h), stack("Blank page energy."), t, case_right), GhostPlace::Hidden, "spot {spot} found room on a stage with none");
        }
    }

    #[test]
    fn a_short_ticket_leaves_room_above_it() {
        // A short ticket low on the stage: the spot above it is the only one that fits on the right
        // of nothing, and the ghost clears the ink there.
        let (w, h) = (527.0, 700.0);
        let t = (71.0, 500.0, 384.0, 120.0);
        let stack = stack("Blank page energy.");
        let GhostPlace::Whole((x, y)) = ghost_place(2, (w, h), stack, t, w / 2.0 + 200.0) else {
            panic!("above the ticket");
        };
        assert!(y + stack.1 <= t.1, "the ghost's foot is over the ticket: {:?}", (x, y));
    }

    #[test]
    fn a_stage_too_cramped_for_the_bubble_keeps_the_face() {
        // 120 pt either side of the paper: the face fits beside it, the comic bubble does not.
        let (w, h) = (624.0, 700.0);
        let t = (120.0, 60.0, 384.0, 540.0);
        let stack = stack("Blank page energy.");
        let case_right = w / 2.0 + MACHINE_W * 0.62 / 2.0;
        let place = ghost_place(0, (w, h), stack, t, case_right);
        let GhostPlace::Face((x, y)) = place else { panic!("expected the face alone, got {place:?}") };
        assert!(!covers((x, y), (GHOST_BODY_W, GHOST_BODY_H), t), "the face sits on the paper");
        assert!(x + GHOST_BODY_W <= w - GHOST_EDGE && y + GHOST_BODY_H <= h - GHOST_EDGE, "the face runs off the stage");
    }

    #[test]
    fn every_spot_is_pulled_inside_the_stage_at_every_width() {
        for w in [1096.0, 900.0, 700.0] {
            let (h, t) = (700.0, ticket(w, 700.0));
            let stack = stack("Bluetooth is playing hide and seek.");
            let case_right = w / 2.0 + MACHINE_W / 2.0;
            for spot in 0..crate::app::GHOST_SPOTS {
                let (x, y, sw, sh) = match ghost_place(spot, (w, h), stack, t, case_right) {
                    GhostPlace::Whole((x, y)) => (x, y, stack.0, stack.1),
                    GhostPlace::Face((x, y)) => (x, y, GHOST_BODY_W, GHOST_BODY_H),
                    GhostPlace::Hidden => continue,
                };
                assert!(x >= GHOST_EDGE - 0.01 && y >= GHOST_EDGE - 0.01, "at {w}: spot {spot} crosses the top-left ({x}, {y})");
                assert!(x + sw <= w - GHOST_EDGE + 0.01 && y + sh <= h - GHOST_EDGE + 0.01, "at {w}: spot {spot} crosses the bottom-right");
                assert!(!covers((x, y), (sw, sh), t), "at {w}: spot {spot} sits on the paper");
            }
        }
    }

    #[test]
    fn the_sheet_rises_by_its_progress_and_never_past_the_ticket() {
        let full = 600.0;
        assert_eq!(rise_height(full, 0.0), 24.0, "the first frame already shows a stub of paper");
        assert!((rise_height(full, 0.45) - 270.0).abs() < 0.01, "45% of the ticket is out");
        assert_eq!(rise_height(full, 1.0), 600.0, "the whole ticket, foot on the slit");
        assert_eq!(rise_height(full, 1.7), 600.0, "a progress past the end shows nothing extra");
        assert_eq!(rise_height(10.0, 1.0), 10.0, "a ticket shorter than the stub is out whole");
    }

    /// A segment gives 12 pt of room each side of its label (`rune-kit`'s segmented control).
    const SEG_PAD: f32 = 24.0;
    /// The face a segmented control draws its labels in.
    fn segment_font() -> rune::TextStyle {
        rune::TextStyle { line_height: 13.0, ..kit().ty.ui }
    }

    #[test]
    fn the_density_choice_holds_its_three_words_whole() {
        // `.fit()` gives every segment the widest label's width: three of them must stay in a card.
        // It replaced `DRAWER - 150`, which left 70 pt a segment and cut "Medium" to "Medi…".
        let (w, _) = rune::text::measure("Medium", &[], segment_font(), None);
        let needed = 3.0 * (w + SEG_PAD);
        assert!(needed <= COL_W, "Light / Medium / Dark need {needed:.0} pt, a card gives {COL_W}");
    }

    #[test]
    fn the_label_print_space_has_no_copies() {
        // One print is one label: a copies count would only lie there.
        assert!(!print_space_shows_copies(true), "a label printer prints one label at a time");
        // The pocket printer still prints a series.
        assert!(print_space_shows_copies(false), "the pocket printer keeps its copies");
    }

    #[test]
    fn a_choice_label_is_never_cut_in_its_share_of_a_card() {
        // `.width(None)` fills the card, so every segment gets an equal share of `COL_W`. These
        // sets fit it whole at the kit's 12 pt segment padding. Two do not and the kit offers no
        // padding knob: the Font set ("Mono" 60, "Script" 61 pt in 59) and the Code set
        // ("Code 128" 82 pt in 74). They need a wider control than a card can give.
        let looks = LOOKS.map(|(n, _)| n);
        let sizes = ["S", "M", "L", "XL"];
        let aligns = ["Left", "Center", "Right"];
        let levels = EcLevel::ALL.map(EcLevel::label);
        for labels in [&looks[..], &sizes[..], &aligns[..], &levels[..]] {
            let share = COL_W / labels.len() as f32;
            for label in labels {
                let (w, _) = rune::text::measure(label, &[], segment_font(), None);
                assert!(w + SEG_PAD <= share + 0.5, "{label:?} needs {:.0} pt of {share:.0}", w + SEG_PAD);
            }
        }
    }

    #[test]
    fn the_window_says_which_device_it_is_talking_to() {
        // The pocket printer: its name, then what it is, then the signal.
        let mut s = App { link: Link::Online("Mini Pocket Printer_BLE".into()), ..App::default() };
        s.rssi = Some(-51);
        assert!(!label_online(&s), "the pocket printer does print");
        assert_eq!(device_line(&s).as_deref(), Some("Mini Pocket Printer_BLE · pocket printer · -51 dBm"));
        // The label printer: the same shape, and it cannot print.
        let mut s = App { link: Link::Online("ML Printer_BLE".into()), ..App::default() };
        s.rssi = Some(-51);
        assert!(label_online(&s), "the label printer does not print");
        assert_eq!(device_line(&s).as_deref(), Some("ML Printer_BLE · label printer · -51 dBm"));
        // Offline: there is nobody to name.
        let s = App::default();
        assert!(!label_online(&s));
        assert_eq!(device_line(&s), None);
    }

    #[test]
    fn the_label_preview_is_a_wide_landscape_sheet() {
        // A label bitmap is 112 dots across by 240 down for the head, but the tag is read lying
        // down: the preview shows it 240 dots wide, on its liner, 112 tall with a pad above/below.
        let mut s = App::default();
        s.composed = Ok(protocol::Bitmap::blank_at(112, 240));
        assert_eq!(preview_w(&s), 252.0, "240 dots across plus the liner margins");
        assert_eq!(s.ticket_height(), 120.0, "112 dots down plus the pad above and below");
        assert_ne!(preview_w(&s), render::PREVIEW_WIDTH as f32, "a label must not use the pocket width");

        // A pocket bitmap keeps the usual sheet width.
        let mut pocket = App::default();
        pocket.composed = Ok(protocol::Bitmap::blank_at(protocol::WIDTH_DOTS, 240));
        assert_eq!(preview_w(&pocket), render::PREVIEW_WIDTH as f32, "a pocket preview keeps its usual width");
        assert_eq!(pocket.ticket_height(), (240 + render::TEETH) as f32, "and its toothed edge");
    }

    #[test]
    fn dragging_the_label_picture_moves_the_crop_window_the_other_way() {
        let mut c = render::Crop {
            aspect: 30.0 / 14.0,
            x: 0.5,
            y: 0.5,
        };
        // tirer +40 px à droite sur 200 px → x = 0.3
        crop_drag(&mut c, 40.0, 0.0, 200.0, 200.0);
        assert!((c.x - 0.3).abs() < 1e-6, "x doit reculer quand on tire à droite: {}", c.x);
        assert!((c.y - 0.5).abs() < 1e-6);
        // tirer vers le bas → y diminue
        crop_drag(&mut c, 0.0, 50.0, 200.0, 200.0);
        assert!((c.y - 0.25).abs() < 1e-6, "y doit diminuer quand on tire vers le bas: {}", c.y);
        // clamps à 0 et 1
        crop_drag(&mut c, 10_000.0, -10_000.0, 200.0, 200.0);
        assert!((c.x - 0.0).abs() < 1e-6, "x clamp à 0: {}", c.x);
        assert!((c.y - 1.0).abs() < 1e-6, "y clamp à 1: {}", c.y);
        // ignore w/h <= 0
        let before = c;
        crop_drag(&mut c, 10.0, 10.0, 0.0, 100.0);
        crop_drag(&mut c, 10.0, 10.0, 100.0, -1.0);
        assert_eq!(c, before);
    }

    #[test]
    fn printer_meta_shows_short_kind_and_signal() {
        assert_eq!(printer_meta(protocol::Kind::Label, Some(-51)), "label · -51 dBm");
        assert_eq!(printer_meta(protocol::Kind::Pocket, None), "pocket");
    }
}
