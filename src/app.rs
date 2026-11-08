//! Application state and what happens to it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use rune::{Image, ImageCache, KeyEvent, TextEdit};
use tokio::sync::mpsc::UnboundedSender;

use crate::ble::{Cmd, DeviceInfo, Event};
use crate::egg::Konami;
use crate::gallery::{self, Gallery};
use crate::ipc::IpcMsg;
use crate::jobs::{clamp_copies, MAX_CM};
use crate::protocol::{self, Bitmap, Density};
use crate::render::{self, CodeOpts, FontChoice, PictureOpts, Source, TextOpts, Theme};
use crate::settings;
use crate::term::{self, Term};
use crate::update::Update;
use crate::voice;

/// How far (in points) a press on a gallery thumbnail must travel to count as a drag, not a click.
pub const DRAG_MIN: f32 = 6.0;

/// A gallery picture being dragged toward the printer.
pub struct ThumbDrag {
    pub path: PathBuf,
    /// Set when the thing dragged is a writing theme from the Text tab, not a picture.
    pub theme: Option<Theme>,
    /// How far the pointer has moved since the press.
    pub travel: f32,
}

/// The preview image can't be taller than this many dots (Rune's limit is 8192 pixels).
const MAX_PREVIEW_ROWS: usize = 8000;

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

/// Where a print is in its life: one value instead of `busy` + `progress` + `done_at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Printing {
    Idle,
    Sending,
    Running { progress: f32 },
    Done { at: f64 },
    Failed { at: f64 },
}

impl Printing {
    /// A job has been handed to the worker, or is on its way through the printer.
    pub fn is_busy(&self) -> bool {
        matches!(self, Printing::Sending | Printing::Running { .. })
    }

    /// How far the running job is, 0 to 1 (0 when no job runs).
    pub fn progress(&self) -> f32 {
        match self {
            Printing::Running { progress } => *progress,
            _ => 0.0,
        }
    }

    /// When the last job finished, for the effects that die down after it.
    pub fn done_at(&self) -> Option<f64> {
        match self {
            Printing::Done { at } => Some(*at),
            _ => None,
        }
    }
}

pub struct App {
    // Infrastructure.
    pub cmd: Option<UnboundedSender<Cmd>>,
    pub events: Option<Receiver<Event>>,
    pub images: Option<ImageCache>,
    pub ipc: Option<Receiver<IpcMsg>>,
    /// Hears from the background update check, which sends at most one message.
    pub update_rx: Option<Receiver<Update>>,
    /// A newer release, once the check has found one, until the user dismisses it.
    pub update: Option<Update>,
    /// The app's mark, shown in the header.
    pub logo: Option<Image>,
    /// The printer's shell, an SVG drawn once at startup.
    pub printer_art: Option<Image>,
    /// The title last given to the window, so it is only set when it changes.
    pub title_shown: String,
    // Effects and fun.
    /// GPU effects on or off (the in-app switch).
    pub fx_on: bool,
    /// The colour palette being worn (kept in the settings).
    pub palette: crate::style::PaletteId,
    /// Rune's clock, in seconds, as of the last frame.
    pub clock: f64,
    /// When the Print button was last pressed (the shockwave starts there).
    pub press_at: Option<f64>,
    pub error_at: Option<f64>,
    /// Konami flames burn until this time.
    pub fire_until: Option<f64>,
    /// The ghost says boo until this time.
    pub boo_until: Option<f64>,
    pub konami: Konami,
    pub logo_taps: u8,
    /// Taps on the ghost's bubble: each one makes it say something else.
    pub ghost_taps: u64,
    /// The ghost's line changes every few minutes: the last turn it showed.
    pub ghost_turn: u64,
    /// When the ghost's line last changed by a tap: its new line types out from there.
    pub ghost_said_at: Option<f64>,
    /// Where the ghost floats round a ticket: 0 right of it, 1 left, 2 above, 3 beside the case.
    /// The stage with a ticket only; the empty state keeps its place over the slit.
    pub ghost_spot: u8,
    /// When the ghost last drifted to another spot, so it does not drift on every frame.
    pub ghost_moved_at: f64,
    /// The point of its column the hand grabbed, in stage points: while the button is held the
    /// column follows the pointer, so the ghost does not jump under the hand.
    pub ghost_drag: Option<(f32, f32)>,
    /// Where the user dragged the ghost, in stage points from its top-left. It wins over the
    /// drifting spots until the next drift clears it.
    pub ghost_at: Option<(f32, f32)>,
    /// The line under the wordmark changes every minute: the last turn it showed.
    pub tagline_turn: u64,
    /// The pointer in window points, so the ghost's eyes can look at it.
    pub pointer_at: Option<(f32, f32)>,
    /// When the timer already asked for is due (see `main::schedule`): one timer at a time.
    pub wake_pending: Option<f64>,
    /// The bitmap the preview picture was last made from, so an unchanged ticket keeps its picture.
    pub shown: Option<Bitmap>,
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
    /// The life of the current (or last) print.
    pub printing: Printing,
    // What to print.
    pub mode: Mode,
    /// The drawer on the left (theme or picture gallery, the mode's extras, density and copies).
    /// Closed by default: the window is the printer and its ticket.
    pub drawer_open: bool,
    pub density: usize,
    pub copies: usize,
    pub text: TextEdit,
    pub text_opts: TextOpts,
    /// How the text is dressed, and the little form that theme asks for.
    pub theme: Theme,
    /// What is written in the form's boxes, by field key. Keyed, not positional: a theme reloaded
    /// under the form cannot move a value into another field's box.
    pub theme_edits: BTreeMap<&'static str, TextEdit>,
    /// The part of the ticket being written on in the preview (a field's key, or `body`).
    /// `&'static str` like the keys of `Field`: a key that no theme has any more cannot be stored.
    pub editing: Option<&'static str>,
    /// Where the parts of the ticket that can be written on are, in dots on the paper.
    pub regions: Vec<render::Region>,
    /// Presses of "Shuffle" on a silly form: each one picks other wording for its boxes and closing line.
    pub form_shuffle: u64,
    /// Whether a silly form prints its closing line.
    pub closing_line: bool,
    /// The port box of the MCP section.
    pub mcp_port: TextEdit,
    /// Small pictures of every theme, for the Text tab's gallery.
    pub theme_thumbs: std::collections::HashMap<Theme, Image>,
    /// Themes whose example would not draw: no picture, and no use trying again every frame.
    pub theme_bad: std::collections::HashSet<Theme>,
    /// What the last read of the themes folder gave (how many, and which files were refused).
    pub theme_report: render::custom::Report,
    /// Fingerprint of the themes folder when it was last read, and when it was last looked at.
    pub theme_sig: u64,
    pub theme_checked: Option<Instant>,
    /// Ids of the starred themes, kept between runs.
    pub fav_themes: Vec<String>,
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
    /// How many rows were cut off the top of the preview (0 when nothing was cropped). Region
    /// coordinates are already shifted by this amount.
    pub preview_crop: usize,
}

impl Default for App {
    fn default() -> Self {
        App {
            cmd: None,
            events: None,
            images: None,
            ipc: None,
            update_rx: None,
            update: None,
            logo: None,
            printer_art: None,
            title_shown: String::new(),
            fx_on: false,
            palette: crate::style::PaletteId::default(),
            clock: 0.0,
            press_at: None,
            error_at: None,
            fire_until: None,
            boo_until: None,
            konami: Konami::default(),
            logo_taps: 0,
            ghost_taps: 0,
            ghost_turn: 0,
            ghost_said_at: None,
            ghost_spot: 0,
            ghost_moved_at: 0.0,
            ghost_drag: None,
            ghost_at: None,
            tagline_turn: 0,
            pointer_at: None,
            wake_pending: None,
            shown: None,
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
            printing: Printing::Idle,
            mode: Mode::Text,
            drawer_open: false,
            density: 1,
            copies: 1,
            text: TextEdit::new(""),
            text_opts: TextOpts::default(),
            theme: Theme::Plain,
            theme_edits: BTreeMap::new(),
            editing: None,
            regions: Vec::new(),
            form_shuffle: 0,
            closing_line: true,
            mcp_port: TextEdit::new(&crate::mcp_setup::http_port().to_string()),
            theme_thumbs: Default::default(),
            theme_bad: Default::default(),
            theme_report: Default::default(),
            theme_sig: 0,
            theme_checked: None,
            fav_themes: Vec::new(),
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
            preview_crop: 0,
        }
    }
}

/// Seconds the ghost keeps one line before saying another. It is a slow, ambient line.
pub const GHOST_TURN: f64 = 15.0 * 60.0;
/// Seconds the ghost floats in one spot before drifting round the ticket to another.
pub const GHOST_MOVE: f64 = 26.0;
/// The spots it drifts between: right of the ticket, left of it, above it, then beside the case.
pub const GHOST_SPOTS: u8 = 4;
/// Seconds the line under the wordmark keeps before printing another one.
pub const TAGLINE_TURN: f64 = 60.0;
/// How fast the typed-out lines appear, in characters per second.
pub const TYPE_CPS: f64 = 28.0;

impl App {
    /// Ask for the preview to be rebuilt before the next frame.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// Open the drawer, or close it. The `touch` is the whole point: `poll` only rebuilds the view
    /// when the app is dirty, so a control that flips the flag alone does nothing on screen.
    pub fn toggle_drawer(&mut self) {
        self.drawer_open = !self.drawer_open;
        self.touch();
    }

    /// Put the drawer away (Escape, and the "×" in its header), and ask for the redraw.
    pub fn close_drawer(&mut self) {
        self.drawer_open = false;
        self.touch();
    }

