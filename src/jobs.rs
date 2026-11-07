//! Structured print jobs from outside the window (the MCP server, scripts): text, a picture, a QR
//! code or barcode, or a question about the printer. Each is answered with one line, `OK …` or
//! `ERR …`, which the caller shows to whoever asked.

use std::path::PathBuf;

use crate::app::{App, Link};
use crate::jobspec::{Job, JobKind};
use crate::protocol::{self, Bitmap};
use crate::render::{self, Align, CodeKind, CodeOpts, Dither, EcLevel, FontChoice, PictureOpts, TextOpts, Theme};

/// A single printed thing longer than this needs `force`: it is almost always a mistake, and paper
/// costs. The window, `tronic-print` and jobs from outside all stop here.
pub const MAX_CM: f32 = 100.0;
/// Nobody needs more copies than this: the window's + button and a job's `copies` stop here.
pub const MAX_COPIES: u32 = 5;

/// The copy count actually sent: at least one, at most `MAX_COPIES`. Every path that prints in
/// several copies goes through here, so the window and the job side cannot drift apart.
pub fn clamp_copies(copies: usize) -> usize {
    copies.clamp(1, MAX_COPIES as usize)
}

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
    match job.kind {
        JobKind::Text => {
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
            let theme = match job.get("theme") {
                None => Theme::Plain,
                Some(name) => Theme::from_id(name).ok_or_else(|| format!("unknown theme '{name}' (use {})", Theme::all().iter().map(|t| t.id()).collect::<Vec<_>>().join(", ")))?,
            };
            // Every other setting is a field for the theme; the ones it does not use are ignored,
            // and the app's own settings (kind among them) are not fields at all.
            let fields: render::Fields = job.fields();
            render::themed(theme, &crate::ansi::clean(&job.body), &fields, o)
        }
        JobKind::Code => {
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
        JobKind::Image => {
            let path = PathBuf::from(job.body.trim());
            let mut o = PictureOpts::default();
            if let Some(d) = choice(job, "look", look_of)? {
                o.dither = d;
            }
            o.brightness = finite(job, "brightness")?.clamp(-1.0, 1.0);
            o.contrast = finite(job, "contrast")?.clamp(-1.0, 1.0);
            o.invert = job.flag("invert");
            o.rotate = (job.number("rotate").unwrap_or(0.0) / 90.0).round().rem_euclid(4.0) as u8;
            o.flip_h = job.flag("flip");
            o.space_before = space_before_mm(job)?;
            let source = render::load_picture(&path)?;
            Ok(render::picture(&source, o))
        }
        // `run_job` answers a status job before asking for a picture; reaching here would be a bug.
        JobKind::Status => Err("a status job asks about the printer, it prints nothing".into()),
    }
}

/// A number option that must be finite when it is given. `NaN` and `inf` are refused rather than
/// clamped, because they would poison the dither or the rotation.
fn finite(job: &Job, key: &str) -> Result<f32, String> {
    match job.number(key) {
        Some(v) if v.is_finite() => Ok(v),
        Some(_) => Err(format!("{key} must be a finite number, not '{}'", job.get(key).unwrap_or("?"))),
        None => Ok(0.0),
    }
}

