//! A named pipe so other programs can print through the running app:
//! `dir | tronic-print`, `tronic-print --live -- cargo build`.
//!
//! The wire format is one header line, then the payload:
//!
//! * `TRONIC1 once <bytes> [force]` followed by that many bytes of UTF-8 text. The server
//!   answers `OK …` or `ERR …` on one line.
//! * `TRONIC1 live`, followed by lines of text until the client closes the pipe.

use std::sync::mpsc::{Sender, channel, Receiver};

use rune::Waker;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::sync::oneshot;

/// Keep in sync with `src/bin/tronic-print.rs`.
pub const PIPE: &str = r"\\.\pipe\tronic-printer";

/// Larger payloads are refused: that is a mistake, not a ticket.
const MAX_BYTES: usize = 4_000_000;

pub enum IpcMsg {
    Once { text: String, force: bool, reply: oneshot::Sender<String> },
    Line(String),
    Live(bool),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Header {
    Once { len: usize, force: bool },
    Live,
}

pub fn parse_header(line: &str) -> Result<Header, String> {
    let mut parts = line.split_whitespace();
    if parts.next() != Some("TRONIC1") {
        return Err("not a tronic-print client".into());
    }
    match parts.next() {
        Some("live") => Ok(Header::Live),
        Some("once") => {
            let len: usize = parts.next().and_then(|n| n.parse().ok()).ok_or("missing length")?;
            if len > MAX_BYTES {
                return Err(format!("{len} bytes is too much to print"));
            }
            Ok(Header::Once { len, force: parts.next() == Some("force") })
        }
        _ => Err("unknown mode".into()),
    }
}

/// Start the pipe server on its own thread. If another copy of the app already owns the pipe,
/// this copy simply has no terminal input.
pub fn spawn(waker: Waker) -> Receiver<IpcMsg> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("ipc".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
            rt.block_on(serve(tx, waker));
        })
        .expect("spawn ipc thread");
    rx
}

async fn serve(tx: Sender<IpcMsg>, waker: Waker) {
    let mut first = true;
    loop {
        let server = match ServerOptions::new().first_pipe_instance(first).reject_remote_clients(true).create(PIPE) {
            Ok(s) => s,
            Err(_) => return,
        };
        first = false;
        if server.connect().await.is_err() {
            continue;
        }
        tokio::spawn(handle(server, tx.clone(), waker.clone()));
    }
}

async fn handle(pipe: tokio::net::windows::named_pipe::NamedPipeServer, tx: Sender<IpcMsg>, waker: Waker) {
    let (read, mut write) = tokio::io::split(pipe);
    let mut read = BufReader::new(read);
    let mut header = String::new();
    if read.read_line(&mut header).await.unwrap_or(0) == 0 {
        return;
    }
    match parse_header(&header) {
        Ok(Header::Once { len, force }) => {
            let mut bytes = vec![0u8; len];
            let reply = match read.read_exact(&mut bytes).await {
                Ok(_) => {
                    let (rtx, rrx) = oneshot::channel();
                    let text = String::from_utf8_lossy(&bytes).into_owned();
                    let _ = tx.send(IpcMsg::Once { text, force, reply: rtx });
                    waker();
                    rrx.await.unwrap_or_else(|_| "ERR the app closed".into())
                }
                Err(e) => format!("ERR {e}"),
            };
            let _ = write.write_all(format!("{reply}\n").as_bytes()).await;
            let _ = write.flush().await;
        }
        Ok(Header::Live) => {
            let _ = tx.send(IpcMsg::Live(true));
            waker();
            let mut line = String::new();
            loop {
                line.clear();
                match read.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let _ = tx.send(IpcMsg::Line(line.trim_end_matches(['\r', '\n']).to_string()));
                        waker();
                    }
                }
            }
            let _ = tx.send(IpcMsg::Live(false));
            waker();
        }
        Err(e) => {
            let _ = write.write_all(format!("ERR {e}\n").as_bytes()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_are_parsed_and_bad_ones_refused() {
        assert_eq!(parse_header("TRONIC1 once 12\n"), Ok(Header::Once { len: 12, force: false }));
        assert_eq!(parse_header("TRONIC1 once 12 force\r\n"), Ok(Header::Once { len: 12, force: true }));
        assert_eq!(parse_header("TRONIC1 live\n"), Ok(Header::Live));
        assert!(parse_header("hello").is_err());
        assert!(parse_header("TRONIC1 once").is_err());
        assert!(parse_header("TRONIC1 once 999999999").is_err());
    }
}
