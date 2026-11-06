//! Tronic Printer: print text, pictures and codes on the Lidl Tronic / Silvercrest 5890
//! "Mini Pocket Printer" from Windows, over Bluetooth LE. UI by Rune.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod ansi;
mod app;
mod ble;
mod chrome;
mod egg;
mod fx;
mod gallery;
mod ipc;
mod protocol;
mod render;
mod settings;
mod style;
mod term;
mod ui;
mod voice;

use std::time::Instant;

use rune::{AppSpec, ImageCache, KeyEvent, Tx, Waker};

use crate::app::App;

const TITLE: &str = "Tronic Printer";

fn setup(s: &mut App, waker: Waker) {
    let images = ImageCache::new(waker.clone());
    s.logo = Some(images.load(&include_bytes!("../assets/logo-96.png")[..]));
    s.images = Some(images);
    s.ipc = Some(ipc::spawn(waker.clone()));
    let (tx, rx) = ble::spawn(waker);
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
    }
}

/// The Konami code, typed anywhere a text field does not have the keys.
fn on_key(s: &mut App, e: &KeyEvent, _: &mut Tx) -> bool {
    match egg::pad_of(&e.key) {
        Some(pad) if s.konami.feed(pad) => {
            s.egg_fire();
            true
        }
        _ => false,
    }
}

fn poll(s: &mut App, clock: f64, tx: &mut Tx) -> bool {
    s.clock = clock;
    let now = Instant::now();
    let events: Vec<ble::Event> = s.events.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
    let messages: Vec<ipc::IpcMsg> = s.ipc.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
    let mut changed = !events.is_empty() || !messages.is_empty();
    for e in events {
        s.apply(e);
    }
    for m in messages {
        s.handle_ipc(m, now);
    }
    let (was_busy, was_link) = (s.busy, s.link.clone());
    if let Some(again) = s.tick_connection(now) {
        tx.wake_after(again);
    }
    if let Some(again) = s.tick_terminal(now) {
        tx.wake_after(again);
    }
    if let Some(again) = s.tick_gallery(now) {
        tx.wake_after(again);
    }
    changed |= s.busy != was_busy || s.link != was_link;
    // Flames fading, the ghost bobbing, an error shaking: the view moves with the clock for now.
    if let Some(every) = s.tick_interval() {
        changed = true;
        tx.wake_after(every);
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
    s.text = rune::TextEdit::new("Hello from Windows!\nGroceries:\n- bread\n- coffee\n- more coffee");
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
    s.busy = true;
    s.progress = 0.45;
    s.status = voice::printing(0.45).into();
}

fn scene_ghost(s: &mut App) {
    s.fx_on = true;
}

fn scene_connecting(s: &mut App) {
    s.fx_on = true;
    s.link = app::Link::Connecting;
    s.status = voice::connecting(1).into();
}

fn scene_done(s: &mut App) {
    scene_online(s);
    s.fx_on = true;
    s.done_at = Some(0.0);
    s.press_at = Some(0.0);
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
    s.gallery.set_dir(dir.clone());
    s.gallery.auto = true;
    s.gallery.selected = Some(dir.join("meme-03.png"));
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
    s.term.live = true;
    let now = Instant::now();
    for line in ["$ cargo build", "   Compiling tronic-printer v0.1.0", "warning: unused variable `x`", "    Finished `dev` profile in 3.2s"] {
        s.term.push(line, now);
    }
}

fn main() {
    // Only a real window has a frame to paint; headless snapshots skip it.
    if !std::env::args().any(|a| a.starts_with("--snap") || a.starts_with("--tree")) {
        chrome::dress_when_ready(chrome::Look { title: TITLE, caption: 0x1a1228, text: 0xf4effa, border: 0x3a2d52 });
    }
    let spec = AppSpec::<App> {
        scenes: &[("text", scene_text), ("online", scene_online), ("gallery", scene_gallery), ("printing", scene_printing), ("ghost", scene_ghost), ("connecting", scene_connecting), ("done", scene_done), ("egg", scene_egg), ("error", scene_error), ("qr", scene_qr), ("image", scene_image), ("terminal", scene_terminal)],
        min_size: Some((860.0, 640.0)),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new(TITLE, (980.0, 820.0), App::default, ui::view)
    };
    // The compositor draws the frame, adding GPU effects at the view's effect slots.
    rune::App::new(spec).compositor(fx::Gfx::default()).run();
}