/// `space_before_mm` of an image job: whole millimetres of blank paper above it, 0 to 50 (more is
/// cut to 50, less than 0 to 0); something that is not a number is an error.
fn space_before_mm(job: &Job) -> Result<u8, String> {
    let Some(raw) = job.get("space_before_mm") else { return Ok(0) };
    match raw.trim().parse::<f32>() {
        Ok(n) if n.is_finite() => Ok(n.clamp(0.0, f32::from(render::MAX_SPACE_MM)).round() as u8),
        _ => Err(format!("space_before_mm must be a number of millimetres from 0 to {}, not '{}'", render::MAX_SPACE_MM, raw.trim())),
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
        line.push_str(&format!(" busy={}", self.printing.is_busy()));
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
        if job.kind == JobKind::Status {
            return self.status_line();
        }
        if !matches!(self.link, Link::Online(_)) {
            return "ERR the printer is not connected: open Tronic Printer and press Connect".into();
        }
        if self.printing.is_busy() {
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
        let copies = match job.number("copies") {
            Some(v) if v.is_finite() => clamp_copies(v.round() as usize),
            Some(_) => return "ERR copies must be a finite number".into(),
            None => 1,
        };
        if !self.send_bitmap(&bitmap, copies, protocol::TEAR_OFF_DOTS) {
            return "ERR cannot reach the printer".into();
        }
        self.status = format!("Printing for an assistant ({} {cm:.1} cm)", job.kind.id());
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
        assert!(app.printing.is_busy());
    }

    #[test]
    fn a_themed_text_job_dresses_the_text_and_passes_its_fields() {
        let (mut app, mut rx) = online();
        let plain = bitmap_for(&Job::new("text").body("Tea 1.00")).unwrap();
        let themed = bitmap_for(&Job::new("text").param("theme", "receipt").param("store", "THE BIG SHOP").body("Tea 1.00")).unwrap();
        let default_store = bitmap_for(&Job::new("text").param("theme", "Receipt").body("Tea 1.00")).unwrap();
        assert!(themed.height > plain.height * 4, "a receipt is much more than the text");
        assert_ne!(themed, default_store, "the store field is used");
        let job = Job::new("text").param("theme", "boarding").param("from", "LYS").param("to", "BCN").body("");
        assert!(app.run_job(&job, false).starts_with("OK printing"), "a boarding pass needs no text");
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        app.printing = crate::app::Printing::Idle;
        assert!(app.run_job(&Job::new("text").param("theme", "hologram").body("x"), false).contains("unknown theme 'hologram'"));
        assert!(app.run_job(&Job::new("text").param("theme", "receipt").body("  "), false).starts_with("ERR"));
    }

    #[test]
    fn codes_and_pictures_are_rendered_and_bad_input_is_explained() {
        let (mut app, mut rx) = online();
        assert!(app.run_job(&Job::new("code").param("type", "code128").param("caption", false).body("ABC123"), false).starts_with("OK"));
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        app.printing = crate::app::Printing::Idle;
        assert!(app.run_job(&Job::new("code").param("type", "qr").body("https://example.com"), false).starts_with("OK"));
        app.printing = crate::app::Printing::Idle;
        assert!(app.run_job(&Job::new("code").param("type", "ean13").body("12"), false).starts_with("ERR"));
        assert!(app.run_job(&Job::new("text").param("font", "comic-papyrus").body("x"), false).contains("unknown font"));
        assert!(app.run_job(&Job::new("text").body("   "), false).starts_with("ERR"));
        assert!(app.run_job(&Job::new("image").body(r"C:\no\such\file.png"), false).contains("cannot open"));
        assert!(Job::parse("kind=hologram").unwrap_err().contains("unknown job kind 'hologram'"));
    }

    #[test]
    fn a_theme_field_cannot_rewrite_the_job_type() {
        let (mut app, mut rx) = online();
        // What the MCP tool hands over, then what the named pipe carries: a field called `kind`
        // must not turn a text print into another job.
        let (job, _) = crate::mcp::job_for("print_text", &serde_json::json!({ "text": "hello", "fields": { "kind": "hologram" } })).unwrap();
        assert_eq!(job.kind, JobKind::Text);
        assert_eq!(job.get("kind"), None, "the field map is not a settings back door");
        let back = Job::parse(&job.encode()).unwrap();
        assert_eq!(back.kind, JobKind::Text, "the wire keeps the type the tool chose");
        let reply = app.run_job(&back, false);
        assert!(reply.starts_with("OK printing"), "{reply}");
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        // And a kind hidden in the settings on the pipe is ignored too.
        assert_eq!(Job::parse("kind=text\nkind=hologram\n\nhi").unwrap().kind, JobKind::Text);
    }

    #[test]
    fn copy_counts_are_clamped_the_same_way_everywhere() {
        assert_eq!(clamp_copies(0), 1);
        assert_eq!(clamp_copies(1), 1);
        assert_eq!(clamp_copies(MAX_COPIES as usize), MAX_COPIES as usize);
        assert_eq!(clamp_copies(9), MAX_COPIES as usize, "the window's old 9 stops at the job limit");
        let (mut app, mut rx) = online();
        let job = Job::new("text").param("copies", 99).body("x");
        assert!(app.run_job(&job, false).starts_with("OK printing 5 ×"));
        let Ok(Cmd::Print(p)) = rx.try_recv() else { panic!("nothing was sent") };
        assert_eq!(p.iter().filter(|c| c.as_slice() == [0x10, 0xFF, 0xF1, 0x03]).count(), MAX_COPIES as usize);
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
    fn the_space_before_an_image_is_read_clamped_and_refused_when_it_is_not_a_number() {
        let read = |v: &str| space_before_mm(&Job::new("image").param("space_before_mm", v));
        assert_eq!(space_before_mm(&Job::new("image")), Ok(0), "left out means none");
        assert_eq!(read("12"), Ok(12));
        assert_eq!(read(" 7.4 "), Ok(7));
        assert_eq!(read("999"), Ok(50), "clamped to 50");
        assert_eq!(read("-3"), Ok(0), "clamped to 0");
        for bad in ["lots", "NaN", "inf"] {
            assert!(read(bad).unwrap_err().contains("space_before_mm"), "{bad}");
        }
        let dir = std::env::temp_dir().join(format!("tronic-space-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("p.png");
        image::RgbaImage::from_pixel(30, 20, image::Rgba([40, 40, 40, 255])).save(&file).unwrap();
        let (mut app, mut rx) = online();
        let height = |app: &mut App, rx: &mut tokio::sync::mpsc::UnboundedReceiver<Cmd>, extra: Option<&str>| {
            let mut job = Job::new("image").body(file.display().to_string());
            if let Some(v) = extra {
                job = job.param("space_before_mm", v);
            }
            assert!(app.run_job(&job, false).starts_with("OK"), "{extra:?}");
            let Ok(Cmd::Print(packets)) = rx.try_recv() else { panic!("a print") };
            packets.iter().map(Vec::len).sum::<usize>()
        };
        let plain = height(&mut app, &mut rx, None);
        app.printing = crate::app::Printing::Idle;
        let spaced = height(&mut app, &mut rx, Some("10"));
        assert!(spaced > plain, "the white space is printed");
        app.printing = crate::app::Printing::Idle;
        assert!(app.run_job(&Job::new("image").param("space_before_mm", "lots").body(file.display().to_string()), false).starts_with("ERR"));
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
        app.printing = crate::app::Printing::Sending;
        assert!(app.run_job(&Job::new("text").body("x"), false).contains("busy"));
        app.printing = crate::app::Printing::Idle;
        assert!(app.run_job(&long, true).starts_with("OK"));
    }
}
