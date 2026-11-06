//! The window: controls on the left, the paper coming out of its slot on the right.

use rune::tok::{motion, type_};
use rune::{Color, Cx, Drag, El, FileDialogOptions, Phase, ScrollSpec, Spring, SurfaceFit, Tx, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Kit, Progress, ScrollBar, ScrollBarStyle, SegmentedControl, Switch, TextArea};

use crate::app::{App, Link, Mode};
use crate::fx;
use crate::render::{self, Align, CodeKind, Dither, EcLevel, FontChoice};
use crate::style::{self, AMBER, CORAL, MINT, PANEL, PINK, RED, TEXT, TEXT_DIM};
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

fn text_controls(s: &App) -> El {
    let o = s.text_opts;
    let font_ix = FontChoice::ALL.iter().position(|f| *f == o.font).unwrap_or(0);
    let size_ix = [22.0, 32.0, 48.0, 72.0].iter().position(|p| *p == o.px).unwrap_or(1);
    let align_ix = [Align::Left, Align::Center, Align::Right].iter().position(|a| *a == o.align).unwrap_or(0);
    col()
        .gap(14.0)
        .child(TextArea::new(&s.text).rows(4).placeholder("Type something to print").on_change(|s: &mut App, _| s.touch()).on_edit(|s: &mut App| &mut s.text).build(kit()).w_full())
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text("Out of ideas?".to_string()).font(type_::LABEL).color(TEXT_DIM).grow(1.0))
                .child(Button::new("Surprise me").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.surprise_me()).build(kit())),
        )
        .child(field(
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
                .on_press(move |s: &mut App| s.pick_from_gallery(path.clone()));
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

fn image_controls(s: &App) -> El {
    let o = s.pic_opts;
    let look_ix = LOOKS.iter().position(|(_, d)| *d == o.dither).unwrap_or(0);
    // The panel fills the column: the gallery takes the height left over and scrolls, the
    // settings below it keep their size and are always in view.
    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(12.0)
        .child(gallery_section(s))
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
    if enabled {
        wrap = wrap
            .child(fx::slot(fx::GLOW, [1.0, fx::PINK, 0.55, 22.0]).absolute(0.0, 0.0).w_full().h(54.0).radius(27.0).effect_bleed(60.0))
            .child(fx::slot(fx::ELECTRIC, [0.85, 0.0, 1.0, fx::PINK]).absolute(0.0, 0.0).w_full().h(54.0).radius(27.0).effect_bleed(24.0));
    }
    if let Some(t0) = s.press_at.filter(|t| s.clock - t < 1.4) {
        wrap = wrap.child(fx::slot(fx::SHOCK, [0.0, 0.0, t0 as f32, 14.0]).effect_backdrop(true).absolute(-300.0, -223.0).size(1000.0, 500.0));
    }
    wrap
}

fn print_bar(s: &App) -> El {
    let k = kit();
    let density = choice(&["Light", "Medium", "Dark"], s.density, |s, i| s.density = i);
    let copies = row()
        .gap(10.0)
        .center_y()
        .child(text(format!("{} {}", s.copies, if s.copies == 1 { "copy" } else { "copies" })).font(type_::BODY).color(TEXT).grow(1.0))
        .child(Button::new("−").on_press(|s: &mut App| s.copies = s.copies.saturating_sub(1).max(1)).build(k))
        .child(Button::new("+").on_press(|s: &mut App| s.copies = (s.copies + 1).min(9)).build(k));
    let length = s.composed.as_ref().map(|b| format!("  ·  {:.1} cm", render::length_cm(b))).unwrap_or_default();
    let label = if s.busy { "Printing…".to_string() } else { format!("Print{length}") };
    let mut bar = col().gap(12.0).child(card(col().gap(14.0).child(field("Darkness", density)).child(copies))).child(hero_button(s, label));
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
    col().gap(10.0).child(top).child(status)
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
    let mut bar = el().w(420.0).h(12.0).radius(6.0).gradient(style::rgb(0xe9e9ee), style::rgb(0x8a8a94)).border(1.0, Color::BLACK.alpha(0.25));
    let mut lamp = el().w(40.0).h(11.0).radius(6.0).fill(light).shadow(0.0, 16.0, light.alpha(0.85));
    if s.fx_on {
        // The tear bar turns to liquid chrome; flames and heat haze come off it while printing.
        let flame = s.flame();
        // The metal only flows while the printer works; at rest it holds still.
        let flow = if s.busy || flame > 0.0 { 0.6 } else { 0.0 };
        bar = bar.child(fx::slot(fx::CHROME, [0.0, 0.0, flow, 0.0]).absolute(0.0, 0.0).size(420.0, 12.0).radius(6.0));
        if flame > 0.0 {
            bar = bar
                .child(fx::slot(fx::FIRE, [110.0 + 120.0 * flame, 0.55 + 1.0 * flame, 0.0, 0.5]).absolute(10.0, 0.0).size(400.0, 12.0).radius(6.0).effect_bleed(220.0))
                .child(fx::slot(fx::HAZE, [170.0, 1.3 * flame, 0.0, 0.0]).effect_backdrop(true).absolute(0.0, -200.0).size(420.0, 190.0));
        }
        if matches!(s.link, Link::Connecting) {
            // A small, slow radar ring around the light only.
            lamp = lamp.child(fx::slot(fx::PULSE, [0.0, fx::AMBER, 2.4, 22.0]).absolute(-4.0, -4.0).size(48.0, 19.0).radius(9.0).effect_bleed(22.0));
        }
    }
    let body = col()
        .w(450.0)
        .h(104.0)
        .radius(36.0)
        .items_center()
        .justify_end()
        .pb(20.0)
        .gradient(style::rgb(0xfdfdfb), style::rgb(0xd6d6d1))
        .border(1.0, Color::WHITE.alpha(0.55))
        .shadow(12.0, 36.0, Color::BLACK.alpha(0.6))
        .child(lamp);
    col().items_center().child(bar).child(body)
}

/// The paper that is not there yet: a see-through cutout of a ticket with a ghost face, bobbing
/// gently (when effects are on). Real paper takes its place as soon as there is something to print.
fn ghost_paper(s: &App) -> El {
    // The view only updates this a few times a second; the spring below smooths it into a float.
    let bob = if s.fx_on { 7.0 * ((s.clock * 2.2).floor() as f32 * 0.9).sin() } else { 0.0 };
    let eye = || el().w(13.0).h(20.0).radius(7.0).fill(Color::WHITE.alpha(0.9));
    let face = col()
        .items_center()
        .gap(10.0)
        .child(row().gap(30.0).child(eye()).child(eye()))
        .child(el().w(26.0).h(14.0).radius(7.0).fill(Color::WHITE.alpha(0.55)));
    let ticket = col()
        .w(384.0)
        .h(250.0)
        .gap(18.0)
        .center()
        .radius(8.0)
        .fill(style::rgb(0xfaf6ec).alpha(0.15))
        .border_dash(8.0, 7.0)
        .border(1.5, style::rgb(0xfaf6ec).alpha(0.4))
        .child(face)
        .child(text(s.ghost_line().to_string()).font(type_::BODY).color(style::rgb(0xfaf6ec).alpha(0.8)));
    col().offset(0.0, bob).offset_motion(Spring::new(0.9, 0.25)).child(ticket)
}

fn paper(s: &App) -> El {
    let info = s.composed.as_ref().map(|b| format!("{:.1} cm", render::length_cm(b))).unwrap_or_default();
    let sheet = match (&s.preview, &s.composed) {
        (Some(image), Ok(bitmap)) => {
            let full = (bitmap.height + render::TEETH) as f32;
            let mut paper = el().key("paper").w(384.0).pixels(image.pixels());
            if s.fx_on && s.done_at.is_some_and(|t| s.clock - t < 2.2) {
                let t0 = s.done_at.unwrap_or(0.0) as f32;
                paper = paper.child(fx::slot(fx::SPARKLES, [t0, 16.0, 9.0, fx::WHITE]).absolute(0.0, 0.0).size(384.0, full).effect_bleed(30.0));
            }
            if s.busy && s.fx_on {
                // The ticket is pushed out of the printer as the job goes: only what has been printed
                // so far shows, the toothed top edge rising with it.
                let shown = (full * s.progress.clamp(0.02, 1.0)).max(24.0);
                col().w(384.0).h(shown).clip_rounded(0.0).child(paper)
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
    let stage = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .radius(24.0)
        .gradient(Color::WHITE.alpha(0.05), Color::WHITE.alpha(0.015))
        .border(1.0, Color::WHITE.alpha(0.07))
        .padding(18.0)
        .gap(0.0)
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
    col().grow(1.0).min_h(0.0).gap(12.0).child(title).child(stage)
}

pub fn view(s: &App, _: &mut Cx<App>) -> El {
    let controls = match s.mode {
        Mode::Text => text_controls(s),
        Mode::Image => image_controls(s),
        Mode::Codes => code_controls(s),
        Mode::Terminal => terminal_controls(s),
    };
    let left = col()
        .w(LEFT)
        .h_full()
        .gap(16.0)
        .child(header(s))
        .child(tabs(s))
        .child(if s.mode == Mode::Image {
            // The Image tab lays itself out in the space it is given (its gallery scrolls).
            col().grow(1.0).min_h(0.0).child(card(controls).grow(1.0).min_h(0.0))
        } else {
            col().grow(1.0).min_h(0.0).child(card(controls).scroll(ScrollSpec { stick_bottom: false }).shrink().min_h(0.0).scroll_bar(kit()))
        })
        .child(print_bar(s));
    let body = row()
        .w_full()
        .h_full()
        .padding(24.0)
        .gap(24.0)
        .gradient(style::BG_TOP, style::BG_BOTTOM)
        .child(left)
        .child(paper(s))
        // Pictures dropped anywhere on the window: shown, or printed at once with the watcher on.
        .on_drop_files(|s: &mut App, files, _| s.drop_files(files));
    rune_kit::root(body, Vec::<El>::new())
}
