//! Writing themes: the same text, dressed as a shop receipt, a parchment scroll, a boarding pass or
//! a dot-matrix report. A theme is a font choice, a layout and a few 1-bit decorations (borders,
//! rules, stamps, barcodes, speckle). Some themes also take a few named fields (a store name, a
//! seat number); the body text is always the main content.
//!
//! Every theme returns an ordinary [`Bitmap`], so the preview, the CLI and the MCP server all go
//! through the same code as the printer.

pub mod custom;
mod banner;
mod effects;
mod forms;
mod greentext;
mod papers;
mod retro;
mod shop;
mod travel;
mod world;

use std::collections::BTreeMap;

use super::draw::{self, Rng};
use super::text::{self, Align, FontChoice, TextOpts};
use super::{CodeKind, CodeOpts};
use crate::protocol::{Bitmap, WIDTH_DOTS};

/// Named extras a theme asks for, by key (`store`, `seat`…).
pub type Fields = BTreeMap<String, String>;

/// A part of a ticket the user can write on: the field or the text it shows, and where it sits
/// on the paper, in dots from the ticket's top left corner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub key: String,
    pub x0: usize,
    pub x1: usize,
    pub y0: usize,
    pub y1: usize,
}

/// A piece of text the ticket knows the origin of: which field (or the body) it came from.
struct Binding {
    key: String,
    /// Lower case, trimmed.
    value: String,
    /// The value is only the field's hint (the field was left empty).
    hint: bool,
}

