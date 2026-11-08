//! The window: a top bar, the stage (the printer and its ticket), and the drawer of galleries.

use rune::tok::{motion, type_};
use rune::{Color, Cx, Drag, El, FileDialogOptions, Phase, ScrollSpec, SurfaceFit, Tx, col, el, memo, row, text};
use rune_kit::{Button, ButtonVariant, Kit, Progress, ScrollBar, SegmentedControl, Switch, TextArea, TextInput};

use crate::app::{App, Link, Mode};
use crate::fx;
use crate::jobs::clamp_copies;
use crate::render::{self, Align, CodeKind, Dither, EcLevel, Family, FontChoice, Theme};
use crate::style::{self, amber, coral, dim, ink, mint, panel, pink, red};
use crate::update::{self, Update};
use crate::version;
use crate::voice;

/// Width of the drawer on the left, in points. The stage takes the rest (all of it when closed).
const DRAWER: f32 = 360.0;
/// Width of the Print button in the top bar, and of the four mode tabs beside it.
const PRINT_W: f32 = 140.0;
const TABS_W: f32 = 252.0;
/// The build stamp is a footnote, smaller than any label.
const STAMP_FONT: rune::TextStyle = rune::TextStyle::new(10.5, 500.0);

pub fn kit() -> &'static Kit {
    style::kit()
}

/// A small, quiet, upper-case label.
fn caption(title: &str) -> El {
    text(title.to_uppercase()).font(type_::LABEL).color(dim())
}

fn field(title: &str, body: El) -> El {
    col().gap(8.0).child(caption(title)).child(body)
}

/// A rounded panel that groups controls.
fn card(body: El) -> El {
    col().w_full().padding(16.0).radius(18.0).fill(panel()).border(1.0, Color::WHITE.alpha(0.06)).child(body)
}

/// A segmented choice over `labels`; `pick` receives the index.
fn choice(labels: &[&str], selected: usize, pick: fn(&mut App, usize)) -> El {
    SegmentedControl::new(labels, Some(selected)).width(Some(DRAWER - 34.0)).on_pick(move |s: &mut App, i, _| pick(s, i)).build(kit())
}

/// A short segmented choice that may have nothing picked (the value is between its choices).
fn choice_small(labels: &[&str], selected: Option<usize>, pick: fn(&mut App, usize)) -> El {
    SegmentedControl::new(labels, selected).width(Some(150.0)).on_pick(move |s: &mut App, i, _| pick(s, i)).build(kit())
}

/// Size of a theme's picture in the gallery, in points.
const THEME_W: f32 = 92.0;
const THEME_H: f32 = 118.0;
/// Size of a picture in the Image gallery: landscape, so a photo is not squashed.
const PIC_W: f32 = 118.0;
const PIC_H: f32 = 92.0;

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
        .child(text(name.to_string()).font(type_::LABEL).color(if picked { ink() } else { dim() }).truncate().max_w(w - 28.0))
}

/// A gallery of the writing themes: a small picture of each ticket, grouped, as plain content in
/// the drawer. Click one to use it; pull it toward the printer to print your text in that
/// theme; star the ones you like and they gather in a Favorites group at the top.
fn theme_gallery(s: &App) -> El {
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
    let favorites = s.favorite_themes();
    if !favorites.is_empty() {
        let mut tiles = row().flex_wrap().gap(10.0);
        for theme in &favorites {
            tiles = tiles.child(tile(*theme, "fav-"));
        }
        grid = grid.child(text("★  Favorites".to_string()).font(type_::LABEL).color(amber())).child(tiles);
    }
    // The plain ticket leads the first group instead of taking a row of its own.
    for (i, family) in Family::ALL.into_iter().enumerate() {
        let mut tiles = row().flex_wrap().gap(10.0);
        let mut label = family.label().to_string();
        if i == 0 {
            tiles = tiles.child(tile(Theme::Plain, ""));
            label = format!("Plain  ·  {label}");
        }
        let themes: Vec<Theme> = Theme::all().into_iter().filter(|t| t.family() == Some(family)).collect();
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
/// it keeps a picker of its own in the panel.
fn avatar_pick(s: &App) -> Option<El> {
    let label = s.theme.fields().iter().find(|f| f.key == "avatar")?.label;
    let path = s.theme_edits.get("avatar").map(|e| e.display().0.trim().to_string()).unwrap_or_default();
    let name = path.rsplit(['\\', '/']).next().unwrap_or("").to_string();
    let choose = if path.is_empty() { "Choose a picture…" } else { "Change picture…" };
    Some(
        col()
            .gap(4.0)
            .child(text(label.to_string()).font(type_::LABEL).color(dim()))
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(
                        Button::new(choose)
                            .on_press_tx(move |_s: &mut App, tx: &mut Tx| {
                                let dialog = FileDialogOptions { title: "Choose a profile picture".into(), ..Default::default() }
                                    .filter("Pictures", &["png", "jpg", "jpeg", "bmp", "gif", "webp"]);
                                tx.open_file(dialog, move |s: &mut App, result, _| {
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
                    .child(text(if name.is_empty() { "a ring with the initials".to_string() } else { name }).font(type_::LABEL).color(dim()).truncate()),
            ),
    )
}

/// What the panel still holds of a theme's little form: the switches that change how a silly form
/// is drawn (Shuffle, Blank form, the closing line). They are not text on the ticket, so they have
/// no place on the paper. Every field itself is written on the ticket now (the bar catches the few
/// parts a theme cannot say where they are).
fn form_switches(s: &App) -> Option<El> {
    let mut out = col().gap(10.0);
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
        out = out.child(Switch::new(s.closing_line).label("Closing line").on_toggle(|s: &mut App| {
            s.closing_line = !s.closing_line;
            s.touch();
        }).build(kit()));
        any = true;
    }
    any.then_some(out)
}

fn text_controls(s: &App) -> El {
    let o = s.text_opts;
    let font_ix = FontChoice::ALL.iter().position(|f| *f == o.font).unwrap_or(0);
    let size_ix = [22.0, 32.0, 48.0, 72.0].iter().position(|p| *p == o.px).unwrap_or(1);
    let align_ix = [Align::Left, Align::Center, Align::Right].iter().position(|a| *a == o.align).unwrap_or(0);
    let mut out = col().gap(14.0).child(TextArea::new(&s.text).rows(3).placeholder("Type something to print").on_change(|s: &mut App, _| s.touch()).on_key(print_key).on_edit(|s: &mut App| &mut s.text).build(kit()).w_full());
    if let Some(pick) = avatar_pick(s) {
        out = out.child(pick);
    }
    out = out
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text("Out of ideas?".to_string()).font(type_::LABEL).color(dim()).grow(1.0))
                .child(Button::new("Surprise me").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.surprise_me()).build(kit())),
        );
    if s.theme != Theme::Plain {
        return out.child(text(s.theme.tagline().to_string()).font(type_::LABEL).color(dim()).wrap());
    }
    out.child(field(
        "Font",
        choice(&FontChoice::ALL.map(FontChoice::label), font_ix, |s, i| {
            s.text_opts.font = FontChoice::ALL[i];
            s.touch();
        }),
    ))
    .child(field(
        "Size",
        choice(&["S", "M", "L", "XL"], size_ix, |s, i| {
            s.text_opts.px = [22.0, 32.0, 48.0, 72.0][i];
            s.touch();
        }),
    ))
    .child(field(
        "Align",
        choice(&["Left", "Center", "Right"], align_ix, |s, i| {
            s.text_opts.align = [Align::Left, Align::Center, Align::Right][i];
            s.touch();
        }),
    ))
    .child(Switch::new(o.invert).label("White on black").on_toggle(|s: &mut App| {
        s.text_opts.invert = !s.text_opts.invert;
        s.touch();
    }).build(kit()))
}

/// Thumbnails per row in the gallery, and their size in points.
const THUMB: f32 = 80.0;

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
                        Err(e) => s.error = Some(e.to_string()),
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
                        Err(e) => s.error = Some(e.to_string()),
                    });
                })
                .build(kit()),
        );
    let mut body = col().w_full().gap(10.0).child(head);
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
            .child(Switch::new(g.auto).label("Print new pictures automatically").on_toggle(|s: &mut App| s.set_auto_print(!s.gallery.auto)).build(kit()))
            .child(text(note).font(type_::LABEL).color(if g.auto { coral() } else { dim() }).truncate());
    }
    col().w_full().gap(8.0).child(caption("Gallery")).child(body)
}

