//! Tronic Printer: print text, pictures and codes on the Lidl Tronic / Silvercrest 5890
//! "Mini Pocket Printer" from Windows, over Bluetooth LE. UI by Rune.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod ansi;
mod app;
mod ble;
mod chrome;
mod client;
mod egg;
mod fx;
mod gallery;
mod ipc;
mod jobs;
mod jobspec;
mod mcp;
mod mcp_http;
mod mcp_setup;
mod protocol;
mod render;
mod settings;
mod style;
mod term;
mod ui;
mod update;
mod voice;

use std::time::Instant;

use rune::{AppSpec, ImageCache, KeyEvent, Tx, Waker};

use crate::app::App;

const TITLE: &str = "Tronic Printer";

/// The value of a `--flag value` pair in the process arguments, for the snapshot tries Rune leaves
/// to the app (`--palette`, say). Rune's own flags are read by Rune itself.
fn arg_after(flag: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == flag {
            return args.next();
        }
    }
    None
}

fn setup(s: &mut App, waker: Waker) {
    let images = ImageCache::new(waker.clone());
    s.logo = Some(images.load(&include_bytes!("../assets/logo-96.png")[..]));
    // The printer body is drawn as an SVG, rasterized sharp for high-DPI screens.
    s.printer_art = Some(images.svg(&include_bytes!("../assets/printer.svg")[..], 600.0, 3.0));
    s.images = Some(images);
    s.reload_themes();
    // A snapshot draws one frame, so it needs every theme's picture now.
    if rune::is_headless() {
        s.make_theme_thumbs(true);
    }
    s.ipc = Some(ipc::spawn(waker.clone()));
    let (tx, rx) = ble::spawn(waker.clone());
    s.cmd = Some(tx);
    s.events = Some(rx);
    // Snapshots render one frame and must not go scanning for Bluetooth.
    if std::env::args().any(|a| a.starts_with("--snap") || a.starts_with("--tree")) {
        s.auto_connect = false;
    } else {
        // Effects follow the last choice, or Windows' "Show animations" when there is none yet.
        let saved = settings::load();
        s.fx_on = saved.effects.unwrap_or_else(chrome::animations_enabled);
        if let Some(dir) = saved.gallery {
            s.gallery.set_dir(dir);
        }
        s.gallery.auto = saved.auto_print;
        s.fav_themes = saved.fav_themes;
        s.gallery.favorites = saved.fav_pictures;
        s.palette = style::PaletteId::from_id(&saved.palette).unwrap_or_default();
        style::set_palette(s.palette);
        s.update_rx = Some(update::spawn(waker));
    }
    // For snapshots (and tries): `TRONIC_PALETTE=matrix` wears that palette whatever was saved.
    if let Some(id) = std::env::var("TRONIC_PALETTE").ok().and_then(|v| style::PaletteId::from_id(&v)) {
        s.palette = id;
        style::set_palette(id);
    }
    // `--palette night`, next to `--snap` and `--scene`: the same ids as the chips, and the last
    // word over what the saved settings or TRONIC_PALETTE say.
    if let Some(id) = arg_after("--palette").and_then(|v| style::PaletteId::from_id(&v)) {
        s.palette = id;
        style::set_palette(id);
    }
    // The frame thread paints the palette it finds when the window appears; this covers a frame
    // that already existed while the saved palette, `TRONIC_PALETTE` or `--palette` overrode it.
    chrome::repaint();
}

/// The Konami code, typed anywhere a text field does not have the keys.
fn on_key(s: &mut App, e: &KeyEvent, _: &mut Tx) -> bool {
    if s.key_shortcut(e) {
        return true;
    }
    match egg::pad_of(&e.key) {
        Some(pad) if s.konami.feed(pad) => {
            s.egg_fire();
            true
        }
        _ => false,
    }
}

