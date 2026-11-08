//! A print job as other programs describe it (the MCP server, scripts): a kind, a few named
//! settings and a body. It travels over the named pipe as plain text, so both sides need only
//! the standard library:
//!
//! ```text
//! kind=text
//! font=mono
//! size=large
//!
//! The text to print…
//! ```
//!
//! Settings are `key=value` lines up to the first blank line; the rest is the body. This file is
//! shared by the app (which parses jobs) and the `tronic-mcp` / `tronic-print` programs (which
//! encode them), so it uses nothing but `std`.

// Each program uses a different half (the app parses, the clients encode).
#![allow(dead_code)]

use std::collections::BTreeMap;

/// The writing themes as the clients see them: (id, comma-separated field keys, what it is).
/// The app's `Theme` is the source of truth; a test keeps this list in step with it.
pub const THEMES: &[(&str, &str, &str)] = &[
("plain", "", "Your text, your font"),
    ("receipt", "store,address,date", "A shop receipt: one item per line, with the price at the end"),
    ("diner", "place,table,server", "A greasy-spoon guest check with a tip line"),
    ("pharmacy", "patient,doctor,refills", "A prescription label with a barcode"),
    ("coupon", "offer,code,expires", "A cut-out discount coupon"),
    ("fine", "offender,amount,number", "An official penalty notice"),
    ("parchment", "title,signed", "An aged scroll with an ornate border and a wax seal"),
    ("decree", "title,signed", "A royal decree under a crown"),
    ("wanted", "name,reward", "A wanted poster with a reward"),
    ("treasure", "place", "A treasure map with a dotted trail and an X"),
    ("boarding", "from,to,name,seat,flight,date,gate,class", "A boarding pass with a stub and barcode"),
    ("flight", "from,to,name,seat,flight,date,gate,class", "A boarding pass lying sideways, with a tear-off stub"),
    ("train-old", "from,to,date,seat,operator,train,price,class", "The classic card train ticket with its magnetic strip"),
    ("train", "from,to,date,seat,operator,train,price,class", "A modern train e-ticket with a QR code"),
    ("movie", "room,seat,time", "A cinema ticket with notches"),
    ("fortune", "", "A fortune-cookie slip with lucky numbers"),
    ("diploma", "name,award,signed", "A certificate with a seal"),
    ("horoscope", "sign,symbol", "Your sign and a reading"),
    ("fridge", "", "A handwritten note held by a magnet"),
    ("checklist", "title", "A to-do list with boxes to tick"),
    ("ransom", "", "Every letter in a different font, cut from a magazine"),
    ("name-tag", "", "HELLO my name is…"),
    ("font-salad", "", "Big lines, every one in a different typeface"),
    ("tweet", "name,handle,avatar,stats,footer", "A post on paper, avatar and all"),
    ("banner", "font", "Big letters printed sideways along the length of the paper"),
    ("greentext", "board,name,no,date", "A thread that never ends"),
    ("terminal", "prompt", "Command-line output with a prompt and a cursor"),
    ("dot-matrix", "title", "A continuous-paper report with sprocket holes"),
    ("label", "", "White-on-black embossed label strips"),
    ("telegram", "to", "ALL CAPS and STOP"),
    ("punch-card", "", "Your text as holes on a punch card"),
    ("poster", "", "Every line as big as the paper allows"),
    ("codes", "caption", "Text, with a QR code or a barcode on any line that asks for one"),
    ("while-out", "to,from,time,boxes,footer", "A message slip with ticked boxes and a note"),
    ("permission", "name,by,boxes,footer", "A signed permission slip, granted"),
    ("incident", "by,place,date,boxes,footer", "An incident report with a severity meter"),
    ("passive", "to,from,footer", "A friendly note that is not that friendly"),
    ("pep", "name,footer", "A pep talk with five stars, no refills"),
    ("excuse", "who,by,boxes,footer", "An excuse note with the reasons ticked"),
    ("complaint", "name,dept,boxes,footer", "A complaint form with an outrage meter"),
    ("apology", "to,from,boxes,footer", "An official apology with a sincerity level"),
    ("iou", "from,to,due,boxes,footer", "An I.O.U. payable in coffee, hugs or pizza"),
    ("review", "name,period,boxes,footer", "Star ratings from lines like 'Cooking 4'"),
    ("butthurt", "user,provider,model,boxes,footer", "A butthurt report: your model was surely quantized"),
    ("cerfa", "name,place,creature,date", "A French administrative form for a creature of your own"),
];

