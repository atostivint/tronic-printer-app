//! The window: controls on the left, the paper coming out of its slot on the right.

use rune::tok::{motion, type_};
use rune::{Color, Cx, Drag, El, FileDialogOptions, Phase, ScrollSpec, Spring, SurfaceFit, Tx, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Kit, Progress, ScrollBar, ScrollBarStyle, SegmentedControl, Switch, TextArea, TextInput};

use crate::app::{App, Link, Mode};
use crate::fx;
use crate::render::{self, Align, CodeKind, Dither, EcLevel, Family, FontChoice, Theme};
use crate::style::{self, AMBER, CORAL, MINT, PANEL, PINK, RED, TEXT, TEXT_DIM};
use crate::update::{self, Update};
use crate::voice;

/// Width of the control column, in points.
const LEFT: f32 = 400.0;

pub fn kit() -> &'static Kit {
    style::kit()
}

/// A small, quiet, upper-case label.
fn caption(title: &str) -> El {
    text(title.to_uppercase()).font(type_::LABEL).color(TEXT_DIM)
}

fn field(title: &str, body: El) -> El {
    col().gap(8.0).child(caption(title)).child(body)
}

/// A rounded panel that groups controls.
fn card(body: El) -> El {
    col().w_full().padding(16.0).radius(18.0).fill(PANEL).border(1.0, Color::WHITE.alpha(0.06)).child(body)
}

/// A segmented choice over `labels`; `pick` receives the index.
fn choice(labels: &[&str], selected: usize, pick: fn(&mut App, usize)) -> El {
    SegmentedControl::new(labels, Some(selected)).width(Some(LEFT - 34.0)).on_pick(move |s: &mut App, i, _| pick(s, i)).build(kit())
}

/// Size of a theme's picture in the gallery, in points.
const THEME_W: f32 = 84.0;
const THEME_H: f32 = 108.0;

/// A gallery of the writing themes: a small picture of each ticket, grouped, in a window that
/// scrolls. Click one to use it; pull it toward the printer to print your text in that theme.
fn theme_gallery(s: &App) -> El {
    let tile = |theme: Theme| {
        let picked = s.theme == theme;
        let dragging = s.drag.as_ref().is_some_and(|d| d.theme == Some(theme) && d.travel >= crate::app::DRAG_MIN);
        let picture = el()
            .key(format!("theme-{}", theme.id()))
            .role("button")
            .label(theme.name().to_string())
            .focusable(true)
            .size(THEME_W, THEME_H)
            .radius(10.0)
            .fill(Color::BLACK.alpha(0.35))
            .border(if picked { 2.0 } else { 1.0 }, if picked { CORAL } else { Color::WHITE.alpha(0.08) })
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
        col().gap(4.0).w(THEME_W).child(picture).child(text(theme.name().to_string()).font(type_::LABEL).color(if picked { TEXT } else { TEXT_DIM }).truncate())
    };
    // The plain ticket leads the first group instead of taking a row of its own.
    let mut grid = col().w_full().gap(10.0);
    for (i, family) in Family::ALL.into_iter().enumerate() {
        let mut tiles = row().flex_wrap().gap(8.0);
        let mut label = family.label().to_string();
        if i == 0 {
            tiles = tiles.child(tile(Theme::Plain));
            label = format!("Plain  ·  {label}");
        }
        let themes: Vec<Theme> = Theme::all().into_iter().filter(|t| t.family() == Some(family)).collect();
        for theme in &themes {
            tiles = tiles.child(tile(*theme));
        }
        if family == Family::Mine {
            // The user's own themes: files in a folder, added and removed without recompiling.
            let header = row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(text(label).font(type_::LABEL).color(TEXT_DIM))
                .child(row().grow(1.0))
                .child(Button::new("Open folder").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.open_themes_folder()).build(kit()))
                .child(Button::new("Reload").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.reload_themes()).build(kit()));
            grid = grid.child(header);
            if themes.is_empty() {
                grid = grid.child(text("Drop a .json theme file in the folder to add your own; delete it to remove it.".to_string()).font(type_::LABEL).color(TEXT_DIM).wrap());
            } else {
                grid = grid.child(tiles);
            }
            for problem in s.theme_report.errors.iter().take(3) {
                grid = grid.child(text(problem.clone()).font(type_::LABEL).color(CORAL).wrap());
            }
            continue;
        }
        grid = grid.child(text(label).font(type_::LABEL).color(TEXT_DIM)).child(tiles);
    }
    let window = col().w_full().grow(1.0).min_h(GALLERY_MIN).scroll(ScrollSpec { stick_bottom: false }).child(grid).scroll_bar_styled(bar_style());
    col().grow(1.0).min_h(0.0).gap(8.0).child(caption("Themes")).child(window)
}

