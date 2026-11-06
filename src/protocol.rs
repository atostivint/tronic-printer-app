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

/// Advertised name prefix.
pub const NAME: &str = "Mini Pocket Printer";

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

/// A 1-bit image, 384 dots wide, as the printer wants it: MSB first, a set bit is a black dot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub height: usize,
    pub data: Vec<u8>,
}

impl Bitmap {
    pub fn blank(height: usize) -> Bitmap {
        Bitmap { height, data: vec![0; height * WIDTH_BYTES] }
    }

    pub fn set(&mut self, x: usize, y: usize) {
        if x < WIDTH_DOTS && y < self.height {
            self.data[y * WIDTH_BYTES + x / 8] |= 0x80 >> (x % 8);
        }
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        x < WIDTH_DOTS && y < self.height && self.data[y * WIDTH_BYTES + x / 8] & (0x80 >> (x % 8)) != 0
    }
}

/// The commands of one print job, in order. Each entry is sent as one logical command; the
/// transport splits it into [`PACKET_SIZE`] packets and waits for credits before each.
#[cfg(test)]
pub fn job(bitmap: &Bitmap, density: Density) -> Vec<Vec<u8>> {
    job_with_feed(bitmap, density, TEAR_OFF_DOTS)
}

/// Paper pushed out after a normal print so the ticket clears the tear bar (80 dots, about 10 mm).
pub const TEAR_OFF_DOTS: u8 = 80;

/// Only advance the paper, as the end of a stream of live batches.
pub fn feed_only(dots: u8) -> Vec<Vec<u8>> {
    vec![ENABLE.to_vec(), WAKE.to_vec(), INIT.to_vec(), vec![0x1B, 0x4A, dots], STOP.to_vec()]
}

/// Like [`job`], but with the paper advance after the picture set by the caller; `0` adds none,
/// so consecutive batches join without a white gap.
pub fn job_with_feed(bitmap: &Bitmap, density: Density, feed_dots: u8) -> Vec<Vec<u8>> {
    let mut out = vec![ENABLE.to_vec(), WAKE.to_vec(), INIT.to_vec(), density.command().to_vec()];
    for start in (0..bitmap.height).step_by(BAND_LINES) {
        let lines = BAND_LINES.min(bitmap.height - start);
        let mut band = vec![0x1D, 0x76, 0x30, 0x00, WIDTH_BYTES as u8, 0x00, (lines & 0xFF) as u8, (lines >> 8) as u8];
        band.extend_from_slice(&bitmap.data[start * WIDTH_BYTES..(start + lines) * WIDTH_BYTES]);
        out.push(band);
    }
    if feed_dots > 0 {
        out.push(vec![0x1B, 0x4A, feed_dots]);
    }
    out.push(STOP.to_vec());
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
        assert_eq!(cmds[cmds.len() - 2], [0x1B, 0x4A, 0x50]);
        assert_eq!(cmds[cmds.len() - 1], [0x10, 0xFF, 0xF1, 0x45]);
    }

    #[test]
    fn live_batches_can_skip_the_paper_advance_and_feed_once_at_the_end() {
        let tight = job_with_feed(&Bitmap::blank(10), Density::Medium, 0);
        assert!(!tight.iter().any(|c| c.as_slice() == [0x1B, 0x4A, 0x50]));
        assert_eq!(tight.last().unwrap().as_slice(), [0x10, 0xFF, 0xF1, 0x45]);
        let feed = feed_only(TEAR_OFF_DOTS);
        assert_eq!(feed[3], [0x1B, 0x4A, 0x50]);
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
}
