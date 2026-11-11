//! The app's personality: every line of microcopy lives here, as plain functions of state and a
//! seed, so it can be tested and never scatters through the UI.

use crate::app::Mode;
use crate::protocol;

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

/// Why a print is refused: the picked printer is a Label, whose protocol this app does not know.
pub fn label_unprintable() -> &'static str {
    "That is a label printer; this app does not know its protocol yet."
}

/// What to do instead on a label printer: it only prints pictures, one per label.
pub fn label_pictures_only() -> &'static str {
    "Labels print pictures: open Image and choose one."
}

/// What kind of Tronic device the name says we are on, for the tooltip.
pub fn device_kind(kind: protocol::Kind) -> &'static str {
    match kind {
        protocol::Kind::Pocket => "pocket printer",
        protocol::Kind::Label => "label printer",
    }
}

/// The dry line under the wordmark, rotated every minute. Short enough for the header.
const TAGLINES: [&str; 12] = [
    "Hot paper, cold take.",
    "Now with 100% more dots.",
    "Less ink than a tweet.",
    "Your thoughts, slightly burnt.",
    "Paper thin, opinion thick.",
    "Print first, regret later.",
    "Receipts for the soul.",
    "Bluetooth, but make it cosy.",
    "The only printer you can pet.",
    "Warm, loud, occasionally jammed.",
    "No cloud. Just paper.",
    "A very small printing press.",
];

pub fn tagline(seed: u64) -> &'static str {
    pick(&TAGLINES, seed)
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

// The ghost's name is T-kay, like a ticket.
const GHOST_TEXT: [&str; 11] = [
    "T-kay here. Ready when you are.",
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

/// What a tab is for, said once when it opens. One line, short enough for the bubble; the ghost
/// goes back to its jokes when it has said it.
pub fn mode_hint(mode: Mode) -> &'static str {
    match mode {
        Mode::Text => "Pick a theme, then write straight on the paper.",
        Mode::Image => "Drop a picture on the paper, or pick one under Options.",
        Mode::Codes => "Type a link or a number, the code draws itself.",
        // There is no command box: the lines arrive from `tronic-print`, piped in once or with --live.
        Mode::Terminal => "Pipe output in: dir | tronic-print --live",
    }
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

/// What T-kay says when something went wrong: the reason in its own dry words, short enough for the
/// bubble (three lines of the label face). The banner under the stage keeps the exact reason, so
/// this line may be dry but never vague: it names what broke, and it never shouts.
///
/// The needles are the reasons the app really produces (`s.error = Some(..)` and `fail(`), narrow
/// before broad, so the first match is the honest one.
pub fn error_talk(error: &str) -> &'static str {
    let r = error.to_lowercase();
    ERROR_TALK.iter().find(|(needle, _)| r.contains(needle)).map_or(ERROR_TALK_FALLBACK, |(_, line)| *line)
}

/// When the reason is one TK has not been taught. Still says something broke, so the bubble is
/// never a shrug; the banner has the detail.
const ERROR_TALK_FALLBACK: &str = "Something broke. It wasn't me.";

