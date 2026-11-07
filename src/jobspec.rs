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
    ("boarding", "from,to,name,seat", "A boarding pass with a stub and barcode"),
    ("flight", "from,to,name,seat", "A boarding pass lying sideways, with a tear-off stub"),
    ("train-old", "from,to,date,seat", "The classic card train ticket with its magnetic strip"),
    ("train", "from,to,date,seat", "A modern train e-ticket with a QR code"),
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
    ("terminal", "prompt", "Command-line output with a prompt and a cursor"),
    ("dot-matrix", "title", "A continuous-paper report with sprocket holes"),
    ("label", "", "White-on-black embossed label strips"),
    ("telegram", "to", "ALL CAPS and STOP"),
    ("punch-card", "", "Your text as holes on a punch card"),
    ("poster", "", "Every line as big as the paper allows"),
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
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Job {
    /// `text`, `image`, `code` or `status`.
    pub kind: String,
    pub params: BTreeMap<String, String>,
    pub body: String,
}

impl Job {
    pub fn new(kind: &str) -> Job {
        Job { kind: kind.into(), ..Job::default() }
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

    pub fn encode(&self) -> String {
        let mut out = format!("kind={}\n", self.kind);
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
        let mut job = Job { body: body.to_string(), ..Job::default() };
        for line in head.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            let (k, v) = line.split_once('=').ok_or_else(|| format!("bad setting line: {line}"))?;
            if k == "kind" {
                job.kind = v.trim().to_string();
            } else {
                job.params.insert(k.trim().to_string(), v.to_string());
            }
        }
        if job.kind.is_empty() {
            return Err("the job has no kind".into());
        }
        Ok(job)
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
        assert_eq!((status.kind.as_str(), status.body.as_str()), ("status", ""));
    }
}
