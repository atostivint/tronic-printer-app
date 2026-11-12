//! Wire protocol of the Lidl Tronic / Silvercrest 5890 "Mini Pocket Printer" (firmware `V1.06LY`,
//! model `A2Y`). It was verified on a real unit; the layout follows the community write-up in
//! github.com/dimer47/PocketPrinter5890 (`docs/PROTOCOL_SPEC.md`).
//!
//! Everything goes to the write characteristic `FF02` (write without response). The printer
//! answers on `FF01` (solicited replies) and `FF03` (status and flow control):
//!
//! * `01 nn` on `FF03` grants `nn` more packets (credits, not a status).
//! * `02 bb 00` on `FF03` is a battery report, `bb` being a percentage.
//!
//! A job is an activation sequence, raster bands (`GS v 0`) and a stop command. Without the
//! activation the printer acknowledges every packet and prints nothing.

/// Printable width in dots (58 mm paper at 203 dpi).
pub const WIDTH_DOTS: usize = 384;
/// Bytes per raster line.
pub const WIDTH_BYTES: usize = WIDTH_DOTS / 8;
/// Largest payload per BLE write; 180 is what the printer takes reliably.
pub const PACKET_SIZE: usize = 180;
/// Raster lines per `GS v 0` command. One large command is dropped by the printer.
pub const BAND_LINES: usize = 24;

pub const WRITE_CHAR: u128 = 0x0000ff02_0000_1000_8000_00805f9b34fb;
pub const NOTIFY_REPLY: u128 = 0x0000ff01_0000_1000_8000_00805f9b34fb;
pub const NOTIFY_STATUS: u128 = 0x0000ff03_0000_1000_8000_00805f9b34fb;

/// Advertised name prefix of the pocket printer.
pub const NAME: &str = "Mini Pocket Printer";

/// The kinds of Tronic printers this app knows how to talk to. A device name maps to at
/// most one kind (see [`Kind::kind_of`]); anything else is not ours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pocket,
    Label,
}

/// Which kind a device name belongs to, if any. The two names Tronic puts on these
/// devices; a third will be a line here.
pub fn kind_of(name: &str) -> Option<Kind> {
    if name.starts_with("Mini Pocket Printer") {
        Some(Kind::Pocket)
    } else if name.starts_with("ML Printer") {
        Some(Kind::Label)
    } else {
        None
    }
}

const ENABLE: [u8; 4] = [0x10, 0xFF, 0xF1, 0x03];
const WAKE: [u8; 12] = [0; 12];
const INIT: [u8; 2] = [0x1B, 0x40];
const STOP: [u8; 4] = [0x10, 0xFF, 0xF1, 0x45];

/// Print darkness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Light,
    Medium,
    Dark,
}

impl Density {
    fn command(self) -> [u8; 5] {
        let n = match self {
            Density::Light => 0,
            Density::Medium => 1,
            Density::Dark => 2,
        };
        [0x10, 0xFF, 0x10, 0x00, n]
    }
}

/// A 1-bit image as the printer wants it: MSB first, a set bit is a black dot. Width is in
/// dots; rows are padded to whole bytes on the right.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Bitmap {
    /// Bytes per raster line for this bitmap (width rounded up to a multiple of 8).
    pub fn width_bytes(&self) -> usize {
        self.width.div_ceil(8)
    }

    /// A blank 384-dot-wide bitmap (the pocket printer), kept for compatibility.
    pub fn blank(height: usize) -> Bitmap {
        Bitmap::blank_at(WIDTH_DOTS, height)
    }

    /// A blank bitmap of the given width in dots. The width is rounded up to a multiple of 8 for
    /// storage so rows stay byte-aligned.
    pub fn blank_at(width: usize, height: usize) -> Bitmap {
        let width_bytes = width.div_ceil(8);
        Bitmap { width, height, data: vec![0; height * width_bytes] }
    }

    pub fn set(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            let wb = self.width_bytes();
            self.data[y * wb + x / 8] |= 0x80 >> (x % 8);
        }
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        if x < self.width && y < self.height {
            let wb = self.width_bytes();
            self.data[y * wb + x / 8] & (0x80 >> (x % 8)) != 0
        } else {
            false
        }
    }
}

/// The commands of one print job, in order. Each entry is sent as one logical command; the
/// transport splits it into [`PACKET_SIZE`] packets and waits for credits before each.
#[cfg(test)]
pub fn job(bitmap: &Bitmap, density: Density) -> Vec<Vec<u8>> {
    job_with_feed(bitmap, density, TEAR_OFF_DOTS)
}

