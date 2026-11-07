//! A local endpoint so other programs can print through the running app:
//! `dir | tronic-print`, `tronic-print --live -- cargo build`. It is a named pipe on Windows and
//! a Unix domain socket (mode 0600, in a private folder) on Linux and macOS; the protocol below
//! is the same on both and lives in `handle`, which only needs an async byte stream.
//!
//! The wire format is one header line, then the payload:
//!
//! * `TRONIC1 once <bytes> [force]` followed by that many bytes of UTF-8 text. The server
//!   answers `OK …` or `ERR …` on one line.
//! * `TRONIC1 live`, followed by lines of text until the client closes the pipe.

use std::sync::mpsc::{Sender, channel, Receiver};

use rune::Waker;

use crate::jobspec::Job;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

/// Larger payloads are refused: that is a mistake, not a ticket.
const MAX_BYTES: usize = 4_000_000;

pub enum IpcMsg {
    Once { text: String, force: bool, reply: oneshot::Sender<String> },
    /// A structured job (text, picture, code, or a status request) from the MCP server or a script.
    Job { job: Job, force: bool, reply: oneshot::Sender<String> },
    Line(String),
    Live(bool),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Header {
    Once { len: usize, force: bool },
    Job { len: usize, force: bool },
    Live,
}

pub fn parse_header(line: &str) -> Result<Header, String> {
    let mut parts = line.split_whitespace();
    if parts.next() != Some("TRONIC1") {
        return Err("not a tronic-print client".into());
    }
    match parts.next() {
        Some("live") => Ok(Header::Live),
        Some(mode @ ("once" | "job")) => {
            let len: usize = parts.next().and_then(|n| n.parse().ok()).ok_or("missing length")?;
            if len > MAX_BYTES {
                return Err(format!("{len} bytes is too much to print"));
            }
            let force = parts.next() == Some("force");
            Ok(if mode == "job" { Header::Job { len, force } } else { Header::Once { len, force } })
        }
        _ => Err("unknown mode".into()),
    }
}

/// Start the pipe server on its own thread. If another copy of the app already owns the pipe,
/// this copy simply has no terminal input.
pub fn spawn(waker: Waker) -> Receiver<IpcMsg> {
    let (tx, rx) = channel();
    // The same app, also reachable as an HTTP MCP server.
    crate::mcp_http::spawn(tx.clone(), waker.clone());
    std::thread::Builder::new()
        .name("ipc".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
            rt.block_on(serve(tx, waker));
        })
        .expect("spawn ipc thread");
    rx
}

/// Windows: one named pipe instance per client; the first instance refuses a second app.
#[cfg(windows)]
async fn serve(tx: Sender<IpcMsg>, waker: Waker) {
    use tokio::net::windows::named_pipe::ServerOptions;
    let mut first = true;
    loop {
        let server = match ServerOptions::new().first_pipe_instance(first).reject_remote_clients(true).create(crate::client::PIPE) {
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

/// Linux and macOS: a Unix domain socket. A second app finds the first one alive and gives up.
#[cfg(unix)]
async fn serve(tx: Sender<IpcMsg>, waker: Waker) {
    let Ok(listener) = bind_socket(&crate::client::socket_path()) else { return };
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle(stream, tx.clone(), waker.clone()));
            }
            // Out of file descriptors and the like: do not spin.
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    }
}