/// A horizontal slider from -1 to +1, shown as a percentage. Drag it, click on the track, or use
/// the arrow keys when it has focus.
fn slider(key: &'static str, title: &str, value: f32, set: fn(&mut App, f32)) -> El {
    const W: f32 = DRAWER - 34.0;
    const H: f32 = 22.0;
    const KNOB: f32 = 16.0;
    const BAR: f32 = 4.0;
    let travel = W - KNOB;
    let u = ((value + 1.0) / 2.0).clamp(0.0, 1.0);
    let at = move |x: f32| (((x - KNOB / 2.0) / travel).clamp(0.0, 1.0) * 2.0 - 1.0 * 1.0).clamp(-1.0, 1.0);
    let snap = |v: f32| (v * 100.0).round() / 100.0;
    let track = el()
        .key(key)
        .role("slider")
        .label(title.to_string())
        .focusable(true)
        .size(W, H)
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
        .child(el().key("track").size(W, BAR).absolute(0.0, (H - BAR) / 2.0).radius(BAR / 2.0).fill(Color::WHITE.alpha(0.12)))
        .child(el().key("fill").size(KNOB / 2.0 + u * travel, BAR).absolute(0.0, (H - BAR) / 2.0).radius(BAR / 2.0).gradient(coral(), pink()))
        .child(el().key("knob").size(KNOB, KNOB).absolute(u * travel, (H - KNOB) / 2.0).radius(KNOB / 2.0).fill(Color::WHITE).shadow(1.0, 4.0, Color::BLACK.alpha(0.5)));
    col()
        .gap(6.0)
        .child(row().w_full().child(caption(title).grow(1.0)).child(text(format!("{:+.0}%", value * 100.0)).font(type_::LABEL).color(dim())))
        .child(track)
}

/// The three looks most pictures want, so the dithering names stay out of the way.
const LOOKS: [(&str, Dither); 4] = [("Photo", Dither::FloydSteinberg), ("Meme", Dither::Atkinson), ("Line art", Dither::Threshold), ("Soft", Dither::Ordered)];

fn image_settings(s: &App) -> El {
    let o = s.pic_opts;
    let look_ix = LOOKS.iter().position(|(_, d)| *d == o.dither).unwrap_or(0);
    col()
        .gap(14.0)
        .child(field(
            "Look",
            choice(&LOOKS.map(|(n, _)| n), look_ix, |s, i| {
                s.pic_opts.dither = LOOKS[i].1;
                s.touch();
            }),
        ))
        .child(slider("brightness", "Brightness", o.brightness, |s, v| {
            s.pic_opts.brightness = v;
            s.touch();
        }))
        .child(slider("contrast", "Contrast", o.contrast, |s, v| {
            s.pic_opts.contrast = v;
            s.touch();
        }))
        .child(field(
            "White space before",
            row()
                .gap(10.0)
                .center_y()
                .child(Button::new("−").on_press(|s: &mut App| {
                    s.pic_opts.space_before = s.pic_opts.space_before.saturating_sub(1);
                    s.touch();
                }).build(kit()))
                .child(text(format!("{} mm", o.space_before)).font(type_::BODY).color(ink()).w(56.0))
                .child(Button::new("+").on_press(|s: &mut App| {
                    s.pic_opts.space_before = (s.pic_opts.space_before + 1).min(render::MAX_SPACE_MM);
                    s.touch();
                }).build(kit()))
                .child(choice_small(&["0", "5", "10", "20"], [0u8, 5, 10, 20].iter().position(|v| *v == o.space_before), |s, i| {
                    s.pic_opts.space_before = [0u8, 5, 10, 20][i];
                    s.touch();
                })),
        ))
        .child(
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
                .child(row().grow(1.0))
                .child(Switch::new(o.invert).label("Invert").on_toggle(|s: &mut App| {
                    s.pic_opts.invert = !s.pic_opts.invert;
                    s.touch();
                }).build(kit())),
        )
}