/// What a job asks for: print one kind of thing, or ask about the printer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JobKind {
    #[default]
    Text,
    Code,
    Image,
    /// A question about the printer, answered without printing (`printer_status`).
    Status,
}

impl JobKind {
    pub const ALL: [JobKind; 4] = [JobKind::Text, JobKind::Code, JobKind::Image, JobKind::Status];

    /// The id used on the wire and in the tool list.
    pub fn id(self) -> &'static str {
        match self {
            JobKind::Text => "text",
            JobKind::Code => "code",
            JobKind::Image => "image",
            JobKind::Status => "status",
        }
    }

    /// The ids, for an error that tells the caller what it may use instead.
    pub fn list() -> String {
        JobKind::ALL.iter().map(|k| k.id()).collect::<Vec<_>>().join(", ")
    }

    /// Case and surrounding spaces do not matter: `Status` and `status ` are status.
    pub fn parse(text: &str) -> Option<JobKind> {
        Some(match text.trim().to_ascii_lowercase().as_str() {
            "text" => JobKind::Text,
            "code" => JobKind::Code,
            "image" => JobKind::Image,
            "status" => JobKind::Status,
            _ => return None,
        })
    }
}

impl std::fmt::Display for JobKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Settings the app reads itself. A theme `fields` map may not use these names: `kind` would
/// rewrite the job type on the way to the app, the others would change the print behind the
/// caller's back. `font` is not here: the banner theme reads it as one of its own fields, and a
/// typeface cannot change what kind of job this is.
const RESERVED: [&str; 8] = ["kind", "body", "theme", "size", "align", "invert", "copies", "density"];