    /// The window title doubles as a status line, so the taskbar says what the printer is doing.
    pub fn window_title(&self) -> String {
        let state = if self.printing.is_busy() {
            format!("Printing {:.0}%", self.printing.progress() * 100.0)
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
            Mode::Text => {
                let body = self.text.display().0;
                if self.theme == Theme::Plain && body.trim().is_empty() {
                    // Plain has no frame, so an empty box would leave the stage with only the
                    // ghost: no paper to click on. Keep a short blank sheet instead. Printing it
                    // is still refused by `print` (and by `jobs`), so this is a surface to write
                    // on, not a ticket.
                    return Ok(self.blank_sheet());
                }
                render::themed(self.theme, &body, &self.theme_fields(), self.text_opts)
            }
            // The other tabs have nothing to draw either: a blank sheet keeps paper on the stage
            // (and the ghost round it) instead of an empty hole. `can_print` and `print` refuse
            // it, so it is a place to start, never an empty job.
            Mode::Image => match &self.source {
                Some(src) => Ok(render::picture(src, self.pic_opts)),
                None => Ok(self.blank_sheet()),
            },
            Mode::Codes => {
                let code = self.code_text.display().0;
                if code.trim().is_empty() {
                    Ok(self.blank_sheet())
                } else {
                    render::code(&code, self.code_opts)
                }
            }
            Mode::Terminal => {
                let text = self.term.text();
                if text.trim().is_empty() {
                    Ok(self.blank_sheet())
                } else {
                    render::text(&text, self.terminal_opts())
                }
            }
        }
    }

    /// The height of the blank sheet every tab shows when it has nothing to compose: three lines
    /// of the text settings, so the empty tabs all hand the stage paper of the same size.
    fn blank_sheet(&self) -> Bitmap {
        let line = (self.text_opts.px + self.text_opts.line_gap).max(1.0) as usize;
        Bitmap::blank(render::TEETH + 3 * line)
    }

    /// What the theme's little form holds, by field key.
    pub fn theme_fields(&self) -> render::Fields {
        let mut out: render::Fields =
            self.theme.fields().iter().map(|f| (f.key.to_string(), self.theme_edits.get(f.key).map_or_else(String::new, |e| e.display().0.to_string()))).collect();
        // Settings of the app, not text on the ticket (see `Ctx`).
        if self.theme.shuffles() {
            out.insert("_shuffle".into(), self.form_shuffle.to_string());
        }
        if self.theme.is_form() {
            out.insert("_closing".into(), if self.closing_line { "on" } else { "off" }.into());
        }
        out
    }

    /// The box a part of the form is written in. Boxes are made when first written on: a theme
    /// that has no field by that key never reads it (`theme_fields` only asks for its own), so the
    /// value cannot land in another field's box.
    pub fn edit_for(&mut self, key: &'static str) -> &mut TextEdit {
        self.theme_edits.entry(key).or_insert_with(|| TextEdit::new(""))
    }

    /// "Shuffle" on a silly form: other wording for its boxes and closing line.
    pub fn shuffle_form(&mut self) {
        self.form_shuffle += 1;
        self.touch();
    }

    /// A clean sheet to write on: no text and no details.
    pub fn blank_form(&mut self) {
        self.text = TextEdit::new("");
        self.theme_edits.clear();
        self.touch();
    }

