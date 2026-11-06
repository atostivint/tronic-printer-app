//! `tronic-print`: send text to the running Tronic Printer app.
//!
//! ```text
//! dir | tronic-print                       print what arrives on stdin, once
//! tronic-print -- git log --oneline -5     run a command and print its output
//! tronic-print --live -- cargo build       print as the output arrives
//! some-command | tronic-print --live       same, from a pipe
//! ```

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Keep in sync with `src/ipc.rs`.
const PIPE: &str = r"\\.\pipe\tronic-printer";

const HELP: &str = "\
tronic-print: print text on the Tronic Printer through the running app

USAGE
    <command> | tronic-print [--live] [--force]
    tronic-print [--live] [--force] -- <command> [args…]

OPTIONS
    --live    print as the output arrives, in batches, until it ends
    --force   allow more than 100 cm of paper for a single print
    -h        this help

The Tronic Printer app must be open and connected to the printer.";

struct Args {
    live: bool,
    force: bool,
    command: Vec<String>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut out = Args { live: false, force: false, command: Vec::new() };
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--live" => out.live = true,
            "--force" => out.force = true,
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

fn connect() -> Result<File, String> {
    for _ in 0..20 {
        match OpenOptions::new().read(true).write(true).open(PIPE) {
            Ok(f) => return Ok(f),
            // ERROR_PIPE_BUSY: the server is between two clients.
            Err(e) if e.raw_os_error() == Some(231) => thread::sleep(Duration::from_millis(100)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err("the Tronic Printer app is not running: open it and press Connect".into());
            }
            Err(e) => return Err(format!("cannot reach the app: {e}")),
        }
    }
    Err("the app is busy, try again".into())
}

fn once(text: Vec<u8>, force: bool) -> Result<(), String> {
    let mut pipe = connect()?;
    let header = format!("TRONIC1 once {}{}\n", text.len(), if force { " force" } else { "" });
    pipe.write_all(header.as_bytes()).and_then(|_| pipe.write_all(&text)).and_then(|_| pipe.flush()).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(pipe).read_line(&mut reply).map_err(|e| e.to_string())?;
    match reply.trim().strip_prefix("OK") {
        Some(info) => {
            eprintln!("printed{info}");
            Ok(())
        }
        None => Err(reply.trim().trim_start_matches("ERR").trim().to_string()),
    }
}

fn live(lines: mpsc::Receiver<String>) -> Result<(), String> {
    let mut pipe = connect()?;
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
            eprintln!("{HELP}");
            return if msg.is_empty() { Ok(()) } else { Err(String::new()) };
        }
    };
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
        if text.is_empty() { Err("nothing to print".into()) } else { once(text, args.force) }
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
