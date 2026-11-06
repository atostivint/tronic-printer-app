//! Structured print jobs from outside the window (the MCP server, scripts): text, a picture, a QR
//! code or barcode, or a question about the printer. Each is answered with one line, `OK …` or
//! `ERR …`, which the caller shows to whoever asked.

use std::path::PathBuf;

use crate::app::{App, Link};
use crate::jobspec::Job;
use crate::protocol::{self, Bitmap};
use crate::render::{self, Align, CodeKind, CodeOpts, Dither, EcLevel, FontChoice, PictureOpts, TextOpts};

/// A single job longer than this needs `force`: it is almost always a mistake, and paper costs.
const MAX_CM: f32 = 100.0;
const MAX_COPIES: usize = 5;

fn font_of(name: &str) -> Option<FontChoice> {
    Some(match name.to_ascii_lowercase().as_str() {
        "sans" => FontChoice::Sans,
        "bold" => FontChoice::Bold,
        "mono" | "monospace" => FontChoice::Mono,
        "serif" => FontChoice::Serif,
        "script" | "handwriting" => FontChoice::Script,
        _ => return None,
    })
}

fn size_of(name: &str) -> Option<f32> {
    Some(match name.to_ascii_lowercase().as_str() {
        "s" | "small" => 22.0,
        "m" | "medium" => 32.0,
        "l" | "large" => 48.0,
        "xl" | "huge" => 72.0,
        _ => return None,
    })
}

fn align_of(name: &str) -> Option<Align> {
    Some(match name.to_ascii_lowercase().as_str() {
        "left" => Align::Left,
        "center" | "centre" => Align::Center,
        "right" => Align::Right,
        _ => return None,
    })
}

fn look_of(name: &str) -> Option<Dither> {
    Some(match name.to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
        "photo" => Dither::FloydSteinberg,
        "meme" => Dither::Atkinson,
        "lineart" | "line" => Dither::Threshold,
        "soft" => Dither::Ordered,
        _ => return None,
    })
}

fn code_kind_of(name: &str) -> Option<CodeKind> {
    Some(match name.to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
        "qr" => CodeKind::Qr,
        "code128" => CodeKind::Code128,
        "code39" => CodeKind::Code39,
        "ean13" => CodeKind::Ean13,
        _ => return None,
    })
}

/// Reads an optional named setting through `parse`, failing on a value that is there but unknown.
fn choice<T>(job: &Job, key: &str, parse: fn(&str) -> Option<T>) -> Result<Option<T>, String> {
    match job.get(key) {
        None => Ok(None),
        Some(v) => parse(v).map(Some).ok_or_else(|| format!("unknown {key} '{v}'")),
    }
}

fn bitmap_for(job: &Job) -> Result<Bitmap, String> {
    match job.kind.as_str() {
        "text" => {
            let mut o = TextOpts::default();
            if let Some(f) = choice(job, "font", font_of)? {
                o.font = f;
            }
            if let Some(p) = choice(job, "size", size_of)? {
                o.px = p;
            }
            if let Some(a) = choice(job, "align", align_of)? {
                o.align = a;
            }
            o.invert = job.flag("invert");
            render::text(&crate::ansi::clean(&job.body), o)
        }
        "code" => {
            let mut o = CodeOpts::default();
            if let Some(k) = choice(job, "type", code_kind_of)? {
                o.kind = k;
            }
            if let Some(level) = job.get("error_correction") {
                o.ec = match level.to_ascii_uppercase().as_str() {
                    "L" => EcLevel::Low,
                    "M" => EcLevel::Medium,
                    "Q" => EcLevel::Quartile,
                    "H" => EcLevel::High,
                    other => return Err(format!("unknown error_correction '{other}' (use L, M, Q or H)")),
                };
            }
            o.caption = job.get("caption").is_none_or(|_| job.flag("caption"));
            render::code(job.body.trim(), o)
        }
        "image" => {
            let path = PathBuf::from(job.body.trim());
            let mut o = PictureOpts::default();
            if let Some(d) = choice(job, "look", look_of)? {
                o.dither = d;
            }
            o.brightness = job.number("brightness").unwrap_or(0.0).clamp(-1.0, 1.0);
            o.contrast = job.number("contrast").unwrap_or(0.0).clamp(-1.0, 1.0);
            o.invert = job.flag("invert");
            o.rotate = (job.number("rotate").unwrap_or(0.0) / 90.0).round().rem_euclid(4.0) as u8;
            o.flip_h = job.flag("flip");
            let source = render::load_picture(&path)?;
            Ok(render::picture(&source, o))
        }
        other => Err(format!("unknown job kind '{other}' (use text, code, image or status)")),
    }
}