/// The Text tab: the text and its settings (the themes are in the preview, see [`paper`]).
fn text_panels(s: &App) -> El {
    col().grow(1.0).min_h(0.0).child(card(text_controls(s)).scroll(ScrollSpec { stick_bottom: false }).shrink().min_h(0.0).scroll_bar(kit()))
}

/// The small form of the picked theme (a store name, a seat number…).
fn theme_form(s: &App) -> Option<El> {
    let fields = s.theme.fields();
    if fields.is_empty() {
        return None;
    }
    let mut form = col().gap(10.0).child(caption(&format!("{} details", s.theme.name())));
    for (i, f) in fields.iter().enumerate() {
        form = form.child(
            col().gap(4.0).child(text(f.label.to_string()).font(type_::LABEL).color(TEXT_DIM)).child(
                TextInput::new(&s.theme_edits[i])
                    .placeholder(f.hint)
                    .width(None)
                    .on_change(|s: &mut App, _| s.touch())
                    .on_edit(move |s: &mut App| &mut s.theme_edits[i])
                    .build(kit())
                    .w_full(),
            ),
        );
    }
    Some(form)
}

fn text_controls(s: &App) -> El {
    let o = s.text_opts;
    let font_ix = FontChoice::ALL.iter().position(|f| *f == o.font).unwrap_or(0);
    let size_ix = [22.0, 32.0, 48.0, 72.0].iter().position(|p| *p == o.px).unwrap_or(1);
    let align_ix = [Align::Left, Align::Center, Align::Right].iter().position(|a| *a == o.align).unwrap_or(0);
    let mut out = col().gap(14.0).child(TextArea::new(&s.text).rows(3).placeholder("Type something to print").on_change(|s: &mut App, _| s.touch()).on_edit(|s: &mut App| &mut s.text).build(kit()).w_full());
    if let Some(form) = theme_form(s) {
        out = out.child(form);
    }
    out = out
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text("Out of ideas?".to_string()).font(type_::LABEL).color(TEXT_DIM).grow(1.0))
                .child(Button::new("Surprise me").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.surprise_me()).build(kit())),
        );
    if s.theme != Theme::Plain {
        return out.child(text(s.theme.tagline().to_string()).font(type_::LABEL).color(TEXT_DIM).wrap());
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
/// The least height the gallery's scrolling window keeps, even in a small window.
const GALLERY_MIN: f32 = 96.0;

/// A folder of pictures to pick from, and the switch that prints new arrivals by itself.
fn gallery_section(s: &App) -> El {
    let g = &s.gallery;
    let folder = g.folder_name().unwrap_or_else(|| "No folder yet".into());
    let head = row()
        .gap(8.0)
        .center_y()
        .child(text(folder).font(type_::BODY).color(if g.dir.is_some() { TEXT } else { TEXT_DIM }).grow(1.0).min_w(0.0).truncate())
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
    let mut body = col().grow(1.0).min_h(0.0).gap(10.0).child(head);
    if g.dir.is_none() {
        body = body.child(
            col().w_full().h(84.0).center().radius(12.0).border_dash(6.0, 5.0).border(1.0, Color::WHITE.alpha(0.16)).gap(2.0)
                .child(text("Point me at your meme folder".to_string()).font(type_::BODY).color(TEXT))
                .child(text("or drop a picture on the window and it prints".to_string()).font(type_::LABEL).color(TEXT_DIM)),
        );
    } else if g.entries.is_empty() {
        body = body.child(text("No pictures in this folder yet.".to_string()).font(type_::LABEL).color(TEXT_DIM));
    } else {
        let mut grid = row().flex_wrap().gap(8.0);
        for (i, e) in g.entries.iter().take(crate::gallery::MAX_THUMBS).enumerate() {
            let picked = g.selected.as_ref() == Some(&e.path);
            let path = e.path.clone();
            let tile = el()
                .key(format!("thumb-{i}"))
                .role("button")
                .label(e.name.clone())
                .focusable(true)
                .size(THUMB, THUMB)
                .radius(10.0)
                .fill(Color::BLACK.alpha(0.35))
                .border(if picked { 2.0 } else { 1.0 }, if picked { CORAL } else { Color::WHITE.alpha(0.08) })
                .opacity(if s.drag.as_ref().is_some_and(|d| d.path == e.path && d.travel >= crate::app::DRAG_MIN) { 0.35 } else { 1.0 })
                // A short press picks the picture; pulling it toward the printer prints it.
                .on_drag(move |s: &mut App, d: &mut Drag, _: &mut Tx| match d.phase {
                    Phase::Down => s.drag_start(path.clone()),
                    Phase::Move => s.drag_move(d.dx, d.dy),
                    Phase::Up => s.drag_end(d.cancelled()),
                });
            grid = grid.child(match g.thumbs.get(&e.path) {
                Some(image) => tile.pixels_fit(image.pixels(), SurfaceFit::Fill),
                None => tile,
            });
        }
        // A window onto the pictures that takes whatever height the settings leave, with a scroll
        // bar you can see and drag: a big folder never pushes the settings out of view.
        let window = col().w_full().grow(1.0).min_h(GALLERY_MIN).scroll(ScrollSpec { stick_bottom: false }).child(grid).scroll_bar_styled(bar_style());
        body = body.child(window);
        let more = g.entries.len().saturating_sub(crate::gallery::MAX_THUMBS);
        if more > 0 {
            body = body.child(text(format!("+ {more} older pictures in the folder, newest shown first")).font(type_::LABEL).color(TEXT_DIM));
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
            .child(text(note).font(type_::LABEL).color(if g.auto { CORAL } else { TEXT_DIM }).truncate());
    }
    col().grow(1.0).min_h(0.0).gap(8.0).child(caption("Gallery")).child(body)
}

/// A scroll bar you can actually see: a light, slightly wide thumb that turns coral under the pointer.
fn bar_style() -> ScrollBarStyle {
    ScrollBarStyle::from_kit(kit()).thumb(Color::WHITE.alpha(0.38)).thumb_hover(CORAL).width(7.0).hover_width(10.0)
}

/// A horizontal slider from -1 to +1, shown as a percentage. Drag it, click on the track, or use
/// the arrow keys when it has focus.
fn slider(key: &'static str, title: &str, value: f32, set: fn(&mut App, f32)) -> El {
    const W: f32 = LEFT - 34.0;
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
        .child(el().key("fill").size(KNOB / 2.0 + u * travel, BAR).absolute(0.0, (H - BAR) / 2.0).radius(BAR / 2.0).gradient(CORAL, PINK))
        .child(el().key("knob").size(KNOB, KNOB).absolute(u * travel, (H - KNOB) / 2.0).radius(KNOB / 2.0).fill(Color::WHITE).shadow(1.0, 4.0, Color::BLACK.alpha(0.5)));
    col()
        .gap(6.0)
        .child(row().w_full().child(caption(title).grow(1.0)).child(text(format!("{:+.0}%", value * 100.0)).font(type_::LABEL).color(TEXT_DIM)))
        .child(track)
}

/// The three looks most pictures want, so the dithering names stay out of the way.
const LOOKS: [(&str, Dither); 4] = [("Photo", Dither::FloydSteinberg), ("Meme", Dither::Atkinson), ("Line art", Dither::Threshold), ("Soft", Dither::Ordered)];

/// The Image tab: the gallery in one card and the settings in another, so they read as two
/// separate things. The gallery card takes the height left over and scrolls; the settings card
/// keeps its size and is always in view.
fn image_panels(s: &App) -> El {
    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(12.0)
        .child(card(gallery_section(s)).grow(1.0).min_h(0.0))
        .child(card(image_settings(s)))
}

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
        .child(TextArea::new(&s.code_text).rows(3).placeholder("A link, some text, or digits for EAN-13").on_change(|s: &mut App, _| s.touch()).on_edit(|s: &mut App| &mut s.code_text).build(kit()).w_full())
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
    let (state, state_color) = if listening { ("Listening", TEXT_DIM) } else { ("Port in use by another program: pick another", CORAL) };
    let exe = crate::mcp_setup::mcp_exe();
    let (code_cmd, json) = (crate::mcp_setup::claude_code_command(&exe), crate::mcp_setup::json_config(&exe));
    let (http_url, http_cmd, http_json) = (crate::mcp_setup::http_url(), crate::mcp_setup::claude_code_http_command(), crate::mcp_setup::json_http_config());
    let copy = |label: &'static str, what: String, note: &'static str| {
        Button::new(label)
            .on_press_tx(move |s: &mut App, tx: &mut Tx| {
                tx.copy(what.clone());
                s.status = note.into();
            })
            .build(kit())
    };
    field(
        "From an AI assistant (MCP)",
        col()
            .gap(8.0)
            .child(text("Let Claude or another assistant print: tools to print text, QR codes, pictures and check the printer.".to_string()).font(type_::LABEL).color(TEXT_DIM).wrap())
            .child(text(format!("While the app is open it is an MCP server at {http_url} (this computer only).")).font(type_::LABEL).color(TEXT_DIM).wrap())
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(text("Port".to_string()).font(type_::LABEL).color(TEXT_DIM))
                    .child(
                        TextInput::new(&s.mcp_port)
                            .placeholder("7391")
                            .width(None)
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
                    .child(copy("Copy Claude Code command", http_cmd, "Claude Code command copied: paste it in a terminal"))
                    .child(copy("Copy URL config", http_json, "JSON config copied: paste it in your assistant's MCP settings")),
            )
            .child(text("Or run it through the helper program (stdio):".to_string()).font(type_::LABEL).color(TEXT_DIM))
            .child(
                row()
                    .gap(8.0)
                    .child(copy("Copy stdio command", code_cmd, "Claude Code command copied: paste it in a terminal"))
                    .child(copy("Copy stdio JSON", json, "JSON config copied: paste it in your assistant's MCP settings")),
            )
            .child(text("Claude Desktop, Cursor and others take the JSON. Run tronic-mcp --config for the full steps.".to_string()).font(type_::LABEL).color(TEXT_DIM).wrap()),
    )
}

fn terminal_controls(s: &App) -> El {
    let usage = |line: &str| {
        row().px(10.0).h(30.0).center_y().radius(8.0).fill(Color::BLACK.alpha(0.35)).child(text(line.to_string()).font(type_::BODY).color(CORAL))
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
                .child(text("Keep the app open and connected.".to_string()).font(type_::LABEL).color(TEXT_DIM)),
        ))
        .child(mcp_section(s))
        .child(field(
            "Text size",
            choice(&["Small", "Medium", "Large"], s.term.size, |s, i| {
                s.term.size = i;
                s.touch();
            }),
        ))
        .child(Switch::new(s.term.auto).label("Print live output as it arrives").on_toggle(|s: &mut App| s.term.auto = !s.term.auto).build(kit()))
        .child(
            row()
                .gap(12.0)
                .center_y()
                .child(
                    text(format!("{state}  ·  {} lines, {} not printed", s.term.total(), s.term.unprinted()))
                        .font(type_::LABEL)
                        .color(TEXT_DIM)
                        .grow(1.0),
                )
                .child(Button::new("Clear").variant(ButtonVariant::Ghost).on_press(|s: &mut App| {
                    s.term.clear();
                    s.touch();
                }).build(kit())),
        )
}

