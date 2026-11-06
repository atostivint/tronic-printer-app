//! Tronic Printer: print text, pictures and codes on the Lidl Tronic / Silvercrest 5890
//! "Mini Pocket Printer" from Windows, over Bluetooth LE. UI by Rune.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod ansi;
mod app;
mod ble;
mod chrome;
mod ipc;
mod protocol;
mod render;
mod style;
mod term;
mod ui;

use std::time::Instant;

use rune::{AppSpec, ImageCache, Tx, Waker};

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
    }
}

fn poll(s: &mut App, _: f64, tx: &mut Tx) -> bool {
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
    changed |= s.busy != was_busy || s.link != was_link;
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

fn scene_error(s: &mut App) {
    scene_text(s);
    s.error = Some("Printer not found. Switch it on, close the phone app, turn the phone's Bluetooth off and keep the printer close to the PC.".into());
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
    rune::run(AppSpec::<App> {
        scenes: &[("text", scene_text), ("online", scene_online), ("error", scene_error), ("qr", scene_qr), ("image", scene_image), ("terminal", scene_terminal)],
        min_size: Some((860.0, 640.0)),
        setup: Some(setup),
        poll: Some(poll),
        ..AppSpec::new(TITLE, (980.0, 820.0), App::default, ui::view)
    });
}
