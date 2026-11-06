//! Application state and what happens to it.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use rune::{Image, ImageCache, TextEdit};
use tokio::sync::mpsc::UnboundedSender;

use crate::ble::{Cmd, DeviceInfo, Event};
use crate::egg::Konami;
use crate::gallery::{self, Gallery};
use crate::ipc::IpcMsg;
use crate::protocol::{self, Bitmap, Density};
use crate::render::{self, CodeOpts, FontChoice, PictureOpts, Source, TextOpts, Theme};
use crate::settings;
use crate::term::{self, Term};
use crate::voice;

/// How far (in points) a press on a gallery thumbnail must travel to count as a drag, not a click.
pub const DRAG_MIN: f32 = 6.0;

/// A gallery picture being dragged toward the printer.
pub struct ThumbDrag {
    pub path: PathBuf,
    /// How far the pointer has moved since the press.
    pub travel: f32,
}

/// The preview image can't be taller than this many dots (Rune's limit is 8192 pixels).
const MAX_PREVIEW_ROWS: usize = 8000;
/// A single command-line print longer than this needs `--force`: it is almost always a mistake.
const MAX_ONCE_CM: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Text,
    Image,
    Codes,
    Terminal,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Text, Mode::Image, Mode::Codes, Mode::Terminal];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Text => "Text",
            Mode::Image => "Image",
            Mode::Codes => "Codes",
            Mode::Terminal => "Terminal",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Offline,
    Connecting,
    Online(String),
}

pub struct App {
    // Infrastructure.
    pub cmd: Option<UnboundedSender<Cmd>>,
    pub events: Option<Receiver<Event>>,
    pub images: Option<ImageCache>,
    pub ipc: Option<Receiver<IpcMsg>>,
    /// The app's mark, shown in the header.
    pub logo: Option<Image>,
    /// The title last given to the window, so it is only set when it changes.
    pub title_shown: String,
    // Effects and fun.
    /// GPU effects on or off (the in-app switch).
    pub fx_on: bool,
    /// Rune's clock, in seconds, as of the last frame.
    pub clock: f64,
    /// When the Print button was last pressed (the shockwave starts there).
    pub press_at: Option<f64>,
    pub done_at: Option<f64>,
    pub error_at: Option<f64>,
    /// Konami flames burn until this time.
    pub fire_until: Option<f64>,
    /// The ghost says boo until this time.
    pub boo_until: Option<f64>,
    pub konami: Konami,
    pub logo_taps: u8,
    pub seed: u64,
    // Printer.
    pub link: Link,
    pub battery: Option<u8>,
    /// Model, firmware, battery and paper status as last reported by the printer.
    pub info: DeviceInfo,
    /// Signal strength of the connected printer, in dBm.
    pub rssi: Option<i16>,
    /// Look for the closest printer at start-up and again whenever the link drops.
    pub auto_connect: bool,
    /// The user pressed Disconnect: stay off until they press Connect.
    pub user_disconnected: bool,
    pub last_attempt: Option<Instant>,
    pub status: String,
    pub error: Option<String>,
    pub busy: bool,
    pub progress: f32,
    // What to print.
    pub mode: Mode,
    pub density: usize,
    pub copies: usize,
    pub text: TextEdit,
    pub text_opts: TextOpts,
    /// How the text is dressed, and the little form that theme asks for.
    pub theme: Theme,
    pub theme_edits: Vec<TextEdit>,
    pub source: Option<Source>,
    /// The scaled picture for (picture id, rotation, flip), kept between slider moves.
    pub prepared: Option<(u64, u8, bool, render::Prepared)>,
    pub pic_opts: PictureOpts,
    pub code_text: TextEdit,
    pub code_opts: CodeOpts,
    pub term: Term,
    pub gallery: Gallery,
    /// A gallery picture is being dragged; the compositor feeds in where the pointer is.
    pub drag: Option<ThumbDrag>,
    pub pointer: Option<(f32, f32)>,
    /// While dragging: the pointer is over the preview, where letting go prints.
    pub over_stage: bool,
    // The composed result, shown as the preview and sent to the printer.
    pub dirty: bool,
    pub composed: Result<Bitmap, String>,
    pub preview: Option<Image>,
    /// The preview shows only the end of a very long ticket.
    pub preview_cropped: bool,
}

impl Default for App {
    fn default() -> Self {
        App {
            cmd: None,
            events: None,
            images: None,
            ipc: None,
            logo: None,
            title_shown: String::new(),
            fx_on: false,
            clock: 0.0,
            press_at: None,
            done_at: None,
            error_at: None,
            fire_until: None,
            boo_until: None,
            konami: Konami::default(),
            logo_taps: 0,
            seed: 0x5EED,
            link: Link::Offline,
            battery: None,
            info: DeviceInfo::default(),
            rssi: None,
            auto_connect: true,
            user_disconnected: false,
            last_attempt: None,
            status: "Not connected".into(),
            error: None,
            busy: false,
            progress: 0.0,
            mode: Mode::Text,
            density: 1,
            copies: 1,
            text: TextEdit::new(""),
            text_opts: TextOpts::default(),
            theme: Theme::Plain,
            theme_edits: (0..render::MAX_FIELDS).map(|_| TextEdit::new("")).collect(),
            source: None,
            prepared: None,
            pic_opts: PictureOpts::default(),
            code_text: TextEdit::new(""),
            code_opts: CodeOpts::default(),
            term: Term::default(),
            gallery: Gallery::default(),
            drag: None,
            pointer: None,
            over_stage: false,
            dirty: true,
            composed: Err("nothing to print".into()),
            preview: None,
            preview_cropped: false,
        }
    }
}