fn code_controls(s: &App) -> El {
    let o = s.code_opts;
    let kind_ix = CodeKind::ALL.iter().position(|k| *k == o.kind).unwrap_or(0);
    let ec_ix = EcLevel::ALL.iter().position(|e| *e == o.ec).unwrap_or(1);
    let mut body = col()
        .gap(14.0)
        .child(TextArea::new(&s.code_text).rows(3).placeholder("A link, some text, or digits for EAN-13").on_change(|s: &mut App, _| s.touch()).on_key(print_key).on_edit(|s: &mut App| &mut s.code_text).build(kit()).w_full())
        .child(field(
            "Type",
            choice(&CodeKind::ALL.map(CodeKind::label), kind_ix, |s, i| {
                s.code_opts.kind = CodeKind::ALL[i];
                s.touch();
            }),
        ));
    if o.kind == CodeKind::Qr {
        body = body.child(field(
            "Error correction",
            choice(&EcLevel::ALL.map(EcLevel::label), ec_ix, |s, i| {
                s.code_opts.ec = EcLevel::ALL[i];
                s.touch();
            }),
        ));
    }
    body.child(Switch::new(o.caption).label("Print the text underneath").on_toggle(|s: &mut App| {
        s.code_opts.caption = !s.code_opts.caption;
        s.touch();
    }).build(kit()))
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
                    .child(Button::new("Apply").on_press(|s: &mut App| s.apply_mcp_port()).build(kit()))
                    .child(text(state.to_string()).font(type_::LABEL).color(state_color)),
            )
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

fn terminal_controls(s: &App) -> El {
    let usage = |line: &str| {
        row().px(10.0).h(30.0).center_y().radius(8.0).fill(Color::BLACK.alpha(0.35)).child(text(line.to_string()).font(type_::BODY).color(coral()))
    };
    let state = if s.term.live { "Live session connected" } else { "No live session" };
    col()
        .gap(14.0)
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
        .child(field(
            "Text size",
            choice(&["Small", "Medium", "Large"], s.term.size, |s, i| {
                s.term.size = i;
                s.touch();
            }),
        ))
        .child(Switch::new(s.term.auto).label("Print live output as it arrives").on_toggle(|s: &mut App| {
            s.term.auto = !s.term.auto;
            s.touch();
        }).build(kit()))
        .child(
            row()
                .gap(12.0)
                .center_y()
                .child(
                    text(format!("{state}  ·  {} lines, {} not printed", s.term.total(), s.term.unprinted()))
                        .font(type_::LABEL)
                        .color(dim())
                        .grow(1.0),
                )
                .child(Button::new("Clear").variant(ButtonVariant::Ghost).on_press(|s: &mut App| {
                    s.term.clear();
                    s.touch();
                }).build(kit())),
        )
}

/// The big glowing button. Disabled, it fades into the panel; ready, it crackles with plasma, and
/// pressing it sends a shockwave through the button. The ticket's family picks the colour of the
/// halo and the lightning.
/// How far the Print button's glow reaches past its edge, in points. Small on purpose: it ends the
/// top bar, beside the drawer switch, and no effect may reach it.
const HALO: f32 = 4.0;

/// The family of the ticket on the paper, when a ticket is what is being printed. The other tabs
/// keep the palette's weather: their theme is not on screen.
fn ticket_family(s: &App) -> Option<Family> {
    if s.mode == Mode::Text { s.theme.family() } else { None }
}

fn hero_button(s: &App, label: String) -> El {
    let enabled = s.can_print();
    let (tint, power) = fx::press_style(ticket_family(s));
    let b = row()
        .key("print")
        .role("button")
        .label("Print")
        .focusable(true)
        .w(PRINT_W)
        .h(46.0)
        // The pill's cap is 23 pt wide: keep the glyphs that far from either end so a long label,
        // cut by `.truncate()`, never runs under the curve.
        .px(24.0)
        .center()
        .radius(23.0)
        // The label grows to the pill's inner width, then centres its glyphs in that box. Without
        // `.align` a grown text draws at the start of its box, which left the label against the cap.
        .child(
            text(label)
                .font(type_::TITLE)
                .color(if enabled { style::on_accent() } else { dim() })
                .align(rune::Align::Center)
                .truncate()
                .grow(1.0)
                .min_w(0.0),
        );
    let b = if enabled {
        b.gradient(style::grad_a(), style::grad_b()).shadow(6.0, 26.0, pink().alpha(if style::pal().glow { 0.45 } else { 0.0 })).on_press(|s: &mut App| s.print())
    } else {
        b.fill(panel()).border(1.0, Color::WHITE.alpha(0.06))
    };
    if !s.fx_on {
        return b;
    }
    // Slots overlay the button: a soft neon halo when it is ready, thin lightning while it is
    // pressed or printing, and a short shockwave just after it was pressed. Every effect stays
    // inside the button's box and its few points of bleed, so the drawer switch beside it and the
    // controls under it stay legible.
    let mut wrap = col().w(PRINT_W).h(46.0).child(b);
    let pressed = s.press_at.is_some_and(|t| s.clock - t < 0.6);
    if style::pal().glow {
        if enabled {
            wrap = wrap.child(fx::slot(fx::GLOW, [0.45, tint, 0.4, 8.0]).absolute(0.0, 0.0).w_full().h(46.0).radius(23.0).effect_bleed(HALO));
        }
        // The lightning is the job's own feedback, so it also runs while the button is disabled by
        // a job in flight. Intensity and colour come from the ticket's family.
        if pressed || s.printing.is_busy() {
            let strength = if pressed { 1.2 } else { 0.7 } * power;
            wrap = wrap.child(fx::slot(fx::ELECTRIC, [strength, 1.5, 0.0, tint]).absolute(0.0, 0.0).w_full().h(46.0).radius(23.0).effect_bleed(HALO));
        }
    }
    if let Some(t0) = s.press_at.filter(|t| style::pal().glow && s.clock - t < 1.0) {
        // A ripple that stays on the button and its few points of bleed: it used to cross the
        // whole left panel.
        wrap = wrap.child(fx::slot(fx::SHOCK, [0.0, 0.0, t0 as f32, 5.0]).effect_backdrop(true).absolute(0.0, 0.0).w_full().h(46.0).radius(23.0).effect_bleed(6.0));
    }
    wrap
}

/// Ctrl+P / Cmd+P prints, also while a text box has the focus and would otherwise keep the key.
fn print_key(s: &mut App, e: &rune::KeyEvent, _: &mut Tx) -> bool {
    s.key_shortcut(e)
}