/// Ask to be polled again in `every`, unless a timer that fires that soon is already out: every
/// poll used to start a timer of its own, and the timers piled up into a redraw storm.
fn schedule(s: &mut App, clock: f64, tx: &mut Tx, every: std::time::Duration) {
    let due = clock + every.as_secs_f64();
    if s.wake_pending.is_some_and(|d| d > clock && d <= due + 0.001) {
        return;
    }
    s.wake_pending = Some(due);
    tx.wake_after(every);
}

fn poll(s: &mut App, clock: f64, tx: &mut Tx) -> bool {
    s.clock = clock;
    if s.wake_pending.is_some_and(|d| d <= clock) {
        s.wake_pending = None;
    }
    let now = Instant::now();
    let events: Vec<ble::Event> = s.events.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
    let messages: Vec<ipc::IpcMsg> = s.ipc.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
    let mut changed = !events.is_empty() || !messages.is_empty();
    if let Some(found) = s.update_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
        s.update = Some(found);
        changed = true;
    }
    for e in events {
        s.apply(e);
    }
    for m in messages {
        s.handle_ipc(m, now);
    }
    let (was_busy, was_link) = (s.printing.is_busy(), s.link.clone());
    for again in [s.tick_connection(now), s.tick_terminal(now), s.tick_gallery(now), s.tick_custom_themes(now)].into_iter().flatten() {
        schedule(s, clock, tx, again);
    }
    // A small picture was drawn: the view must show it, but the ticket itself did not change.
    if let Some(again) = s.tick_theme_thumbs() {
        changed = true;
        schedule(s, clock, tx, again);
    }
    changed |= s.printing.is_busy() != was_busy || s.link != was_link;
    // Flames fading, an error shaking: the view moves with the clock for now.
    if let Some(every) = s.tick_interval() {
        changed = true;
        schedule(s, clock, tx, every);
    }
    // The line under the wordmark changes every minute, and types itself out only then.
    if s.tick_tagline(clock) {
        changed = true;
    }
    schedule(s, clock, tx, std::time::Duration::from_secs_f64(app::TAGLINE_TURN - clock % app::TAGLINE_TURN + 0.05));
    // The ghost says something else every quarter of an hour, and only then is the view redrawn.
    if s.composed.is_err() {
        let turn = (clock / app::GHOST_TURN) as u64;
        if turn != s.ghost_turn {
            s.ghost_turn = turn;
            changed = true;
        }
        schedule(s, clock, tx, std::time::Duration::from_secs_f64(app::GHOST_TURN - clock % app::GHOST_TURN + 0.05));
    }
    let title = s.window_title();
    if title != s.title_shown {
        tx.set_title(title.clone());
        s.title_shown = title;
    }
    if s.dirty {
        s.refresh();
        changed = true;
    }
    changed
}

/// Named states for headless snapshots: `--snap out.png --scene text`.
fn scene_text(s: &mut App) {
    s.drawer_open = false;
    s.text = rune::TextEdit::new("Hello from Windows!\nGroceries:\n- bread\n- coffee\n- more coffee");
}

fn scene_theme(s: &mut App) {
    scene_online(s);
    s.text = rune::TextEdit::new("");
    s.set_theme(render::Theme::Receipt);
    s.fav_themes = vec![render::Theme::Receipt.id().into(), render::Theme::Fortune.id().into()];
}

/// A silly form with its parts outlined, ready to be written on. The drawer is open: the theme
/// gallery is where a different form is picked.
fn scene_form(s: &mut App) {
    scene_online(s);
    s.drawer_open = true;
    s.text = rune::TextEdit::new("");
    s.set_theme(render::Theme::Complaint);
    s.text = rune::TextEdit::new("The coffee machine ate my coin and gave me nothing.");
    s.edit_for("name").replace_text("Alex");
    s.touch();
}

/// A long form whose whole ticket has to fit on the stage.
fn scene_butthurt(s: &mut App) {
    scene_online(s);
    s.set_theme(render::Theme::Butthurt);
    s.text = rune::TextEdit::new(render::Theme::Butthurt.sample());
    s.edit_for("user").replace_text("u/xX_prompt_Xx");
    s.edit_for("provider").replace_text("OpenCode");
    s.edit_for("model").replace_text("DeepSeek v4.1 Flash");
    s.touch();
}