impl App {
    /// Ask for the preview to be rebuilt before the next frame.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// The window title doubles as a status line, so the taskbar says what the printer is doing.
    pub fn window_title(&self) -> String {
        let state = if self.busy {
            format!("Printing {:.0}%", self.progress * 100.0)
        } else {
            match &self.link {
                Link::Offline => return "Tronic Printer".into(),
                Link::Connecting => "Connecting…".into(),
                Link::Online(_) => match self.battery {
                    Some(b) => format!("Connected · {b}%"),
                    None => "Connected".into(),
                },
            }
        };
        format!("Tronic Printer · {state}")
    }

    pub fn compose(&self) -> Result<Bitmap, String> {
        match self.mode {
            Mode::Text => render::themed(self.theme, &self.text.display().0, &self.theme_fields(), self.text_opts),
            Mode::Image => match &self.source {
                Some(src) => Ok(render::picture(src, self.pic_opts)),
                None => Err("Choose a picture.".into()),
            },
            Mode::Codes => render::code(&self.code_text.display().0, self.code_opts),
            Mode::Terminal => render::text(&self.term.text(), self.terminal_opts()),
        }
    }

    /// What the theme's little form holds, by field key.
    pub fn theme_fields(&self) -> render::Fields {
        self.theme.fields().iter().zip(&self.theme_edits).map(|(f, e)| (f.key.to_string(), e.display().0.to_string())).collect()
    }