/// The big glowing button. Disabled, it fades into the panel; ready, it crackles with plasma, and
/// pressing it sends a shockwave through the window.
fn hero_button(s: &App, label: String) -> El {
    let enabled = s.can_print();
    let b = row()
        .key("print")
        .role("button")
        .label("Print")
        .focusable(true)
        .w_full()
        .h(54.0)
        .center()
        .radius(27.0)
        .child(text(label).font(type_::TITLE).color(if enabled { Color::WHITE } else { TEXT_DIM }));
    let b = if enabled {
        b.gradient(CORAL, PINK).shadow(6.0, 26.0, PINK.alpha(0.45)).on_press(|s: &mut App| s.print())
    } else {
        b.fill(PANEL).border(1.0, Color::WHITE.alpha(0.06))
    };
    if !s.fx_on {
        return b;
    }
    // Slots overlay the button: plasma and a neon glow when it is ready, a shockwave just after
    // it was pressed (a wide box, so the ripple has room to travel).
    let mut wrap = col().w_full().h(54.0).child(b);
    let surge = if s.press_at.is_some_and(|t| s.clock - t < 0.6) { 2.2 } else { 1.35 };
    if enabled {
        wrap = wrap
            .child(fx::slot(fx::GLOW, [1.0, fx::PINK, 0.55, 22.0]).absolute(0.0, 0.0).w_full().h(54.0).radius(27.0).effect_bleed(60.0))
            // Real lightning, not a hairline: two strands with forks that jump off the edge. It
            // surges for a moment when the button is pressed.
            .child(fx::slot(fx::ELECTRIC, [surge, 4.0, 0.0, fx::PINK]).absolute(0.0, 0.0).w_full().h(54.0).radius(27.0).effect_bleed(34.0));
    }
    if let Some(t0) = s.press_at.filter(|t| s.clock - t < 1.4) {
        wrap = wrap.child(fx::slot(fx::SHOCK, [0.0, 0.0, t0 as f32, 14.0]).effect_backdrop(true).absolute(-300.0, -223.0).size(1000.0, 500.0));
    }
    wrap
}