/// The tweet: avatar placeholder, name, handle, text and numbers.
fn scene_tweet(s: &mut App) {
    scene_online(s);
    s.set_theme(render::Theme::Tweet);
    s.text = rune::TextEdit::new(render::Theme::Tweet.sample());
    s.edit_for("name").replace_text("Alex");
    s.edit_for("handle").replace_text("@alex");
    s.edit_for("stats").replace_text("1.2K likes · 348 reposts · 92 quotes");
    s.edit_for("footer").replace_text("printed, not posted");
    s.touch();
}

/// Big lines in different typefaces.
fn scene_salad(s: &mut App) {
    scene_online(s);
    s.set_theme(render::Theme::FontSalad);
    s.text = rune::TextEdit::new(render::Theme::FontSalad.sample());
    s.touch();
}

/// The same form with the message being typed on the paper.
fn scene_write(s: &mut App) {
    scene_form(s);
    s.refresh();
    s.edit_region("body");
}

/// A theme with one part being written on (or, for a theme that cannot be read, in the bar).
fn writing(s: &mut App, theme: render::Theme, key: &str) {
    scene_online(s);
    s.set_theme(theme);
    s.text = rune::TextEdit::new(theme.sample());
    s.refresh();
    s.edit_region(key);
}

fn scene_write_receipt(s: &mut App) {
    writing(s, render::Theme::Receipt, "store");
}

fn scene_write_parchment(s: &mut App) {
    writing(s, render::Theme::Parchment, "body");
}

fn scene_write_diploma(s: &mut App) {
    writing(s, render::Theme::Diploma, "name");
}

fn scene_write_flight(s: &mut App) {
    writing(s, render::Theme::Flight, "from");
}

fn scene_write_banner(s: &mut App) {
    writing(s, render::Theme::Banner, "body");
    s.text = rune::TextEdit::new("lolol");
}

fn scene_write_wanted(s: &mut App) {
    writing(s, render::Theme::Wanted, "reward");
}

fn scene_online(s: &mut App) {
    scene_text(s);
    s.link = app::Link::Online("Mini Pocket Printer_BLE".into());
    s.battery = Some(87);
    s.status = "Connected to Mini Pocket Printer_BLE".into();
}

fn scene_printing(s: &mut App) {
    scene_online(s);
    s.fx_on = true;
    s.printing = app::Printing::Running { progress: 0.45 };
    s.status = voice::printing(0.45).into();
}

fn scene_ghost(s: &mut App) {
    s.drawer_open = false;
    s.fx_on = true;
}

/// The ghost looking toward the top-left, for checking that the eyes follow the pointer.
fn scene_look(s: &mut App) {
    scene_ghost_online(s);
    s.pointer_at = Some((120.0, 120.0));
}

fn scene_ghost_online(s: &mut App) {
    s.fx_on = true;
    s.link = app::Link::Online("Mini Pocket Printer".into());
    s.text = rune::TextEdit::new("");
}

fn scene_connecting(s: &mut App) {
    s.fx_on = true;
    s.link = app::Link::Connecting;
    s.status = voice::connecting(1).into();
}

fn scene_done(s: &mut App) {
    scene_online(s);
    s.fx_on = true;
    s.printing = app::Printing::Done { at: 0.0 };
    s.press_at = Some(0.0);
}

fn scene_drag(s: &mut App) {
    scene_gallery(s);
    s.drag = Some(app::ThumbDrag { path: s.gallery.selected.clone().unwrap_or_default(), theme: None, travel: 120.0 });
    s.pointer = Some((1180.0, 640.0));
    s.over_stage = true;
}

fn scene_egg(s: &mut App) {
    scene_online(s);
    s.fx_on = true;
    s.egg_fire();
}

fn scene_error(s: &mut App) {
    scene_text(s);
    s.error = Some("Printer not found. Switch it on, close the phone app, turn the phone's Bluetooth off and keep the printer close to the PC.".into());
}