/// Paper pushed out after a normal print so the ticket clears the tear bar (80 dots, about 10 mm).
/// 60 was tried and tore the last rows of a ticket off inside the printer.
pub const TEAR_OFF_DOTS: u8 = 80;

/// Blank dots kept above and below the ink of a ticket (about 0.5 mm).
pub const EDGE_DOTS: usize = 4;

impl Bitmap {
    /// The bitmap without the blank rows at its top and bottom beyond `keep` of each, so a ticket
    /// does not burn paper on white. A wholly blank bitmap is left as it is.
    pub fn trimmed(&self, keep: usize) -> Bitmap {
        let wb = self.width_bytes();
        let blank = |y: usize| self.data[y * wb..(y + 1) * wb].iter().all(|b| *b == 0);
        let Some(first) = (0..self.height).find(|y| !blank(*y)) else { return self.clone() };
        let last = (0..self.height).rev().find(|y| !blank(*y)).unwrap_or(first);
        let (y0, y1) = (first.saturating_sub(keep), (last + 1 + keep).min(self.height));
        Bitmap { width: self.width, height: y1 - y0, data: self.data[y0 * wb..y1 * wb].to_vec() }
    }
}

/// Only advance the paper, as the end of a stream of live batches.
pub fn feed_only(dots: u8) -> Vec<Vec<u8>> {
    vec![ENABLE.to_vec(), WAKE.to_vec(), INIT.to_vec(), vec![0x1B, 0x4A, dots], STOP.to_vec()]
}

/// Like [`job`], but with the paper advance after the picture set by the caller; `0` adds none,
/// so consecutive batches join without a white gap.
pub fn job_with_feed(bitmap: &Bitmap, density: Density, feed_dots: u8) -> Vec<Vec<u8>> {
    let mut out = vec![ENABLE.to_vec(), WAKE.to_vec(), INIT.to_vec(), density.command().to_vec()];
    let wb = bitmap.width_bytes();
    for start in (0..bitmap.height).step_by(BAND_LINES) {
        let lines = BAND_LINES.min(bitmap.height - start);
        let mut band = vec![0x1D, 0x76, 0x30, 0x00, wb as u8, 0x00, (lines & 0xFF) as u8, (lines >> 8) as u8];
        band.extend_from_slice(&bitmap.data[start * wb..(start + lines) * wb]);
        out.push(band);
    }
    if feed_dots > 0 {
        out.push(vec![0x1B, 0x4A, feed_dots]);
    }
    out.push(STOP.to_vec());
    out
}

/// DP-L13 / ML Printer_BLE: activation, wake, raster bands (content then one blank band), the
/// alignment tail (`10 0C` then `1B 4A 28`), and stop last. No density command.
pub fn label_job(bitmap: &Bitmap, _density: Density) -> Vec<Vec<u8>> {
    let mut out = vec![
        vec![0x10, 0xFF, 0xF1, 0x03],
        vec![0x00; 12],
    ];
    let wb = bitmap.width_bytes();
    for start in (0..bitmap.height).step_by(BAND_LINES) {
        let lines = BAND_LINES.min(bitmap.height - start);
        let mut band = vec![0x1D, 0x76, 0x30, 0x00, wb as u8, 0x00, (lines & 0xFF) as u8, (lines >> 8) as u8];
        band.extend_from_slice(&bitmap.data[start * wb..(start + lines) * wb]);
        out.push(band);
    }
    // One blank band after the content, as measured on the real device.
    let mut blank_band = vec![0x1D, 0x76, 0x30, 0x00, wb as u8, 0x00, BAND_LINES as u8, 0x00];
    blank_band.extend(vec![0u8; BAND_LINES * wb]);
    out.push(blank_band);
    out.push(vec![0x10, 0x0C]);
    out.push(vec![0x1B, 0x4A, 0x28]);
    out.push(vec![0x10, 0xFF, 0xF1, 0x45]);
    out
}

/// Read-only queries. Each is sent inside the activation sequence and answered on `FF01`.
pub const QUERY_MODEL: [u8; 4] = [0x10, 0xFF, 0x20, 0xF0];
pub const QUERY_FIRMWARE: [u8; 4] = [0x10, 0xFF, 0x20, 0xF1];
pub const QUERY_BATTERY: [u8; 4] = [0x10, 0xFF, 0x50, 0xF1];
/// Answers on real hardware, in a format nobody has documented yet.
pub const QUERY_PAPER: [u8; 3] = [0x10, 0xFF, 0x40];