    /// Pick a theme. An empty box, or one still holding another theme's example, gets this theme's
    /// example so the preview shows it at once; text you typed is kept.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.editing = None;
        self.form_shuffle = 0;
        self.theme_edits.clear();
        // Text that is empty, or still some theme's example, was never written by the user: the
        // new theme gets its own example instead.
        let current = self.text.display().0.to_string();
        if current.trim().is_empty() || Theme::all().iter().any(|t| t.sample() == current) {
            self.text = TextEdit::new(if theme == Theme::Plain { "" } else { theme.sample() });
        }
        self.touch();
    }

    /// Monospace, breaking anywhere so columns stay put.
    pub fn terminal_opts(&self) -> TextOpts {
        TextOpts { px: term::SIZES[self.term.size.min(2)], font: FontChoice::Mono, break_anywhere: true, margin_y: 2.0, ..TextOpts::default() }
    }

    /// The same, without the vertical margin: a live batch printed with feed 0 has to
    /// continue the previous batch, so it carries no blank rows of its own.
    fn terminal_opts_tight(&self) -> TextOpts {
        TextOpts { margin_y: 0.0, ..self.terminal_opts() }
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
        // Any ticket can be written on in the preview: it says where its writable parts are (the
        // text, each field), and a silly form being typed on is drawn blank there (the others
        // are covered by the text box instead). Drawn once: when nothing is being typed on,
        // that drawing is the ticket itself.
        self.regions.clear();
        // Every part of the form gets its (empty) box before the ticket is drawn: the paper reads
        // the boxes by key, and the UI shows the same ones.
        for f in self.theme.fields() {
            self.theme_edits.entry(f.key).or_insert_with(|| TextEdit::new(""));
        }
        let mut shown_form: Option<Bitmap> = None;
        if self.mode == Mode::Text {
            let mut fields = self.theme_fields();
            if let (true, Some(key)) = (self.theme.is_form(), self.editing) {
                fields.insert("_editing".into(), key.to_string());
            }
            if let Ok((bitmap, regions)) = render::themed_mapped(self.theme, &self.text.display().0, &fields, self.text_opts) {
                self.regions = regions;
                shown_form = Some(bitmap);
            }
        } else if !(self.mode == Mode::Codes && self.editing == Some("_code")) {
            // Leaving the Text tab ends any form writing; the Codes sheet is the exception: its
            // box stays up while the first character turns the blank sheet into a code.
            self.editing = None;
        }
        let blank_part = self.theme.is_form() && self.editing.is_some();
        self.composed = match &shown_form {
            Some(b) if !blank_part => Ok(b.clone()),
            // Image goes through `compose_image` normally; with no picture there is nothing to
            // prepare, so it gets the same blank sheet as the other empty tabs.
            _ if self.mode == Mode::Image && self.source.is_none() => Ok(self.blank_sheet()),
            _ if self.mode == Mode::Image => self.compose_image(),
            _ => self.compose(),
        };
        // The blank sheets come from `compose`, not from a theme, so nothing marked their inside
        // as writable. Plain is all body; an empty Codes sheet is all code text.
        let blank_plain = self.mode == Mode::Text && self.theme == Theme::Plain && self.text.display().0.trim().is_empty();
        let blank_code = self.mode == Mode::Codes && self.code_text.display().0.trim().is_empty();
        if blank_plain || blank_code {
            if let Ok(sheet) = &self.composed {
                let key = if blank_code { "_code" } else { "body" };
                self.regions.push(render::Region { key: key.into(), x0: 0, x1: protocol::WIDTH_DOTS, y0: 0, y1: sheet.height });
            }
        }
        self.preview_cropped = false;
        self.preview_crop = 0;
        let source: Option<&Bitmap> = shown_form.as_ref().or(self.composed.as_ref().ok());
        let unchanged = self.preview.is_some() && source.is_some() && source == self.shown.as_ref();
        if !unchanged {
            self.preview = match (source, &self.images) {
                (Some(bitmap), Some(cache)) => {
                    let tail;
                    let shown = if bitmap.height > MAX_PREVIEW_ROWS {
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
            self.shown = source.cloned();
        }
        self.preview_cropped = source.is_some_and(|b| b.height > MAX_PREVIEW_ROWS);
        if self.preview_cropped {
            // Keep the regions that fall in the visible tail, shifted up by the cropped rows, so
            // the in-place boxes still line up with what is on screen. Parts above the crop land in
            // unmapped() and are typed in the bar under the ticket instead.
            if let Some(b) = source {
                let offset = b.height.saturating_sub(MAX_PREVIEW_ROWS);
                self.preview_crop = offset;
                self.regions.retain_mut(|r| {
                    if r.y1 <= offset {
                        return false;
                    }
                    r.y0 = r.y0.saturating_sub(offset);
                    r.y1 = (r.y1 - offset).min(MAX_PREVIEW_ROWS);
                    true
                });
            }
        }
    }

    /// The parts of the ticket that cannot be written on in the preview (the theme draws them in
    /// a way that cannot be read back): `(key, name)`, to be typed in the bar under the ticket.
    pub fn unmapped(&self) -> Vec<(&'static str, &'static str)> {
        if self.mode != Mode::Text || self.composed.is_err() {
            return Vec::new();
        }
        let mut all: Vec<(&'static str, &'static str)> = vec![("body", "Text")];
        all.extend(self.theme.fields().iter().map(|f| (f.key, f.label)));
        all.retain(|(key, _)| !self.regions.iter().any(|r| r.key == *key));
        all
    }

    /// Themes drawn lying on their side (they print down the paper but read across it): the
    /// preview shows them turned, so a long ticket uses the width of the stage, not its height.
    pub fn preview_turned(&self) -> bool {
        self.mode == Mode::Text && matches!(self.theme.id(), "flight" | "train-old" | "banner")
    }

    /// The text box behind a part of the ticket: the text itself, the code text, or the field's own
    /// box, made when first written on. A key the theme does not have gets a box of its own, never
    /// the body's.
    pub fn region_edit_mut(&mut self, key: &'static str) -> &mut TextEdit {
        match key {
            "body" => &mut self.text,
            "_code" => &mut self.code_text,
            _ => self.edit_for(key),
        }
    }

    /// The `'static` key of a part of the picked theme (`body` counts, and `code` on the Codes tab),
    /// when it exists: a key left over from a theme loaded before (the themes folder reloaded under
    /// an open form) is not one, and nothing can be written there.
    fn region_key(&self, key: &str) -> Option<&'static str> {
        if key == "body" {
            return Some("body");
        }
        if key == "_code" && self.mode == Mode::Codes {
            return Some("_code");
        }
        self.theme.fields().iter().find(|f| f.key == key).map(|f| f.key)
    }

    /// Start writing on a part of the ticket in the preview.
    pub fn edit_region(&mut self, key: &str) {
        let Some(key) = self.region_key(key) else { return };
        if let Some(old) = self.editing.take() {
            self.region_edit_mut(old).focused = false;
        }
        self.region_edit_mut(key).focused = true;
        self.editing = Some(key);
        self.touch();
    }

    /// Done writing on the preview: the ticket shows the text again.
    pub fn stop_editing(&mut self) {
        if let Some(old) = self.editing.take() {
            self.region_edit_mut(&old).focused = false;
            self.touch();
        }
    }

    pub fn can_print(&self) -> bool {
        let empty_unprintable = match self.mode {
            Mode::Text => self.text.display().0.trim().is_empty() && !self.theme.body_optional(),
            Mode::Image => self.source.is_none(),
            Mode::Codes => self.code_text.display().0.trim().is_empty(),
            Mode::Terminal => self.term.text().trim().is_empty(),
        };
        !self.printing.is_busy() && matches!(self.link, Link::Online(_)) && self.composed.is_ok() && !empty_unprintable
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

    /// Ctrl+P (Cmd+P on a Mac) is the Print button, wherever the focus is.
    pub fn is_print_shortcut(e: &KeyEvent) -> bool {
        let p = e.is_command('p') || matches!(&e.key, rune::Key::Command(k) if k.eq_ignore_ascii_case(&'p'));
        p && !e.mods.shift && !e.mods.alt
    }

    /// The shortcut as the button shows it.
    pub fn print_shortcut_label() -> &'static str {
        if cfg!(target_os = "macos") { "⌘P" } else { "Ctrl+P" }
    }

    /// A key the app owns whatever has the focus: true when it was one (and has been dealt with).
    pub fn key_shortcut(&mut self, e: &KeyEvent) -> bool {
        if App::is_print_shortcut(e) {
            self.print();
            return true;
        }
        // Escape puts the drawer away, wherever the focus is: the stage gets the window back.
        if matches!(e.key, rune::Key::Escape) && self.drawer_open {
            self.close_drawer();
            return true;
        }
        false
    }

    /// Refuse a print that cannot run now, saying why where the banner shows it. True when the
    /// printer is ready to take a job.
    fn printer_ready(&mut self) -> bool {
        if self.printing.is_busy() {
            self.error = Some("The printer is busy with another job, try again in a few seconds.".into());
            return false;
        }
        if !matches!(self.link, Link::Online(_)) {
            self.error = Some("The printer is not connected: press Connect.".into());
            return false;
        }
        true
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
        // An emptied text box is refused here, not by the renderer: the preview keeps showing the
        // theme's frame with a blank body. A silly form prints blank (it is meant to be filled in
        // by hand), and so does a theme that makes something of an empty box (a fortune, a boarding
        // pass): `body_optional` covers both. Plain has no frame, so it is already refused above.
        if self.mode == Mode::Text && self.text.display().0.trim().is_empty() && !self.theme.body_optional() {
            self.error = Some("nothing to print".into());
            return;
        }
        // The other tabs put a blank sheet on the stage when they have nothing to compose; the
        // refusal stays here, where it was when `compose` returned the error itself.
        if self.mode == Mode::Image && self.source.is_none() {
            self.error = Some("Choose a picture.".into());
            return;
        }
        if self.mode == Mode::Codes && self.code_text.display().0.trim().is_empty() {
            self.error = Some("nothing to encode".into());
            return;
        }
        if self.mode == Mode::Terminal && self.term.text().trim().is_empty() {
            self.error = Some("nothing to print".into());
            return;
        }
        // One guard for every entry point: the button, Ctrl+P and a theme dropped on the stage all
        // end up here, so none of them can queue a job while offline, connecting or already busy.
        if !self.printer_ready() {
            return;
        }
        if self.mode == Mode::Terminal {
            // Printing from the Terminal tab means "what has not been printed yet".
            if self.term.unprinted() == 0 {
                self.error = Some("Everything in the buffer is already printed.".into());
                return;
            }
            self.flush_terminal(protocol::TEAR_OFF_DOTS);
            return;
        }
        self.send_bitmap(&bitmap, clamp_copies(self.copies), protocol::TEAR_OFF_DOTS);
    }

    /// `feed` is the paper pushed out afterwards; live batches pass 0 so they join up.
    pub(crate) fn send_bitmap(&mut self, bitmap: &Bitmap, copies: usize, feed: u8) -> bool {
        // A finished ticket sheds its blank edges; live batches must join up, so they stay whole.
        let trimmed;
        let bitmap = if feed > 0 {
            trimmed = bitmap.trimmed(protocol::EDGE_DOTS);
            &trimmed
        } else {
            bitmap
        };
        let mut job = Vec::new();
        for _ in 0..copies {
            job.extend(protocol::job_with_feed(bitmap, self.density_level(), feed));
        }
        self.send_job(job)
    }

    fn send_job(&mut self, job: Vec<Vec<u8>>) -> bool {
        self.printing = Printing::Sending;
        self.status = "Printing…".into();
        let sent = self.send(Cmd::Print(job));
        if !sent {
            self.printing = Printing::Idle;
        }
        sent
    }

    /// Print the terminal lines that have not been printed yet.
    pub fn flush_terminal(&mut self, feed: u8) {
        let text = self.term.pending_text();
        // A batch with feed 0 is one of several live batches: without its margin the
        // batches join up; a one-shot or the closing batch keeps its small margin.
        let opts = if feed == 0 { self.terminal_opts_tight() } else { self.terminal_opts() };
        match render::text(&text, opts) {
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
        if self.printing.is_busy() {
            return "ERR the printer is busy".into();
        }
        let bitmap = match render::text(&crate::ansi::clean(text), self.terminal_opts()) {
            Ok(b) => b,
            Err(e) => return format!("ERR {e}"),
        };
        let cm = render::length_cm(&bitmap);
        if cm > MAX_CM && !force {
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
                if self.printing.is_busy() {
                    return soon;
                }
                // Keep the debt until the worker really took the job, or a dying link would
                // leave the paper sticking out of the printer.
                if self.send_job(protocol::feed_only(protocol::TEAR_OFF_DOTS)) {
                    self.term.owes_feed = false;
                }
            }
            return None;
        }
        if !self.term.auto {
            return None;
        }
        if self.printing.is_busy() {
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
                // A fresh link starts clean: a print that died with the old link must not latch
                // the machine. `Done` stays, the effects still read its stamp.
                if matches!(self.printing, Printing::Failed { .. }) {
                    self.printing = Printing::Idle;
                }
            }
            Event::Disconnected => {
                self.link = Link::Offline;
                self.battery = None;
                self.info = DeviceInfo::default();
                self.rssi = None;
                // A job in flight dies with the link; a finished one keeps its effects.
                if self.printing.is_busy() {
                    self.printing = Printing::Failed { at: self.clock };
                }
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
                self.printing = Printing::Running { progress: p };
                self.status = voice::printing(p).into();
            }
            Event::PrintDone => {
                self.printing = Printing::Done { at: self.clock };
                self.status = voice::done(self.next_seed()).into();
            }
            Event::Error(e) => {
                // Only a print that was running ends here: an error while idle belongs to the link
                // (a scan that found nothing) and must not leave the Print button locked.
                if self.printing.is_busy() {
                    self.printing = Printing::Failed { at: self.clock };
                }
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

    /// Wear another colour palette, now and for the next runs.
    pub fn set_palette(&mut self, id: crate::style::PaletteId) {
        self.palette = id;
        crate::style::set_palette(id);
        // The native title bar is painted by Windows, not by Rune: tell it the new colours now,
        // or it would keep the ones it got at startup until the next run.
        #[cfg(windows)]
        crate::chrome::repaint();
        settings::update(|s| s.palette = id.id().to_string());
        self.touch();
    }

    pub fn set_effects(&mut self, on: bool) {
        self.set_effects_quiet(on);
        settings::update(|s| s.effects = Some(on));
    }

    /// The part of `set_effects` that does not touch the settings file, so the redraw contract can
    /// be tested without writing on the machine running the tests.
    fn set_effects_quiet(&mut self, on: bool) {
        self.fx_on = on;
        // The FX switch lives in the top bar, which only redraws when the app is dirty.
        self.touch();
    }

    /// Move the HTTP MCP endpoint to the port typed in the box, now and for the next runs.
    pub fn apply_mcp_port(&mut self) {
        let typed = self.mcp_port.display().0.trim().to_string();
        match typed.parse::<u16>() {
            Ok(port) if port >= 1024 => {
                crate::mcp_setup::set_http_port(port);
                settings::update(|s| s.port = Some(port));
                crate::mcp_http::rebind();
                self.status = format!("MCP endpoint moved to port {port}");
            }
            _ => self.status = "Pick a port between 1024 and 65535".into(),
        }
    }

    // ---- favorites

    pub fn is_fav_theme(&self, theme: Theme) -> bool {
        self.fav_themes.iter().any(|id| id == theme.id())
    }

    /// Star or unstar a theme, and remember it.
    pub fn toggle_fav_theme(&mut self, theme: Theme) {
        let id = theme.id();
        match self.fav_themes.iter().position(|t| t == id) {
            Some(i) => {
                self.fav_themes.remove(i);
            }
            None => self.fav_themes.push(id.to_string()),
        }
        let favs = self.fav_themes.clone();
        settings::update(|s| s.fav_themes = favs);
        self.touch();
    }

    /// The starred themes that exist right now (a deleted custom theme just drops out), in the
    /// order they were starred.
    pub fn favorite_themes(&self) -> Vec<Theme> {
        self.fav_themes.iter().filter_map(|id| Theme::from_id(id)).collect()
    }

    /// Star or unstar a picture, and remember it.
    pub fn toggle_fav_picture(&mut self, path: PathBuf) {
        self.gallery.toggle_favorite(path);
        let favs = self.gallery.favorites.clone();
        settings::update(|s| s.fav_pictures = favs);
        self.make_thumbs();
        self.touch();
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
        // The switch and its "Watching" line live in the drawer, which only redraws when dirty.
        self.touch();
    }

    /// A thumbnail was clicked: show that picture, ready to print.
    pub fn pick_from_gallery(&mut self, path: PathBuf) {
        self.mode = Mode::Image;
        self.gallery.selected = Some(path.clone());
        self.choose_picture(path);
    }

    /// Pictures dropped on the window are shown in the Image tab, ready to print: a drop never
    /// prints by itself. Only the watched folder (with "Print new pictures automatically") does.
    pub fn drop_files(&mut self, files: &[PathBuf]) {
        let pictures: Vec<PathBuf> = files.iter().filter(|f| gallery::is_image(f)).cloned().collect();
        if pictures.is_empty() {
            self.error = Some("That is not a picture I can print (png, jpg, bmp, gif or webp).".into());
            return;
        }
        let count = pictures.len();
        self.pick_from_gallery(pictures[0].clone());
        self.status = if count == 1 {
            "Ready: press Print (Ctrl+P) when it looks right.".into()
        } else {
            format!("{count} pictures dropped; showing the first. Press Print (Ctrl+P) when it looks right.")
        };
    }

    // ---- dragging a gallery picture onto the printer

    /// A press on a thumbnail starts a gesture: a short one is a click, a long one a drag.
    pub fn drag_start(&mut self, path: PathBuf) {
        self.drag = Some(ThumbDrag { path, theme: None, travel: 0.0 });
    }

    /// A press on a theme tile: a click picks the theme, a pull toward the printer prints with it.
    pub fn theme_drag_start(&mut self, theme: Theme) {
        self.drag = Some(ThumbDrag { path: PathBuf::new(), theme: Some(theme), travel: 0.0 });
    }

    pub fn drag_move(&mut self, dx: f32, dy: f32) {
        if let Some(d) = &mut self.drag {
            d.travel = dx.hypot(dy);
        }
    }

    /// The thumbnail was let go. A click picks the picture; a drag released over the preview
    /// (where the printer is) shows it there, ready to print; anywhere else, or cancelled, it does
    /// nothing. Dropping never prints by itself.
    pub fn drag_end(&mut self, cancelled: bool) {
        let Some(d) = self.drag.take() else { return };
        let over = std::mem::take(&mut self.over_stage);
        self.pointer = None;
        if cancelled {
            return;
        }
        if let Some(theme) = d.theme {
            if d.travel < DRAG_MIN {
                self.set_theme(theme);
            } else if over {
                // Let go over the printer: your text, dressed in that theme, prints now.
                self.mode = Mode::Text;
                self.set_theme(theme);
                self.refresh();
                self.print();
            }
            return;
        }
        if d.travel < DRAG_MIN || over {
            self.pick_from_gallery(d.path);
            if over {
                self.status = "Ready: press Print (Ctrl+P) when it looks right.".into();
            }
        }
    }

    /// Draw the next theme that has no small picture yet (all of them when `all`). True while
    /// some are still missing.
    pub fn make_theme_thumbs(&mut self, all: bool) -> bool {
        let Some(cache) = &self.images else { return false };
        for theme in Theme::all() {
            if self.theme_thumbs.contains_key(&theme) || self.theme_bad.contains(&theme) {
                continue;
            }
            let opts = TextOpts { px: 56.0, align: render::Align::Center, ..TextOpts::default() };
            let Ok(bitmap) = render::themed(theme, theme.sample(), &render::Fields::new(), opts) else {
                self.theme_bad.insert(theme);
                continue;
            };
            let (w, h, rgba) = render::thumb_rgba(&bitmap);
            let image = cache.rgba(w, h, rgba);
            if rune::is_headless() {
                let _ = image.wait();
            }
            self.theme_thumbs.insert(theme, image);
            if !all {
                break;
            }
        }
        self.thumbs_missing()
    }

    fn thumbs_missing(&self) -> bool {
        Theme::all().iter().any(|t| !self.theme_thumbs.contains_key(t) && !self.theme_bad.contains(t))
    }

    /// Called every frame: the gallery's small pictures appear one by one, so the window opens at
    /// once. Returns when to draw the next.
    pub fn tick_theme_thumbs(&mut self) -> Option<std::time::Duration> {
        if !self.thumbs_missing() {
            return None;
        }
        let more = self.make_theme_thumbs(false);
        // No `touch()`: the new thumbnail only needs a view rebuild, not a full ticket re-render.
        more.then_some(std::time::Duration::from_millis(30))
    }

    /// Read the themes folder again: new files appear, deleted ones go, edited ones change. The
    /// picked theme stays picked if it is still there.
    pub fn reload_themes(&mut self) {
        let Some(dir) = render::custom::dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        let picked = self.theme.id();
        let was_custom = matches!(self.theme, Theme::Custom(_));
        self.theme_sig = render::custom::signature(&dir);
        self.theme_report = render::custom::load_dir(&dir);
        self.theme_thumbs.retain(|t, _| !matches!(t, Theme::Custom(_)));
        self.theme_bad.retain(|t| !matches!(t, Theme::Custom(_)));
        if matches!(self.drag.as_ref().and_then(|d| d.theme), Some(Theme::Custom(_))) {
            self.drag = None;
        }
        if was_custom {
            let back = Theme::from_id(picked).unwrap_or(Theme::Plain);
            self.theme = back;
            if back == Theme::Plain {
                self.set_theme(Theme::Plain);
            }
        }
        self.dirty = true;
        self.touch();
    }

    /// Show the themes folder in the file manager.
    pub fn open_themes_folder(&mut self) {
        let Some(dir) = render::custom::dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        if let Err(e) = crate::update::open_folder(&dir) {
            self.error = Some(format!("Could not open {}: {e}", dir.display()));
            self.touch();
        }
    }

    /// Called every frame: once in a while, look at the themes folder and reload it if a file was
    /// added, removed or changed. Returns when to look again.
    pub fn tick_custom_themes(&mut self, now: Instant) -> Option<std::time::Duration> {
        const EVERY: std::time::Duration = std::time::Duration::from_secs(2);
        let dir = render::custom::dir()?;
        if self.theme_checked.is_some_and(|at| now.duration_since(at) < EVERY) {
            return Some(EVERY);
        }
        self.theme_checked = Some(now);
        if render::custom::signature(&dir) != self.theme_sig {
            self.reload_themes();
        }
        Some(EVERY)
    }

    fn make_thumbs(&mut self) {
        let Some(cache) = &self.images else { return };
        let paths: Vec<PathBuf> = self.gallery.shown().map(|e| e.path.clone()).collect();
        for path in paths {
            let image = self.gallery.thumbs.entry(path.clone()).or_insert_with(|| cache.file(&path, 108.0, 2.0));
            if rune::is_headless() {
                // A snapshot draws one frame: let the thumbnails finish decoding first.
                let _ = image.wait();
            }
        }
    }

    /// Print the next waiting picture when the printer is free.
    fn drain_gallery_queue(&mut self) {
        if !matches!(self.link, Link::Online(_)) || self.printing.is_busy() {
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
                self.send_bitmap(&bitmap, clamp_copies(self.copies), protocol::TEAR_OFF_DOTS);
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
        if self.printing.is_busy() {
            hot = 0.3 + 0.7 * self.printing.progress();
        } else if let Some(d) = self.printing.done_at() {
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
        let mut smoke: f32 = if self.printing.is_busy() { 0.25 } else { 0.0 };
        if let Some(done) = self.printing.done_at() {
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
        // A line being typed out needs smooth frames; once it is done, the window can rest.
        let typing = self.tagline_shown().1 || (self.composed.is_err() && self.ghost_line_shown().1);
        // The ghost floats beside a ticket: a slow bob needs a frame now and then, ten a second.
        let floating = self.preview.is_some();
        if quick || typing {
            Some(Duration::from_millis(33))
        } else if floating {
            Some(Duration::from_millis(80))
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

    /// How the ghost feels, from the printer's state.
    pub fn ghost_mood(&self) -> voice::Mood {
        match self.link {
            Link::Online(_) => voice::Mood::Happy,
            Link::Connecting => voice::Mood::Curious,
            Link::Offline => voice::Mood::Asleep,
        }
    }

    /// A tap on the ghost's bubble: it says something else, and the new line types itself out.
    /// It also sends it drifting: a tap is attention, and the ghost answers by moving.
    pub fn ghost_poke(&mut self) {
        self.ghost_taps += 1;
        self.ghost_said_at = Some(self.clock);
        self.ghost_move();
    }

    /// A press on the ghost's face: remember the point of its column that was grabbed (in stage
    /// points), so it follows the hand without jumping. The next drift is put off, so it does not
    /// slip away mid-drag.
    pub fn ghost_grab(&mut self, grab: (f32, f32)) {
        self.ghost_drag = Some(grab);
        self.ghost_moved_at = self.clock;
        self.touch();
    }

    /// The hand moves: the ghost's column sits with the grabbed point under the pointer, at `at`
    /// in the stage's points from its top-left. Ignored when nothing grabbed it.
    pub fn ghost_drag_to(&mut self, at: (f32, f32)) {
        if self.ghost_drag.is_none() {
            return;
        }
        self.ghost_at = Some(at);
        self.ghost_moved_at = self.clock;
        self.touch();
    }

    /// Let go: the ghost stays where it was put until the next drift.
    pub fn ghost_release(&mut self) {
        if self.ghost_drag.take().is_some() {
            self.touch();
        }
    }

    /// The next spot round the ticket, and the clock noted so the next drift waits its turn. The
    /// drift also takes the ghost back from a place the user dragged it to.
    pub fn ghost_move(&mut self) {
        self.ghost_spot = (self.ghost_spot + 1) % GHOST_SPOTS;
        self.ghost_moved_at = self.clock;
        self.ghost_at = None;
    }

    /// The ghost drifts to another spot every `GHOST_MOVE` seconds, while effects are on. Off, it
    /// stays where it is and the clock is kept up to date, so turning effects on does not make it
    /// jump elsewhere at once. True when the spot just changed.
    pub fn tick_ghost(&mut self, clock: f64) -> bool {
        if !self.fx_on {
            self.ghost_moved_at = clock;
            return false;
        }
        if clock - self.ghost_moved_at < GHOST_MOVE {
            return false;
        }
        self.ghost_move();
        true
    }

    /// The ghost's line when there is nothing to print (and its Easter-egg lines). It changes by
    /// itself every quarter of an hour, and at every tap.
    pub fn ghost_line(&self) -> &'static str {
        if self.boo_until.is_some_and(|t| self.clock < t) {
            return "Boo!";
        }
        let turn = (self.clock / GHOST_TURN) as u64;
        voice::ghost(self.mode, self.ghost_mood(), voice::mix(self.seed ^ turn, self.ghost_taps))
    }

    /// The ghost's line as far as it has printed out, and whether the cursor is still running.
    pub fn ghost_line_shown(&self) -> (String, bool) {
        let line = self.ghost_line();
        let turn_start = (self.clock / GHOST_TURN).floor() * GHOST_TURN;
        // A tap starts typing at once; a new turn starts typing at the turn's edge. The first
        // line of the session is simply there, like the paper it sits on.
        let start = match self.ghost_said_at {
            Some(t) if t >= turn_start => t,
            _ => turn_start,
        };
        if self.ghost_said_at.is_none() && turn_start == 0.0 || !self.fx_on {
            return (line.to_string(), false);
        }
        reveal(line, self.clock - start)
    }

    /// The dry line under the wordmark: a new one each minute, typed out like the printer.
    pub fn tagline(&self) -> &'static str {
        let turn = (self.clock / TAGLINE_TURN) as u64;
        voice::tagline(voice::mix(self.seed ^ 0x7a6_11e, turn))
    }

    /// The tagline as far as it has printed out, and whether the cursor is still running.
    pub fn tagline_shown(&self) -> (String, bool) {
        let turn = (self.clock / TAGLINE_TURN) as u64;
        if turn == 0 || !self.fx_on {
            return (self.tagline().to_string(), false);
        }
        reveal(self.tagline(), self.clock % TAGLINE_TURN)
    }

    /// Note the minute the tagline is on. True when it just changed.
    pub fn tick_tagline(&mut self, clock: f64) -> bool {
        let turn = (clock / TAGLINE_TURN) as u64;
        let changed = turn != self.tagline_turn;
        self.tagline_turn = turn;
        changed
    }
}

/// The start of `text` after `elapsed` seconds of printing, and whether the cursor still runs.
/// The cursor is kept one fraction of a second past the last character so the final frame lands.
fn reveal(text: &str, elapsed: f64) -> (String, bool) {
    let total = text.chars().count();
    let shown = ((elapsed.max(0.0) * TYPE_CPS) as usize).min(total);
    let typing = shown < total || elapsed < total as f64 / TYPE_CPS + 0.12;
    (text.chars().take(shown).collect(), typing)
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
    fn an_empty_plain_ticket_is_a_blank_sheet_with_a_body_to_write_in() {
        let mut app = App::default();
        assert_eq!((app.mode, app.theme), (Mode::Text, Theme::Plain), "the startup state");
        let sheet = app.compose().expect("a blank sheet, not an error");
        assert!(sheet.height > render::TEETH, "the paper stands out of the slot");
        app.refresh();
        assert_eq!(app.composed.as_ref().unwrap().height, sheet.height, "the stage shows it");
        let body = app.regions.iter().find(|r| r.key == "body").expect("the sheet says where to write");
        assert_eq!((body.x0, body.x1), (0, protocol::WIDTH_DOTS), "the full width is writable");
        assert_eq!((body.y0, body.y1), (0, sheet.height), "and the whole sheet is");
        // Clicking the sheet starts the text box; one character brings the real rendering back.
        app.edit_region("body");
        assert_eq!(app.editing.as_deref(), Some("body"));
        assert!(app.text.focused);
        app.text = TextEdit::new("A");
        app.refresh();
        assert_eq!(app.composed.as_ref().unwrap().height, render::text("A", app.text_opts).unwrap().height);
    }

    #[test]
    fn the_blank_plain_sheet_is_still_refused_at_print_time() {
        let (mut app, mut rx) = online_app();
        app.refresh();
        assert!(app.composed.is_ok(), "the blank sheet sits on the stage");
        app.print();
        assert_eq!(app.error.as_deref(), Some("nothing to print"));
        assert_eq!(app.printing, Printing::Idle);
        assert!(rx.try_recv().is_err(), "nothing was sent to the printer");
    }

    #[test]
    fn switching_mode_changes_what_is_composed() {
        let mut app = App::default();
        app.code_text = TextEdit::new("abc");
        app.mode = Mode::Codes;
        assert!(app.compose().is_ok());
        app.mode = Mode::Image;
        let sheet = app.compose().expect("no picture chosen: a blank sheet, not an error");
        assert!(sheet.height > render::TEETH, "the paper still stands out of the slot");
    }

    #[test]
    fn an_empty_code_is_a_blank_sheet_with_a_place_to_type_it() {
        let mut app = App { mode: Mode::Codes, ..App::default() };
        let sheet = app.compose().expect("a blank sheet, not an error");
        assert!(sheet.height > render::TEETH, "the paper stands out of the slot");
        app.refresh();
        assert_eq!(app.composed.as_ref().unwrap().height, sheet.height, "the stage shows it");
        let code = app.regions.iter().find(|r| r.key == "_code").expect("the sheet says where the code goes");
        assert_eq!((code.x0, code.x1), (0, protocol::WIDTH_DOTS), "the full width is the code box");
        assert_eq!((code.y0, code.y1), (0, sheet.height), "and the whole sheet is");
        // Clicking the sheet starts the code box; the first character turns it into the real code.
        app.edit_region("_code");
        assert_eq!(app.editing.as_deref(), Some("_code"));
        assert!(app.code_text.focused);
        app.code_text = TextEdit::new("A");
        app.refresh();
        assert_eq!(app.composed.as_ref().unwrap().height, render::code("A", app.code_opts).unwrap().height);
        assert_eq!(app.editing.as_deref(), Some("_code"), "the box stays up while the code is typed");
    }

    #[test]
    fn no_picture_is_a_blank_sheet_with_nothing_to_type_on() {
        let mut app = App { mode: Mode::Image, ..App::default() };
        let sheet = app.compose().expect("a blank sheet, not an error");
        assert!(sheet.height > render::TEETH, "the paper stands out of the slot");
        app.refresh();
        assert!(app.composed.is_ok(), "the stage shows it");
        assert!(app.regions.is_empty(), "the picture comes from the panel, not the paper");
    }

    #[test]
    fn an_empty_terminal_is_a_blank_sheet_with_nothing_to_type_on() {
        let mut app = App { mode: Mode::Terminal, ..App::default() };
        let sheet = app.compose().expect("a blank sheet, not an error");
        assert!(sheet.height > render::TEETH, "the paper stands out of the slot");
        app.refresh();
        assert!(app.composed.is_ok(), "the stage shows it");
        assert!(app.regions.is_empty(), "the lines arrive from the pipe, not the paper");
    }

    #[test]
    fn the_blank_sheets_of_the_other_tabs_are_still_refused_at_print_time() {
        for mode in [Mode::Image, Mode::Codes, Mode::Terminal] {
            let (mut app, mut rx) = online_app();
            app.mode = mode;
            app.refresh();
            assert!(app.composed.is_ok(), "{mode:?}: the sheet sits on the stage");
            assert!(!app.can_print(), "{mode:?}: the button is off");
            app.print();
            assert!(app.error.is_some(), "{mode:?}: the press says why");
            assert_eq!(app.printing, Printing::Idle, "{mode:?}: a refused print leaves the lifecycle alone");
            assert!(rx.try_recv().is_err(), "{mode:?}: nothing was sent to the printer");
        }
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
        busy.printing = Printing::Sending;
        assert!(ask(&mut busy, "x", false).contains("busy"));
        let (mut app, _rx) = online_app();
        let long = "line\n".repeat(400);
        assert!(ask(&mut app, &long, false).contains("--force"));
        assert!(ask(&mut app, &long, true).starts_with("OK"));
    }

    #[test]
    fn a_print_is_refused_while_offline_and_nothing_is_queued() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App { cmd: Some(tx), ..App::default() };
        app.text = TextEdit::new("hello");
        app.refresh();
        app.print();
        assert_eq!(app.printing, Printing::Idle, "a refused print leaves the lifecycle alone");
        assert!(rx.try_recv().is_err(), "offline: no job reaches the worker");
        assert!(app.error.as_deref().is_some_and(|e| e.contains("not connected")), "{:?}", app.error);
    }

    #[test]
    fn an_empty_ticket_is_refused_at_print_time_but_a_blank_form_still_prints() {
        let (mut app, mut rx) = online_app();
        app.set_theme(Theme::Receipt);
        app.text = TextEdit::new("");
        app.refresh();
        assert!(app.composed.is_ok(), "an emptied text box keeps the ticket on screen");
        app.print();
        assert_eq!(app.error.as_deref(), Some("nothing to print"));
        assert_eq!(app.printing, Printing::Idle, "a refused print leaves the lifecycle alone");
        assert!(rx.try_recv().is_err(), "an empty ticket sends no job");

        // A silly form prints blank: it is meant to be filled in by hand.
        let (mut form, mut rx) = online_app();
        form.set_theme(Theme::Complaint);
        form.text = TextEdit::new("");
        form.refresh();
        assert!(form.composed.is_ok());
        form.print();
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))), "a blank form prints");
        assert!(form.error.is_none(), "{:?}", form.error);
    }

    #[test]
    fn a_second_print_is_refused_while_one_is_sending_or_running() {
        let (mut app, mut rx) = online_app();
        app.text = TextEdit::new("hello");
        app.refresh();
        app.print();
        assert_eq!(app.printing, Printing::Sending);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))), "the first job went out");
        app.print();
        assert_eq!(app.printing, Printing::Sending, "the refused press changed nothing");
        assert!(rx.try_recv().is_err(), "no second job queued");
        assert!(app.error.as_deref().is_some_and(|e| e.contains("busy")), "{:?}", app.error);

        app.apply(Event::Progress(0.4));
        assert_eq!(app.printing, Printing::Running { progress: 0.4 });
        app.error = None;
        app.print();
        assert!(rx.try_recv().is_err(), "still running: nothing queued either");
        assert!(app.error.as_deref().is_some_and(|e| e.contains("busy")));

        // The Terminal tab's Print goes through the same guard.
        app.mode = Mode::Terminal;
        app.term.push("line", Instant::now());
        app.error = None;
        app.print();
        assert!(rx.try_recv().is_err(), "a terminal print waits for the running job");
        assert!(app.error.as_deref().is_some_and(|e| e.contains("busy")));
    }

    #[test]
    fn a_print_walks_from_sending_to_running_to_done_with_the_clock_stamp() {
        let (mut app, mut rx) = online_app();
        app.text = TextEdit::new("hello");
        app.refresh();
        app.print();
        assert_eq!(app.printing, Printing::Sending);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        assert!(app.printing.is_busy());
        app.apply(Event::Progress(0.25));
        assert_eq!(app.printing, Printing::Running { progress: 0.25 });
        assert_eq!(app.printing.progress(), 0.25);
        app.clock = 9.5;
        app.apply(Event::PrintDone);
        assert_eq!(app.printing, Printing::Done { at: 9.5 });
        assert_eq!(app.printing.done_at(), Some(9.5));
        assert!(!app.printing.is_busy(), "the printer is free again");
    }

    #[test]
    fn a_print_that_fails_ends_in_failed() {
        let (mut app, _rx) = online_app();
        app.text = TextEdit::new("hello");
        app.refresh();
        app.print();
        app.clock = 4.0;
        app.apply(Event::Error("paper jam".into()));
        assert_eq!(app.printing, Printing::Failed { at: 4.0 });
        assert!(!app.printing.is_busy() && app.printing.done_at().is_none());
        // An error with no job running (a scan that found nothing) belongs to the link.
        app.printing = Printing::Idle;
        app.apply(Event::Error("Printer not found".into()));
        assert_eq!(app.printing, Printing::Idle, "and leaves the Print button available");
    }

    #[test]
    fn a_failed_print_does_not_block_the_next_one_once_online() {
        let (mut app, mut rx) = online_app();
        app.text = TextEdit::new("hello");
        app.refresh();
        app.print();
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        app.apply(Event::Error("paper jam".into()));
        assert!(matches!(app.printing, Printing::Failed { .. }));
        assert!(app.can_print(), "a failure does not disable the button");
        app.print();
        assert_eq!(app.printing, Printing::Sending, "the next job goes out");
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
    }

    #[test]
    fn a_link_that_drops_mid_print_starts_idle_again_when_it_comes_back() {
        let (mut app, _rx) = online_app();
        app.text = TextEdit::new("hello");
        app.refresh();
        app.print();
        app.apply(Event::Disconnected);
        assert!(matches!(app.printing, Printing::Failed { .. }), "the job died with the link");
        app.apply(Event::Connected("p".into(), None));
        assert_eq!(app.printing, Printing::Idle, "a fresh link is a fresh start");
        assert!(app.can_print());
        // A finished job keeps its stamp: the effects still die down after it.
        app.printing = Printing::Done { at: 3.0 };
        app.apply(Event::Connected("p".into(), None));
        assert_eq!(app.printing, Printing::Done { at: 3.0 });
    }

    #[test]
    fn a_print_cannot_start_while_one_is_sending_or_running() {
        let (mut app, mut rx) = online_app();
        app.text = TextEdit::new("hello");
        app.refresh();
        app.print();
        assert_eq!(app.printing, Printing::Sending);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))));
        assert!(!app.can_print(), "sending: the button is off");
        app.apply(Event::Progress(0.3));
        assert!(!app.can_print(), "running: still off");
        app.print();
        assert!(rx.try_recv().is_err(), "and nothing is queued behind the running job");
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
        app.printing = Printing::Idle;
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
        app.printing = Printing::Idle;

        app.handle_ipc(IpcMsg::Line("two".into()), t0 + term::IDLE);
        app.tick_terminal(t0 + term::IDLE * 3);
        let Ok(Cmd::Print(second)) = rx.try_recv() else { panic!("no second batch") };
        assert_eq!(feed_dots(&second), None);
        app.printing = Printing::Idle;

        app.handle_ipc(IpcMsg::Live(false), t0 + term::IDLE * 3);
        app.tick_terminal(t0 + term::IDLE * 4);
        let Ok(Cmd::Print(end)) = rx.try_recv() else { panic!("no closing feed") };
        assert_eq!(feed_dots(&end), Some(protocol::TEAR_OFF_DOTS), "one tear-off at the end");
        app.printing = Printing::Idle;
        app.tick_terminal(t0 + term::IDLE * 5);
        assert!(rx.try_recv().is_err(), "and only one");
    }

    #[test]
    fn live_terminal_opts_have_no_vertical_margin() {
        let app = App::default();
        assert_eq!(app.terminal_opts_tight().margin_y, 0.0, "a live batch must join the previous one");
        // Only the margin differs: the size and the monospace breaking stay put.
        assert_eq!(app.terminal_opts_tight(), TextOpts { margin_y: 0.0, ..app.terminal_opts() });
    }

    #[test]
    fn one_shot_terminal_opts_keep_their_margin() {
        let app = App::default();
        assert_eq!(app.terminal_opts().margin_y, 2.0, "a lone buffer gets its tear-off space");
    }

    #[test]
    fn a_tight_batch_stacks_its_lines_without_extra_blank_rows() {
        let app = App::default();
        let tall = render::text("WWWW\nWWWW\nWWWW", app.terminal_opts_tight()).unwrap();
        let one = render::text("WWWW", app.terminal_opts_tight()).unwrap();
        assert_eq!(tall.height, 3 * one.height, "no blank rows between lines or around the batch");
        // The regular opts of the same batch pay exactly the margin dots top and bottom.
        assert_eq!(
            render::text("WWWW\nWWWW\nWWWW", app.terminal_opts()).unwrap().height,
            tall.height + 4
        );
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
        app.printing = Printing::Running { progress: 0.426 };
        assert_eq!(app.window_title(), "Tronic Printer · Printing 43%");
    }

    #[test]
    fn the_flames_rise_with_the_job_die_down_after_it_and_stay_out_when_effects_are_off() {
        let mut app = App { fx_on: true, printing: Printing::Running { progress: 0.5 }, ..App::default() };
        assert!((app.flame() - 0.65).abs() < 1e-6);
        app.printing = Printing::Done { at: 10.0 };
        app.clock = 10.0;
        assert!(app.flame() > 0.5, "just after the job the flames are still up");
        app.clock = 11.0;
        assert_eq!(app.flame(), 0.0, "and a second later they are out");
        app.fx_on = false;
        app.printing = Printing::Sending;
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
        assert!(app.composed.is_ok(), "the blank sheet keeps the paper on the stage");
        assert_eq!(app.tick_interval(), None, "no preview picture without a window: nothing floats on its own");
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
    fn the_ghost_drifts_round_the_ticket_only_with_effects_on_and_jumps_at_a_tap() {
        let mut app = App::default();
        assert_eq!(app.ghost_spot, 0, "it starts to the right of the ticket");
        assert!(!app.tick_ghost(10.0) && !app.tick_ghost(30.0), "effects off: it stays put");
        assert_eq!(app.ghost_spot, 0);
        app.fx_on = true;
        app.clock = 31.0;
        assert!(!app.tick_ghost(31.0), "the clock was noted while effects were off: nothing is overdue");
        app.clock = GHOST_MOVE + 31.0;
        assert!(app.tick_ghost(app.clock), "about every half minute it drifts");
        assert_eq!(app.ghost_spot, 1);
        app.clock += 1.0;
        app.ghost_poke();
        assert_eq!(app.ghost_spot, 2, "a tap sends it elsewhere at once");
        assert!(!app.tick_ghost(app.clock + 1.0), "and the drift waits its turn again");
        app.clock = app.ghost_moved_at + GHOST_MOVE;
        assert!(app.tick_ghost(app.clock));
        assert_eq!(app.ghost_spot, 3);
        app.clock = app.ghost_moved_at + GHOST_MOVE;
        assert!(app.tick_ghost(app.clock));
        assert_eq!(app.ghost_spot, 0, "and round again");
    }

    #[test]
    fn the_ghost_follows_the_hand_and_stays_where_it_is_let_go() {
        let mut app = App { fx_on: true, ..App::default() };
        app.dirty = false;
        assert_eq!((app.ghost_drag, app.ghost_at), (None, None), "it starts on its own spots");
        // A hand on the face does not move the ghost until the pointer moves, but the view is told.
        app.ghost_grab((24.0, 150.0));
        assert_eq!(app.ghost_drag, Some((24.0, 150.0)), "the grabbed point is kept");
        assert!(app.dirty, "grabbing it rebuilds the view");
        app.dirty = false;
        app.ghost_drag_to((300.0, 120.0));
        assert_eq!(app.ghost_at, Some((300.0, 120.0)), "the column follows the hand");
        assert!(app.dirty, "moving it rebuilds the view");
        app.dirty = false;
        app.ghost_release();
        assert_eq!(app.ghost_drag, None, "the hand is off");
        assert_eq!(app.ghost_at, Some((300.0, 120.0)), "letting go keeps it where it was put");
        assert!(app.dirty, "letting go rebuilds the view");
        app.dirty = false;
        // A hand that never grabbed it cannot move it.
        app.ghost_drag_to((10.0, 10.0));
        assert_eq!(app.ghost_at, Some((300.0, 120.0)), "nothing moves the ghost but a drag");
        assert!(!app.dirty);
        // The next automatic drift takes it back to its round, so a spot is never lost for good.
        app.clock = app.ghost_moved_at + GHOST_MOVE;
        assert!(app.tick_ghost(app.clock));
        assert_eq!(app.ghost_at, None, "the drift wins again");
    }

    #[test]
    fn the_tagline_prints_out_each_minute_and_the_ghost_line_each_quarter_hour() {
        let mut app = App { fx_on: true, ..App::default() };
        assert_eq!(app.tagline_shown(), (app.tagline().to_string(), false), "the first line is already on the paper");
        app.clock = TAGLINE_TURN + 0.2;
        let (start, typing) = app.tagline_shown();
        assert!(typing && start.chars().count() < app.tagline().chars().count(), "a new line is appearing: {start:?}");
        app.clock = TAGLINE_TURN + 3.0;
        assert_eq!(app.tagline_shown(), (app.tagline().to_string(), false), "and then it is whole");
        assert!(app.tick_tagline(app.clock) && !app.tick_tagline(app.clock), "the turn is noticed once");

        let mut ghost = App { fx_on: true, ..App::default() };
        ghost.refresh();
        assert!(!ghost.ghost_line_shown().1, "the first ghost line sits still");
        ghost.clock = GHOST_TURN + 0.2;
        assert!(ghost.ghost_line_shown().1, "a new ghost line prints itself out");
        ghost.clock = GHOST_TURN + 5.0;
        assert!(!ghost.ghost_line_shown().1, "then it stays there for fifteen minutes");
        ghost.ghost_poke();
        assert!(ghost.ghost_line_shown().1, "a tap starts another line at once");
    }

    #[test]
    fn a_typed_line_keeps_its_last_frame() {
        let (shown, typing) = reveal("abc", 0.0);
        assert_eq!(shown, "");
        assert!(typing, "the cursor runs before the first character");
        let done = 3.0 / TYPE_CPS;
        let (shown, typing) = reveal("abc", done);
        assert_eq!(shown, "abc");
        assert!(typing, "one last frame shows the whole line with its cursor");
        assert!(!reveal("abc", done + 0.2).1, "then the cursor stops and the window can rest");
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
        app.edit_for("store").replace_text("MY SHOP");
        assert_eq!(app.theme_fields().get("store").map(String::as_str), Some("MY SHOP"));
        app.text = TextEdit::new("Mine 1.00");
        app.set_theme(Theme::Diner);
        assert_eq!(app.text.display().0, "Mine 1.00", "your own text is kept");
        assert_eq!(app.theme_fields().get("place").map(String::as_str), Some(""), "the form starts empty");
        app.set_theme(Theme::Plain);
        assert_eq!(app.compose().unwrap().height, render::text("Mine 1.00", app.text_opts).unwrap().height);
    }

    #[test]
    fn a_theme_tile_clicked_picks_the_theme_and_pulled_onto_the_printer_prints_with_it() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App { cmd: Some(tx), link: Link::Online("p".into()), ..App::default() };
        app.theme_drag_start(Theme::Fortune);
        app.drag_end(false);
        assert_eq!(app.theme, Theme::Fortune, "a click picks it");
        assert!(rx.try_recv().is_err(), "and prints nothing");
        app.theme_drag_start(Theme::Receipt);
        app.drag_move(80.0, 0.0);
        app.over_stage = true;
        app.drag_end(false);
        assert_eq!(app.theme, Theme::Receipt);
        assert!(matches!(rx.try_recv(), Ok(Cmd::Print(_))), "let go over the printer, it prints");
        app.printing = Printing::Idle;
        app.theme_drag_start(Theme::Diner);
        app.drag_move(80.0, 0.0);
        app.drag_end(false);
        assert_eq!(app.theme, Theme::Receipt, "let go anywhere else, nothing happens");
    }

    #[test]
    fn without_an_image_cache_there_are_no_gallery_pictures_and_no_ticking() {
        let mut app = App::default();
        assert!(app.tick_theme_thumbs().is_none());
        assert!(app.theme_thumbs.is_empty());
    }

    #[test]
    fn a_silly_form_can_be_shuffled_blanked_and_lose_its_closing_line() {
        let mut app = App::default();
        app.set_theme(Theme::Complaint);
        let first = app.compose().unwrap();
        app.shuffle_form();
        let shuffled = app.compose().unwrap();
        assert_ne!(shuffled, first, "other wording after a shuffle");
        app.closing_line = false;
        assert!(app.compose().unwrap().height < shuffled.height, "no closing line");
        app.blank_form();
        assert!(app.text.display().0.is_empty() && app.compose().is_ok(), "a blank sheet still prints");
        app.set_theme(Theme::Diner);
        assert_eq!(app.form_shuffle, 0);
    }

    #[test]
    fn writing_on_the_preview_edits_the_form_and_never_changes_what_is_printed() {
        let mut app = App::default();
        app.set_theme(Theme::Complaint);
        app.refresh();
        assert!(app.regions.iter().any(|r| r.key == "name"), "the form says where it can be written on");
        let clean = app.composed.clone().unwrap();
        app.edit_region("name");
        app.edit_for("name").replace_text("Alex");
        app.edit_for("name").focused = true;
        app.refresh();
        assert_eq!(app.editing.as_deref(), Some("name"));
        assert_ne!(app.composed.clone().unwrap(), clean, "the typed name is in the ticket");
        let with_name = app.composed.clone().unwrap();
        app.edit_region("body");
        assert!(!app.edit_for("name").focused && app.text.focused, "focus moves with the region");
        app.refresh();
        assert_eq!(app.composed.clone().unwrap(), with_name, "what prints is the real ticket, not the one with a blank part");
        app.stop_editing();
        assert!(app.editing.is_none() && !app.text.focused);
        app.set_theme(Theme::Receipt);
        app.refresh();
        assert!(!app.regions.is_empty() && app.editing.is_none());
    }

    #[test]
    fn every_part_of_every_theme_can_be_written_on_in_the_preview_or_in_the_bar() {
        for theme in Theme::all() {
            let mut app = App::default();
            app.set_theme(theme);
            app.text = TextEdit::new("First text");
            app.refresh();
            let before = app.composed.clone().unwrap_or_else(|e| panic!("{}: {e}", theme.id()));
            let mut keys: Vec<&str> = vec!["body"];
            keys.extend(theme.fields().iter().map(|f| f.key));
            let unmapped = app.unmapped();
            for key in &keys {
                // A part is written on where it is drawn, or, when the theme cannot say where, in the bar.
                let on_paper = app.regions.iter().any(|r| r.key == *key);
                assert!(on_paper || unmapped.iter().any(|(k, _)| k == key), "{}: {key} has nowhere to be written", theme.id());
                assert!(!(on_paper && unmapped.iter().any(|(k, _)| k == key)), "{}: {key} is in both", theme.id());
                app.edit_region(key);
                assert_eq!(app.editing.as_deref(), Some(*key), "{}", theme.id());
                app.refresh();
                assert_eq!(app.editing.as_deref(), Some(*key), "{}: a refresh keeps the editing open", theme.id());
            }
            app.region_edit_mut("body").replace_text("Second text, a little longer");
            assert_eq!(app.text.display().0, "Second text, a little longer", "{}", theme.id());
            for f in theme.fields() {
                let want: *const TextEdit = app.theme_edits.get(f.key).unwrap_or_else(|| panic!("{}: no box for {}", theme.id(), f.key));
                let got: *const TextEdit = app.region_edit_mut(f.key);
                assert!(std::ptr::eq(want, got), "{}: field {} reaches its own box", theme.id(), f.key);
            }
            app.refresh();
            assert_ne!(app.composed.clone().unwrap(), before, "{}: what is typed changes the ticket", theme.id());
            app.stop_editing();
            assert!(app.editing.is_none(), "{}", theme.id());
        }
    }

    #[test]
    fn writing_on_the_preview_goes_with_the_text_tab_and_the_theme() {
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        app.refresh();
        assert!(app.regions.iter().any(|r| r.key == "store"), "a receipt says where its store name is");
        app.edit_region("store");
        app.mode = Mode::Codes;
        app.refresh();
        assert!(app.editing.is_none(), "leaving the Text tab ends the writing");
        assert!(!app.regions.iter().any(|r| r.key == "store"), "and drops the form's parts");
        assert!(app.regions.iter().all(|r| r.key == "_code"), "the blank Codes sheet has its own box");
        app.mode = Mode::Text;
        app.edit_region("body");
        app.set_theme(Theme::Fortune);
        assert!(app.editing.is_none(), "so does another theme");
    }

    #[test]
    fn each_field_keeps_its_own_value_when_another_is_written_next() {
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        app.edit_for("store").replace_text("MY SHOP");
        app.edit_for("address").replace_text("12 Rue du Pain");
        app.refresh();
        app.edit_for("date").replace_text("yesterday");
        let fields = app.theme_fields();
        assert_eq!(fields.get("store").map(String::as_str), Some("MY SHOP"));
        assert_eq!(fields.get("address").map(String::as_str), Some("12 Rue du Pain"));
        assert_eq!(fields.get("date").map(String::as_str), Some("yesterday"));
    }

    #[test]
    fn a_theme_swap_under_the_form_does_not_move_a_value_into_another_key() {
        // `reload_themes` puts another theme under the form without `set_theme` (it assigns
        // `self.theme` when a custom theme was edited or removed). Boxes are keyed, so the store
        // name cannot land in the first box of whatever theme comes back.
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        app.edit_for("store").replace_text("MY SHOP");
        app.theme = Theme::Diner;
        let fields = app.theme_fields();
        assert_eq!(fields.get("place").map(String::as_str), Some(""), "the new form starts empty");
        assert_eq!(fields.get("table").map(String::as_str), Some(""));
        assert!(!fields.values().any(|v| v == "MY SHOP"), "the value is in no field of the new theme");
        let kept = app.theme_edits.get("store").map(|e| e.display().0.clone()).unwrap_or_default();
        assert_eq!(kept, "MY SHOP", "it stayed on its own key");
        // The old key is not part of the new theme: asking for it starts nothing.
        app.edit_region("store");
        assert!(app.editing.is_none(), "a key the theme no longer has cannot be written on");
        assert!(!app.text.focused, "and it does not take over the text box");
    }

    #[test]
    fn a_new_theme_starts_with_empty_boxes() {
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        app.edit_for("store").replace_text("MY SHOP");
        app.edit_for("date").replace_text("yesterday");
        app.set_theme(Theme::Receipt);
        for f in Theme::Receipt.fields() {
            assert_eq!(app.theme_fields().get(f.key).map(String::as_str), Some(""), "{} starts empty", f.key);
        }
        assert!(app.theme_edits.values().all(|e| e.display().0.is_empty()), "no box keeps an old value");
    }

    #[test]
    fn a_part_the_theme_no_longer_has_does_not_write_into_the_body() {
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        app.refresh();
        app.text.replace_text("keep me");
        assert!(!app.regions.iter().any(|r| r.key == "gone"), "not on the paper");
        assert!(!app.unmapped().iter().any(|(k, _)| *k == "gone"), "not in the bar either");
        app.edit_region("gone");
        assert!(app.editing.is_none(), "a key no theme has starts no writing");
        assert!(!app.text.focused && app.text.display().0 == "keep me", "and leaves the body box alone");
    }

    #[test]
    fn an_unchanged_ticket_keeps_its_picture_and_an_idle_window_asks_for_no_redraw() {
        let mut app = App::default();
        app.set_theme(Theme::Receipt);
        app.refresh();
        let first = app.shown.clone();
        assert!(first.is_some());
        app.touch();
        app.refresh();
        assert_eq!(app.shown, first);
        assert_eq!(app.tick_interval(), None);
    }

    /// `cargo test --release refresh_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn refresh_cost() {
        for theme in [Theme::Plain, Theme::Receipt, Theme::Complaint, Theme::Parchment] {
            let mut app = App::default();
            app.set_theme(theme);
            app.refresh();
            let t = std::time::Instant::now();
            for i in 0..20 {
                app.text = TextEdit::new(format!("Line {i} of the text
and another one"));
                app.refresh();
            }
            println!("{}: {:.1} ms per refresh", theme.id(), t.elapsed().as_secs_f64() * 1000.0 / 20.0);
        }
    }

    #[test]
    fn ctrl_or_cmd_p_is_the_print_button() {
        use rune::{Key, Mods};
        let chord = |c: char, mods: Mods| KeyEvent { key: Key::Command(c), mods };
        let cmd = Mods { cmd: true, ..Mods::default() };
        assert!(App::is_print_shortcut(&chord('p', cmd)));
        assert!(App::is_print_shortcut(&chord('P', cmd)), "either case");
        assert!(!App::is_print_shortcut(&chord('p', Mods { cmd: true, shift: true, ..Mods::default() })), "Shift+Ctrl+P is not Print");
        assert!(!App::is_print_shortcut(&chord('o', cmd)));
        assert!(!App::is_print_shortcut(&KeyEvent { key: Key::Text("p".into()), mods: Mods::default() }), "typing p prints nothing");
        // It does what the button does: offline, the same refusal and no crash.
        let mut app = App::default();
        app.text = TextEdit::new("hello");
        app.refresh();
        assert!(app.key_shortcut(&chord('p', cmd)));
        assert!(app.press_at.is_some(), "the button's own press happened");
        assert!(!app.key_shortcut(&chord('x', cmd)));
    }

    #[test]
    fn drawer_fx_and_escape_controls_all_ask_for_a_redraw() {
        // `poll` rebuilds the view only when `dirty` is set, so a drawer control that just flips
        // the flag is a dead button: opening it changed nothing on screen.
        let mut app = App::default();
        app.dirty = false;
        app.toggle_drawer();
        assert!(app.drawer_open);
        assert!(app.dirty, "opening the drawer must mark the app dirty");
        app.dirty = false;
        app.toggle_drawer();
        assert!(!app.drawer_open);
        assert!(app.dirty, "closing it from the button must mark the app dirty too");

        app.toggle_drawer();
        assert!(app.drawer_open);
        app.dirty = false;
        let esc = KeyEvent { key: rune::Key::Escape, mods: rune::Mods::default() };
        assert!(app.key_shortcut(&esc), "Escape is the app's");
        assert!(!app.drawer_open);
        assert!(app.dirty, "Escape must mark the app dirty");

        // The FX switch in the top bar had the same dead-control shape.
        app.dirty = false;
        let fx = app.fx_on;
        app.set_effects_quiet(!fx);
        assert_eq!(app.fx_on, !fx);
        assert!(app.dirty, "flipping FX must mark the app dirty");
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
        let mut app = App { clock: 12.5, printing: Printing::Running { progress: 0.5 }, ..App::default() };
        app.apply(Event::PrintDone);
        assert_eq!(app.printing.done_at(), Some(12.5));
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
        app.printing = Printing::Sending;
        app.tick_gallery(t0 + gallery::SCAN_EVERY * 3);
        assert!(rx.try_recv().is_err(), "and while it is busy");
        app.printing = Printing::Idle;
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
    fn a_dropped_picture_is_shown_ready_to_print_never_printed_by_itself() {
        let (mut app, mut rx, dir) = gallery_app("drop", false, false);
        let pic = dir.join("dropped.png");
        write_png(&pic);
        app.drop_files(&[dir.join("notes.txt"), pic.clone()]);
        assert_eq!(app.mode, Mode::Image);
        assert_eq!(app.source.as_ref().map(|s| s.name.as_str()), Some("dropped.png"));
        assert!(rx.try_recv().is_err(), "a drop only shows the picture");

        app.drop_files(&[dir.join("only.txt")]);
        assert!(app.error.as_deref().is_some_and(|e| e.contains("not a picture")));

        app.link = Link::Online("p".into());
        app.drop_files(&[pic]);
        assert!(rx.try_recv().is_err(), "even connected, a drop waits for Print");
        assert_eq!(app.source.as_ref().map(|s| s.name.as_str()), Some("dropped.png"));
        assert!(app.status.contains("Print"), "the status says what to do next: {}", app.status);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flames_burn_while_printing_then_give_way_to_smoke_that_clears() {
        let mut app = App { fx_on: true, printing: Printing::Running { progress: 0.5 }, clock: 10.0, ..App::default() };
        assert!(app.flame() > 0.5 && app.smoke() > 0.0 && app.smoke() < 0.5, "printing: flames and a light haze");
        app.printing = Printing::Done { at: 10.0 };
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
    fn a_short_press_on_a_thumbnail_picks_it_and_a_pull_onto_the_preview_shows_it() {
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
        assert!(rx.try_recv().is_err(), "let go over the printer: it is shown, not printed");
        assert_eq!(app.source.as_ref().map(|s| s.name.as_str()), Some("dog.png"));
        assert!(app.status.contains("Print"), "the status says what to do next: {}", app.status);
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
        assert!(app.printing.is_busy());
    }

    #[test]
    fn cropped_preview_keeps_clickable_regions() {
        let mut app = App::default();
        app.text = TextEdit::new("line\n".repeat(400));
        app.refresh();
        println!("CROPTEST height={} cropped={} crop={} regions={}", app.composed.as_ref().unwrap().height, app.preview_cropped, app.preview_crop, app.regions.len());
        assert!(app.preview_cropped, "long ticket is cropped for the preview");
        assert!(!app.regions.is_empty(), "cropped tail still has in-place regions");
        for r in &app.regions {
            assert!(r.y1 <= 8000, "region outside the visible tail: {r:?}");
        }
    }
}