/// A folder of made-up pictures, so the gallery can be photographed without anyone's memes.
fn scene_gallery(s: &mut App) {
    use image::{Rgba, RgbaImage};
    let dir = std::env::temp_dir().join("tronic-scene-gallery");
    let _ = std::fs::create_dir_all(&dir);
    for i in 0..10u32 {
        let img = RgbaImage::from_fn(160, 160, |x, y| {
            let (fx, fy) = (x as f32 / 160.0, y as f32 / 160.0);
            let hue = (i as f32 * 0.11 + fx * 0.3) % 1.0;
            let ring = (((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt() * (6.0 + i as f32)).sin() * 0.5 + 0.5;
            let v = (0.25 + 0.75 * ring) * (0.6 + 0.4 * fy);
            Rgba([(255.0 * v * (0.5 + 0.5 * hue)) as u8, (255.0 * v * (1.0 - hue)) as u8, (255.0 * v * (0.3 + 0.7 * fx)) as u8, 255])
        });
        let _ = img.save(dir.join(format!("meme-{i:02}.png")));
    }
    scene_online(s);
    s.mode = app::Mode::Image;
    s.drawer_open = true;
    s.gallery.set_dir(dir.clone());
    s.gallery.auto = true;
    s.gallery.selected = Some(dir.join("meme-03.png"));
    s.gallery.favorites = vec![dir.join("meme-07.png"), dir.join("meme-02.png")];
    s.fx_on = true;
}

fn scene_qr(s: &mut App) {
    s.mode = app::Mode::Codes;
    s.code_text = rune::TextEdit::new("https://github.com/dimer47/PocketPrinter5890");
}

fn scene_image(s: &mut App) {
    use image::{DynamicImage, Rgba, RgbaImage};
    let gradient = RgbaImage::from_fn(256, 160, |x, y| {
        let v = ((x as f32 / 255.0) * 200.0 + (y as f32 / 160.0) * 55.0) as u8;
        Rgba([v, v, v, 255])
    });
    s.mode = app::Mode::Image;
    s.source = Some(render::Source::from_image("gradient.png", DynamicImage::ImageRgba8(gradient)));
}

fn scene_terminal(s: &mut App) {
    s.mode = app::Mode::Terminal;
    s.drawer_open = true;
    s.term.live = true;
    let now = Instant::now();
    for line in ["$ cargo build", "   Compiling tronic-printer v0.1.0", "warning: unused variable `x`", "    Finished `dev` profile in 3.2s"] {
        s.term.push(line, now);
    }
}

fn main() {
    // Only a real window has a frame to paint; headless snapshots skip it.
    if !std::env::args().any(|a| a.starts_with("--snap") || a.starts_with("--tree")) {
        chrome::dress_when_ready(TITLE);
    }
    let spec = AppSpec::<App> {
        scenes: &[("text", scene_text), ("online", scene_online), ("gallery", scene_gallery), ("drag", scene_drag), ("printing", scene_printing), ("ghost", scene_ghost), ("ghost-online", scene_ghost_online), ("look", scene_look), ("connecting", scene_connecting), ("done", scene_done), ("egg", scene_egg), ("error", scene_error), ("qr", scene_qr), ("image", scene_image), ("terminal", scene_terminal), ("theme", scene_theme), ("form", scene_form), ("butthurt", scene_butthurt), ("tweet", scene_tweet), ("salad", scene_salad), ("write", scene_write), ("write-receipt", scene_write_receipt), ("write-parchment", scene_write_parchment), ("write-diploma", scene_write_diploma), ("write-flight", scene_write_flight), ("write-wanted", scene_write_wanted), ("write-banner", scene_write_banner)],
        min_size: Some((1140.0, 640.0)),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new(TITLE, (1180.0, 940.0), App::default, ui::view)
    };
    // The compositor draws the frame, adding GPU effects at the view's effect slots.
    rune::App::new(spec).compositor(fx::Gfx::default()).run();
}