fn print_bar(s: &App) -> El {
    let k = kit();
    // Darkness and the number of copies share one slim row, so the galleries above keep their height.
    let density = SegmentedControl::new(&["Light", "Medium", "Dark"], Some(s.density)).width(Some(LEFT - 150.0)).on_pick(|s: &mut App, i, _| s.density = i).build(k);
    let copies = row()
        .gap(6.0)
        .center_y()
        .child(Button::new("−").on_press(|s: &mut App| s.copies = s.copies.saturating_sub(1).max(1)).build(k))
        .child(text(format!("×{}", s.copies)).font(type_::BODY).color(TEXT).w(26.0))
        .child(Button::new("+").on_press(|s: &mut App| s.copies = (s.copies + 1).min(9)).build(k));
    let options = row().gap(10.0).center_y().child(density).child(copies);
    let length = s.composed.as_ref().map(|b| format!("  ·  {:.1} cm", render::length_cm(b))).unwrap_or_default();
    let label = if s.busy { "Printing…".to_string() } else { format!("Print{length}") };
    let mut bar = col().gap(12.0).child(options).child(hero_button(s, label));
    if s.busy {
        bar = bar.child(Progress::new(Some(s.progress)).build(k));
    }
    if let Some(e) = s.error.clone() {
        bar = bar.child(error_banner(s, e));
    }
    bar
}