pub fn is_reserved(key: &str) -> bool {
    RESERVED.contains(&key.trim())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Job {
    pub kind: JobKind,
    pub params: BTreeMap<String, String>,
    pub body: String,
}

impl Job {
    /// A job of a known kind. A typo here is a bug in one of our own programs, not user input, so
    /// it panics: `Job::parse` is the fallible door for what arrives from outside.
    pub fn new(kind: &str) -> Job {
        Job { kind: JobKind::parse(kind).unwrap_or_else(|| panic!("unknown job kind '{kind}'")), ..Job::default() }
    }

    pub fn param(mut self, key: &str, value: impl ToString) -> Job {
        self.params.insert(key.into(), value.to_string().replace(['\r', '\n'], " "));
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Job {
        self.body = body.into();
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str).filter(|v| !v.is_empty())
    }

    /// "1", "true", "yes" and "on" are true; anything else, or nothing, is false.
    pub fn flag(&self, key: &str) -> bool {
        self.get(key).is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
    }

    pub fn number(&self, key: &str) -> Option<f32> {
        self.get(key)?.trim().parse().ok()
    }

    /// The theme fields of the job: every setting that is not one of the app's own.
    pub fn fields(&self) -> BTreeMap<String, String> {
        self.params.iter().filter(|(k, _)| !is_reserved(k)).map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub fn encode(&self) -> String {
        let mut out = format!("kind={}\n", self.kind.id());
        for (k, v) in &self.params {
            out.push_str(&format!("{k}={v}\n"));
        }
        out.push('\n');
        out.push_str(&self.body);
        out
    }

    pub fn parse(payload: &str) -> Result<Job, String> {
        let (head, body) = match payload.split_once("\n\n") {
            Some((h, b)) => (h, b),
            None => (payload.trim_end_matches('\n'), ""),
        };
        let mut kind = None;
        let mut params = BTreeMap::new();
        for line in head.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            let (k, v) = line.split_once('=').ok_or_else(|| format!("bad setting line: {line}"))?;
            if k.trim() == "kind" {
                // The first kind wins: a `kind` smuggled in through the settings (a theme field
                // map, say) must not turn a text job into something else.
                if kind.is_none() {
                    let raw = v.trim();
                    kind = Some(JobKind::parse(raw).ok_or_else(|| format!("unknown job kind '{raw}' (use {})", JobKind::list()))?);
                }
            } else {
                params.insert(k.trim().to_string(), v.to_string());
            }
        }
        let Some(kind) = kind else { return Err("the job has no kind".into()) };
        Ok(Job { kind, params, body: body.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_survives_the_trip_with_its_settings_and_a_multiline_body() {
        let job = Job::new("text").param("font", "mono").param("invert", true).body("line one\n\nline three = 3\n");
        let back = Job::parse(&job.encode()).unwrap();
        assert_eq!(back, job);
        assert_eq!(back.get("font"), Some("mono"));
        assert!(back.flag("invert") && !back.flag("missing"));
    }

    #[test]
    fn settings_cannot_smuggle_a_new_line_and_numbers_are_read() {
        let job = Job::new("image").param("path", "a\nb").param("brightness", "0.25").param("contrast", "x");
        assert_eq!(job.get("path"), Some("a b"));
        assert_eq!(job.number("brightness"), Some(0.25));
        assert_eq!(job.number("contrast"), None);
    }

    #[test]
    fn bad_payloads_are_refused_with_a_reason() {
        assert!(Job::parse("font=mono\n\nhi").unwrap_err().contains("no kind"));
        assert!(Job::parse("kind=text\nnot a setting\n\nhi").unwrap_err().contains("bad setting"));
        let status = Job::parse("kind=status").unwrap();
        assert_eq!((status.kind.id(), status.body.as_str()), ("status", ""));
        let err = Job::parse("kind=hologram").unwrap_err();
        assert!(err.contains("unknown job kind 'hologram'"), "{err}");
        for id in ["text", "code", "image", "status"] {
            assert!(err.contains(id), "the error lists {id}: {err}");
        }
    }

    #[test]
    fn a_kind_is_read_whatever_its_case_and_spaces() {
        for text in ["Status", " STATUS ", "status"] {
            assert_eq!(Job::parse(&format!("kind={text}")).unwrap().kind, JobKind::Status, "{text}");
        }
        assert_eq!(JobKind::parse("Text"), Some(JobKind::Text));
        assert_eq!(JobKind::parse(" image\n"), Some(JobKind::Image));
        assert_eq!(JobKind::parse("code "), Some(JobKind::Code));
        assert_eq!(JobKind::parse(""), None);
        assert_eq!(JobKind::parse("hologram"), None);
    }

    #[test]
    fn a_kind_hidden_in_the_settings_cannot_rewrite_the_job() {
        // A theme field map lands in `params`; encoding then parsing it back must keep the type.
        let job = Job::new("text").param("kind", "hologram").body("hello");
        assert_eq!(Job::parse(&job.encode()).unwrap().kind, JobKind::Text);
        // The field is not offered to the theme either.
        assert_eq!(job.fields().get("kind"), None);
        let fields = Job::new("text").param("theme", "receipt").param("store", "SHOP").param("copies", 2).fields();
        assert_eq!(fields.get("store").map(String::as_str), Some("SHOP"));
        assert!(fields.get("theme").is_none() && fields.get("copies").is_none());
        // The banner theme picks its typeface from a field called `font`: it must survive.
        assert_eq!(Job::new("text").param("font", "mono").fields().get("font").map(String::as_str), Some("mono"));
    }
}
