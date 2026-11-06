//! Talking to the running Tronic Printer app over its named pipe. Shared by `tronic-print` and
//! `tronic-mcp` (each includes this file with `#[path]`), so it uses nothing but `std`.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::thread;
use std::time::Duration;

/// Keep in sync with `src/ipc.rs`.
pub const PIPE: &str = r"\\.\pipe\tronic-printer";

/// Open the pipe, waiting a moment if the app is between two clients.
pub fn connect() -> Result<File, String> {
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

/// Send one request (`mode` is `once` or `job`, with an optional `force`) and wait for the app's
/// one-line answer. `Ok` is an `OK …` answer; an `ERR …` answer comes back as `Err`.
pub fn request(mode: &str, force: bool, payload: &[u8]) -> Result<String, String> {
    let mut pipe = connect()?;
    let header = format!("TRONIC1 {mode} {}{}\n", payload.len(), if force { " force" } else { "" });
    pipe.write_all(header.as_bytes()).and_then(|_| pipe.write_all(payload)).and_then(|_| pipe.flush()).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(pipe).read_line(&mut reply).map_err(|e| e.to_string())?;
    let reply = reply.trim();
    match reply.strip_prefix("OK") {
        Some(info) => Ok(info.trim().to_string()),
        None => Err(reply.trim_start_matches("ERR").trim().to_string()),
    }
}