/// A joke headline when we have one, the real reason always underneath. A fresh error shakes and
/// throws a red zap.
fn error_banner(s: &App, reason: String) -> El {
    let fresh = s.fx_on && s.error_at.is_some_and(|t| s.clock - t < 0.9);
    let mut body = col().gap(2.0);
    if let Some(headline) = voice::error_headline(&reason) {
        body = body.child(text(headline.to_string()).font(type_::BODY).color(RED));
    }
    body = body.child(text(reason).font(type_::LABEL).color(RED.alpha(0.85)).wrap());
    let mut banner = col().w_full().padding(10.0).radius(12.0).fill(RED.alpha(0.14)).border(1.0, RED.alpha(0.35)).child(body);
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
    let color = if percent <= 15 { RED } else if percent <= 40 { AMBER } else { MINT };
    row().gap(6.0).center_y().child(
        el().w(26.0).h(12.0).radius(3.0).border(1.0, TEXT_DIM).child(el().w(24.0 * p).h(10.0).radius(2.0).fill(color)),
    ).child(text(format!("{percent}%")).font(type_::LABEL).color(TEXT_DIM))
}

fn status_chip(s: &App) -> El {
    let (color, label) = match &s.link {
        Link::Offline => (RED, "Offline".to_string()),
        Link::Connecting => (AMBER, "Connecting…".to_string()),
        Link::Online(_) => (MINT, "Connected".to_string()),
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
        .child(text(label).font(type_::LABEL).color(TEXT));
    if let Some(b) = s.battery {
        chip = chip.child(battery(b));
    }
    chip
}

fn header(s: &App) -> El {
    let k = kit();
    let connect = match &s.link {
        Link::Offline => Button::new("Connect").variant(ButtonVariant::Contrast).on_press(|s: &mut App| s.connect()).build(k),
        Link::Connecting => Button::new("Connecting…").disabled(true).build(k),
        Link::Online(_) => Button::new("Disconnect").variant(ButtonVariant::Ghost).disabled(s.busy).on_press(|s: &mut App| s.disconnect()).build(k),
    };
    // Two rows, so nothing is pushed out of the column when everything is showing.
    let mut top = row().gap(10.0).center_y().child(brand(s));
    if matches!(s.link, Link::Online(_)) {
        top = top.child(Button::new("Refresh").variant(ButtonVariant::Ghost).disabled(s.busy).on_press(|s: &mut App| s.refresh_info()).build(k));
    }
    let top = top.child(connect);
    let mut status = row().gap(10.0).center_y().child(status_chip(s));
    if let Some(line) = device_line(s) {
        status = status.child(text(line).font(type_::LABEL).color(TEXT_DIM).truncate().grow(1.0).min_w(0.0));
    }
    let mut head = col().gap(10.0).child(top).child(status);
    if let Some(update) = &s.update {
        head = head.child(update_banner(update));
    }
    head
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
        .fill(MINT.alpha(0.12))
        .border(1.0, MINT.alpha(0.35))
        .child(text(format!("Version {} is available", update.version)).font(type_::LABEL).color(MINT).grow(1.0).min_w(0.0).truncate())
        .child(Button::new("Get it").variant(ButtonVariant::Contrast).on_press(move |_: &mut App| update::open(&url)).build(kit()))
        .child(Button::new("Later").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.update = None).build(kit()))
}