/// A query wrapped the way the printer wants it: activation, wake-up, the query, stop.
pub fn query_job(query: &[u8]) -> Vec<Vec<u8>> {
    vec![ENABLE.to_vec(), WAKE.to_vec(), query.to_vec(), STOP.to_vec()]
}

/// An ASCII reply such as the model `A2Y` or the firmware `V1.06LY`.
pub fn parse_text(reply: &[u8]) -> Option<String> {
    let text: String = reply.iter().filter(|b| (0x20..0x7f).contains(*b)).map(|b| *b as char).collect();
    let text = text.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

/// The battery reply `00 nn`, `nn` being a percentage.
pub fn parse_battery(reply: &[u8]) -> Option<u8> {
    match reply {
        [0x00, n] if *n <= 100 => Some(*n),
        _ => None,
    }
}

/// What a status notification on `FF03` means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The printer accepts this many more packets.
    Credits(u32),
    /// Battery percentage.
    Battery(u8),
    /// Anything else, including the `AA 0D 0A` service frame.
    Other,
}

pub fn parse_status(frame: &[u8]) -> Status {
    match frame {
        [0x01, n] => Status::Credits(u32::from(*n)),
        [0x02, b, 0x00] if *b <= 100 => Status::Battery(*b),
        _ => Status::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_starts_with_the_activation_and_ends_with_stop() {
        let cmds = job(&Bitmap::blank(10), Density::Medium);
        assert_eq!(cmds[0], [0x10, 0xFF, 0xF1, 0x03]);
        assert_eq!(cmds[1], [0u8; 12], "the wake-up is its own write");
        assert_eq!(cmds[3], [0x10, 0xFF, 0x10, 0x00, 0x01]);
        assert_eq!(cmds[cmds.len() - 2], [0x1B, 0x4A, TEAR_OFF_DOTS]);
        assert_eq!(cmds[cmds.len() - 1], [0x10, 0xFF, 0xF1, 0x45]);
    }

    #[test]
    fn live_batches_can_skip_the_paper_advance_and_feed_once_at_the_end() {
        let tight = job_with_feed(&Bitmap::blank(10), Density::Medium, 0);
        assert!(!tight.iter().any(|c| c.as_slice() == [0x1B, 0x4A, TEAR_OFF_DOTS]));
        assert_eq!(tight.last().unwrap().as_slice(), [0x10, 0xFF, 0xF1, 0x45]);
        let feed = feed_only(TEAR_OFF_DOTS);
        assert_eq!(feed[3], [0x1B, 0x4A, TEAR_OFF_DOTS]);
        assert_eq!(feed[0], [0x10, 0xFF, 0xF1, 0x03], "even a feed needs the activation");
    }

    #[test]
    fn the_raster_goes_out_in_bands_of_24_lines() {
        let cmds = job(&Bitmap::blank(60), Density::Dark);
        let bands: Vec<_> = cmds.iter().filter(|c| c.starts_with(&[0x1D, 0x76, 0x30])).collect();
        assert_eq!(bands.len(), 3);
        assert_eq!(&bands[0][..8], &[0x1D, 0x76, 0x30, 0x00, 0x30, 0x00, 24, 0]);
        assert_eq!(bands[0].len(), 8 + 24 * WIDTH_BYTES);
        assert_eq!(&bands[2][6..8], &[12, 0], "the last band carries the remainder");
    }

    #[test]
    fn bits_are_msb_first_and_set_means_black() {
        let mut b = Bitmap::blank(1);
        b.set(0, 0);
        b.set(9, 0);
        assert_eq!(b.data[0], 0x80);
        assert_eq!(b.data[1], 0x40);
        assert!(b.get(0, 0) && b.get(9, 0) && !b.get(1, 0));
    }

    #[test]
    fn query_replies_are_decoded() {
        assert_eq!(parse_text(b"A2Y"), Some("A2Y".into()));
        assert_eq!(parse_text(&[0x00, b'V', b'1', b'.', b'0', b'6', b'L', b'Y', 0x0D]), Some("V1.06LY".into()));
        assert_eq!(parse_text(&[0x00, 0x01]), None);
        assert_eq!(parse_battery(&[0x00, 0x62]), Some(98));
        assert_eq!(parse_battery(&[0x01, 0x04]), None);
        assert_eq!(parse_battery(&[0x00, 0xFF]), None);
        let q = query_job(&QUERY_BATTERY);
        assert_eq!(q.first().unwrap().as_slice(), [0x10, 0xFF, 0xF1, 0x03]);
        assert_eq!(q[2], QUERY_BATTERY);
        assert_eq!(q.last().unwrap().as_slice(), [0x10, 0xFF, 0xF1, 0x45]);
    }

    #[test]
    fn status_frames_are_told_apart() {
        assert_eq!(parse_status(&[0x01, 0x04]), Status::Credits(4));
        assert_eq!(parse_status(&[0x02, 0x64, 0x00]), Status::Battery(100));
        assert_eq!(parse_status(&[0xAA, 0x0D, 0x0A]), Status::Other);
    }

    #[test]
    fn printer_names_map_to_different_kinds() {
        assert_eq!(kind_of("Mini Pocket Printer_BLE"), Some(Kind::Pocket));
        assert_eq!(kind_of("ML Printer_BLE"), Some(Kind::Label));
        assert_ne!(kind_of("Mini Pocket Printer_BLE"), kind_of("ML Printer_BLE"));
    }

    #[test]
    fn an_unknown_printer_name_maps_to_none() {
        assert_eq!(kind_of("Some Other Printer"), None);
        assert_eq!(kind_of(""), None);
    }

    #[test]
    fn trimming_keeps_a_thin_edge_around_the_ink() {
        let mut b = Bitmap::blank(100);
        b.set(5, 40);
        b.set(5, 60);
        let t = b.trimmed(8);
        assert_eq!(t.height, 21 + 16);
        assert!(t.get(5, 8) && t.get(5, 28));
        assert_eq!(Bitmap::blank(30).trimmed(8).height, 30, "blank stays as it is");
        assert_eq!(b.trimmed(500), b, "never grows");
    }

    #[test]
    fn a_label_bitmap_is_112_dots_wide_with_14_byte_lines() {
        let b = Bitmap::blank_at(112, 240);
        assert_eq!(b.width, 112);
        assert_eq!(b.width_bytes(), 14);
        assert_eq!(b.data.len(), 240 * 14);
    }

    #[test]
    fn a_label_job_activates_prints_bands_and_stops() {
        let cmds = label_job(&Bitmap::blank_at(112, 240), Density::Medium);
        assert_eq!(cmds[0], [0x10, 0xFF, 0xF1, 0x03], "activation");
        assert_eq!(cmds[1], vec![0u8; 12], "wake-up is its own write");
        let bands: Vec<_> = cmds.iter().filter(|c| c.starts_with(&[0x1D, 0x76, 0x30])).collect();
        assert_eq!(bands.len(), 11, "10 content bands + 1 blank band");
        for (i, band) in bands.iter().enumerate() {
            assert_eq!(&band[..8], &[0x1D, 0x76, 0x30, 0x00, 14, 0x00, 24, 0], "band {i} header");
            assert_eq!(band.len(), 8 + 24 * 14, "band {i} length");
            if i == 10 {
                assert!(band[8..].iter().all(|b| *b == 0), "trailing band is blank");
            }
        }
        assert_eq!(cmds[cmds.len() - 3], [0x10, 0x0C]);
        assert_eq!(cmds[cmds.len() - 2], [0x1B, 0x4A, 0x28]);
        assert_eq!(cmds[cmds.len() - 1], [0x10, 0xFF, 0xF1, 0x45], "stop is last");
        assert!(!cmds.iter().any(|c| c.starts_with(&[0x10, 0xFF, 0x10, 0x00])), "no density command");
        // activation + wake + 11 bands + tail(2) + stop
        assert_eq!(cmds.len(), 2 + 11 + 2 + 1);
    }

    #[test]
    fn a_pocket_bitmap_still_uses_384_dots() {
        let b = Bitmap::blank(10);
        assert_eq!(b.width, WIDTH_DOTS);
        assert_eq!(b.width_bytes(), WIDTH_BYTES);
        assert_eq!(b.data.len(), 10 * WIDTH_BYTES);
    }

    #[test]
    fn blank_at_keeps_rows_aligned() {
        let b = Bitmap::blank_at(113, 3);
        assert_eq!(b.width, 113);
        assert_eq!(b.width_bytes(), 15, "113 dots round up to 15 bytes");
        assert_eq!(b.data.len(), 3 * 15);
    }
}