/// Darkness and the number of copies, at the foot of the drawer: the stage keeps the whole window
/// otherwise, so they only take room when the drawer is open.
fn print_options(s: &App) -> El {
    let k = kit();
    let density = SegmentedControl::new(&["Light", "Medium", "Dark"], Some(s.density)).width(Some(DRAWER - 150.0)).on_pick(|s: &mut App, i, _| {
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
    col()
        .w_full()
        .gap(8.0)
        .child(caption("Print"))
        .child(row().w_full().gap(10.0).center_y().child(density).child(copies))
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

/// A glowing dot: green when connected, amber while connecting, red when not.
fn led(color: Color) -> El {
    el().size(10.0, 10.0).radius(5.0).fill(color).shadow(0.0, 10.0, color.alpha(0.9))
}

fn battery(percent: u8) -> El {
    let p = f32::from(percent.min(100)) / 100.0;
    let color = if percent <= 15 { red() } else if percent <= 40 { amber() } else { mint() };
    row().gap(6.0).center_y().child(
        el().w(26.0).h(12.0).radius(3.0).border(1.0, dim()).child(el().w(24.0 * p).h(10.0).radius(2.0).fill(color)),
    ).child(text(format!("{percent}%")).font(type_::LABEL).color(dim()))
}

fn status_chip(s: &App) -> El {
    let (color, label) = match &s.link {
        Link::Offline => (red(), "Offline".to_string()),
        Link::Connecting => (amber(), "Connecting…".to_string()),
        Link::Online(_) => (mint(), "Connected".to_string()),
    };
    // The chip is the connection button: pressing it looks for the printer, or lets it go.
    let help = match &s.link {
        Link::Offline => "Press to look for the printer".to_string(),
        Link::Connecting => "Looking for the printer".to_string(),
        Link::Online(_) => device_line(s).unwrap_or_else(|| "Press to disconnect".into()),
    };
    let mut chip = row()
        .self_start()
        .gap(8.0)
        .center_y()
        .px(12.0)
        .h(32.0)
        .radius(16.0)
        .fill(Color::WHITE.alpha(0.05))
        .border(1.0, Color::WHITE.alpha(0.08))
        .child(led(color))
        .child(text(label).font(type_::LABEL).color(ink()).truncate().max_w(92.0))
        .on_press(|s: &mut App| match s.link.clone() {
            Link::Offline => s.connect(),
            Link::Online(_) if !s.printing.is_busy() => s.disconnect(),
            _ => {}
        })
        .help(help);
    if let Some(b) = s.battery {
        chip = chip.child(battery(b));
    }
    chip
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

/// The mark and the wordmark, with what the printer is doing under it.
fn brand(s: &App) -> El {
    let mark = match &s.logo {
        Some(image) => el().key("logo").size(40.0, 40.0).radius(10.0).pixels(image.pixels()),
        None => el().size(40.0, 40.0).radius(10.0).gradient(coral(), pink()),
    }
    // Five taps on the mark: the ghost says boo.
    .on_press(|s: &mut App| s.logo_tap());
    // The wordmark keeps a dry line under it, printed out one character at a time every minute.
    // When something is actually happening (connecting, printing, an error, a fresh result), the
    // line says that instead: the quip can wait.
    let fresh_done = s.printing.done_at().is_some_and(|t| s.clock - t < 6.0);
    let (line, typing) = if s.printing.is_busy() || matches!(s.link, Link::Connecting) || s.error.is_some() || fresh_done {
        (s.status.clone(), false)
    } else {
        s.tagline_shown()
    };
    let line = if typing { format!("{line}▌") } else { line };
    row().grow(1.0).min_w(0.0).gap(10.0).center_y().child(mark).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .child(text("TRONIC".to_string()).font(type_::TITLE).color(coral()))
            .child(text(line).font(type_::LABEL).color(dim()).truncate()),
    )
}

/// "A2Y · V1.06LY · paper 00" once the printer has answered, so you can see what you are
/// talking to.
fn device_line(s: &App) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(m) = &s.info.model {
        parts.push(m.clone());
    }
    if let Some(f) = &s.info.firmware {
        parts.push(f.clone());
    }
    // The paper query's reply format is undocumented, so the raw hex is not user-facing; show
    // nothing until it can be read as OK/empty.
    if parts.is_empty() { None } else { Some(parts.join(" · ")) }
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
        .on_press(move |s: &mut App| {
            s.mode = mode;
            s.touch();
        })
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
/// ticket's length and fit, the palettes, FX, the drawer switch and Print.
fn top_bar(s: &App, drop_h: Option<f32>, stage_w: Option<f32>) -> El {
    let fit = stage_fit(s, drop_h, stage_w).0;
    let cm = s.composed.as_ref().map(|b| format!("{:.1} cm", render::length_cm(b))).unwrap_or_else(|_| "— cm".to_string());
    let size = text(format!("{cm} · fit {:.0}%", fit * 100.0)).font(type_::LABEL).color(if fit < 0.999 { coral() } else { dim() }).w(112.0).truncate();
    let label = if s.printing.is_busy() { "Printing…".to_string() } else { format!("Print  ·  {}", App::print_shortcut_label()) };
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(brand(s))
        .child(status_chip(s))
        .child(tabs(s))
        .child(size)
        .child(palette_chips(s))
        .child(Switch::new(s.fx_on).label("FX").on_toggle(|s: &mut App| s.set_effects(!s.fx_on)).build(kit()))
        .child(drawer_button(s))
        .child(hero_button(s, label))
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

/// The drawer on the left: one scrolling column with the mode's galleries and extras, and the
/// density/copies row at its foot. Built only when open, so the stage keeps the whole window.
fn drawer(s: &App) -> El {
    let k = kit();
    // One line under the title, so the drawer says what it is for: the mode's gallery first, the
    // switches and the print settings below it.
    let hint = match s.mode {
        Mode::Text => "Themes and settings",
        Mode::Image => "Pictures and settings",
        Mode::Codes => "Codes and settings",
        Mode::Terminal => "Terminal and settings",
    };
    let head = col()
        .w_full()
        .gap(6.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(text(DRAWER_LABEL.to_string()).font(type_::TITLE).color(ink()).grow(1.0).min_w(0.0).truncate())
                .child(
                    row()
                        .key("drawer-close")
                        .role("button")
                        .label("Close the drawer")
                        .focusable(true)
                        .size(28.0, 28.0)
                        .center()
                        .radius(14.0)
                        .fill(Color::WHITE.alpha(0.06))
                        .on_press(|s: &mut App| s.close_drawer())
                        .child(text("×".to_string()).font(type_::BODY).color(dim())),
                ),
        )
        .child(text(hint.to_string()).font(type_::LABEL).color(dim()).truncate());
    let content = match s.mode {
        Mode::Text => {
            let mut body = col().w_full().gap(14.0).child(theme_gallery(s));
            if let Some(switches) = form_switches(s) {
                body = body.child(card(switches));
            }
            // The text itself is typed on the paper; the box stays for a ticket with no text yet,
            // and for the type choices a plain ticket keeps.
            body.child(card(text_controls(s)))
        }
        Mode::Image => col().w_full().gap(14.0).child(gallery_section(s)).child(card(image_settings(s))),
        Mode::Codes => col().w_full().child(card(code_controls(s))),
        Mode::Terminal => col().w_full().child(card(terminal_controls(s))),
    };
    col()
        .key("drawer")
        .w(DRAWER)
        .h_full()
        .gap(12.0)
        .padding(16.0)
        .radius(20.0)
        .fill(style::bg_top())
        .border(1.0, Color::WHITE.alpha(0.08))
        .child(head)
        .child(col().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(content).scroll_bar(k))
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

/// The ghost's bubble above the slit, and the narrower one that stands beside the printer.
const GHOST_W: f32 = 340.0;
const GHOST_SIDE_W: f32 = 260.0;
/// The gap between the printer's case and the bubble beside it, and how far the bubble is
/// lifted off the floor (so its tail points at the case, not at the desk).
const GHOST_SIDE_GAP: f32 = 16.0;
const GHOST_SIDE_LIFT: f32 = 64.0;
/// The room the stage's inside needs to stand the bubble beside the printer without moving it:
/// the machine, both sides, and a last gap so the bubble never touches the stage's edge.
const GHOST_SIDE_MIN_W: f32 = MACHINE_W + 2.0 * (GHOST_SIDE_W + GHOST_SIDE_GAP) + GHOST_SIDE_GAP;

/// The ghost that floats round a ticket: a small comic bubble over the paper face, one column. The
/// bubble is cream with a dark outline; its tail is a little square stood on a corner, pointing
/// down at the face. `GHOST_STACK_*` is the box the column takes, which the placement keeps off the
/// ticket; `GHOST_STACK_MID` is where the face sits, so that is what a spot centres on the paper.
const GHOST_BUBBLE_W: f32 = 140.0;
const GHOST_BUBBLE_H: f32 = 64.0;
const GHOST_BUBBLE_RADIUS: f32 = 14.0;
const GHOST_BODY_H: f32 = 88.0;
/// Between the bubble's tail and the top of the face: room for the tail to point, not touch.
const GHOST_TAIL_GAP: f32 = 12.0;
const GHOST_STACK_W: f32 = GHOST_BUBBLE_W;
const GHOST_STACK_H: f32 = GHOST_BUBBLE_H + GHOST_TAIL_GAP + GHOST_BODY_H;
const GHOST_STACK_MID: f32 = GHOST_BUBBLE_H + GHOST_TAIL_GAP + GHOST_BODY_H / 2.0;
/// Clearance between the ghost and the paper, and between it and the stage's edge.
const GHOST_GAP: f32 = 12.0;
const GHOST_EDGE: f32 = 8.0;
/// How far the column's foot floats above the stage's floor when it keeps the case company.
const GHOST_LIFT: f32 = 48.0;

/// The paper slit: left edge, width, top, and the front lip where the paper enters.
const SLIT_X: f32 = 64.0;
const SLIT_W: f32 = 472.0;
const SLIT_Y: f32 = 78.0;
const SLIT_FRONT: f32 = 86.0;

/// The printer as it looks on the desk: a soft white cube with the paper leaving a slit across its
/// top, one lamp low on the front (green when connected, amber while working or connecting, red
/// when off) and a USB-C port on the side. The ticket stands above it and runs into the slit.
fn printer_body(s: &App, armed: bool) -> El {
    let light = match &s.link {
        Link::Online(_) if s.printing.is_busy() => amber(),
        Link::Online(_) => mint(),
        Link::Connecting => amber(),
        Link::Offline => red(),
    };
    let online = matches!(s.link, Link::Online(_));
    let has_paper = matches!((&s.preview, &s.composed), (Some(_), Ok(_)));
    let mut m = col().w(MACHINE_W).h(MACHINE_H);
    if let Some(art) = &s.printer_art {
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
        let w = SLIT_W - 8.0;
        // The ticket above fades to a darker, shaded paper at its bottom edge (as if going back
        // into the slot), so a ticket's tail picks that up and darkens on towards the slit. A bare
        // stub is fresh paper: bright, shading only a little. The outer strips are the margins the
        // print head cannot reach, a touch darker, as in the ticket.
        let shade = |k: f32| style::rgb(((250.0 * k) as u32) << 16 | ((246.0 * k) as u32) << 8 | (236.0 * k) as u32);
        let (top, bottom) = if armed { (style::rgb(0xffd9c9), style::rgb(0xe8a98f)) } else { (shade(1.0), shade(0.72)) };
        let rim = |x: f32| el().absolute(x, 0.0).size(40.0, tail).fill(Color::BLACK.alpha(0.06));
        m = m.child(
            el()
                .absolute(SLIT_X + 4.0, SLIT_FRONT - tail)
                .size(w, tail)
                .gradient(top, bottom)
                .shadow(0.0, 8.0, Color::BLACK.alpha(0.28))
                .child(rim(0.0))
                .child(rim(w - 40.0)),
        );
    }
    let mut lamp = el().absolute(278.0, 200.0).size(44.0, 12.0).radius(6.0).fill(light).shadow(0.0, 16.0, light.alpha(0.85));
    if s.fx_on && matches!(s.link, Link::Connecting) {
        // A small, slow radar ring around the light only.
        lamp = lamp.child(fx::slot(fx::PULSE, [0.0, fx::AMBER, 2.4, 22.0]).absolute(-4.0, -4.0).size(52.0, 20.0).radius(10.0).effect_bleed(22.0));
    }
    m.child(lamp)
}

/// What happens on top of the printer and the ticket: the chrome lip of the slit, flames and heat
/// haze, lightning. Drawn above the ticket so the flames lick the paper rather than hide behind it.
fn printer_fx(s: &App, _armed: bool) -> El {
    let mut m = col().w(MACHINE_W).h(MACHINE_H);
    if s.fx_on {
        let (flame, smoke) = (s.flame(), s.smoke());
        // The slit's front lip is liquid chrome; the metal only flows while the printer works.
        let flow = if s.printing.is_busy() || flame > 0.0 { 0.6 } else { 0.0 };
        m = m.child(fx::slot(fx::CHROME, [0.0, 0.0, flow, 0.0]).absolute(SLIT_X, SLIT_FRONT - 1.0).size(SLIT_W, 3.0).radius(1.5));
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
            m = m.child(fx::slot(fx::FIRE, [height, intensity, 0.0, rate]).absolute(SLIT_X, SLIT_Y).size(SLIT_W, 8.0).radius(4.0).effect_bleed(220.0));
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

/// The ghost's face: a torn scrap of receipt paper, with ink-dot eyes that follow the pointer.
fn ghost_face(mood: voice::Mood, gaze: (f32, f32)) -> El {
    let paper = style::rgb(0xfaf6ec);
    let ink = style::rgb(0x241d16);
    let dot = |w: f32, h: f32| el().size(w, h).radius(h / 2.0).fill(ink);
    // An outlined eye with a pupil that slides toward the pointer, never past the rim.
    let eye = |size: f32| {
        let pupil = (size * 0.45).max(4.0);
        let room = size - pupil;
        let dx = gaze.0 * (room / 2.0 - 0.8);
        let dy = gaze.1 * (room / 2.0 - 0.8);
        el()
            .size(size, size)
            .radius(size / 2.0)
            .fill(paper)
            .border(1.5, ink)
            .child(el().size(pupil, pupil).radius(pupil / 2.0).fill(ink).absolute(room / 2.0 + dx, room / 2.0 + dy))
    };
    let eyes = match mood {
        voice::Mood::Asleep => row().gap(18.0).child(dot(12.0, 2.0)).child(dot(12.0, 2.0)),
        voice::Mood::Curious => row().gap(14.0).child(eye(13.0)).child(eye(9.0)),
        voice::Mood::Happy => row().gap(20.0).child(eye(12.0)).child(eye(12.0)),
    };
    // The mouth is the receipt's barcode: bars of different widths, hung along a curve so the
    // row reads as a smile (happy), a small "o" (curious) or a flat line (asleep).
    let bar_row = |widths: &[f32], drops: &[f32], h: f32| {
        widths.iter().zip(drops).fold(row().gap(1.5).h(h + 6.0), |r, (w, dy)| r.child(el().w(*w).h(h).fill(ink).offset(0.0, *dy)))
    };
    let mouth = match mood {
        voice::Mood::Happy => bar_row(
            &[2.0, 3.0, 1.5, 2.5, 2.0, 3.5, 1.5, 2.5, 2.0, 3.0, 1.5, 2.0],
            &[0.0, 0.5, 1.5, 2.5, 3.5, 4.0, 4.0, 3.5, 2.5, 1.5, 0.5, 0.0],
            7.0,
        ),
        voice::Mood::Curious => bar_row(&[2.0, 3.0, 1.5, 2.5, 2.0], &[0.0, 0.0, 0.0, 0.0, 0.0], 4.0),
        voice::Mood::Asleep => bar_row(&[2.0, 3.0, 1.5, 2.5, 2.0, 3.0, 1.5, 2.0], &[2.0; 8], 2.0),
    };
    // The torn top edge: uneven teeth (turned squares), half hidden behind the strip.
    let tears = [(11.0, 0.80), (8.0, 0.68), (12.0, 0.86), (9.0, 0.73), (10.0, 0.82), (7.0, 0.66), (11.0, 0.78), (9.0, 0.72), (10.0, 0.84)];
    let teeth = tears.iter().fold(row().gap(0.5), |r, (size, angle)| r.child(el().size(*size, *size).radius(1.5).fill(paper).rotate(*angle)));
    let strip = col().items_center().gap(8.0).px(26.0).py(14.0).radius(0.0).fill(paper).child(eyes).child(mouth);
    // The same tear along the bottom, a little different so the two edges do not mirror.
    let teeth_bottom = tears.iter().rev().fold(row().gap(0.5), |r, (size, angle)| r.child(el().size(*size, *size).radius(1.5).fill(paper).rotate(*angle)));
    let face = col().items_center().child(teeth).child(strip.offset(0.0, -6.0)).child(teeth_bottom.offset(0.0, -12.0));
    if mood == voice::Mood::Asleep {
        row().gap(8.0).center_y().child(face).child(text("z Z".to_string()).font(type_::LABEL).color(Color::WHITE.alpha(0.4)))
    } else {
        face
    }
}

/// The empty state: the printer talks. A speech bubble over the slit with a little paper face and
/// a line that changes with the tab, bobbing gently (when effects are on). The real ticket takes
/// its place as soon as there is something to print.
fn ghost_paper(s: &App, gaze: (f32, f32)) -> El {
    // A picture held over the preview: the stage says "Let go to preview" itself, so the bubble steps aside.
    if s.over_stage && s.drag.as_ref().is_some_and(|d| d.travel >= crate::app::DRAG_MIN) {
        return col();
    }
    // The bubble's tail points down at the slit.
    let tail = el().w(14.0).h(14.0).radius(3.0).fill(glass()).rotate(0.785).offset(0.0, -9.0);
    col().items_center().child(ghost_bubble(s, gaze, GHOST_W)).child(tail)
}

/// The empty state beside the printer, used when the stage is wide enough to hold both: the bubble
/// stands to the right of the case, its tail pointing left at it, so the printer keeps the room its
/// ticket will need (it must not move when a ticket appears).
fn ghost_beside(s: &App, gaze: (f32, f32)) -> El {
    if s.over_stage && s.drag.as_ref().is_some_and(|d| d.travel >= crate::app::DRAG_MIN) {
        return col();
    }
    let stage_w = memo::layout_size("stage-frame").map_or(0.0, |r| r[0]);
    let left = (stage_w - 2.0 * STAGE_PAD) / 2.0 + MACHINE_W / 2.0 + GHOST_SIDE_GAP;
    // The tail is the bubble's left edge, tucked into its border.
    let tail = el().w(14.0).h(14.0).radius(3.0).fill(glass()).rotate(0.785).offset(6.0, 0.0);
    col()
        .pin(Some(left), None, None, Some(GHOST_SIDE_LIFT))
        .child(row().center_y().child(tail).child(ghost_bubble(s, gaze, GHOST_SIDE_W)))
}

/// The bubble itself: the face, a line of talk, and the tap that changes it. `width` lets the empty
/// state stand either above the slit (wide) or beside the printer (narrow).
fn ghost_bubble(s: &App, gaze: (f32, f32), width: f32) -> El {
    let (line, typing) = s.ghost_line_shown();
    let line = if typing { format!("{line}▌") } else { line };
    col()
        .items_center()
        .gap(8.0)
        .px(30.0)
        .py(16.0)
        .radius(20.0)
        .fill(glass())
        .border(1.0, Color::WHITE.alpha(0.16))
        .w(width)
        .child(ghost_face(s.ghost_mood(), gaze).key("ghost-face"))
        .child(col().h(44.0).center().child(text(line).font(type_::BODY).color(style::rgb(0xfaf6ec).alpha(0.85)).wrap().max_w(width - 50.0)))
        .on_press(|s: &mut App| s.ghost_poke())
}

/// The bubble's glass: a faint white pane, so it reads as frosted glass over the stage.
fn glass() -> Color {
    Color::WHITE.alpha(0.09)
}

/// The comic bubble the floating ghost talks through: cream paper, a dark outline, rounded corners
/// and a tail pointing down at the face. Its text is the ghost's line as far as it has typed out.
fn ghost_comic(s: &App) -> El {
    let paper = style::rgb(0xfaf6ec);
    let ink = style::rgb(0x1a1228);
    let (line, typing) = s.ghost_line_shown();
    let line = if typing { format!("{line}▌") } else { line };
    // The tail goes down first, so the bubble's fill hides the half that overlaps it; the mask then
    // rubs out the stretch of outline the tail opens, and the join reads as one shape rather than
    // a diamond stuck on a card. The tail's point stops a point short of the face.
    let tail = el()
        .absolute((GHOST_BUBBLE_W - 16.0) / 2.0, GHOST_BUBBLE_H - 8.0)
        .size(16.0, 16.0)
        .radius(2.0)
        .fill(paper)
        .border(2.0, ink)
        .rotate(std::f32::consts::FRAC_PI_4);
    let mask = el().absolute((GHOST_BUBBLE_W - 20.0) / 2.0, GHOST_BUBBLE_H - 3.0).size(20.0, 5.0).fill(paper);
    col()
        .w(GHOST_BUBBLE_W)
        .h(GHOST_BUBBLE_H)
        .child(tail)
        .child(
            col()
                .absolute(0.0, 0.0)
                .w(GHOST_BUBBLE_W)
                .h(GHOST_BUBBLE_H)
                .px(14.0)
                .py(8.0)
                .radius(GHOST_BUBBLE_RADIUS)
                .fill(paper)
                .border(2.0, ink)
                .justify_center()
                .child(text(line).font(type_::LABEL).color(ink).wrap().max_w(GHOST_BUBBLE_W - 30.0)),
        )
        .child(mask)
        .on_press(|s: &mut App| s.ghost_poke())
}

/// The ghost when a ticket takes the stage: the comic bubble and, under it, the paper face, bobbing
/// a few points and leaning a hair so it reads as flying. Off, the effects switch leaves it still;
/// the eyes keep following the pointer either way. A press asks for another line and another spot.
fn ghost_flying(s: &App, gaze: (f32, f32)) -> El {
    let (bob, tilt) = if s.fx_on {
        let t = s.clock as f32;
        ((t * std::f32::consts::TAU / 3.2).sin() * 4.0, (t * std::f32::consts::TAU / 5.3).sin() * 0.02)
    } else {
        (0.0, 0.0)
    };
    col()
        .key("ghost-stack")
        .w(GHOST_STACK_W)
        .h(GHOST_STACK_H)
        .items_center()
        .gap(GHOST_TAIL_GAP)
        .offset(0.0, bob)
        .child(ghost_comic(s))
        .child(ghost_face(s.ghost_mood(), gaze).key("ghost-face").rotate(tilt))
}

/// Where the floating ghost stands in the stage's inside: the top-left of its column, or `None`
/// when the stage holds none of its spots. `spot` is the one it picked; the others are tried in
/// turn, so a narrow stage still gets a ghost wherever it fits. `ticket` is the ticket's rectangle
/// and `case_right` the printer's right edge, in the stage's points, from its top-left. A spot is
/// taken only when the whole column is inside the stage and clear of the ticket's rectangle, so
/// the ghost never covers the ink (its tail stops short of the paper too).
fn ghost_placement(spot: u8, inside: (f32, f32), ticket: (f32, f32, f32, f32), case_right: f32) -> Option<(f32, f32)> {
    let (w, h) = inside;
    let (tx, ty, tw, th) = ticket;
    // Beside the ticket, the face keeps the paper company at the ticket's middle; the column is
    // lifted by where the face sits in it.
    let top = (ty + th / 2.0 - GHOST_STACK_MID).clamp(GHOST_EDGE, (h - GHOST_STACK_H - GHOST_EDGE).max(GHOST_EDGE));
    let spots = [
        (tx + tw + GHOST_GAP, top),
        (tx - GHOST_GAP - GHOST_STACK_W, top),
        ((w - GHOST_STACK_W) / 2.0, ty - GHOST_GAP - GHOST_STACK_H),
        (case_right + GHOST_GAP, h - GHOST_LIFT - GHOST_STACK_H),
    ];
    (0..spots.len())
        .map(|k| spots[(spot as usize + k) % spots.len()])
        .find(|&(x, y)| x >= GHOST_EDGE && y >= GHOST_EDGE && x + GHOST_STACK_W <= w - GHOST_EDGE && y + GHOST_STACK_H <= h - GHOST_EDGE && !covers((x, y), ticket))
}

/// Whether the ghost's column would sit over the ticket's rectangle.
fn covers((x, y): (f32, f32), (tx, ty, tw, th): (f32, f32, f32, f32)) -> bool {
    x < tx + tw && tx < x + GHOST_STACK_W && y < ty + th && ty < y + GHOST_STACK_H
}

/// What lets you write straight on the preview: a faint outline round each part of the ticket
/// you can write on (not printed), a press on it starts typing there, in a text box right on the
/// paper, and a press on the paper elsewhere stops.
fn write_layer(s: &App, full: f32, turned: bool) -> El {
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
fn write_bar(s: &App) -> Option<El> {
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

/// How a ticket is scaled to fit the stage whole, and (for a turned ticket) the scale and height of
/// the printer-plus-ticket group. Shared by the stage and by the top bar's `fit NN%`, so the two
/// never disagree.
fn stage_fit(s: &App, drop_h: Option<f32>, stage_w: Option<f32>) -> (f32, f32, f32) {
    let has_paper = matches!((&s.preview, &s.composed), (Some(_), Ok(_)));
    let turned = has_paper && s.preview_turned();
    // A long ticket is scaled down to fit whole on the stage, never below half size; while a part
    // is being typed on it goes back to full size, and the scroll follows the field.
    let ticket_h = s.composed.as_ref().map(|b| (b.height + render::TEETH) as f32).unwrap_or(0.0);
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
    // is being typed on it goes back to full size, and the scroll follows the field.
    let ticket_h = s.composed.as_ref().map(|b| (b.height + render::TEETH) as f32).unwrap_or(0.0);
    let (fit, group_k, frame_h) = stage_fit(s, drop_h, stage_w);
    // How much room the printer and its ticket have, to shrink the printer when the ticket is long.
    let room = drop_h.map(|h| h - 8.0).unwrap_or(f32::MAX);
    // On a wide stage with no ticket the printer talks from beside its case, so the space above the
    // slit stays free for the ticket that will come; a narrow stage keeps the bubble over the slit.
    let beside = s.preview.is_none() && stage_w.is_some_and(|w| w - 2.0 * STAGE_PAD >= GHOST_SIDE_MIN_W);
    let sheet = match (&s.preview, &s.composed) {
        (Some(image), Ok(bitmap)) => {
            let full = (bitmap.height + render::TEETH) as f32;
            let mut paper = el().key("paper").w(render::PREVIEW_WIDTH as f32).pixels(image.pixels());
            if s.fx_on && s.printing.done_at().is_some_and(|t| s.clock - t < 2.2) {
                let t0 = s.printing.done_at().unwrap_or(0.0) as f32;
                paper = paper.child(fx::slot(fx::SPARKLES, [t0, 16.0, 9.0, fx::WHITE]).absolute(0.0, 0.0).size(render::PREVIEW_WIDTH as f32, full).effect_bleed(30.0));
            }
            if !s.printing.is_busy() && s.mode == Mode::Text {
                paper = paper.child(write_layer(s, full, turned));
            }
            let inner = if s.printing.is_busy() && s.fx_on {
                // The ticket is pushed out of the printer as the job goes: only the band that has left
                // the slit shows. That band is the foot of the ticket, its bottom edge on the slit, so
                // the sheet reads as fed out of the machine instead of as a window sliding over it.
                // The paper is taller than its window; `justify_end` pins its foot to the bottom and
                // the part still inside the printer overflows above, where the clip cuts it.
                let shown = rise_height(full, s.printing.progress());
                col().w(render::PREVIEW_WIDTH as f32).h(shown).clip_rounded(0.0).justify_end().child(paper)
            } else {
                paper
            };
            if fit < 0.999 {
                // The layout box shrinks with the picture, so the scroll window stops guessing.
                col().w(render::PREVIEW_WIDTH as f32 * fit).h(full * fit).clip_rounded(0.0).child(inner.scale(fit).origin(0.0, 0.0))
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
    let shown_ticket_h = ticket_h * fit;
    let k = if turned {
        1.0
    } else if has_paper && room.is_finite() {
        ((room - shown_ticket_h) / (MACHINE_H - SLIT_FRONT)).clamp(MACHINE_MIN, 1.0)
    } else {
        1.0
    };
    let ticket_floor = if has_paper { (MACHINE_H - SLIT_FRONT) * k } else { MACHINE_H - 14.0 };
    // The printer and its ticket are one group, so a sideways theme can turn all of it, and the
    // turn (and the change of scale) is animated by Rune because the group keeps its key.
    let group = col()
        .key("stage-group")
        .child(at_machine(printer_body(s, armed), k))
        .child(
            col()
                .items_center()
                .justify_end()
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(ticket_floor))
                .child(col().items_center().child(sheet).scroll(ScrollSpec { stick_bottom: true }).shrink().min_h(0.0)),
        )
        .child(at_machine(if k < 0.999 { col() } else { printer_fx(s, armed) }, k));
    let group = if turned {
        group.w(MACHINE_W).h(frame_h).scale(group_k).rotate(-std::f32::consts::FRAC_PI_2)
    } else {
        group.w_full().h_full().scale(1.0).rotate(0.0)
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
    // the stage and leaves it no room; a stage too narrow for every spot simply goes without.
    if has_paper && !turned && !armed {
        if let (Some(sw), Some(dh)) = (stage_w, drop_h) {
            let (w, h) = (sw - 2.0 * STAGE_PAD, dh);
            let tw = render::PREVIEW_WIDTH as f32 * fit;
            let shown = shown_ticket_h.min((dh - ticket_floor).max(0.0));
            let ticket = ((w - tw) / 2.0, dh - ticket_floor - shown, tw, shown);
            let case_right = w / 2.0 + MACHINE_W * k / 2.0;
            if let Some((x, y)) = ghost_placement(s.ghost_spot, (w, h), ticket, case_right) {
                drop_zone = drop_zone.child(ghost_flying(s, gaze).pin(Some(x), Some(y), None, None));
            }
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
    // One bar on top, then the drawer (when asked for) and the stage, which takes the rest.
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
        middle = middle.child(drawer(s));
    }
    body = body
        .child(middle.child(paper(s, drop_h, stage_w, gaze)))
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

    /// The ticket of a stage with a drawer closed at 1180 pt: 384 wide, standing nearly the whole
    /// height, its bottom on the slit.
    fn ticket(w: f32, h: f32) -> (f32, f32, f32, f32) {
        let tw = render::PREVIEW_WIDTH as f32;
        ((w - tw) / 2.0, 90.0, tw, h - 300.0)
    }

    #[test]
    fn the_floating_ghost_never_covers_the_ticket() {
        let (w, h) = (1096.0, 700.0);
        let t = ticket(w, h);
        for spot in 0..crate::app::GHOST_SPOTS {
            let (x, y) = ghost_placement(spot, (w, h), t, w / 2.0 + MACHINE_W / 2.0).expect("a wide stage holds every spot");
            assert!(!covers((x, y), t), "spot {spot}: the ghost sits on the paper at ({x}, {y})");
            assert!(x >= GHOST_EDGE && y >= GHOST_EDGE, "spot {spot}: off the stage at ({x}, {y})");
            assert!(x + GHOST_STACK_W <= w - GHOST_EDGE && y + GHOST_STACK_H <= h - GHOST_EDGE, "spot {spot}: the column runs off the stage");
        }
    }

    #[test]
    fn a_narrow_stage_refuses_every_spot_rather_than_draw_half_a_ghost() {
        // The stage of the drawer-open window: 527 pt inside, the ticket leaves 71 pt on either
        // side and reaches the top, so no 140 pt column fits anywhere.
        let (w, h) = (527.0, 700.0);
        let t = ticket(w, h);
        let case_right = w / 2.0 + MACHINE_W * 0.62 / 2.0;
        for spot in 0..crate::app::GHOST_SPOTS {
            assert_eq!(ghost_placement(spot, (w, h), t, case_right), None, "spot {spot} found room on a stage with none");
        }
    }

    #[test]
    fn a_short_ticket_leaves_room_above_it() {
        // A short ticket low on the stage: the spot above it is the only one that fits on the right
        // of nothing, and the ghost clears the ink there.
        let (w, h) = (527.0, 700.0);
        let t = (71.0, 500.0, 384.0, 120.0);
        let (x, y) = ghost_placement(2, (w, h), t, w / 2.0 + 200.0).expect("above the ticket");
        assert!(y + GHOST_STACK_H <= t.1, "the ghost's foot is over the ticket: {:?}", (x, y));
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
}