/// The mark and the wordmark, with what the printer is doing under it.
fn brand(s: &App) -> El {
    let mark = match &s.logo {
        Some(image) => el().key("logo").size(40.0, 40.0).radius(10.0).pixels(image.pixels()),
        None => el().size(40.0, 40.0).radius(10.0).gradient(CORAL, PINK),
    }
    // Five taps on the mark: the ghost says boo.
    .on_press(|s: &mut App| s.logo_tap());
    row().grow(1.0).min_w(0.0).gap(10.0).center_y().child(mark).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .child(text("TRONIC".to_string()).font(type_::TITLE).color(CORAL))
            .child(text(s.status.clone()).font(type_::LABEL).color(TEXT_DIM).truncate()),
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
    if let Some(p) = &s.info.paper {
        parts.push(format!("paper status {p}"));
    }
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
        .h(38.0)
        .center()
        .radius(19.0)
        .on_press(move |s: &mut App| {
            s.mode = mode;
            s.touch();
        })
        .child(text(label.to_string()).font(type_::LABEL).color(if selected { Color::WHITE } else { TEXT_DIM }))
}

fn tabs(s: &App) -> El {
    const GAP: f32 = 4.0;
    let each = (LEFT - 2.0 * GAP - (Mode::ALL.len() as f32 - 1.0) * GAP) / Mode::ALL.len() as f32;
    let at = Mode::ALL.iter().position(|m| *m == s.mode).unwrap_or(0) as f32;
    // The lit pill springs from tab to tab.
    let pill = el()
        .key("tab-pill")
        .absolute(GAP + at * (each + GAP), GAP)
        .size(each, 38.0)
        .radius(19.0)
        .gradient(CORAL, PINK)
        .shadow(4.0, 16.0, PINK.alpha(0.35))
        .motion(motion::SNAPPY);
    let mut r = row().w_full().gap(GAP).padding(GAP).radius(23.0).fill(PANEL).border(1.0, Color::WHITE.alpha(0.06)).child(pill);
    for m in Mode::ALL {
        r = r.child(tab(m.label(), m == s.mode, m));
    }
    r
}