/// Create the private folder (0700) and listen on `path` (socket mode 0600).
///
/// A leftover file from a crashed app is removed, but only when nobody answers on it: if another
/// copy of the app is alive this fails with `AddrInUse`, and this copy runs without terminal input.
#[cfg(unix)]
fn bind_socket(path: &std::path::Path) -> std::io::Result<tokio::net::UnixListener> {
    use std::io::{Error, ErrorKind};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    let dir = path.parent().ok_or_else(|| Error::new(ErrorKind::InvalidInput, "socket path has no folder"))?;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    // The folder may already exist (or be planted in a shared temp dir): it must be ours and private.
    let meta = std::fs::symlink_metadata(dir)?;
    if !meta.is_dir() || meta.uid() != crate::client::current_uid() {
        return Err(Error::new(ErrorKind::PermissionDenied, format!("{} is not a folder of yours", dir.display())));
    }
    if meta.mode() & 0o077 != 0 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    if path.exists() {
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return Err(Error::new(ErrorKind::AddrInUse, "another Tronic Printer is already running"));
        }
        std::fs::remove_file(path)?;
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// One client, on any kind of stream.
async fn handle<S: AsyncRead + AsyncWrite + Unpin>(pipe: S, tx: Sender<IpcMsg>, waker: Waker) {
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
        Ok(Header::Job { len, force }) => {
            let mut bytes = vec![0u8; len];
            let reply = match read.read_exact(&mut bytes).await {
                Ok(_) => match Job::parse(&String::from_utf8_lossy(&bytes)) {
                    Ok(job) => {
                        let (rtx, rrx) = oneshot::channel();
                        let _ = tx.send(IpcMsg::Job { job, force, reply: rtx });
                        waker();
                        rrx.await.unwrap_or_else(|_| "ERR the app closed".into())
                    }
                    Err(e) => format!("ERR {e}"),
                },
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
        assert_eq!(parse_header("TRONIC1 job 40\n"), Ok(Header::Job { len: 40, force: false }));
        assert_eq!(parse_header("TRONIC1 job 40 force\n"), Ok(Header::Job { len: 40, force: true }));
        assert_eq!(parse_header("TRONIC1 live\n"), Ok(Header::Live));
        assert!(parse_header("hello").is_err());
        assert!(parse_header("TRONIC1 once").is_err());
        assert!(parse_header("TRONIC1 once 999999999").is_err());
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
    }

    /// The protocol code runs on any byte stream: here an in-memory one, on every OS.
    #[test]
    fn a_client_on_any_stream_gets_its_job_through_and_an_answer_back() {
        block_on(async {
            let (mut client, server) = tokio::io::duplex(4096);
            let (tx, rx) = channel();
            let waker: Waker = std::sync::Arc::new(|| {});
            let task = tokio::spawn(handle(server, tx, waker));
            client.write_all(b"TRONIC1 once 5 force
hello").await.unwrap();
            let IpcMsg::Once { text, force, reply } = tokio::task::spawn_blocking(move || rx.recv().unwrap()).await.unwrap() else { panic!("not a once") };
            assert_eq!((text.as_str(), force), ("hello", true));
            reply.send("OK 2 cm".into()).unwrap();
            let mut answer = String::new();
            BufReader::new(&mut client).read_line(&mut answer).await.unwrap();
            assert_eq!(answer, "OK 2 cm
");
            task.await.unwrap();
        });
    }

    #[test]
    fn garbage_gets_an_err_line() {
        block_on(async {
            let (mut client, server) = tokio::io::duplex(4096);
            let (tx, _rx) = channel();
            let waker: Waker = std::sync::Arc::new(|| {});
            let task = tokio::spawn(handle(server, tx, waker));
            client.write_all(b"GET / HTTP/1.1
").await.unwrap();
            let mut answer = String::new();
            BufReader::new(&mut client).read_line(&mut answer).await.unwrap();
            assert!(answer.starts_with("ERR "), "{answer}");
            task.await.unwrap();
        });
    }

    #[cfg(unix)]
    #[test]
    fn the_socket_is_private_a_stale_one_is_replaced_and_a_second_app_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("tronic-ipc-test-{}", std::process::id()));
        let path = base.join("sub").join("printer.sock");
        block_on(async {
            let first = bind_socket(&path).expect("first app binds");
            let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&path), 0o600, "socket is private");
            assert_eq!(mode(path.parent().unwrap()), 0o700, "folder is private");
            let second = bind_socket(&path);
            assert_eq!(second.err().map(|e| e.kind()), Some(std::io::ErrorKind::AddrInUse), "second app is refused");
            drop(first);
            // The dropped listener leaves its file behind: that is a stale socket, and it is replaced.
            assert!(path.exists());
            let third = bind_socket(&path).expect("stale socket is replaced");
            drop(third);
        });
        let _ = std::fs::remove_dir_all(&base);
    }
}
