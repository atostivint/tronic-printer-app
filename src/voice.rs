//! The app's personality: every line of microcopy lives here, as plain functions of state and a
//! seed, so it can be tested and never scatters through the UI.

use crate::app::Mode;

fn pick<'a>(lines: &'a [&'a str], seed: u64) -> &'a str {
    lines[(seed % lines.len() as u64) as usize]
}

pub fn offline() -> &'static str {
    "Not connected"
}

pub fn connecting(seed: u64) -> &'static str {
    pick(&["Sniffing for the printer…", "Whispering to Bluetooth…", "Is it on? Is it the right one?", "Waving at the printer…"], seed)
}

/// What the printer is up to, by how far along the job is (0 to 1).
pub fn printing(progress: f32) -> &'static str {
    match progress {
        p if p < 0.15 => "Waking the thermal head…",
        p if p < 0.5 => "Burning dots into paper…",
        p if p < 0.85 => "Dots are going in hot…",
        _ => "Almost toast…",
    }
}

pub fn done(seed: u64) -> &'static str {
    pick(&["Fresh off the press.", "Hot paper, cold take.", "Printed. Rip it dramatically.", "Another one for the fridge."], seed)
}

/// How the ghost feels: it follows the printer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    /// No printer: sleepy, a little lost.
    Asleep,
    /// Looking for the printer.
    Curious,
    /// Connected.
    Happy,
}

const GHOST_TEXT: [&str; 10] = [
    "No paper yet. Just vibes.",
    "Type something. I dare you.",
    "Blank page energy.",
    "I'd print a receipt for your thoughts.",
    "Waiting for words…",
    "Paper's ready. Are you?",
    "Say something printable.",
    "Empty paper, full potential.",
    "Psst. Try Surprise me.",
    "Anything you write gets hot.",
];
const GHOST_IMAGE: [&str; 7] = [
    "Pick a picture and I'll solidify.",
    "Drop a photo here. I'll make it crunchy.",
    "Selfies welcome. Dithered with love.",
    "Cat pictures print best.",
    "Show me something pretty.",
    "Drag a picture onto the printer.",
    "Black and white, but make it art.",
];
const GHOST_CODES: [&str; 6] = [
    "Give me a link to turn into a QR.",
    "Feed me a URL.",
    "Squares with secrets, coming up.",
    "Type a link. I'll make it scannable.",
    "Barcodes: the original emojis.",
    "Wifi password? I won't tell.",
];
const GHOST_TERMINAL: [&str; 6] = [
    "Pipe something in: dir | tronic-print",
    "Try: git log | tronic-print",
    "Logs, but on paper.",
    "Your stack trace, but tangible.",
    "tail -f, but make it paper.",
    "Run: tronic-print --live -- cargo build",
];
const GHOST_ASLEEP: [&str; 5] = [
    "I can't find my printer body.",
    "Is the printer on? Blink twice.",
    "Bluetooth is playing hide and seek.",
    "Zzz… wake the printer?",
    "Lonely without a printer.",
];
const GHOST_CURIOUS: [&str; 4] = ["Sniffing for the printer…", "Waving at the printer…", "Hold on, saying hi to Bluetooth…", "I think I hear it breathing…"];

/// What the ghost paper says when there is nothing to print yet: a line for the tab, mixed with
/// some for how the printer is doing. The same seed says the same thing.
pub fn ghost(mode: Mode, mood: Mood, seed: u64) -> &'static str {
    let mut pool: Vec<&'static str> = match mode {
        Mode::Text => GHOST_TEXT.to_vec(),
        Mode::Image => GHOST_IMAGE.to_vec(),
        Mode::Codes => GHOST_CODES.to_vec(),
        Mode::Terminal => GHOST_TERMINAL.to_vec(),
    };
    match mood {
        Mood::Asleep => pool.extend(GHOST_ASLEEP),
        Mood::Curious => pool.extend(GHOST_CURIOUS),
        Mood::Happy => {}
    }
    pool[(seed % pool.len() as u64) as usize]
}

/// A joke headline for the errors people actually hit. The real reason always stays visible
/// under it, so the joke never hides what went wrong.
pub fn error_headline(raw: &str) -> Option<&'static str> {
    let r = raw.to_lowercase();
    let table: [(&str, &str); 7] = [
        ("not found", "The printer ghosted us."),
        ("bluetooth is not available", "Bluetooth took the day off."),
        ("connection failed", "Bluetooth said no."),
        ("disconnected", "The printer wandered off."),
        ("no credits", "The printer stopped listening."),
        ("not connected", "Nobody home."),
        ("busy", "One at a time, please."),
    ];
    table.iter().find(|(needle, _)| r.contains(needle)).map(|(_, headline)| *headline)
}