/// The printer as it looks on the desk: a white pebble with a steel tear bar and one light that
/// is green when connected, amber while working or connecting, red when off.
fn printer(s: &App) -> El {
    let light = match &s.link {
        Link::Online(_) if s.busy => AMBER,
        Link::Online(_) => MINT,
        Link::Connecting => AMBER,
        Link::Offline => RED,
    };
    let mut bar = el().w(480.0).h(12.0).radius(6.0).gradient(style::rgb(0xe9e9ee), style::rgb(0x8a8a94)).border(1.0, Color::BLACK.alpha(0.25));
    let mut lamp = el().w(40.0).h(11.0).radius(6.0).fill(light).shadow(0.0, 16.0, light.alpha(0.85));
    if s.fx_on {
        // The tear bar turns to liquid chrome; flames and heat haze come off it while printing.
        let (flame, smoke) = (s.flame(), s.smoke());
        // The metal only flows while the printer works; at rest it holds still.
        let flow = if s.busy || flame > 0.0 { 0.6 } else { 0.0 };
        bar = bar.child(fx::slot(fx::CHROME, [0.0, 0.0, flow, 0.0]).absolute(0.0, 0.0).size(480.0, 12.0).radius(6.0));
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
            bar = bar.child(fx::slot(fx::FIRE, [height, intensity, 0.0, rate]).absolute(10.0, 0.0).size(460.0, 12.0).radius(6.0).effect_bleed(220.0));
        }
        if flame > 0.0 {
            bar = bar.child(fx::slot(fx::HAZE, [170.0, 1.3 * flame, 0.0, 0.0]).effect_backdrop(true).absolute(0.0, -200.0).size(480.0, 190.0));
        }
        if matches!(s.link, Link::Connecting) {
            // A small, slow radar ring around the light only.
            lamp = lamp.child(fx::slot(fx::PULSE, [0.0, fx::AMBER, 2.4, 22.0]).absolute(-4.0, -4.0).size(48.0, 19.0).radius(9.0).effect_bleed(22.0));
        }
    }
    let mut body = col()
        .w(480.0)
        .h(104.0)
        .radius(36.0)
        .items_center()
        .justify_end()
        .pb(20.0)
        // The SVG shell covers this; it is the fallback until the picture is ready.
        .gradient(style::rgb(0xfdfdfb), style::rgb(0xd6d6d1))
        .shadow(12.0, 36.0, Color::BLACK.alpha(0.6));
    if let Some(art) = &s.printer_art {
        body = body.child(el().absolute(0.0, 0.0).size(480.0, 104.0).pixels_fit(art.pixels(), SurfaceFit::Fit));
    }
    if s.fx_on && matches!(s.link, Link::Online(_)) {
        // The printer is electrified: a faint blue current crawls around its shell while it waits,
        // and it goes wild while it prints or the moment the button is pressed.
        let hot = s.busy || s.press_at.is_some_and(|t| s.clock - t < 0.6);
        let d = if hot { [1.7, 5.5, 0.0, fx::CYAN] } else { [0.75, 2.4, 1.0, fx::CYAN] };
        body = body.child(fx::slot(fx::ELECTRIC, d).absolute(0.0, 0.0).size(480.0, 104.0).radius(36.0).effect_bleed(34.0));
    }
    let body = body.child(lamp);
    col().items_center().child(bar).child(body)
}

/// The paper that is not there yet: a see-through cutout of a ticket with a ghost face, bobbing
/// gently (when effects are on). Real paper takes its place as soon as there is something to print.
fn ghost_paper(s: &App) -> El {
    // A new resting height every second or so; the slow spring below glides there, so the ghost
    // drifts gently instead of jittering.
    let bob = if s.fx_on { 5.0 * ((s.clock * 0.7).floor() as f32 * 1.7).sin() } else { 0.0 };
    let eye = || el().w(13.0).h(20.0).radius(7.0).fill(Color::WHITE.alpha(0.9));
    let face = col()
        .items_center()
        .gap(10.0)
        .child(row().gap(30.0).child(eye()).child(eye()))
        .child(el().w(26.0).h(14.0).radius(7.0).fill(Color::WHITE.alpha(0.55)));
    let ticket = col()
        .w(render::PREVIEW_WIDTH as f32)
        .h(250.0)
        .gap(18.0)
        .center()
        .radius(8.0)
        .fill(style::rgb(0xfaf6ec).alpha(0.15))
        .border_dash(8.0, 7.0)
        .border(1.5, style::rgb(0xfaf6ec).alpha(0.4))
        .child(face)
        .child(text(s.ghost_line().to_string()).font(type_::BODY).color(style::rgb(0xfaf6ec).alpha(0.8)));
    col().offset(0.0, bob).offset_motion(Spring::new(1.6, 0.0)).child(ticket)
}

