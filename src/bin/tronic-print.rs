//! `tronic-print`: send text to the running Tronic Printer app.
//!
//! ```text
//! dir | tronic-print                       print what arrives on stdin, once
//! tronic-print -- git log --oneline -5     run a command and print its output
//! tronic-print --live -- cargo build       print as the output arrives
//! some-command | tronic-print --live       same, from a pipe
//! 'Tea 1.20' | tronic-print --theme receipt --field store=CAFE
//! ```

#[path = "../client.rs"]
mod client;
#[path = "../jobspec.rs"]
mod jobspec;

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::thread;

const HELP_TEMPLATE: &str = "\
tronic-print: print text on the Tronic Printer through the running app

USAGE
    <command> | tronic-print [--live] [--force] [--theme NAME] [--field KEY=VALUE]...
    tronic-print [--live] [--force] -- <command> [args…]

OPTIONS
    --live          print as the output arrives, in batches, until it ends
    --force         allow more than 100 cm of paper for a single print
    --theme NAME    dress the text as a document (see THEMES); not with --live
    --field K=V     an extra for the theme, e.g. --field store=FRESH (repeatable)
    -h              this help

THEMES
{themes}

The Tronic Printer app must be open and connected to the printer.";

fn help() -> String {
    let themes: Vec<String> = jobspec::THEMES
        .iter()
        .filter(|t| t.0 != "plain")
        .map(|(id, fields, what)| format!("    {id:<11} {what}{}", if fields.is_empty() { String::new() } else { format!(" [{}]", fields.replace(',', ", ")) }))
        .collect();
    HELP_TEMPLATE.replace("{themes}", &themes.join("
"))
}

struct Args {
    live: bool,
    force: bool,
    theme: Option<String>,
    fields: Vec<(String, String)>,
    command: Vec<String>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut out = Args { live: false, force: false, theme: None, fields: Vec::new(), command: Vec::new() };
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--live" => out.live = true,
            "--force" => out.force = true,
            "--theme" => {
                let name = args.next().ok_or("--theme needs a name")?;
                // The app knows the user's own themes too, and names every one if this is unknown.
                out.theme = Some(name);
            }
            "--field" => {
                let pair = args.next().ok_or("--field needs KEY=VALUE")?;
                let (k, v) = pair.split_once('=').ok_or_else(|| format!("--field wants KEY=VALUE, got '{pair}'"))?;
                out.fields.push((k.trim().to_string(), v.to_string()));
            }
            "-h" | "--help" => return Err(String::new()),
            "--" => {
                out.command = args.by_ref().collect();
                break;
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(out)
}

fn once(text: Vec<u8>, force: bool, theme: Option<&str>, fields: &[(String, String)]) -> Result<(), String> {
    let info = match theme {
        None => client::request("once", force, &text)?,
        Some(theme) => {
            let mut job = jobspec::Job::new("text").param("theme", theme).body(String::from_utf8_lossy(&text).into_owned());
            for (k, v) in fields {
                job = job.param(k, v);
            }
            client::request("job", force, job.encode().as_bytes())?
        }
    };
    eprintln!("printed {info}");
    Ok(())
}

fn live(lines: mpsc::Receiver<String>) -> Result<(), String> {
    let mut pipe = client::connect()?;
    pipe.write_all(b"TRONIC1 live\n").map_err(|e| e.to_string())?;
    for line in lines {
        pipe.write_all(line.trim_end_matches(['\r', '\n']).as_bytes()).and_then(|_| pipe.write_all(b"\n")).map_err(|e| e.to_string())?;
    }
    pipe.flush().map_err(|e| e.to_string())
}

/// Lines from a reader, on their own thread.
fn lines_of(reader: impl Read + Send + 'static, tx: mpsc::Sender<String>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        while reader.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                return;
            }
            buf.clear();
        }
    })
}

fn run() -> Result<(), String> {
    let args = match parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("tronic-print: {msg}\n");
            }
            eprintln!("{}", help());
            return if msg.is_empty() { Ok(()) } else { Err(String::new()) };
        }
    };
    if args.live && args.theme.is_some() {
        return Err("--theme prints once, it cannot be used with --live".into());
    }
    let (tx, rx) = mpsc::channel();
    let mut readers = Vec::new();
    let mut child = None;
    if let Some((program, rest)) = args.command.split_first() {
        let mut c = Command::new(program)
            .args(rest)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run {program}: {e}"))?;
        readers.push(lines_of(c.stdout.take().unwrap(), tx.clone()));
        readers.push(lines_of(c.stderr.take().unwrap(), tx.clone()));
        child = Some(c);
    } else {
        readers.push(lines_of(std::io::stdin(), tx.clone()));
    }
    drop(tx);

    let result = if args.live {
        live(rx)
    } else {
        let mut text = Vec::new();
        for line in rx {
            text.extend_from_slice(line.as_bytes());
        }
        if text.is_empty() { Err("nothing to print".into()) } else { once(text, args.force, args.theme.as_deref(), &args.fields) }
    };
    for r in readers {
        let _ = r.join();
    }
    if let Some(mut c) = child {
        let _ = c.wait();
    }
    result
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("tronic-print: {msg}");
            }
            ExitCode::FAILURE
        }
    }
}