/// A random thing to print for fun. `mono` means it only looks right in a monospace font.
pub struct Surprise {
    pub text: &'static str,
    pub mono: bool,
}

const SURPRISES: [Surprise; 12] = [
    Surprise { text: "It works on my machine.\n\n(The machine is now\nprinting its own excuses.)", mono: false },
    Surprise { text: "There are 10 kinds of people:\nthose who read binary\nand those who don't.", mono: false },
    Surprise { text: "A thermal printer walks\ninto a bar.\n\nBartender: \"Why so\nblack and white?\"\n\nPrinter: \"I only do\nhot takes.\"", mono: false },
    Surprise { text: "Fortune:\nYou will soon be asked\nto reboot something.\nDo it.", mono: false },
    Surprise { text: "Fortune:\nThe bug is in the part\nyou were sure about.", mono: false },
    Surprise { text: "Roses are red\nPaper is white\nThis ticket cost\nzero ink tonight", mono: false },
    Surprise { text: "TODO\n[ ] drink water\n[ ] touch grass\n[x] print a receipt\n    for no reason", mono: false },
    Surprise { text: "RECEIPT\n------------------\n1x good idea     0.00\n1x bad idea      0.00\n------------------\nTOTAL: priceless", mono: true },
    Surprise { text: " /\\_/\\\n( o.o )\n > ^ <\n\nmeow.", mono: true },
    Surprise { text: "   (  )   (   )  )\n    ) (   )  (  (\n    ( )  (    ) )\n  ________________\n |  THIS IS FINE  |\n |________________|", mono: true },
    Surprise { text: "  _____\n | o o |\n |  ^  |   boo.\n |_____|\n  |   |", mono: true },
    Surprise { text: "$ sudo make me a sandwich\nOkay.\n\n(this is a printer,\nthough. have a coupon.)", mono: true },
];

pub fn surprise(seed: u64) -> &'static Surprise {
    &SURPRISES[(seed % SURPRISES.len() as u64) as usize]
}

/// A little time-seeded value for picking lines: changes every call site that mixes its own salt.
pub fn mix(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_add(0xD1B5_4A32_D192_ED03);
    x ^= x >> 29;
    x.wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_depend_only_on_the_seed_and_never_panic() {
        for seed in 0..200 {
            assert!(!connecting(seed).is_empty() && !done(seed).is_empty());
            assert!(!surprise(seed).text.is_empty());
        }
        assert_eq!(connecting(7), connecting(7));
    }

    #[test]
    fn the_status_follows_the_progress_of_the_job() {
        assert_eq!(printing(0.0), "Waking the thermal head…");
        assert_eq!(printing(0.3), "Burning dots into paper…");
        assert_eq!(printing(0.7), "Dots are going in hot…");
        assert_eq!(printing(1.0), "Almost toast…");
    }

    #[test]
    fn known_errors_get_a_headline_and_unknown_ones_do_not() {
        assert_eq!(error_headline("Printer not found. Switch it on, close the phone app"), Some("The printer ghosted us."));
        assert_eq!(error_headline("connection failed: timeout"), Some("Bluetooth said no."));
        assert_eq!(error_headline("the printer stopped accepting data (no credits)"), Some("The printer stopped listening."));
        assert_eq!(error_headline("something entirely new"), None);
    }

    #[test]
    fn every_mode_and_mood_has_ghost_lines_and_the_seed_changes_them() {
        for m in Mode::ALL {
            for mood in [Mood::Asleep, Mood::Curious, Mood::Happy] {
                let lines: std::collections::HashSet<&str> = (0..60).map(|seed| ghost(m, mood, seed)).collect();
                assert!(lines.len() >= 5 && lines.iter().all(|l| !l.is_empty()), "{m:?} {mood:?}");
            }
        }
        assert_eq!(ghost(Mode::Text, Mood::Happy, 3), ghost(Mode::Text, Mood::Happy, 3));
    }

    #[test]
    fn the_surprises_fit_on_the_paper_in_monospace() {
        // 384 dots at the Small mono size is about 46 columns; keep the art well inside it.
        for s in SURPRISES.iter().filter(|s| s.mono) {
            assert!(s.text.lines().all(|l| l.chars().count() <= 30), "too wide: {}", s.text);
        }
    }

    #[test]
    fn mixing_spreads_nearby_seeds() {
        assert_ne!(mix(1, 2), mix(1, 3));
        assert_ne!(mix(1, 2), mix(2, 2));
    }
}