fn paper(s: &App) -> El {
    let info = s.composed.as_ref().map(|b| format!("{:.1} cm", render::length_cm(b))).unwrap_or_default();
    let sheet = match (&s.preview, &s.composed) {
        (Some(image), Ok(bitmap)) => {
            let full = (bitmap.height + render::TEETH) as f32;
            let mut paper = el().key("paper").w(render::PREVIEW_WIDTH as f32).pixels(image.pixels());
            if s.fx_on && s.done_at.is_some_and(|t| s.clock - t < 2.2) {
                let t0 = s.done_at.unwrap_or(0.0) as f32;
                paper = paper.child(fx::slot(fx::SPARKLES, [t0, 16.0, 9.0, fx::WHITE]).absolute(0.0, 0.0).size(render::PREVIEW_WIDTH as f32, full).effect_bleed(30.0));
            }
            if s.busy && s.fx_on {
                // The ticket is pushed out of the printer as the job goes: only what has been printed
                // so far shows, the toothed top edge rising with it.
                let shown = (full * s.progress.clamp(0.02, 1.0)).max(24.0);
                col().w(render::PREVIEW_WIDTH as f32).h(shown).clip_rounded(0.0).child(paper)
            } else {
                paper
            }
        }
        _ => ghost_paper(s),
    };
    let crop = if s.preview_cropped { "  ·  last part" } else { "" };
    let title = row()
        .gap(12.0)
        .center_y()
        .child(caption("Preview").grow(1.0))
        .child(text(format!("{info}{crop}")).font(type_::LABEL).color(CORAL))
        .child(Switch::new(s.fx_on).label("FX").on_toggle(|s: &mut App| s.set_effects(!s.fx_on)).build(kit()));
    // A picture is being pulled out of the gallery: the preview lights up as the place to drop it.
    let dragging = s.drag.as_ref().is_some_and(|d| d.travel >= crate::app::DRAG_MIN);
    let armed = dragging && s.over_stage;
    // Where a dragged picture is let go to print: the ticket and the printer, not the gallery.
    let mut drop_zone = col()
        .key("stage")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .items_center()
        // The ticket stands out of the printer, newest line lowest, like on the desk.
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .items_center()
                .justify_end()
                .child(col().items_center().child(sheet).scroll(ScrollSpec { stick_bottom: true }).shrink().min_h(0.0)),
        )
        .child(printer(s));
    if armed {
        drop_zone = drop_zone.child(row().absolute(0.0, 0.0).w_full().h(48.0).center().child(text("Let go to print".to_string()).font(type_::TITLE).color(CORAL)));
    }
    let mut stage = col()
        .key("stage-frame")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .radius(24.0)
        .gradient(if armed { CORAL.alpha(0.16) } else { Color::WHITE.alpha(0.05) }, if armed { PINK.alpha(0.10) } else { Color::WHITE.alpha(0.015) })
        .border(if armed { 2.0 } else { 1.0 }, if armed { CORAL } else if dragging { CORAL.alpha(0.45) } else { Color::WHITE.alpha(0.07) })
        .padding(18.0)
        .gap(12.0)
        .items_center();
    // On the Text tab the themes live up here, in the room the short tickets leave free.
    if s.mode == Mode::Text {
        stage = stage.child(card(theme_gallery(s)).w_full().h(250.0));
    }
    let stage = stage.child(drop_zone);
    col().grow(1.0).min_h(0.0).gap(12.0).child(title).child(stage)
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

pub fn view(s: &App, _: &mut Cx<App>) -> El {
    let left = col().w(LEFT).h_full().gap(16.0).child(header(s)).child(tabs(s));
    let left = match s.mode {
        // The Image tab is two cards, the gallery and the settings, laid out in the space it is given.
        Mode::Image => left.child(image_panels(s)),
        Mode::Text => left.child(text_panels(s)),
        mode => {
            let controls = match mode {
                Mode::Text => text_controls(s),
                Mode::Codes => code_controls(s),
                _ => terminal_controls(s),
            };
            left.child(col().grow(1.0).min_h(0.0).child(card(controls).scroll(ScrollSpec { stick_bottom: false }).shrink().min_h(0.0).scroll_bar(kit())))
        }
    }
    .child(print_bar(s));
    let mut body = row()
        .w_full()
        .h_full()
        .padding(24.0)
        .gap(24.0)
        .gradient(style::BG_TOP, style::BG_BOTTOM)
        .child(left)
        .child(paper(s))
        // Pictures dropped on the window from another app: printed at once when connected.
        .on_drop_files(|s: &mut App, files, _| s.drop_files(files));
    if let Some(ghost) = drag_ghost(s) {
        body = body.child(ghost);
    }
    rune_kit::root(body, Vec::<El>::new())
}
