//! Talking to the running Tronic Printer app over its local endpoint: a named pipe on Windows,
//! a Unix domain socket on Linux and macOS. Shared by `tronic-print`, `tronic-mcp` and the app's
//! own server (`ipc.rs`), so it uses nothing but `std`. The bins include this file with `#[path]`.

// The app only uses the endpoint helpers; the bins use the rest.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Where the app listens on Windows. `ipc.rs` reads it from here.
pub const PIPE: &str = r"\\.\pipe\tronic-printer";

/// File name of the socket on Linux and macOS.
pub const SOCKET_FILE: &str = "printer.sock";

/// The socket's path, from the pieces that decide it (so a test can try every case on any OS).
///
/// * `runtime_dir` is `$XDG_RUNTIME_DIR` when it is set to an absolute path. It already belongs to
///   the user with mode 0700, so the socket sits right in a `tronic-printer` folder there.
/// * Otherwise (macOS has no such variable) it is `<temp dir>/tronic-<uid>/`, a folder the app
///   creates with mode 0700 and refuses to use if somebody else owns it.
pub fn socket_path_for(runtime_dir: Option<&str>, temp_dir: &Path, uid: u32) -> PathBuf {
    match runtime_dir.map(Path::new).filter(|d| d.is_absolute()) {
        Some(dir) => dir.join("tronic-printer").join(SOCKET_FILE),
        None => temp_dir.join(format!("tronic-{uid}")).join(SOCKET_FILE),
    }
}

/// The socket path for this user on this machine.
#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok();
    socket_path_for(runtime.as_deref(), &std::env::temp_dir(), current_uid())
}

/// The user id of this process.
#[cfg(unix)]
pub fn current_uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { getuid() }
}

#[cfg(unix)]
unsafe extern "C" {
    fn getuid() -> u32;
}

/// A human-readable name for the endpoint, for messages.
pub fn endpoint_name() -> String {
    #[cfg(windows)]
    return PIPE.to_string();
    #[cfg(unix)]
    return socket_path().display().to_string();
}

const NOT_RUNNING: &str = "the Tronic Printer app is not running: open it and press Connect";

/// The connection to the app: a pipe file on Windows, a socket elsewhere. Both read and write.
#[cfg(windows)]
pub type Conn = std::fs::File;
#[cfg(unix)]
pub type Conn = std::os::unix::net::UnixStream;

/// Open the endpoint, waiting a moment if the app is between two clients.
#[cfg(windows)]
pub fn connect() -> Result<Conn, String> {
    use std::fs::OpenOptions;
    use std::{thread, time::Duration};
    for _ in 0..20 {
        match OpenOptions::new().read(true).write(true).open(PIPE) {
            Ok(f) => return Ok(f),
            // ERROR_PIPE_BUSY: the server is between two clients.
            Err(e) if e.raw_os_error() == Some(231) => thread::sleep(Duration::from_millis(100)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(NOT_RUNNING.into()),
            Err(e) => return Err(format!("cannot reach the app: {e}")),
        }
    }
    Err("the app is busy, try again".into())
}

/// Open the endpoint, waiting a moment if the app's queue of clients is full.
#[cfg(unix)]
pub fn connect() -> Result<Conn, String> {
    use std::os::unix::net::UnixStream;
    use std::{thread, time::Duration};
    let path = socket_path();
    for _ in 0..20 {
        match UnixStream::connect(&path) {
            Ok(s) => return Ok(s),
            // Nothing there, or a stale file left by a crashed app.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused) => return Err(NOT_RUNNING.into()),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(100)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_goes_in_the_runtime_dir_when_there_is_one() {
        let (runtime, tmp) = if cfg!(windows) { (r"C:\run\user\1000", r"C:\Temp") } else { ("/run/user/1000", "/tmp") };
        let p = socket_path_for(Some(runtime), Path::new(tmp), 1000);
        assert_eq!(p, Path::new(runtime).join("tronic-printer").join("printer.sock"));
    }

    #[test]
    fn socket_falls_back_to_a_per_user_temp_folder() {
        let tmp = if cfg!(windows) { r"C:\Temp" } else { "/tmp" };
        for runtime in [None, Some(""), Some("relative/dir")] {
            let p = socket_path_for(runtime, Path::new(tmp), 501);
            assert_eq!(p, Path::new(tmp).join("tronic-501").join("printer.sock"), "runtime dir {runtime:?}");
        }
        assert_ne!(socket_path_for(None, Path::new(tmp), 1), socket_path_for(None, Path::new(tmp), 2), "users do not share a socket");
    }

    #[test]
    fn socket_path_stays_short_enough_for_sun_path() {
        // macOS allows 104 bytes, Linux 108. A typical macOS $TMPDIR is about 49 bytes long.
        let tmp = "/var/folders/zz/zyxvpxvq6csfxvn_n0000000000000/T";
        let p = socket_path_for(None, Path::new(tmp), 501);
        assert!(p.as_os_str().len() < 100, "{}", p.display());
    }
}