thread_local! {
    /// Where each themed [`Page`] put its writable parts; collected by [`render_mapped`].
    static REGIONS: std::cell::RefCell<Vec<Region>> = const { std::cell::RefCell::new(Vec::new()) };
    /// What the current ticket's text and fields say, to tell what each element shows.
    static BINDINGS: std::cell::RefCell<Vec<Binding>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Whether pages find their writable parts by themselves (the forms mark theirs by hand).
    static AUTO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Whether regions handed over by a theme that draws by hand (a sideways sheet) are kept.
    static MAPPING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn bind(key: &str, value: &str, hint: bool) {
    let value = value.trim().to_lowercase();
    if !value.is_empty() {
        BINDINGS.with(|b| b.borrow_mut().push(Binding { key: key.to_string(), value, hint }));
    }
}

/// Which field, or `body`, the string `drawn` on the ticket shows: its value (also in capitals, or
/// as part of a longer line), then its hint, then a piece of the text.
fn binding_of(drawn: &str) -> Option<String> {
    let d = drawn.trim().to_lowercase();
    if d.is_empty() {
        return None;
    }
    BINDINGS.with(|b| {
        let b = b.borrow();
        let long = |v: &str| v.chars().count() >= 3;
        for hint in [false, true] {
            if let Some(f) = b.iter().find(|f| f.key != "body" && f.hint == hint && (f.value == d || (long(&f.value) && d.contains(&f.value)))) {
                return Some(f.key.clone());
            }
        }
        b.iter().find(|f| f.key == "body" && d.chars().count() >= 2 && f.value.contains(&d)).map(|f| f.key.clone())
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Shop,
    Travel,
    OldWorld,
    Papers,
    Forms,
    Retro,
    /// Themes from the user's own files that do not say which group they belong to.
    Mine,
}

impl Family {
    pub const ALL: [Family; 7] = [Family::Shop, Family::Travel, Family::OldWorld, Family::Papers, Family::Forms, Family::Retro, Family::Mine];

    pub fn label(self) -> &'static str {
        match self {
            Family::Shop => "Shop & diner",
            Family::Travel => "Travel tickets",
            Family::OldWorld => "Parchment & old world",
            Family::Papers => "Fun official papers",
            Family::Forms => "Silly forms",
            Family::Retro => "Techy & retro",
            Family::Mine => "My themes",
        }
    }
}

/// What a theme asks the user for, besides the text.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    /// Shown greyed in the empty field, and used when the field is left empty.
    pub hint: &'static str,
}

const fn f(key: &'static str, label: &'static str, hint: &'static str) -> Field {
    Field { key, label, hint }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    #[default]
    Plain,
    // Shop & diner
    Receipt,
    Diner,
    Pharmacy,
    Coupon,
    Fine,
    // Parchment & old world
    Parchment,
    Decree,
    Wanted,
    Treasure,
    // Travel tickets
    Boarding,
    Flight,
    TrainOld,
    Train,
    // Fun official papers
    Movie,
    Fortune,
    Diploma,
    Horoscope,
    Fridge,
    Checklist,
    Ransom,
    NameTag,
    FontSalad,
    Tweet,
    Banner,
    Greentext,
    // Techy & retro
    Terminal,
    DotMatrix,
    Label,
    Telegram,
    PunchCard,
    Poster,
    // Silly forms
    WhileOut,
    Permission,
    Incident,
    Passive,
    Pep,
    Excuse,
    Complaint,
    Apology,
    Iou,
    Review,
    Butthurt,
    /// A theme loaded from a file in the themes folder: its place in the list of loaded ones.
    Custom(u16),
}

const NO_FIELDS: &[Field] = &[];

impl Theme {
    /// The themes built into the program; themes from files come after them in [`Theme::all`].
    pub const BUILTIN: [Theme; 43] = [
        Theme::Plain,
        Theme::Receipt,
        Theme::Diner,
        Theme::Pharmacy,
        Theme::Coupon,
        Theme::Fine,
        Theme::Parchment,
        Theme::Decree,
        Theme::Wanted,
        Theme::Treasure,
        Theme::Boarding,
        Theme::Flight,
        Theme::TrainOld,
        Theme::Train,
        Theme::Movie,
        Theme::Fortune,
        Theme::Diploma,
        Theme::Horoscope,
        Theme::Fridge,
        Theme::Checklist,
        Theme::Ransom,
        Theme::NameTag,
        Theme::FontSalad,
        Theme::Tweet,
        Theme::Banner,
        Theme::Greentext,
        Theme::Terminal,
        Theme::DotMatrix,
        Theme::Label,
        Theme::Telegram,
        Theme::PunchCard,
        Theme::Poster,
        Theme::WhileOut,
        Theme::Permission,
        Theme::Incident,
        Theme::Passive,
        Theme::Pep,
        Theme::Excuse,
        Theme::Complaint,
        Theme::Apology,
        Theme::Iou,
        Theme::Review,
        Theme::Butthurt,
    ];

    /// Every theme: the built-in ones, then those loaded from the themes folder.
    pub fn all() -> Vec<Theme> {
        Theme::BUILTIN.into_iter().chain((0..custom::count_loaded()).map(|i| Theme::Custom(i as u16))).collect()
    }

    /// The word used on the command line and in the MCP tool: `receipt`, `name-tag`…
    pub fn id(self) -> &'static str {
        match self {
            Theme::Custom(i) => custom::get(i).map_or("custom", |c| c.id),
            Theme::Plain => "plain",
            Theme::Receipt => "receipt",
            Theme::Diner => "diner",
            Theme::Pharmacy => "pharmacy",
            Theme::Coupon => "coupon",
            Theme::Fine => "fine",
            Theme::Parchment => "parchment",
            Theme::Decree => "decree",
            Theme::Wanted => "wanted",
            Theme::Treasure => "treasure",
            Theme::Boarding => "boarding",
            Theme::Flight => "flight",
            Theme::TrainOld => "train-old",
            Theme::Train => "train",
            Theme::Movie => "movie",
            Theme::Fortune => "fortune",
            Theme::Diploma => "diploma",
            Theme::Horoscope => "horoscope",
            Theme::Fridge => "fridge",
            Theme::Checklist => "checklist",
            Theme::Ransom => "ransom",
            Theme::NameTag => "name-tag",
            Theme::FontSalad => "font-salad",
            Theme::Tweet => "tweet",
            Theme::Banner => "banner",
            Theme::Greentext => "greentext",
            Theme::Terminal => "terminal",
            Theme::DotMatrix => "dot-matrix",
            Theme::Label => "label",
            Theme::Telegram => "telegram",
            Theme::PunchCard => "punch-card",
            Theme::Poster => "poster",
            Theme::WhileOut => "while-out",
            Theme::Permission => "permission",
            Theme::Incident => "incident",
            Theme::Passive => "passive",
            Theme::Pep => "pep",
            Theme::Excuse => "excuse",
            Theme::Complaint => "complaint",
            Theme::Apology => "apology",
            Theme::Iou => "iou",
            Theme::Review => "review",
            Theme::Butthurt => "butthurt",
        }
    }

    pub fn from_id(name: &str) -> Option<Theme> {
        let want = name.trim().to_ascii_lowercase().replace([' ', '_'], "-");
        Theme::all().into_iter().find(|t| t.id() == want || t.id().replace('-', "") == want.replace('-', ""))
    }

    pub fn name(self) -> &'static str {
        match self {
            Theme::Custom(i) => custom::get(i).map_or("Custom", |c| c.name),
            Theme::Plain => "Plain",
            Theme::Receipt => "Receipt",
            Theme::Diner => "Diner check",
            Theme::Pharmacy => "Pharmacy",
            Theme::Coupon => "Coupon",
            Theme::Fine => "Fine",
            Theme::Parchment => "Parchment",
            Theme::Decree => "Royal decree",
            Theme::Wanted => "Wanted",
            Theme::Treasure => "Treasure map",
            Theme::Boarding => "Boarding pass",
            Theme::Flight => "Flight ticket",
            Theme::TrainOld => "Train (classic)",
            Theme::Train => "Train e-ticket",
            Theme::Movie => "Movie ticket",
            Theme::Fortune => "Fortune",
            Theme::Diploma => "Diploma",
            Theme::Horoscope => "Horoscope",
            Theme::Fridge => "Fridge note",
            Theme::Checklist => "Checklist",
            Theme::Ransom => "Ransom note",
            Theme::NameTag => "Name tag",
            Theme::FontSalad => "Font salad",
            Theme::Tweet => "Tweet",
            Theme::Banner => "Sideways banner",
            Theme::Greentext => "Greentext",
            Theme::Terminal => "Terminal",
            Theme::DotMatrix => "Dot matrix",
            Theme::Label => "Label maker",
            Theme::Telegram => "Telegram",
            Theme::PunchCard => "Punch card",
            Theme::Poster => "Big poster",
            Theme::WhileOut => "While you were out",
            Theme::Permission => "Permission slip",
            Theme::Incident => "Incident report",
            Theme::Passive => "Passive-aggressive note",
            Theme::Pep => "Pep talk",
            Theme::Excuse => "Excuse note",
            Theme::Complaint => "Complaint form",
            Theme::Apology => "Official apology",
            Theme::Iou => "I.O.U.",
            Theme::Review => "Performance review",
            Theme::Butthurt => "Butthurt report",
        }
    }

    /// One line on what it does, for the picker and the MCP tool.
    pub fn tagline(self) -> &'static str {
        match self {
            Theme::Custom(i) => custom::get(i).map_or("A theme of your own", |c| c.tagline),
            Theme::Plain => "Your text, your font",
            Theme::Receipt => "A shop receipt: one item per line, with the price at the end",
            Theme::Diner => "A greasy-spoon guest check with a tip line",
            Theme::Pharmacy => "A prescription label with a barcode",
            Theme::Coupon => "A cut-out discount coupon",
            Theme::Fine => "An official penalty notice",
            Theme::Parchment => "An aged scroll with an ornate border and a wax seal",
            Theme::Decree => "A royal decree under a crown",
            Theme::Wanted => "A wanted poster with a reward",
            Theme::Treasure => "A treasure map with a dotted trail and an X",
            Theme::Boarding => "A boarding pass with a stub and barcode",
            Theme::Flight => "A boarding pass lying sideways, with a tear-off stub",
            Theme::TrainOld => "The classic card train ticket with its magnetic strip",
            Theme::Train => "A modern train e-ticket with a QR code",
            Theme::Movie => "A cinema ticket with notches",
            Theme::Fortune => "A fortune-cookie slip with lucky numbers",
            Theme::Diploma => "A certificate with a seal",
            Theme::Horoscope => "Your sign and a reading",
            Theme::Fridge => "A handwritten note held by a magnet",
            Theme::Checklist => "A to-do list with boxes to tick",
            Theme::Ransom => "Every letter in a different font, cut from a magazine",
            Theme::NameTag => "HELLO my name is…",
            Theme::FontSalad => "Big lines, every one in a different typeface",
            Theme::Tweet => "A post on paper, avatar and all",
            Theme::Banner => "Big letters printed sideways along the length of the paper",
            Theme::Greentext => "A thread that never ends",
            Theme::Terminal => "Command-line output with a prompt and a cursor",
            Theme::DotMatrix => "A continuous-paper report with sprocket holes",
            Theme::Label => "White-on-black embossed label strips",
            Theme::Telegram => "ALL CAPS and STOP",
            Theme::PunchCard => "Your text as holes on a punch card",
            Theme::Poster => "Every line as big as the paper allows",
            Theme::WhileOut => "A message slip with ticked boxes and a note",
            Theme::Permission => "A signed permission slip, granted",
            Theme::Incident => "An incident report with a severity meter",
            Theme::Passive => "A friendly note that is not that friendly",
            Theme::Pep => "A pep talk with five stars, no refills",
            Theme::Excuse => "An excuse note with the reasons ticked",
            Theme::Complaint => "A complaint form with an outrage meter",
            Theme::Apology => "An official apology with a sincerity level",
            Theme::Iou => "An I.O.U. payable in coffee, hugs or pizza",
            Theme::Review => "Star ratings from lines like 'Cooking 4'",
            Theme::Butthurt => "A butthurt report: your model was surely quantized",
        }
    }

    pub fn family(self) -> Option<Family> {
        Some(match self {
            Theme::Plain => return None,
            Theme::Custom(i) => custom::get(i).map_or(Family::Mine, |c| c.family),
            Theme::Receipt | Theme::Diner | Theme::Pharmacy | Theme::Coupon | Theme::Fine => Family::Shop,
            Theme::Parchment | Theme::Decree | Theme::Wanted | Theme::Treasure => Family::OldWorld,
            Theme::Boarding | Theme::Flight | Theme::TrainOld | Theme::Train => Family::Travel,
            Theme::Movie | Theme::Fortune | Theme::Diploma | Theme::Horoscope | Theme::Fridge | Theme::Checklist | Theme::Ransom | Theme::NameTag | Theme::FontSalad | Theme::Tweet | Theme::Banner | Theme::Greentext => Family::Papers,
            Theme::WhileOut | Theme::Permission | Theme::Incident | Theme::Passive | Theme::Pep | Theme::Excuse | Theme::Complaint | Theme::Apology | Theme::Iou | Theme::Review | Theme::Butthurt => Family::Forms,
            Theme::Terminal | Theme::DotMatrix | Theme::Label | Theme::Telegram | Theme::PunchCard | Theme::Poster => Family::Retro,
        })
    }

    /// The little form for this theme.
    pub fn fields(self) -> &'static [Field] {
        const RECEIPT: &[Field] = &[f("store", "Store", "FRESH MART"), f("address", "Address", "12 Rue du Pain"), f("date", "Date", "today")];
        const DINER: &[Field] = &[f("place", "Place", "Joe's Diner"), f("table", "Table", "12"), f("server", "Server", "Dolores")];
        const PHARMACY: &[Field] = &[f("patient", "Patient", "A. Person"), f("doctor", "Doctor", "Dr. Feelgood"), f("refills", "Refills", "0")];
        const COUPON: &[Field] = &[f("offer", "Offer", "-50%"), f("code", "Code", "SAVE-4U"), f("expires", "Expires", "never")];
        const FINE: &[Field] = &[f("offender", "Offender", "Citizen"), f("amount", "Amount", "42.00"), f("number", "Number", "000123")];
        const PARCHMENT: &[Field] = &[f("title", "Heading", "Hear ye!"), f("signed", "Seal letter", "")];
        const DECREE: &[Field] = &[f("title", "Title", "Royal Decree"), f("signed", "Signed", "The King")];
        const WANTED: &[Field] = &[f("name", "Name", "Bad Bart"), f("reward", "Reward", "$5,000")];
        const TREASURE: &[Field] = &[f("place", "Place", "Skull Island")];
        const BOARDING: &[Field] = &[f("from", "From", "PAR"), f("to", "To", "NYC"), f("name", "Passenger", "A. PERSON"), f("seat", "Seat", "12A")];
        const TRAIN_OLD: &[Field] = &[
            f("from", "From", "PARIS LYON"),
            f("to", "To", "MARSEILLE ST CHARLES"),
            f("date", "Date", "today"),
            f("seat", "Car / seat", "12 / 45"),
            f("operator", "Operator", "SNCF"),
            f("train", "Train no.", "random"),
            f("price", "Price", "random"),
            f("class", "Class", "2"),
        ];
        const TRAIN: &[Field] = &[
            f("from", "From", "PARIS GARE DE LYON"),
            f("to", "To", "LYON PART-DIEU"),
            f("date", "Date", "today"),
            f("seat", "Car / seat", "12 / 45"),
            f("operator", "Operator", "TGV"),
            f("train", "Train no.", "random"),
            f("price", "Price", "random"),
            f("class", "Class", "2de"),
        ];
        const MOVIE: &[Field] = &[f("room", "Room", "Room 7"), f("seat", "Seat", "H12"), f("time", "Time", "20:15")];
        const DIPLOMA: &[Field] = &[f("name", "Name", "A. Person"), f("award", "Award", "Certificate of Awesomeness"), f("signed", "Signed", "The Management")];
        const HOROSCOPE: &[Field] = &[f("sign", "Sign", "random: Leo, Coffee, Pirate… or your own"), f("symbol", "Symbol", "your own, for your own sign")];
        const CHECKLIST: &[Field] = &[f("title", "Title", "TO DO")];
        const TERMINAL: &[Field] = &[f("prompt", "Prompt", "user@printer:~$")];
        const DOTMATRIX: &[Field] = &[f("title", "Report", "DAILY REPORT")];
        const TELEGRAM: &[Field] = &[f("to", "To", "")];
        const F_BANNER: &[Field] = &[f("font", "Typeface", "mix: a different one on each line")];
        const F_TWEET: &[Field] = &[f("name", "Name", "Alex"), f("handle", "Handle", "@alex"), f("avatar", "Profile picture", "choose a picture"), f("stats", "Stats", "1.2K likes · 348 reposts · 92 quotes"), f("footer", "Footer", "printed, not posted")];
        const F_GREENTEXT: &[Field] = &[f("board", "Board", "/g/"), f("name", "Name", "Anonymous"), f("no", "Post no.", "48219631"), f("date", "Date", "today")];
        const F_WHILEOUT: &[Field] = &[f("to", "For", "Alex"), f("from", "From", "Someone important"), f("time", "Time", "today"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_PERMISSION: &[Field] = &[f("name", "Name", "Alex"), f("by", "Signed", "The Management"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_INCIDENT: &[Field] = &[f("by", "Reported by", "Alex"), f("place", "Location", "The kitchen"), f("date", "Date", "today"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_PASSIVE: &[Field] = &[f("to", "Dear", "Roommate"), f("from", "Signed", "A friend"), f("footer", "Closing line", "random (off = none)")];
        const F_PEP: &[Field] = &[f("name", "For", "You"), f("footer", "Closing line", "random (off = none)")];
        const F_EXCUSE: &[Field] = &[f("who", "Excuse", "Alex"), f("by", "Signed", "A concerned parent"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_COMPLAINT: &[Field] = &[f("name", "Complainer", "Alex"), f("dept", "Sent to", "Dept. of Sighs"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_APOLOGY: &[Field] = &[f("to", "To", "you"), f("from", "Signed", "Me"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_IOU: &[Field] = &[f("from", "From", "Me"), f("to", "To", "You"), f("due", "Due", "someday"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_REVIEW: &[Field] = &[f("name", "Employee", "Alex"), f("period", "Period", "this week"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        const F_BUTTHURT: &[Field] = &[f("user", "Complainer", "u/xX_prompt_Xx"), f("provider", "Provider", "their servers"), f("model", "Model", "the-latest-one"), f("boxes", "Boxes", "ticked? start with x:  x Yes | No | Maybe"), f("footer", "Closing line", "random (off = none)")];
        match self {
            Theme::Custom(i) => custom::get(i).map_or(NO_FIELDS, |c| c.fields),
            Theme::Receipt => RECEIPT,
            Theme::Diner => DINER,
            Theme::Pharmacy => PHARMACY,
            Theme::Coupon => COUPON,
            Theme::Fine => FINE,
            Theme::Parchment => PARCHMENT,
            Theme::Decree => DECREE,
            Theme::Wanted => WANTED,
            Theme::Treasure => TREASURE,
            Theme::Boarding | Theme::Flight => BOARDING,
            Theme::TrainOld => TRAIN_OLD,
            Theme::Train => TRAIN,
            Theme::Movie => MOVIE,
            Theme::Diploma => DIPLOMA,
            Theme::Horoscope => HOROSCOPE,
            Theme::Checklist => CHECKLIST,
            Theme::Terminal => TERMINAL,
            Theme::DotMatrix => DOTMATRIX,
            Theme::Telegram => TELEGRAM,
            Theme::Tweet => F_TWEET,
            Theme::Banner => F_BANNER,
            Theme::Greentext => F_GREENTEXT,
            Theme::WhileOut => F_WHILEOUT,
            Theme::Permission => F_PERMISSION,
            Theme::Incident => F_INCIDENT,
            Theme::Passive => F_PASSIVE,
            Theme::Pep => F_PEP,
            Theme::Excuse => F_EXCUSE,
            Theme::Complaint => F_COMPLAINT,
            Theme::Apology => F_APOLOGY,
            Theme::Iou => F_IOU,
            Theme::Review => F_REVIEW,
            Theme::Butthurt => F_BUTTHURT,
            _ => NO_FIELDS,
        }
    }

    /// Example text that shows the theme off; also what the preview uses when the box is empty.
    pub fn sample(self) -> &'static str {
        match self {
            Theme::Custom(i) => custom::get(i).map_or("Hello, world!", |c| c.sample),
            Theme::Plain => "Hello,\nworld!",
            Theme::Receipt => "Coffee 2.50\nCroissant 1.80\n2 x Orange juice 6.40\nBaguette 1.10\nBirthday cake 14.90",
            Theme::Diner => "Cheeseburger 9.50\nFries 3.00\nMilkshake 4.50\nApple pie 3.75",
            Theme::Pharmacy => "Take one tablet twice daily with food. Do not operate heavy machinery or argue on the internet.",
            Theme::Coupon => "On your next cup of coffee. One per customer, while the mugs last.",
            Theme::Fine => "Excessive enthusiasm in a quiet zone",
            Theme::Parchment => "Be it known to all who read this scroll that the coffee machine is, from this day, out of order. The realm shall endure.",
            Theme::Decree => "By order of the crown, every Friday shall henceforth be a half day, and all cakes shall be shared in equal parts.",
            Theme::Wanted => "Stole the last biscuit from the office kitchen. Last seen heading for the coffee machine. Armed with crumbs.",
            Theme::Treasure => "Ten paces from the old oak\nTurn left at the sleeping dragon\nDig where the shadow ends",
            Theme::Boarding | Theme::Flight => "Have a safe flight and don't forget your towel.",
            Theme::TrainOld | Theme::Train => "Alex Dupont",
            Theme::Movie => "THE PRINTER STRIKES BACK",
            Theme::Fortune => "A pleasant surprise is waiting for you in the paper tray.",
            Theme::Diploma => "For outstanding dedication to printing things that nobody asked for.",
            Theme::Horoscope => "Mercury is in a good mood. Expect a small win before lunch and an unread message you will enjoy. Avoid decisions about furniture.",
            Theme::Fridge => "Don't forget:\n- milk\n- call grandma\n- water the plant\nBack at 6, love you!",
            Theme::Checklist => "Buy milk\nCall the plumber\nx Print the tickets\nWater the plants",
            Theme::Ransom => "WE HAVE YOUR COFFEE\nLEAVE 5 EUROS\nIN THE JAR",
            Theme::NameTag => "Alex",
            Theme::FontSalad => "MAKE\nIT\nLOUD",
            Theme::Tweet => "the printer asked me for a firmware update,\nso I printed its last words.\n\n#thermal #paper",
            Theme::Banner => "BIG\nBANNER",
            Theme::Greentext => ">be me\n>writes a long post\n>printer has 384 dots\n>>48219631\n>no colour at all\n>prints the thread anyway",
            Theme::Terminal => "cargo build --release\n   Compiling tronic-printer v0.1.0\n    Finished release in 42.0s",
            Theme::DotMatrix => "Printer connected\nPaper loaded\nBattery 87 percent\nAll systems nominal",
            Theme::Label => "KITCHEN\nSPARE KEYS\nDO NOT TOUCH",
            Theme::Telegram => "Arrived safely. Weather lovely. Send more coffee.",
            Theme::PunchCard => "HELLO WORLD 1964",
            Theme::Poster => "BIG\nNEWS\nTODAY",
            Theme::WhileOut => "Your cat called. It says the bowl is emotionally empty. Please advise.",
            Theme::Permission => "do absolutely nothing today, guilt-free",
            Theme::Incident => "The last biscuit vanished around 3pm. Crumbs were found at the scene. The suspect denies everything.",
            Theme::Passive => "The dishes are still in the sink.
It has been 4 days.
No pressure!",
            Theme::Pep => "You got this. Seriously. The spreadsheet is more afraid of you than you are of it.",
            Theme::Excuse => "the 9am meeting",
            Theme::Complaint => "Monday arrived far too early and it is not what I ordered.",
            Theme::Apology => "eating the last piece of cake and blaming the dog",
            Theme::Iou => "one (1) large coffee, with the good milk",
            Theme::Review => "Cooking 4
Punctuality 2
Charm 5
Dishes 1
Snack stealing 5",
            Theme::Butthurt => "Yesterday it solved my bug in one go. Today it forgot what a semicolon is.",
        }
    }

    /// Themes with a "Shuffle" button: the forms, and the horoscope (which picks another sign).
    pub fn shuffles(self) -> bool {
        self.is_form() || self == Theme::Horoscope
    }

    /// The silly forms: with no text they print blank, for writing on by hand.
    pub fn is_form(self) -> bool {
        self.family() == Some(Family::Forms)
    }

    /// Themes that make something of an empty text box (a fortune, a boarding pass).
    pub fn body_optional(self) -> bool {
        if let Theme::Custom(i) = self {
            return custom::get(i).is_some_and(|c| c.body_optional);
        }
        self.is_form() || matches!(self, Theme::Boarding | Theme::Flight | Theme::TrainOld | Theme::Train | Theme::Fortune | Theme::Horoscope | Theme::Banner)
    }
}

/// The text and fields a theme is drawn from.
pub struct Ctx<'a> {
    pub body: &'a str,
    fields: &'a Fields,
    /// The same text always gets the same numbers, speckle and codes.
    pub seed: u64,
    /// How many times the user pressed "Shuffle": 0 keeps the usual wording, anything else picks
    /// other wording for boxes and closing lines.
    pub dice: u64,
    /// False when the closing line of a form is switched off.
    pub closing: bool,
    /// The part being written on in the preview: it is drawn blank, because the text box over
    /// it shows what is typed.
    pub editing: String,
}

impl<'a> Ctx<'a> {
    pub fn new(body: &'a str, fields: &'a Fields) -> Ctx<'a> {
        // Empty fields count as missing, so they do not change the numbers and speckle.
        // (Keys starting with an underscore are settings of the app, not text on the ticket.)
        let given: Vec<&str> = fields.iter().filter(|(k, _)| !k.starts_with('_')).map(|(_, v)| v.trim()).filter(|v| !v.is_empty()).collect();
        let dice = fields.get("_shuffle").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        let closing = fields.get("_closing").map(|v| v.trim() != "off").unwrap_or(true);
        let editing = fields.get("_editing").cloned().unwrap_or_default();
        BINDINGS.with(|b| b.borrow_mut().clear());
        bind("body", body, false);
        for line in body.lines() {
            bind("body", line, false);
        }
        Ctx { body, fields, seed: draw::seed_of(&format!("{body}|{}", given.join("|"))), dice, closing, editing }
    }

    /// A field's value, or its default when left empty.
    pub fn field(&self, theme: Theme, key: &str) -> String {
        self.field_in(theme.fields(), key)
    }

    /// What the user typed in a field, or nothing: forms leave a blank line to write on.
    pub fn given(&self, key: &str) -> String {
        self.fields.get(key).map(|v| v.trim().to_string()).unwrap_or_default()
    }

    /// Like [`Ctx::field`], with the theme's form given directly.
    pub fn field_in(&self, defs: &[Field], key: &str) -> String {
        let given = self.fields.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());
        let hint = defs.iter().find(|d| d.key == key).map(|d| d.hint).unwrap_or_default();
        if let Some(v) = given {
            bind(key, v, false);
        }
        bind(key, hint, given.is_some() == false);
        if hint == "today" {
            bind(key, &today(), true);
        }
        match given {
            Some(v) => v.to_string(),
            None => hint.to_string(),
        }
    }

    /// A field's typed value, or `default` when it is empty. Unlike [`Ctx::field`] the fallback is
    /// made while drawing (a number taken from the text), so a ticket nobody filled in keeps the
    /// look it had before the field existed.
    pub fn field_or(&self, theme: Theme, key: &str, default: impl Into<String>) -> String {
        let default = default.into();
        let given = self.fields.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());
        match given {
            Some(v) => {
                bind(key, v, false);
                v.to_string()
            }
            None => {
                bind(key, &default, true);
                default
            }
        }
    }

    /// Non-empty lines of the body.
    pub fn lines(&self) -> Vec<&str> {
        self.body.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
    }

    /// Like [`Ctx::rng`], and different again after each press of "Shuffle".
    pub fn dice_rng(&self, salt: u64) -> Rng {
        Rng::new(self.seed ^ self.dice.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0x517C_C1B7_2722_0A95))
    }

    pub fn rng(&self, salt: u64) -> Rng {
        Rng::new(self.seed ^ salt.wrapping_mul(0x517C_C1B7_2722_0A95))
    }

    /// A number made from the text, `digits` long.
    pub fn number(&self, salt: u64, digits: usize) -> String {
        let mut rng = self.rng(salt);
        (0..digits).map(|_| char::from(b'0' + rng.below(10) as u8)).collect()
    }
}

/// Draw `body` as `theme`. `base` is what the Plain theme uses (font, size, alignment…); the others
/// bring their own.
pub fn render(theme: Theme, body: &str, fields: &Fields, base: TextOpts) -> Result<Bitmap, String> {
    if theme == Theme::Plain {
        return text::text(body, base);
    }
    let body = if body.trim().is_empty() && theme.body_optional() && !theme.is_form() { theme.sample() } else { body };
    if body.trim().is_empty() && !theme.is_form() {
        return Err("nothing to print".into());
    }
    let ctx = Ctx::new(body, fields);
    match theme {
        Theme::Plain => unreachable!(),
        Theme::Custom(i) => {
            #[cfg(test)]
            if let Some(c) = test_custom_theme() {
                return custom::draw(c, &ctx);
            }
            match custom::get(i) {
                Some(c) => custom::draw(c, &ctx),
                None => Err("that theme is no longer in the themes folder".into()),
            }
        }
        Theme::Receipt => shop::receipt(&ctx),
        Theme::Diner => shop::diner(&ctx),
        Theme::Pharmacy => shop::pharmacy(&ctx),
        Theme::Coupon => shop::coupon(&ctx),
        Theme::Fine => shop::fine(&ctx),
        Theme::Parchment => world::parchment(&ctx),
        Theme::Decree => world::decree(&ctx),
        Theme::Wanted => world::wanted(&ctx),
        Theme::Treasure => world::treasure(&ctx),
        Theme::Boarding => papers::boarding(&ctx),
        Theme::Flight => travel::flight(&ctx),
        Theme::TrainOld => travel::train_old(&ctx),
        Theme::Train => travel::train(&ctx),
        Theme::Movie => papers::movie(&ctx),
        Theme::Fortune => papers::fortune(&ctx),
        Theme::Diploma => papers::diploma(&ctx),
        Theme::Horoscope => papers::horoscope(&ctx),
        Theme::Fridge => papers::fridge(&ctx),
        Theme::Checklist => papers::checklist(&ctx),
        Theme::Ransom => papers::ransom(&ctx),
        Theme::NameTag => papers::name_tag(&ctx),
        Theme::FontSalad => papers::font_salad(&ctx),
        Theme::Tweet => papers::tweet(&ctx),
        Theme::Banner => banner::banner(&ctx),
        Theme::Greentext => greentext::greentext(&ctx),
        Theme::Terminal => retro::terminal(&ctx),
        Theme::DotMatrix => retro::dot_matrix(&ctx),
        Theme::Label => retro::label(&ctx),
        Theme::Telegram => retro::telegram(&ctx),
        Theme::PunchCard => retro::punch_card(&ctx),
        Theme::Poster => retro::poster(&ctx),
        Theme::WhileOut => forms::while_out(&ctx),
        Theme::Permission => forms::permission(&ctx),
        Theme::Incident => forms::incident(&ctx),
        Theme::Passive => forms::passive(&ctx),
        Theme::Pep => forms::pep(&ctx),
        Theme::Excuse => forms::excuse(&ctx),
        Theme::Complaint => forms::complaint(&ctx),
        Theme::Apology => forms::apology(&ctx),
        Theme::Iou => forms::iou(&ctx),
        Theme::Review => forms::review(&ctx),
        Theme::Butthurt => forms::butthurt(&ctx),
    }
}

/// [`render`], and where the writable parts of the ticket are, merged into one region for each
/// field and one for the text. The silly forms mark their parts by hand; the other themes are
/// read as they are drawn (see [`Page::push`]); a theme that cannot be read has none.
pub fn render_mapped(theme: Theme, body: &str, fields: &Fields, base: TextOpts) -> Result<(Bitmap, Vec<Region>), String> {
    REGIONS.with(|r| r.borrow_mut().clear());
    AUTO.with(|a| a.set(!theme.is_form()));
    MAPPING.with(|m| m.set(true));
    text::record_drawn(true);
    let bitmap = render(theme, body, fields, base);
    text::record_drawn(false);
    MAPPING.with(|m| m.set(false));
    AUTO.with(|a| a.set(false));
    let found = REGIONS.with(|r| std::mem::take(&mut *r.borrow_mut()));
    let bitmap = bitmap?;
    let mut regions: Vec<Region> = Vec::new();
    if theme == Theme::Plain {
        regions.push(Region { key: "body".into(), x0: 0, x1: WIDTH_DOTS, y0: 0, y1: bitmap.height });
    }
    for r in found {
        let r = Region { y1: r.y1.min(bitmap.height), ..r };
        if r.y0 >= r.y1 {
            continue;
        }
        match regions.iter_mut().find(|m| m.key == r.key) {
            Some(m) => {
                m.x0 = m.x0.min(r.x0);
                m.x1 = m.x1.max(r.x1);
                m.y0 = m.y0.min(r.y0);
                m.y1 = m.y1.max(r.y1);
            }
            None => regions.push(r),
        }
    }
    // The text first, so that a field drawn inside it stays on top of it.
    regions.sort_by_key(|r| (r.key != "body", r.y0));
    Ok((bitmap, regions))
}

/// Keep a region a theme found while drawing by hand (a sideways sheet, turned afterwards).
/// Only collected while [`render_mapped`] is looking.
pub(super) fn mark(region: Region) {
    if MAPPING.with(|m| m.get()) {
        REGIONS.with(|r| r.borrow_mut().push(region));
    }
}

#[cfg(test)]
thread_local! {
    /// A theme a test wants [`render`] to draw instead of the shared folder registry, which other
    /// tests read at the same time.
    static TEST_CUSTOM: std::cell::RefCell<Option<&'static custom::Custom>> = const { std::cell::RefCell::new(None) };
}

/// Make [`render`] use `c` for `Theme::Custom` on this thread (tests only).
#[cfg(test)]
pub(crate) fn test_custom(c: Option<&'static custom::Custom>) {
    TEST_CUSTOM.with(|t| *t.borrow_mut() = c);
}

#[cfg(test)]
fn test_custom_theme() -> Option<&'static custom::Custom> {
    TEST_CUSTOM.with(|t| *t.borrow())
}

// ---- the toolkit the themes share ----------------------------------------------------------

/// A ticket built from the top down: lines of text and pictures stacked, then finished.
pub struct Page {
    parts: Vec<Bitmap>,
    /// Blank dots kept free left and right of the text.
    pub mx: f32,
    marks: Vec<Region>,
    open: Option<(String, usize)>,
}

type R = Result<(), String>;

impl Page {
    pub fn new(mx: f32) -> Page {
        Page { parts: Vec::new(), mx, marks: Vec::new(), open: None }
    }

    /// What is added from here to the next [`Page::end`] is the part of the ticket named `key`
    /// that the user can write on.
    pub fn begin(&mut self, key: &str) {
        self.open = Some((key.to_string(), self.height()));
    }

    pub fn end(&mut self) {
        if let Some((key, y0)) = self.open.take() {
            let (x0, x1) = (self.mx as usize, WIDTH_DOTS - self.mx as usize);
            self.marks.push(Region { key, x0, x1, y0, y1: self.height() });
        }
    }

    pub fn gap(&mut self, rows: usize) {
        if rows > 0 {
            self.parts.push(Bitmap::blank(rows));
        }
    }

    pub fn push(&mut self, b: Bitmap) {
        self.note(b.height);
        self.parts.push(b);
    }

    /// What a pushed element shows: the strings drawn since the last one are matched with what
    /// the ticket knows (the text, the fields), and the element becomes a writable part of it.
    fn note(&mut self, rows: usize) {
        let drawn = text::take_drawn();
        if self.open.is_some() || drawn.is_empty() || !AUTO.with(|a| a.get()) {
            return;
        }
        let mut keys: Vec<String> = Vec::new();
        for d in &drawn {
            if let Some(k) = binding_of(d) {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
        // Two parts on one line (a from and a to) share it, side by side in the order drawn.
        let (y0, y1) = (self.height(), self.height() + rows);
        let (left, right) = (self.mx as usize, WIDTH_DOTS - self.mx as usize);
        let n = keys.len();
        for (i, key) in keys.into_iter().enumerate() {
            self.marks.push(Region { key, x0: left + (right - left) * i / n, x1: left + (right - left) * (i + 1) / n, y0, y1 });
        }
    }

    pub fn height(&self) -> usize {
        self.parts.iter().map(|p| p.height).sum()
    }

    fn opts(&self, font: FontChoice, px: f32, align: Align) -> TextOpts {
        TextOpts { px, font, align, invert: false, break_anywhere: false, line_gap: 0.0, margin_y: 0.0, margin_x: self.mx }
    }

    /// Wrapped text; a blank string is just a gap.
    pub fn text(&mut self, s: &str, font: FontChoice, px: f32, align: Align) -> R {
        if s.trim().is_empty() {
            self.gap((px * 0.6) as usize);
            return Ok(());
        }
        let b = text::text(s, self.opts(font, px, align))?;
        self.push(b);
        Ok(())
    }

    /// Wrapped at any character, for columns that must stay put (terminal output, listings).
    pub fn text_any(&mut self, s: &str, font: FontChoice, px: f32) -> R {
        let mut o = self.opts(font, px, Align::Left);
        o.break_anywhere = true;
        self.push(text::text(s, o)?);
        Ok(())
    }

    /// Like [`Page::text`], tightening the gap between lines by `squeeze` dots.
    pub fn text_gap(&mut self, s: &str, font: FontChoice, px: f32, align: Align, line_gap: f32) -> R {
        let mut o = self.opts(font, px, align);
        o.line_gap = line_gap;
        self.push(text::text(s, o)?);
        Ok(())
    }

    /// White on a full-width black band.
    pub fn band(&mut self, s: &str, font: FontChoice, px: f32, align: Align) -> R {
        let mut o = self.opts(font, px, align);
        o.invert = true;
        o.margin_y = 5.0;
        self.push(text::text(s, o)?);
        Ok(())
    }

    /// Text each line of which is scaled to fill the width (as large as `max_px`).
    pub fn shout(&mut self, s: &str, font: FontChoice, max_px: f32) -> R {
        for line in s.lines().map(str::trim).filter(|l| !l.is_empty()) {
            self.push(fit_line(line, font, self.mx, max_px)?);
        }
        Ok(())
    }

    /// `left` ........ `right`
    pub fn leader(&mut self, left: &str, right: &str, font: FontChoice, px: f32) -> R {
        self.push(leader_line(left, right, font, px, self.mx)?);
        Ok(())
    }

    /// A horizontal rule between the margins.
    pub fn rule(&mut self, dash: (usize, usize), thick: usize, space: usize) {
        let mut b = Bitmap::blank(thick + 2 * space);
        draw::hline(&mut b, space, self.mx as usize, WIDTH_DOTS - 1 - self.mx as usize, thick, dash);
        self.push(b);
    }

    /// A rule with a diamond in the middle.
    pub fn flourish(&mut self) {
        let mut b = Bitmap::blank(22);
        let (mx, mid) = (self.mx as usize + 10, WIDTH_DOTS as i32 / 2);
        draw::hline(&mut b, 10, mx, mid as usize - 14, 2, (0, 0));
        draw::hline(&mut b, 10, mid as usize + 14, WIDTH_DOTS - mx, 2, (0, 0));
        for k in 0..9i32 {
            let w = 9 - k;
            for dx in -w..=w {
                draw::put(&mut b, mid + dx, 10 + k - 4, true);
                draw::put(&mut b, mid + dx, 10 - k + 4, true);
            }
        }
        self.push(b);
    }

    /// A barcode of `data`, `rows` tall.
    pub fn barcode(&mut self, data: &str, rows: usize) -> R {
        let code = super::code(data, CodeOpts { kind: CodeKind::Code128, caption: false, ..CodeOpts::default() })?;
        self.push(draw::slice(&code, 12, (12 + rows).min(code.height)));
        Ok(())
    }

    pub fn done(self) -> Bitmap {
        REGIONS.with(|r| r.borrow_mut().extend(self.marks.iter().cloned()));
        super::stack(&self.parts, 0)
    }
}

/// One line of text at the largest size (up to `max_px`) that fits between the margins.
pub fn fit_line(s: &str, font: FontChoice, mx: f32, max_px: f32) -> Result<Bitmap, String> {
    let room = WIDTH_DOTS as f32 - 2.0 * mx;
    let mut lo = 12.0f32;
    let mut hi = max_px;
    let render = |px: f32| -> Result<Bitmap, String> {
        text::text(s, TextOpts { px, font, align: Align::Center, margin_x: mx, margin_y: 0.0, break_anywhere: true, ..TextOpts::default() })
    };
    // The widest single line wins: shrink until the ink fits on one line.
    for _ in 0..7 {
        let mid = (lo + hi) / 2.0;
        let b = render(mid)?;
        let one_line = b.height as f32 <= mid * 1.7;
        let wide = text::ink_span(&b).map(|(a, z)| (z - a + 1) as f32).unwrap_or(0.0);
        if one_line && wide <= room {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    render(lo)
}

/// `left` and `right` on one line joined by dots.
pub fn leader_line(left: &str, right: &str, font: FontChoice, px: f32, mx: f32) -> Result<Bitmap, String> {
    let one = |s: &str, align: Align| -> Result<Bitmap, String> {
        if s.trim().is_empty() {
            return Ok(Bitmap::blank(1));
        }
        text::text(s, TextOpts { px, font, align, margin_x: mx, margin_y: 0.0, break_anywhere: true, ..TextOpts::default() })
    };
    let right_b = one(right, Align::Right)?;
    let right_from = text::ink_span(&right_b).map(|(a, _)| a).unwrap_or(WIDTH_DOTS - mx as usize);
    // A long name is cut short so that it never runs into the price.
    let mut name: String = left.trim().to_string();
    let mut left_b = one(&name, Align::Left)?;
    while left_b.height as f32 > px * 1.7 || text::ink_span(&left_b).is_some_and(|(_, z)| z + 18 > right_from) {
        if name.chars().count() <= 3 {
            break;
        }
        name.pop();
        left_b = one(&name, Align::Left)?;
    }
    let height = left_b.height.max(right_b.height).max(1);
    let mut out = Bitmap::blank(height);
    draw::or_at(&mut out, &left_b, 0, 0);
    draw::or_at(&mut out, &right_b, 0, 0);
    let left_to = text::ink_span(&left_b).map(|(_, z)| z).unwrap_or(mx as usize);
    let y = height.saturating_sub((px * 0.22) as usize + 1);
    let mut x = left_to + 8;
    while x + 6 < right_from {
        out.set(x, y);
        out.set(x + 1, y);
        x += 6;
    }
    Ok(out)
}

/// Today's date as `DD/MM/YYYY`, from the system clock (UTC; a ticket does not need more).
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = civil(secs as i64 / 86_400);
    format!("{d:02}/{m:02}/{y}")
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The price at the end of a line: `Coffee 2.50` is ("Coffee", Some(250 cents)).
pub fn split_price(line: &str) -> (String, Option<i64>) {
    let line = line.trim().trim_end_matches(['€', '$', '£']).trim_end();
    let Some((head, last)) = line.rsplit_once(char::is_whitespace) else { return (line.to_string(), None) };
    let cleaned = last.trim_start_matches(['€', '$', '£']).trim_end_matches(['€', '$', '£']).replace(',', ".");
    match cleaned.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 && v < 1e7 => (head.trim().to_string(), Some((v * 100.0).round() as i64)),
        _ => (line.to_string(), None),
    }
}

pub fn money(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(b: &Bitmap) -> u32 {
        b.data.iter().map(|x| x.count_ones()).sum()
    }

    fn draw_theme(theme: Theme, body: &str) -> Bitmap {
        render(theme, body, &Fields::new(), TextOpts::default()).unwrap_or_else(|e| panic!("{}: {e}", theme.id()))
    }

    #[test]
    fn every_theme_has_a_name_an_id_a_family_and_a_sample_that_draws() {
        let mut ids = std::collections::HashSet::new();
        for theme in Theme::BUILTIN {
            assert!(ids.insert(theme.id()), "ids are unique: {}", theme.id());
            assert!(!theme.name().is_empty() && !theme.tagline().is_empty());
            assert_eq!(Theme::from_id(theme.id()), Some(theme));
            assert_eq!(theme.family().is_none(), theme == Theme::Plain);
            assert!(theme.fields().len() <= MAX_FIELDS, "{} has too many fields", theme.id());
            let b = draw_theme(theme, theme.sample());
            assert!(b.height > 40 && ink(&b) > 200, "{} drew almost nothing ({} rows)", theme.id(), b.height);
            assert!(b.height < 3000, "{} is {} rows, too long for its sample", theme.id(), b.height);
        }
        assert_eq!(Theme::from_id("Name Tag"), Some(Theme::NameTag));
        assert_eq!(Theme::from_id("dotmatrix"), Some(Theme::DotMatrix));
        assert_eq!(Theme::from_id("nonsense"), None);
    }

    #[test]
    fn the_client_list_of_themes_matches_the_real_ones() {
        let listed: Vec<&str> = crate::jobspec::THEMES.iter().map(|t| t.0).collect();
        assert_eq!(listed, Theme::BUILTIN.map(Theme::id));
        for (theme, (_, keys, _)) in Theme::BUILTIN.iter().zip(crate::jobspec::THEMES) {
            let real: Vec<&str> = theme.fields().iter().map(|f| f.key).collect();
            assert_eq!(real.join(","), *keys, "{}", theme.id());
        }
    }

    #[test]
    fn the_same_text_gives_the_same_ticket() {
        for theme in [Theme::Parchment, Theme::Ransom, Theme::Receipt, Theme::Fridge] {
            assert_eq!(draw_theme(theme, theme.sample()), draw_theme(theme, theme.sample()), "{} is not repeatable", theme.id());
        }
    }

    #[test]
    fn an_empty_text_is_refused_unless_the_theme_can_make_something() {
        for theme in Theme::BUILTIN {
            let r = render(theme, "  ", &Fields::new(), TextOpts::default());
            assert_eq!(r.is_ok(), theme.body_optional(), "{}", theme.id());
        }
    }

    #[test]
    fn fields_change_the_ticket_and_empty_fields_use_the_hint() {
        let mut named = Fields::new();
        named.insert("store".into(), "THE BIG SHOP".into());
        let a = render(Theme::Receipt, "Tea 1.00", &named, TextOpts::default()).unwrap();
        let b = draw_theme(Theme::Receipt, "Tea 1.00");
        assert_ne!(a, b);
        let blank = Fields::from([("store".to_string(), "   ".to_string())]);
        assert_eq!(render(Theme::Receipt, "Tea 1.00", &blank, TextOpts::default()).unwrap(), b, "blank means default");
    }

    #[test]
    fn a_field_is_its_typed_value_or_the_default_made_while_drawing() {
        let blank = Fields::from([("train".to_string(), "   ".to_string())]);
        let c = Ctx::new("Alex", &blank);
        assert_eq!(c.field_or(Theme::Train, "train", format!("TGV {}", 6000)), "TGV 6000");
        assert_eq!(c.field_or(Theme::Train, "class", "2de"), "2de");
        let typed = Fields::from([("train".to_string(), "  TER 8251 ".to_string())]);
        let c = Ctx::new("Alex", &typed);
        assert_eq!(c.field_or(Theme::Train, "train", "TGV 6000"), "TER 8251");
    }

    /// The parts of a theme that cannot be written on in the preview, because the theme draws
    /// them in a way that cannot be read back: they are typed in the bar under the ticket. This
    /// list is on purpose: a theme joining it (or leaving it) has to be said here.
    const UNMAPPED: &[(&str, &[&str])] = &[
        // Drawn by hand on a sheet that is then turned sideways.
        ("flight", &["body"]),
        ("train-old", &["body"]),
        ("banner", &["font"]),
        // Drawn letter by letter, or as holes, or in a way no string says.
        ("ransom", &["body"]),
        ("punch-card", &["body"]),
        ("telegram", &["body", "to"]),
        ("dot-matrix", &["body"]),
        ("treasure", &["body"]),
        // A part that is only shown through a symbol, a stamp or a count.
        ("parchment", &["signed"]),
        ("horoscope", &["sign", "symbol"]),
        // The silly forms: the tick boxes are not text on the paper; the others are drawn inside a phrase.
        ("while-out", &["boxes"]),
        ("permission", &["boxes"]),
        ("incident", &["boxes"]),
        ("passive", &["to", "from"]),
        ("pep", &["name"]),
        ("excuse", &["boxes"]),
        ("complaint", &["boxes"]),
        ("apology", &["to", "boxes"]),
        ("iou", &["boxes"]),
        ("review", &["body", "boxes"]),
        ("butthurt", &["boxes"]),
        // The tweet: the header picture, the handle line and the footer are drawn outside the
        // page's text blocks, so the bar handles them.
        ("tweet", &["avatar", "footer"]),
    ];

    #[test]
    fn every_part_of_a_theme_is_written_on_in_place_or_is_on_the_unmapped_list() {
        for t in Theme::BUILTIN {
            let (b, regions) = render_mapped(t, t.sample(), &Fields::new(), TextOpts::default()).unwrap();
            for r in &regions {
                assert!(r.x0 < r.x1 && r.x1 <= WIDTH_DOTS && r.y0 < r.y1 && r.y1 <= b.height, "{}: {} is outside the ticket", t.id(), r.key);
            }
            let mut missing: Vec<&str> = Vec::new();
            if !regions.iter().any(|r| r.key == "body") {
                missing.push("body");
            }
            missing.extend(t.fields().iter().map(|f| f.key).filter(|k| !regions.iter().any(|r| r.key == *k)));
            let expected: &[&str] = UNMAPPED.iter().find(|(id, _)| *id == t.id()).map(|(_, keys)| *keys).unwrap_or(&[]);
            assert_eq!(missing, expected, "{}: the parts without a place on the paper changed", t.id());
        }
    }

    #[test]
    #[ignore]
    fn print_meta_table() {
        for t in Theme::BUILTIN {
            let keys: Vec<&str> = t.fields().iter().map(|f| f.key).collect();
            println!("    (\"{}\", \"{}\", \"{}\"),", t.id(), keys.join(","), t.tagline().replace('"', "'"));
        }
    }

    /// `THEME_SHEETS=dir cargo test sheets -- --ignored` writes one PNG per theme, to look at.
    #[test]
    #[ignore]
    fn sheets() {
        let Ok(dir) = std::env::var("THEME_SHEETS") else { return };
        std::fs::create_dir_all(&dir).unwrap();
        for theme in Theme::BUILTIN {
            let b = draw_theme(theme, theme.sample());
            let img = image::GrayImage::from_fn(WIDTH_DOTS as u32, b.height as u32, |x, y| image::Luma([if b.get(x as usize, y as usize) { 20 } else { 244 }]));
            img.save(format!("{dir}/{}.png", theme.id())).unwrap();
            if theme == Theme::Horoscope {
                for (i, sign) in ["Coffee", "Pirate", "Static Shock", "Hot Soup", "Pancake", "Unread Mail"].iter().enumerate() {
                    let f = Fields::from([("sign".to_string(), sign.to_string())]);
                    let b = render(theme, "A short reading.", &f, TextOpts::default()).unwrap();
                    let img = image::GrayImage::from_fn(WIDTH_DOTS as u32, b.height as u32, |x, y| image::Luma([if b.get(x as usize, y as usize) { 20 } else { 244 }]));
                    img.save(format!("{dir}/horo-{i}.png")).unwrap();
                }
            }
            if theme.is_form() {
                let b = draw_theme(theme, "");
                let img = image::GrayImage::from_fn(WIDTH_DOTS as u32, b.height as u32, |x, y| image::Luma([if b.get(x as usize, y as usize) { 20 } else { 244 }]));
                img.save(format!("{dir}/{}-blank.png", theme.id())).unwrap();
            }
        }
    }

    #[test]
    fn the_sideways_tickets_are_turned_to_print_down_the_paper() {
        for theme in [Theme::Flight, Theme::TrainOld] {
            let b = draw_theme(theme, theme.sample());
            assert!((650..900).contains(&b.height), "{} is {} rows long", theme.id(), b.height);
        }
        let named = Fields::from([("from".to_string(), "LYS".to_string()), ("to".to_string(), "BCN".to_string())]);
        let flight = render(Theme::Flight, "", &named, TextOpts::default()).unwrap();
        assert_ne!(flight, draw_theme(Theme::Flight, ""), "the airports are used");
    }

    #[test]
    fn plain_is_just_the_text() {
        let base = TextOpts::default();
        assert_eq!(render(Theme::Plain, "Hi", &Fields::new(), base).unwrap(), text::text("Hi", base).unwrap());
    }

    #[test]
    fn prices_are_found_at_the_end_of_a_line() {
        assert_eq!(split_price("Coffee 2.50"), ("Coffee".into(), Some(250)));
        assert_eq!(split_price("2 x Juice 6,40 €"), ("2 x Juice".into(), Some(640)));
        assert_eq!(split_price("Just a note"), ("Just a note".into(), None));
        assert_eq!(split_price("Soup"), ("Soup".into(), None));
        assert_eq!(money(1480), "14.80");
        assert_eq!(money(5), "0.05");
    }

    #[test]
    fn the_calendar_maths_is_right() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_723), (2024, 1, 1));
        assert_eq!(civil(20_508), (2026, 2, 24));
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn leader_lines_keep_the_price_clear_of_a_long_name() {
        let b = leader_line(&"Extremely long product name ".repeat(4), "12.99", FontChoice::Mono, 24.0, 10.0).unwrap();
        let (_, right) = text::ink_span(&b).unwrap();
        assert!(right < WIDTH_DOTS - 4);
        assert!(b.height < 50, "stays on one line");
    }

    #[test]
    fn shouted_lines_fill_the_paper() {
        let b = fit_line("HELLO", FontChoice::Impact, 8.0, 200.0).unwrap();
        let (a, z) = text::ink_span(&b).unwrap();
        assert!(z - a > 300, "wide: {}", z - a);
        assert!(z - a < WIDTH_DOTS - 12);
    }
}

/// The most fields any theme asks for (the app keeps this many text boxes).
pub const MAX_FIELDS: usize = 8;