impl App {
    /// One line about the printer, for `printer_status`.
    pub fn status_line(&self) -> String {
        let name = match &self.link {
            Link::Online(n) => n.as_str(),
            _ => "",
        };
        let mut line = format!("OK connected={}", matches!(self.link, Link::Online(_)));
        if !name.is_empty() {
            line.push_str(&format!(" name={name}"));
        }
        line.push_str(&format!(" busy={}", self.busy));
        if let Some(b) = self.battery {
            line.push_str(&format!(" battery={b}%"));
        }
        if let Some(m) = &self.info.model {
            line.push_str(&format!(" model={m}"));
        }
        if let Some(f) = &self.info.firmware {
            line.push_str(&format!(" firmware={f}"));
        }
        if let Some(r) = self.rssi {
            line.push_str(&format!(" signal={r}dBm"));
        }
        line
    }

    /// Run a job from outside. The printer must be connected and idle; a long job needs `force`.
    pub fn run_job(&mut self, job: &Job, force: bool) -> String {
        if job.kind == "status" {
            return self.status_line();
        }
        if !matches!(self.link, Link::Online(_)) {
            return "ERR the printer is not connected: open Tronic Printer and press Connect".into();
        }
        if self.busy {
            return "ERR the printer is busy with another job, try again in a few seconds".into();
        }
        let bitmap = match bitmap_for(job) {
            Ok(b) => b,
            Err(e) => return format!("ERR {e}"),
        };
        let cm = render::length_cm(&bitmap);
        if cm > MAX_CM && !force {
            return format!("ERR that would print {cm:.0} cm of paper; pass force=true if you really mean it");
        }
        let copies = (job.number("copies").unwrap_or(1.0).round() as usize).clamp(1, MAX_COPIES);
        if !self.send_bitmap(&bitmap, copies, protocol::TEAR_OFF_DOTS) {
            return "ERR cannot reach the printer".into();
        }
        self.status = format!("Printing for an assistant ({} {cm:.1} cm)", job.kind);
        format!("OK printing {copies} × {cm:.1} cm")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ble::Cmd;

    fn online() -> (App, tokio::sync::mpsc::UnboundedReceiver<Cmd>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (App { cmd: Some(tx), link: Link::Online("Mini Pocket Printer_BLE".into()), battery: Some(87), ..App::default() }, rx)
    }

    #[test]
    fn a_text_job_prints_with_its_settings_and_copies() {
        let (mut app, mut rx) = online();
        let job = Job::new("text").param("font", "mono").param("size", "large").param("align", "center").param("copies", 2).body("hello");
        let reply = app.run_job(&job, false);
        assert!(reply.starts_with("OK printing 2 ×"), "{reply}");
        let Ok(Cmd::Print(p)) = rx.try_recv() else { panic!("nothing was sent") };
        assert_eq!(p.iter().filter(|c| c.as_slice() == [0x10, 0xFF, 0xF1, 0x03]).count(), 2);
        assert!(app.busy);
    }

    #[test]
    fn codes_and_pictures_are_rendered_and_bad_input_is_explained() {
        let (mut app, mut rx) = online();
        assert!(app.run_job(&Job::new("code").param("type", "code128").param("caption", false).body("ABC123"), false).starts_with("OK"));
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        app.busy = false;
        assert!(app.run_job(&Job::new("code").param("type", "qr").body("https://example.com"), false).starts_with("OK"));
        app.busy = false;
        assert!(app.run_job(&Job::new("code").param("type", "ean13").body("12"), false).starts_with("ERR"));
        assert!(app.run_job(&Job::new("text").param("font", "comic-papyrus").body("x"), false).contains("unknown font"));
        assert!(app.run_job(&Job::new("text").body("   "), false).starts_with("ERR"));
        assert!(app.run_job(&Job::new("image").body(r"C:\no\such\file.png"), false).contains("cannot open"));
        assert!(app.run_job(&Job::new("hologram"), false).contains("unknown job kind"));
    }

    #[test]
    fn an_image_job_reads_the_file_and_applies_the_look() {
        let dir = std::env::temp_dir().join(format!("tronic-jobs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("p.png");
        image::RgbaImage::from_pixel(30, 20, image::Rgba([40, 40, 40, 255])).save(&file).unwrap();
        let (mut app, mut rx) = online();
        let job = Job::new("image").param("look", "line art").param("brightness", "0.1").param("rotate", 90).body(file.display().to_string());
        assert!(app.run_job(&job, false).starts_with("OK"));
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jobs_are_refused_when_offline_busy_or_enormous_and_status_always_answers() {
        let mut offline = App::default();
        assert!(offline.run_job(&Job::new("text").body("x"), false).contains("not connected"));
        assert_eq!(offline.run_job(&Job::new("status"), false), "OK connected=false busy=false");
        let (mut app, _rx) = online();
        let status = app.run_job(&Job::new("status"), false);
        assert!(status.contains("connected=true") && status.contains("battery=87%") && status.contains("name=Mini Pocket Printer_BLE"), "{status}");
        let long = Job::new("text").param("size", "xl").body("line\n".repeat(120));
        assert!(app.run_job(&long, false).contains("force=true"));
        app.busy = true;
        assert!(app.run_job(&Job::new("text").body("x"), false).contains("busy"));
        app.busy = false;
        assert!(app.run_job(&long, true).starts_with("OK"));
    }
}
