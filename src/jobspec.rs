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