const ERROR_TALK: [(&str, &str); 22] = [
    // The link: the scan, the worker, the handful of ways Bluetooth says no.
    ("not found", "The printer ghosted me. Again."),
    ("not available", "Bluetooth took the day off again."),
    ("not running", "Bluetooth took the day off again."),
    ("adapter", "Bluetooth took the day off again."),
    ("connection failed", "Bluetooth said no. Not my fault."),
    ("disconnect", "The printer wandered off again."),
    ("no credits", "The printer stopped listening to me."),
    ("not connected", "Nobody home. Press Connect."),
    ("busy", "One job at a time, please."),
    // A picked printer whose protocol this app does not know.
    ("label printer", "That one's a label printer. I don't speak it."),
    // There is nothing to send yet.
    ("nothing to print", "There's nothing to print yet."),
    ("nothing to encode", "Nothing to turn into a code."),
    ("already printed", "All of that is already on paper."),
    // Pictures, and the folders that hold them.
    ("choose a picture", "Pick a picture first."),
    ("not a picture", "That file is not a picture."),
    ("picture is empty", "That picture has nothing in it."),
    // A refused dialog is not a picture failing: checked before the broad "cannot open".
    ("file dialog", "The file picker won't open."),
    ("cannot open", "I can't open that file."),
    ("could not open", "That folder won't open."),
    // The files the app was handed: a theme that will not load, a font that is not there.
    ("theme", "That theme file needs fixing."),
    ("json", "That theme file needs fixing."),
    ("font", "That font is not on this PC."),
];

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
    fn the_taglines_are_short_and_steady_for_a_seed() {
        assert_eq!(tagline(7), tagline(7));
        let all: std::collections::HashSet<&str> = (0..40).map(tagline).collect();
        assert!(all.len() >= 10, "the header does not repeat itself too soon");
        assert!(all.iter().all(|l| !l.is_empty() && l.chars().count() <= 40), "keep it on one header line");
    }

    #[test]
    fn every_tab_gets_one_short_hint_in_the_house_tone() {
        for m in Mode::ALL {
            let hint = mode_hint(m);
            assert!(!hint.is_empty(), "{m:?}");
            assert!(hint.chars().count() <= 60, "{m:?}: too long for the bubble: {hint}");
            assert!(!hint.contains('!'), "{m:?}: no shouting: {hint}");
            assert!(!hint.to_lowercase().contains("simply"), "{m:?}: no filler: {hint}");
        }
        let all: std::collections::HashSet<&str> = Mode::ALL.iter().map(|m| mode_hint(*m)).collect();
        assert_eq!(all.len(), Mode::ALL.len(), "each tab says something of its own");
    }

    #[test]
    fn known_errors_get_a_headline_and_unknown_ones_do_not() {
        assert_eq!(error_headline("Printer not found. Switch it on, close the phone app"), Some("The printer ghosted us."));
        assert_eq!(error_headline("connection failed: timeout"), Some("Bluetooth said no."));
        assert_eq!(error_headline("the printer stopped accepting data (no credits)"), Some("The printer stopped listening."));
        assert_eq!(error_headline("something entirely new"), None);
    }

    #[test]
    fn every_reason_the_app_produces_gets_its_own_line_from_tk() {
        // The real strings, as they are set in `app.rs`, `main.rs` and the workers.
        let reasons = [
            "Printer not found. Switch it on, close the phone app, turn the phone's Bluetooth off and keep the printer close to the PC.",
            "Bluetooth is not available: no adapter",
            "no Bluetooth adapter found",
            "The Bluetooth worker is not running.",
            "connection failed: timeout",
            "the printer disconnected",
            "the printer stopped accepting data (no credits)",
            "The printer is not connected: press Connect.",
            "The printer is busy with another job, try again in a few seconds.",
            "That is a label printer; this app does not know its protocol yet.",
            "nothing to print",
            "Nothing to print",
            "nothing to encode",
            "Everything in the buffer is already printed.",
            "Choose a picture.",
            "That is not a picture I can print (png, jpg, bmp, gif or webp).",
            "the picture is empty",
            "cannot open C:\\pics\\cat.png: not a valid image",
            "Could not open C:\\themes: access denied",
            "that theme is no longer in the themes folder",
            "not valid JSON (expected value at line 1)",
            "no marker font found in C:\\fonts",
            "cannot open a file dialog while the window is hidden",
            "native file dialogs are unsupported on web",
        ];
        let talks: Vec<&str> = reasons.iter().map(|r| error_talk(r)).collect();
        // Every line in the table is reached by a real reason, and none falls through to the shrug.
        for (reason, line) in reasons.iter().zip(&talks) {
            assert_ne!(*line, ERROR_TALK_FALLBACK, "a known reason got the catch-all: {reason}");
        }
        let said: std::collections::HashSet<&str> = talks.iter().copied().collect();
        let table_lines: std::collections::HashSet<&str> = ERROR_TALK.iter().map(|(_, line)| *line).collect();
        assert_eq!(said, table_lines, "every line in the table is said by a real reason");
        // The one-liners differ where the reason differs, except where they share a whole sentence.
        assert_ne!(error_talk("nothing to print"), error_talk("Choose a picture."));
        assert_eq!(error_talk("Bluetooth is not available"), error_talk("no Bluetooth adapter found"), "one Bluetooth moment, one line");
    }

    #[test]
    fn an_unknown_reason_gets_the_catch_all_line() {
        assert_eq!(error_talk("something entirely new"), ERROR_TALK_FALLBACK);
        assert_eq!(error_talk(""), ERROR_TALK_FALLBACK);
        assert_ne!(ERROR_TALK_FALLBACK, "", "the bubble is never empty");
    }

    #[test]
    fn every_line_tk_says_fits_the_bubble_and_never_shouts() {
        let all = ERROR_TALK.iter().map(|(_, line)| *line).chain(std::iter::once(ERROR_TALK_FALLBACK));
        for line in all {
            assert!(!line.is_empty());
            assert!(line.chars().count() <= 48, "too long for the bubble: {line}");
            assert!(!line.contains('!'), "no shouting: {line}");
        }
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

    #[test]
    fn every_device_kind_has_a_name_for_the_tooltip() {
        assert_eq!(device_kind(protocol::Kind::Pocket), "pocket printer");
        assert_eq!(device_kind(protocol::Kind::Label), "label printer");
        for kind in [protocol::Kind::Pocket, protocol::Kind::Label] {
            let name = device_kind(kind);
            assert!(!name.is_empty());
            assert!(!name.ends_with('.'), "a fragment, not a sentence: {name}");
        }
    }

    #[test]
    fn a_label_printer_points_to_pictures() {
        let line = label_pictures_only();
        assert!(!line.is_empty());
        assert!(line.chars().count() <= 60, "short enough: {line}");
        assert!(line.contains("picture"), "it says what to print: {line}");
    }
}