    /// Pick a theme. An empty box, or one still holding another theme's example, gets this theme's
    /// example so the preview shows it at once; text you typed is kept.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        for edit in &mut self.theme_edits {
            *edit = TextEdit::new("");
        }
        // Text that is empty, or still some theme's example, was never written by the user: the
        // new theme gets its own example instead.
        let current = self.text.display().0.to_string();
        if current.trim().is_empty() || Theme::ALL.iter().any(|t| t.sample() == current) {
            self.text = TextEdit::new(if theme == Theme::Plain { "" } else { theme.sample() });
        }
        self.touch();
    }

    /// Monospace, breaking anywhere so columns stay put.
    pub fn terminal_opts(&self) -> TextOpts {
        TextOpts { px: term::SIZES[self.term.size.min(2)], font: FontChoice::Mono, break_anywhere: true, margin_y: 2.0, ..TextOpts::default() }
    }

    /// The picture, with its slow rescale done once and kept: only a different picture, rotation
    /// or flip redoes it, so brightness and contrast follow a slider without stutter.
    fn compose_image(&mut self) -> Result<Bitmap, String> {
        let Some(src) = &self.source else { return Err("Choose a picture.".into()) };
        let (id, rotate, flip) = (src.id, self.pic_opts.rotate, self.pic_opts.flip_h);
        let fresh = matches!(&self.prepared, Some((i, r, f, _)) if (*i, *r, *f) == (id, rotate, flip));
        if !fresh {
            self.prepared = Some((id, rotate, flip, render::prepare(src, rotate, flip)));
        }
        let Some((_, _, _, prepared)) = &self.prepared else { return Err("Choose a picture.".into()) };
        Ok(render::tone(prepared, self.pic_opts))
    }

    /// Rebuild the composed bitmap and its preview picture.
    pub fn refresh(&mut self) {
        self.dirty = false;
        self.composed = if self.mode == Mode::Image { self.compose_image() } else { self.compose() };
        self.preview_cropped = false;
        self.preview = match (&self.composed, &self.images) {
            (Ok(bitmap), Some(cache)) => {
                let tail;
                let shown = if bitmap.height > MAX_PREVIEW_ROWS {
                    self.preview_cropped = true;
                    tail = render::tail(bitmap, MAX_PREVIEW_ROWS);
                    &tail
                } else {
                    bitmap
                };
                let (w, h, rgba) = render::preview_rgba(shown);
                let image = cache.rgba(w, h, rgba);
                if rune::is_headless() {
                    // A snapshot draws a single frame, so don't leave the picture pending. A real
                    // window never waits: that made dragging a slider stutter.
                    let _ = image.wait();
                }
                Some(image)
            }
            _ => None,
        };
    }

    pub fn can_print(&self) -> bool {
        matches!(self.link, Link::Online(_)) && !self.busy && self.composed.is_ok()
    }

    pub fn send(&mut self, cmd: Cmd) -> bool {
        match &self.cmd {
            Some(tx) if tx.send(cmd).is_ok() => true,
            _ => {
                self.error = Some("The Bluetooth worker is not running.".into());
                false
            }
        }
    }

    pub fn connect(&mut self) {
        self.error = None;
        self.user_disconnected = false;
        self.last_attempt = Some(Instant::now());
        self.link = Link::Connecting;
        self.status = voice::connecting(self.next_seed()).into();
        self.send(Cmd::Connect);
    }

    pub fn disconnect(&mut self) {
        self.user_disconnected = true;
        self.send(Cmd::Disconnect);
    }

    /// Ask the printer again for its battery and status.
    pub fn refresh_info(&mut self) {
        self.send(Cmd::Refresh);
    }

    /// Called every frame: connect by itself when asked to, and try again after a miss. Returns
    /// when to look again.
    pub fn tick_connection(&mut self, now: Instant) -> Option<std::time::Duration> {
        const RETRY: std::time::Duration = std::time::Duration::from_secs(12);
        if !self.auto_connect || self.user_disconnected || self.link != Link::Offline || self.cmd.is_none() {
            return None;
        }
        match self.last_attempt {
            Some(t) if now.duration_since(t) < RETRY => Some(RETRY - now.duration_since(t)),
            _ => {
                self.connect();
                // The scan takes a while; look again once it has had time to end.
                Some(RETRY)
            }
        }
    }

    pub fn density_level(&self) -> Density {
        [Density::Light, Density::Medium, Density::Dark][self.density.min(2)]
    }

    pub fn print(&mut self) {
        self.error = None;
        self.press_at = Some(self.clock);
        let bitmap = match &self.composed {
            Ok(b) => b.clone(),
            Err(e) => {
                self.error = Some(e.clone());
                return;
            }
        };
        if self.mode == Mode::Terminal {
            // Printing from the Terminal tab means "what has not been printed yet".
            if self.term.unprinted() == 0 {
                self.error = Some("Everything in the buffer is already printed.".into());
                return;
            }
            self.flush_terminal(protocol::TEAR_OFF_DOTS);
            return;
        }
        self.send_bitmap(&bitmap, self.copies.max(1), protocol::TEAR_OFF_DOTS);
    }

    /// `feed` is the paper pushed out afterwards; live batches pass 0 so they join up.
    pub(crate) fn send_bitmap(&mut self, bitmap: &Bitmap, copies: usize, feed: u8) -> bool {
        let mut job = Vec::new();
        for _ in 0..copies {
            job.extend(protocol::job_with_feed(bitmap, self.density_level(), feed));
        }
        self.send_job(job)
    }

    fn send_job(&mut self, job: Vec<Vec<u8>>) -> bool {
        self.busy = true;
        self.progress = 0.0;
        self.status = "Printing…".into();
        let sent = self.send(Cmd::Print(job));
        if !sent {
            self.busy = false;
        }
        sent
    }

    /// Print the terminal lines that have not been printed yet.
    pub fn flush_terminal(&mut self, feed: u8) {
        let text = self.term.pending_text();
        match render::text(&text, self.terminal_opts()) {
            Ok(bitmap) => {
                if self.send_bitmap(&bitmap, 1, feed) {
                    self.term.mark_printed();
                    self.term.owes_feed = feed == 0;
                }
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Something arrived on the named pipe.
    pub fn handle_ipc(&mut self, msg: IpcMsg, now: Instant) {
        match msg {
            IpcMsg::Once { text, force, reply } => {
                let _ = reply.send(self.print_once(&text, force));
            }
            IpcMsg::Job { job, force, reply } => {
                let _ = reply.send(self.run_job(&job, force));
            }
            IpcMsg::Line(line) => {
                self.term.push(&line, now);
                self.touch();
            }
            IpcMsg::Live(on) => {
                self.term.live = on;
                self.status = if on { "Terminal connected".into() } else { "Terminal closed".into() };
            }
        }
    }

    /// One-shot printing for `cmd | tronic-print`. The answer goes back to the command line.
    fn print_once(&mut self, text: &str, force: bool) -> String {
        if !matches!(self.link, Link::Online(_)) {
            return "ERR the printer is not connected: press Connect in Tronic Printer".into();
        }
        if self.busy {
            return "ERR the printer is busy".into();
        }
        let bitmap = match render::text(&crate::ansi::clean(text), self.terminal_opts()) {
            Ok(b) => b,
            Err(e) => return format!("ERR {e}"),
        };
        let cm = render::length_cm(&bitmap);
        if cm > MAX_ONCE_CM && !force {
            return format!("ERR that is {cm:.0} cm of paper; add --force if you mean it");
        }
        if self.send_bitmap(&bitmap, 1, protocol::TEAR_OFF_DOTS) { format!("OK {cm:.1} cm") } else { "ERR cannot reach the printer".into() }
    }

    /// Called every frame: print live terminal output once it is due. Returns when to look again.
    pub fn tick_terminal(&mut self, now: Instant) -> Option<std::time::Duration> {
        if !matches!(self.link, Link::Online(_)) {
            return None;
        }
        let soon = Some(term::IDLE / 3);
        if self.term.unprinted() == 0 {
            // The stream ended and its last batch left no gap: push the paper out once.
            if self.term.owes_feed && !self.term.live {
                if self.busy {
                    return soon;
                }
                self.term.owes_feed = false;
                self.send_job(protocol::feed_only(protocol::TEAR_OFF_DOTS));
            }
            return None;
        }
        if !self.term.auto {
            return None;
        }
        if self.busy {
            return soon;
        }
        if self.term.due(now) {
            // While a stream runs the batches join up; a lone buffer gets its tear-off space.
            self.flush_terminal(if self.term.live { 0 } else { protocol::TEAR_OFF_DOTS });
            return soon;
        }
        soon
    }

    pub fn choose_picture(&mut self, path: PathBuf) {
        match render::load_picture(&path) {
            Ok(src) => {
                self.source = Some(src);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
        self.touch();
    }

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Status(s) => self.status = s,
            Event::Connected(name, rssi) => {
                self.status = match rssi {
                    Some(r) => format!("Connected to {name} ({r} dBm)"),
                    None => format!("Connected to {name}"),
                };
                self.rssi = rssi;
                self.link = Link::Online(name);
            }
            Event::Disconnected => {
                self.link = Link::Offline;
                self.battery = None;
                self.info = DeviceInfo::default();
                self.rssi = None;
                self.busy = false;
                self.status = voice::offline().into();
                // Start the retry clock now, so a lost link is picked up again soon.
                self.last_attempt = Some(Instant::now());
            }
            Event::Battery(b) => self.battery = Some(b),
            Event::Info(info) => {
                if info.battery.is_some() {
                    self.battery = info.battery;
                }
                self.info = info;
            }
            Event::Progress(p) => {
                self.progress = p;
                self.status = voice::printing(p).into();
            }
            Event::PrintDone => {
                self.busy = false;
                self.done_at = Some(self.clock);
                self.status = voice::done(self.next_seed()).into();
            }
            Event::Error(e) => {
                self.busy = false;
                if self.link == Link::Connecting {
                    self.link = Link::Offline;
                    self.last_attempt = Some(Instant::now());
                }
                self.error_at = Some(self.clock);
                self.error = Some(e);
            }
        }
    }

    // ---- effects and fun

    /// A fresh number for picking a line of microcopy.
    pub fn next_seed(&mut self) -> u64 {
        self.seed = voice::mix(self.seed, self.clock.to_bits());
        self.seed
    }

    pub fn set_effects(&mut self, on: bool) {
        self.fx_on = on;
        settings::update(|s| s.effects = Some(on));
    }

    // ---- the picture gallery

    pub fn set_gallery_dir(&mut self, dir: PathBuf) {
        settings::update(|s| s.gallery = Some(dir.clone()));
        self.gallery.set_dir(dir);
        self.error = None;
        self.touch();
    }

    pub fn set_auto_print(&mut self, on: bool) {
        self.gallery.auto = on;
        self.gallery.auto_printed = 0;
        if !on {
            self.gallery.queue.clear();
        }
        settings::update(|s| s.auto_print = on);
    }

    /// A thumbnail was clicked: show that picture, ready to print.
    pub fn pick_from_gallery(&mut self, path: PathBuf) {
        self.mode = Mode::Image;
        self.gallery.selected = Some(path.clone());
        self.choose_picture(path);
    }

    /// Pictures dropped on the window print at once when the printer is connected (one after the
    /// other when there are several); without a printer they are just shown.
    pub fn drop_files(&mut self, files: &[PathBuf]) {
        let pictures: Vec<PathBuf> = files.iter().filter(|f| gallery::is_image(f)).cloned().collect();
        if pictures.is_empty() {
            self.error = Some("That is not a picture I can print (png, jpg, bmp, gif or webp).".into());
            return;
        }
        self.print_pictures(pictures);
    }

    /// Print these pictures now (the printer must be connected), or just show the first.
    fn print_pictures(&mut self, pictures: Vec<PathBuf>) {
        let Some(first) = pictures.first().cloned() else { return };
        self.mode = Mode::Image;
        if matches!(self.link, Link::Online(_)) {
            self.gallery.queue.extend(pictures);
            self.drain_gallery_queue();
        } else {
            self.pick_from_gallery(first);
            self.status = "Connect the printer, then press Print.".into();
        }
    }

    // ---- dragging a gallery picture onto the printer

    /// A press on a thumbnail starts a gesture: a short one is a click, a long one a drag.
    pub fn drag_start(&mut self, path: PathBuf) {
        self.drag = Some(ThumbDrag { path, travel: 0.0 });
    }

    pub fn drag_move(&mut self, dx: f32, dy: f32) {
        if let Some(d) = &mut self.drag {
            d.travel = dx.hypot(dy);
        }
    }

    /// The thumbnail was let go. A click picks the picture; a drag released over the preview
    /// (where the printer is) prints it; anywhere else, or cancelled, it does nothing.
    pub fn drag_end(&mut self, cancelled: bool) {
        let Some(d) = self.drag.take() else { return };
        let over = std::mem::take(&mut self.over_stage);
        self.pointer = None;
        if cancelled {
            return;
        }
        if d.travel < DRAG_MIN {
            self.pick_from_gallery(d.path);
        } else if over {
            self.print_pictures(vec![d.path]);
        }
    }

    fn make_thumbs(&mut self) {
        let Some(cache) = &self.images else { return };
        for e in self.gallery.entries.iter().take(gallery::MAX_THUMBS) {
            let image = self.gallery.thumbs.entry(e.path.clone()).or_insert_with(|| cache.file(&e.path, 82.0, 2.0));
            if rune::is_headless() {
                // A snapshot draws one frame: let the thumbnails finish decoding first.
                let _ = image.wait();
            }
        }
    }

    /// Print the next waiting picture when the printer is free.
    fn drain_gallery_queue(&mut self) {
        if !matches!(self.link, Link::Online(_)) || self.busy {
            return;
        }
        let Some(path) = self.gallery.queue.pop_front() else { return };
        match render::load_picture(&path) {
            Ok(src) => {
                let bitmap = render::picture(&src, self.pic_opts);
                self.status = format!("Printing {}", src.name);
                self.source = Some(src);
                self.gallery.selected = Some(path);
                self.gallery.auto_printed += 1;
                self.mode = Mode::Image;
                self.error = None;
                self.touch();
                self.send_bitmap(&bitmap, self.copies.max(1), protocol::TEAR_OFF_DOTS);
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Called every frame: look at the gallery folder, queue what arrived, print what is waiting.
    /// Returns when to look again.
    pub fn tick_gallery(&mut self, now: Instant) -> Option<std::time::Duration> {
        if self.gallery.dir.is_none() && self.gallery.queue.is_empty() {
            return None;
        }
        let (changed, arrived) = self.gallery.rescan(now);
        if changed {
            self.make_thumbs();
            self.touch();
        }
        if self.gallery.auto && !arrived.is_empty() {
            if let Some(name) = arrived.last().and_then(|p| p.file_name()) {
                self.status = format!("New picture: {}", name.to_string_lossy());
            }
            self.gallery.queue.extend(arrived);
        }
        self.drain_gallery_queue();
        Some(gallery::SCAN_EVERY)
    }

    /// How hot the flames at the paper slot are, 0 to 1: rising with the job, dying down after it,
    /// and full blast for the Konami code.
    pub fn flame(&self) -> f32 {
        if !self.fx_on {
            return 0.0;
        }
        let dying = |since: f64, dur: f64, peak: f32| ((1.0 - (self.clock - since) / dur).clamp(0.0, 1.0) as f32) * peak;
        let mut hot = 0.0f32;
        if self.busy {
            hot = 0.3 + 0.7 * self.progress;
        } else if let Some(d) = self.done_at {
            hot = dying(d, 0.9, 0.6);
        }
        if let Some(until) = self.fire_until {
            hot = hot.max(dying(until - 0.6, 0.6, 1.0).max(if self.clock < until - 0.6 { 1.0 } else { 0.0 }));
        }
        hot
    }

    /// How much smoke drifts off the paper slot, 0 to 1: a light haze while printing, and once the
    /// job is done the flames give way to smoke that thickens for a second and then clears over a
    /// few more.
    pub fn smoke(&self) -> f32 {
        if !self.fx_on {
            return 0.0;
        }
        let mut smoke: f32 = if self.busy { 0.25 } else { 0.0 };
        if let Some(done) = self.done_at {
            let t = (self.clock - done) as f32;
            let after = match t {
                t if t < 0.0 => 0.0,
                t if t < 1.0 => t,
                t => (1.0 - (t - 1.0) / 5.0).max(0.0),
            };
            smoke = smoke.max(after);
        }
        smoke
    }

    /// How soon the view must update again on its own: quickly while flames fade or an error
    /// shakes, slowly for the floating ghost (a spring smooths its steps), never when nothing moves.
    pub fn tick_interval(&self) -> Option<std::time::Duration> {
        use std::time::Duration;
        if !self.fx_on {
            return None;
        }
        let quick = self.flame() > 0.0
            || self.smoke() > 0.0
            || self.error_at.is_some_and(|t| self.clock - t < 0.9)
            || self.boo_until.is_some_and(|t| self.clock < t);
        if quick {
            Some(Duration::from_millis(33))
        } else if self.composed.is_err() {
            Some(Duration::from_millis(700))
        } else {
            None
        }
    }

    /// The Konami code was completed: flames, a shockwave and a ghost.
    pub fn egg_fire(&mut self) {
        self.fire_until = Some(self.clock + 4.0);
        self.press_at = Some(self.clock);
        self.boo_until = Some(self.clock + 4.0);
        self.status = "↑↑↓↓←→←→BA. You found it.".into();
    }

    /// Five taps on the logo make the ghost say boo.
    pub fn logo_tap(&mut self) {
        self.logo_taps += 1;
        if self.logo_taps >= 5 {
            self.logo_taps = 0;
            self.boo_until = Some(self.clock + 3.0);
        }
    }

    /// Fill the text box with something silly to print.
    pub fn surprise_me(&mut self) {
        let seed = self.next_seed();
        let s = voice::surprise(seed);
        self.text = TextEdit::new(s.text);
        self.text_opts.font = if s.mono { FontChoice::Mono } else { FontChoice::Sans };
        self.text_opts.px = if s.mono { 22.0 } else { 32.0 };
        self.mode = Mode::Text;
        self.touch();
    }

    /// The ghost's line when there is nothing to print (and its Easter-egg lines).
    pub fn ghost_line(&self) -> &'static str {
        if self.boo_until.is_some_and(|t| self.clock < t) { "Boo!" } else { voice::ghost(self.mode) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_text_cannot_be_printed_even_when_connected() {
        let mut app = App { link: Link::Online("p".into()), ..App::default() };
        app.refresh();
        assert!(!app.can_print());
        app.text = TextEdit::new("hello");
        app.refresh();
        assert!(app.can_print());
    }

    #[test]
    fn switching_mode_changes_what_is_composed() {
        let mut app = App::default();
        app.code_text = TextEdit::new("abc");
        app.mode = Mode::Codes;
        assert!(app.compose().is_ok());
        app.mode = Mode::Image;
        assert!(app.compose().is_err(), "no picture chosen yet");
    }

    fn online_app() -> (App, tokio::sync::mpsc::UnboundedReceiver<Cmd>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (App { cmd: Some(tx), link: Link::Online("p".into()), ..App::default() }, rx)
    }

    #[test]
    fn the_app_connects_by_itself_and_retries_after_a_miss() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App { cmd: Some(tx), ..App::default() };
        let t0 = Instant::now();
        assert!(app.tick_connection(t0).is_some());
        assert!(matches!(rx.try_recv(), Ok(Cmd::Connect)), "first call connects");
        assert_eq!(app.link, Link::Connecting);
        assert!(app.tick_connection(t0).is_none(), "no second scan while one runs");

        app.apply(Event::Error("Printer not found".into()));
        assert_eq!(app.link, Link::Offline);
        assert!(app.tick_connection(Instant::now()).is_some());
        assert!(rx.try_recv().is_err(), "too soon to retry");
        let later = Instant::now() + std::time::Duration::from_secs(13);
        app.tick_connection(later);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Connect)), "retries after a pause");
    }

    #[test]
    fn pressing_disconnect_stops_the_automatic_connection_until_connect_is_pressed() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App { cmd: Some(tx), link: Link::Online("p".into()), ..App::default() };
        app.disconnect();
        assert!(matches!(rx.try_recv(), Ok(Cmd::Disconnect)));
        app.apply(Event::Disconnected);
        let later = Instant::now() + std::time::Duration::from_secs(60);
        assert!(app.tick_connection(later).is_none());
        assert!(rx.try_recv().is_err(), "the user wanted it off");
        app.connect();
        assert!(matches!(rx.try_recv(), Ok(Cmd::Connect)));
    }

    #[test]
    fn device_info_and_signal_are_kept_and_cleared_with_the_link() {
        let mut app = App::default();
        app.apply(Event::Connected("Mini Pocket Printer_BLE".into(), Some(-51)));
        assert_eq!(app.status, "Connected to Mini Pocket Printer_BLE (-51 dBm)");
        app.apply(Event::Info(DeviceInfo { model: Some("A2Y".into()), battery: Some(98), ..DeviceInfo::default() }));
        assert_eq!((app.battery, app.info.model.as_deref()), (Some(98), Some("A2Y")));
        app.apply(Event::Disconnected);
        assert_eq!((app.battery, app.info.model.as_deref(), app.rssi), (None, None, None));
    }

    #[test]
    fn a_one_shot_print_from_the_command_line_is_sent_and_answered() {
        let (mut app, mut rx) = online_app();
        let (reply, mut answer) = tokio::sync::oneshot::channel();
        app.handle_ipc(IpcMsg::Once { text: "\x1b[32mhello\x1b[0m\n".into(), force: false, reply }, Instant::now());
        assert!(answer.try_recv().unwrap().starts_with("OK"));
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
    }

    #[test]
    fn a_one_shot_print_is_refused_when_offline_busy_or_huge() {
        let ask = |app: &mut App, text: &str, force: bool| {
            let (reply, mut answer) = tokio::sync::oneshot::channel();
            app.handle_ipc(IpcMsg::Once { text: text.into(), force, reply }, Instant::now());
            answer.try_recv().unwrap()
        };
        let mut offline = App::default();
        assert!(ask(&mut offline, "x", false).contains("not connected"));
        let (mut busy, _rx) = online_app();
        busy.busy = true;
        assert!(ask(&mut busy, "x", false).contains("busy"));
        let (mut app, _rx) = online_app();
        let long = "line\n".repeat(400);
        assert!(ask(&mut app, &long, false).contains("--force"));
        assert!(ask(&mut app, &long, true).starts_with("OK"));
    }

    #[test]
    fn live_lines_print_after_a_pause_and_only_once() {
        let (mut app, mut rx) = online_app();
        let t0 = Instant::now();
        app.handle_ipc(IpcMsg::Live(true), t0);
        app.handle_ipc(IpcMsg::Line("building…".into()), t0);
        assert!(app.tick_terminal(t0).is_some(), "too early: asks to be called again");
        assert!(rx.try_recv().is_err());
        app.tick_terminal(t0 + term::IDLE);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        app.busy = false;
        assert!(app.tick_terminal(t0 + term::IDLE * 2).is_none());
        assert!(rx.try_recv().is_err(), "nothing new, nothing printed");
    }

    fn feed_dots(job: &[Vec<u8>]) -> Option<u8> {
        job.iter().find(|c| c.len() == 3 && c[..2] == [0x1B, 0x4A]).map(|c| c[2])
    }

    #[test]
    fn live_batches_join_up_and_the_paper_advances_once_when_the_stream_ends() {
        let (mut app, mut rx) = online_app();
        let t0 = Instant::now();
        app.handle_ipc(IpcMsg::Live(true), t0);
        app.handle_ipc(IpcMsg::Line("one".into()), t0);
        app.tick_terminal(t0 + term::IDLE);
        let Ok(Cmd::Print(first)) = rx.try_recv() else { panic!("no batch") };
        assert_eq!(feed_dots(&first), None, "no white gap between batches");
        app.busy = false;

        app.handle_ipc(IpcMsg::Line("two".into()), t0 + term::IDLE);
        app.tick_terminal(t0 + term::IDLE * 3);
        let Ok(Cmd::Print(second)) = rx.try_recv() else { panic!("no second batch") };
        assert_eq!(feed_dots(&second), None);
        app.busy = false;

        app.handle_ipc(IpcMsg::Live(false), t0 + term::IDLE * 3);
        app.tick_terminal(t0 + term::IDLE * 4);
        let Ok(Cmd::Print(end)) = rx.try_recv() else { panic!("no closing feed") };
        assert_eq!(feed_dots(&end), Some(protocol::TEAR_OFF_DOTS), "one tear-off at the end");
        app.busy = false;
        app.tick_terminal(t0 + term::IDLE * 5);
        assert!(rx.try_recv().is_err(), "and only one");
    }

    #[test]
    fn the_window_title_tells_what_the_printer_is_doing() {
        let mut app = App::default();
        assert_eq!(app.window_title(), "Tronic Printer");
        app.link = Link::Connecting;
        assert_eq!(app.window_title(), "Tronic Printer · Connecting…");
        app.link = Link::Online("p".into());
        app.battery = Some(87);
        assert_eq!(app.window_title(), "Tronic Printer · Connected · 87%");
        app.busy = true;
        app.progress = 0.426;
        assert_eq!(app.window_title(), "Tronic Printer · Printing 43%");
    }

    #[test]
    fn the_flames_rise_with_the_job_die_down_after_it_and_stay_out_when_effects_are_off() {
        let mut app = App { fx_on: true, busy: true, progress: 0.5, ..App::default() };
        assert!((app.flame() - 0.65).abs() < 1e-6);
        app.busy = false;
        app.done_at = Some(10.0);
        app.clock = 10.0;
        assert!(app.flame() > 0.5, "just after the job the flames are still up");
        app.clock = 11.0;
        assert_eq!(app.flame(), 0.0, "and a second later they are out");
        app.fx_on = false;
        app.busy = true;
        assert_eq!(app.flame(), 0.0, "no effects, no flames");
    }

    #[test]
    fn the_konami_flames_burn_then_fade_and_keep_the_view_ticking() {
        let mut app = App { fx_on: true, clock: 5.0, ..App::default() };
        app.egg_fire();
        assert_eq!(app.flame(), 1.0);
        assert_eq!(app.tick_interval(), Some(std::time::Duration::from_millis(33)));
        app.clock = 5.0 + 3.7;
        assert!(app.flame() > 0.0 && app.flame() < 1.0, "fading out");
        app.clock = 5.0 + 4.5;
        assert_eq!(app.flame(), 0.0);
        assert!(app.boo_until.is_some_and(|t| app.clock > t));
    }

    #[test]
    fn the_ghost_ticks_only_with_effects_on_and_says_boo_after_five_logo_taps() {
        let mut app = App::default();
        assert_eq!(app.tick_interval(), None, "effects off: the view never ticks on its own");
        app.fx_on = true;
        app.refresh();
        assert!(app.composed.is_err());
        assert_eq!(app.tick_interval(), Some(std::time::Duration::from_millis(700)), "nothing to print: the ghost floats slowly");
        app.clock = 3.0;
        for _ in 0..4 {
            app.logo_tap();
        }
        assert_ne!(app.ghost_line(), "Boo!");
        app.logo_tap();
        assert_eq!(app.ghost_line(), "Boo!");
        app.clock = 7.0;
        assert_ne!(app.ghost_line(), "Boo!", "and the boo wears off");
    }

    #[test]
    fn picking_a_theme_fills_an_empty_box_resets_the_form_and_changes_the_ticket() {
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        assert!(app.text.display().0.contains("Coffee"), "an empty box gets the example");
        let plain_h = render::text(&app.text.display().0, app.text_opts).unwrap().height;
        assert!(app.compose().unwrap().height > plain_h + 200);
        app.set_theme(Theme::Diner);
        assert_eq!(app.text.display().0, Theme::Diner.sample(), "an untouched example follows the theme");
        app.set_theme(Theme::Receipt);
        app.theme_edits[0] = TextEdit::new("MY SHOP");
        assert_eq!(app.theme_fields().get("store").map(String::as_str), Some("MY SHOP"));
        app.text = TextEdit::new("Mine 1.00");
        app.set_theme(Theme::Diner);
        assert_eq!(app.text.display().0, "Mine 1.00", "your own text is kept");
        assert_eq!(app.theme_fields().get("place").map(String::as_str), Some(""), "the form starts empty");
        app.set_theme(Theme::Plain);
        assert_eq!(app.compose().unwrap().height, render::text("Mine 1.00", app.text_opts).unwrap().height);
    }

    #[test]
    fn surprise_me_fills_the_text_box_and_picks_a_fitting_font() {
        let mut app = App::default();
        for _ in 0..24 {
            app.surprise_me();
            assert!(!app.text.display().0.trim().is_empty());
            assert_eq!(app.mode, Mode::Text);
        }
    }

    #[test]
    fn finishing_a_print_and_an_error_stamp_the_clock_for_the_effects() {
        let mut app = App { clock: 12.5, busy: true, ..App::default() };
        app.apply(Event::PrintDone);
        assert_eq!(app.done_at, Some(12.5));
        app.apply(Event::Error("connection failed".into()));
        assert_eq!(app.error_at, Some(12.5));
        app.clock = 20.0;
        app.print();
        assert_eq!(app.press_at, Some(20.0));
    }

    fn write_png(path: &std::path::Path) {
        image::RgbaImage::from_pixel(24, 16, image::Rgba([20, 20, 20, 255])).save(path).unwrap();
    }

    fn gallery_app(tag: &str, auto: bool, online: bool) -> (App, tokio::sync::mpsc::UnboundedReceiver<Cmd>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tronic-app-gallery-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App { cmd: Some(tx), link: if online { Link::Online("p".into()) } else { Link::Offline }, ..App::default() };
        app.gallery.set_dir(dir.clone());
        app.gallery.auto = auto;
        (app, rx, dir)
    }

    #[test]
    fn a_picture_dropped_in_the_watched_folder_prints_by_itself_once_it_has_settled() {
        let (mut app, mut rx, dir) = gallery_app("auto", true, true);
        write_png(&dir.join("old.png"));
        let t0 = Instant::now();
        app.tick_gallery(t0);
        write_png(&dir.join("new.png"));
        app.tick_gallery(t0 + gallery::SCAN_EVERY);
        assert!(rx.try_recv().is_err(), "first sight: not printed yet, it may still be copying");
        app.tick_gallery(t0 + gallery::SCAN_EVERY * 2);
        let Ok(Cmd::Print(job)) = rx.try_recv() else { panic!("the new picture was not printed") };
        assert!(job.len() > 4);
        assert_eq!(app.gallery.auto_printed, 1);
        assert_eq!(app.source.as_ref().map(|s| s.name.as_str()), Some("new.png"));
        assert_eq!(app.mode, Mode::Image);
        assert!(rx.try_recv().is_err(), "and the old one never printed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn arrivals_wait_for_a_free_connected_printer_and_never_print_with_the_watcher_off() {
        let (mut app, mut rx, dir) = gallery_app("wait", true, false);
        let t0 = Instant::now();
        app.tick_gallery(t0);
        write_png(&dir.join("a.png"));
        app.tick_gallery(t0 + gallery::SCAN_EVERY);
        app.tick_gallery(t0 + gallery::SCAN_EVERY * 2);
        assert_eq!(app.gallery.queue.len(), 1, "queued while the printer is offline");
        assert!(rx.try_recv().is_err());
        app.link = Link::Online("p".into());
        app.busy = true;
        app.tick_gallery(t0 + gallery::SCAN_EVERY * 3);
        assert!(rx.try_recv().is_err(), "and while it is busy");
        app.busy = false;
        app.tick_gallery(t0 + gallery::SCAN_EVERY * 4);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        assert!(app.gallery.queue.is_empty());

        let (mut quiet, mut rx2, dir2) = gallery_app("quiet", false, true);
        quiet.tick_gallery(t0);
        write_png(&dir2.join("b.png"));
        quiet.tick_gallery(t0 + gallery::SCAN_EVERY);
        quiet.tick_gallery(t0 + gallery::SCAN_EVERY * 2);
        assert!(rx2.try_recv().is_err(), "the watcher is off: the folder is left alone");
        assert_eq!(quiet.gallery.entries.len(), 1, "but the gallery still lists it");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[test]
    fn switching_the_watcher_off_forgets_what_was_waiting() {
        let (mut app, _rx, dir) = gallery_app("off", true, false);
        app.gallery.queue.push_back(dir.join("x.png"));
        app.gallery.auto_printed = 3;
        app.set_auto_print(false);
        assert!(app.gallery.queue.is_empty() && app.gallery.auto_printed == 0 && !app.gallery.auto);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dropped_picture_prints_at_once_when_connected_and_is_only_shown_otherwise() {
        let (mut app, mut rx, dir) = gallery_app("drop", false, false);
        let pic = dir.join("dropped.png");
        write_png(&pic);
        app.drop_files(&[dir.join("notes.txt"), pic.clone()]);
        assert_eq!(app.mode, Mode::Image);
        assert_eq!(app.source.as_ref().map(|s| s.name.as_str()), Some("dropped.png"));
        assert!(rx.try_recv().is_err(), "no printer: shown, not printed");

        app.drop_files(&[dir.join("only.txt")]);
        assert!(app.error.as_deref().is_some_and(|e| e.contains("not a picture")));

        app.link = Link::Online("p".into());
        app.drop_files(&[pic]);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))), "connected: a drop prints at once, watcher or not");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flames_burn_while_printing_then_give_way_to_smoke_that_clears() {
        let mut app = App { fx_on: true, busy: true, progress: 0.5, clock: 10.0, ..App::default() };
        assert!(app.flame() > 0.5 && app.smoke() > 0.0 && app.smoke() < 0.5, "printing: flames and a light haze");
        app.busy = false;
        app.done_at = Some(10.0);
        app.clock = 10.5;
        assert!(app.flame() > 0.0 && app.smoke() > 0.4, "just finished: the flames are dying and smoke is rising");
        app.clock = 12.0;
        assert_eq!(app.flame(), 0.0);
        assert!(app.smoke() > 0.7, "later: no flames, only smoke");
        app.clock = 15.5;
        assert!(app.smoke() > 0.0 && app.smoke() < 0.15, "the smoke is nearly gone");
        app.clock = 16.5;
        assert_eq!(app.smoke(), 0.0);
        app.composed = Ok(Bitmap::blank(5));
        assert_eq!(app.tick_interval(), None, "and the window rests again (with a ticket on the paper)");
        app.fx_on = false;
        app.clock = 11.5;
        assert_eq!(app.smoke(), 0.0, "no effects, no smoke");
    }

    #[test]
    fn moving_a_slider_does_not_rescale_the_picture_again() {
        let mut app = App::default();
        app.source = Some(render::Source::from_image("t", image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(64, 48, image::Rgba([90, 90, 90, 255])))));
        app.mode = Mode::Image;
        app.refresh();
        let first = app.prepared.as_ref().map(|(_, _, _, p)| p.identity());
        assert!(first.is_some());
        app.pic_opts.brightness = 0.4;
        app.pic_opts.contrast = -0.3;
        app.refresh();
        assert_eq!(app.prepared.as_ref().map(|(_, _, _, p)| p.identity()), first, "same scaled picture: only the tone changed");
        app.pic_opts.rotate = 1;
        app.refresh();
        assert_ne!(app.prepared.as_ref().map(|(_, _, _, p)| p.identity()), first, "a rotation needs a new one");
    }

    #[test]
    fn a_short_press_on_a_thumbnail_picks_it_and_a_pull_onto_the_preview_prints_it() {
        let (mut app, mut rx, dir) = gallery_app("dnd", false, true);
        let pic = dir.join("dog.png");
        write_png(&pic);

        app.drag_start(pic.clone());
        app.drag_move(1.0, 2.0);
        app.drag_end(false);
        assert_eq!(app.gallery.selected.as_deref(), Some(pic.as_path()), "a click picks it");
        assert!(rx.try_recv().is_err(), "and does not print");

        app.drag_start(pic.clone());
        app.drag_move(120.0, 40.0);
        app.over_stage = false;
        app.drag_end(false);
        assert!(rx.try_recv().is_err(), "let go away from the printer: nothing happens");

        app.drag_start(pic.clone());
        app.drag_move(300.0, 10.0);
        app.over_stage = true;
        app.drag_end(true);
        assert!(rx.try_recv().is_err(), "a cancelled drag (Escape) prints nothing");

        app.drag_start(pic.clone());
        app.drag_move(300.0, 10.0);
        app.over_stage = true;
        app.drag_end(false);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))), "let go over the printer: it prints");
        assert!(app.drag.is_none() && !app.over_stage && app.pointer.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn printing_repeats_the_job_for_each_copy() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App { cmd: Some(tx), link: Link::Online("p".into()), copies: 3, ..App::default() };
        app.text = TextEdit::new("hi");
        app.refresh();
        app.print();
        let Some(Cmd::Print(job)) = rx.try_recv().ok() else { panic!("no job sent") };
        let activations = job.iter().filter(|c| c.as_slice() == [0x10, 0xFF, 0xF1, 0x03]).count();
        assert_eq!(activations, 3);
        assert!(app.busy);
    }
}
