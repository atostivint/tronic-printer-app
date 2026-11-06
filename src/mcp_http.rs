//! The app as an HTTP MCP server (the "streamable HTTP" transport): one endpoint,
//! `http://127.0.0.1:<port>/mcp`, where every POST carries a JSON-RPC message and the answer
//! comes back as JSON. It speaks exactly the protocol of `tronic-mcp` (`crate::mcp`), but needs no
//! helper program: any MCP host that takes a URL can be pointed straight at the running app.
//!
//! Only this computer can reach it (it listens on the loopback address) and web pages are
//! refused (an `Origin` or `Host` that is not local is a DNS-rebinding attempt), so a page you
//! visit cannot make your printer spit paper.

use std::sync::mpsc::Sender;

use rune::Waker;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::ipc::IpcMsg;
use crate::jobspec::Job;
use crate::mcp;

/// Largest request body accepted: a tool call is a few lines, never megabytes.
const MAX_BODY: usize = 1_000_000;
const MAX_HEADERS: usize = 32 * 1024;

pub struct Resp {
    pub status: u16,
    pub body: String,
    /// Tells the client which method the endpoint does take (for a 405).
    pub allow_post: bool,
}

impl Resp {
    fn new(status: u16, body: impl Into<String>) -> Self {
        Resp { status, body: body.into(), allow_post: false }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Error",
    }
}

/// `localhost`, `127.0.0.1` or `[::1]`, with or without a port.
fn is_local_host(host: &str) -> bool {
    let host = host.trim();
    let name = if host.starts_with('[') { host.split_inclusive(']').next().unwrap_or(host) } else { host.split(':').next().unwrap_or(host) };
    matches!(name.to_ascii_lowercase().as_str(), "localhost" | "127.0.0.1" | "[::1]")
}

/// An `Origin` header is `scheme://host[:port]`; browsers send it, scripts and MCP hosts do not.
fn is_local_origin(origin: &str) -> bool {
    origin.split_once("://").is_some_and(|(_, rest)| is_local_host(rest.split('/').next().unwrap_or(rest)))
}

/// Answer one HTTP request. Pure: the printing is done by `call`, so this is tested without a
/// socket or a printer.
pub fn route(method: &str, path: &str, host: Option<&str>, origin: Option<&str>, body: &str, call: &mut dyn FnMut(&Job, bool) -> Result<String, String>) -> Resp {
    if host.is_some_and(|h| !is_local_host(h)) || origin.is_some_and(|o| !is_local_origin(o)) {
        return Resp::new(403, "only local programs may use this endpoint");
    }
    let path = path.split('?').next().unwrap_or("");
    if path != "/mcp" && path != "/" {
        return Resp::new(404, "the MCP endpoint is /mcp");
    }
    if method != "POST" {
        // There are no server-initiated messages and no sessions to end.
        return Resp { allow_post: true, ..Resp::new(405, "POST JSON-RPC messages to this URL") };
    }
    let json_rpc = |line: &str, call: &mut dyn FnMut(&Job, bool) -> Result<String, String>| mcp::handle_line(line, call);
    let answer = match serde_json::from_str::<serde_json::Value>(body) {
        // A batch: one answer per request in it, none when it held only notifications.
        Ok(serde_json::Value::Array(items)) => {
            let answers: Vec<String> = items.iter().filter_map(|m| json_rpc(&m.to_string(), call)).collect();
            if answers.is_empty() { None } else { Some(format!("[{}]", answers.join(","))) }
        }
        _ => json_rpc(body, call),
    };
    match answer {
        Some(json) => Resp::new(200, json),
        None => Resp::new(202, ""),
    }
}

/// Start the server on its own thread. If the port is taken (another copy of the app), this copy
/// simply has no HTTP endpoint.
pub fn spawn(tx: Sender<IpcMsg>, waker: Waker) {
    let _ = std::thread::Builder::new().name("mcp-http".into()).spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return };
        rt.block_on(serve(tx, waker));
    });
}

async fn serve(tx: Sender<IpcMsg>, waker: Waker) {
    let Ok(listener) = TcpListener::bind(("127.0.0.1", crate::mcp_setup::http_port())).await else { return };
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        tokio::spawn(handle(stream, tx.clone(), waker.clone()));
    }
}

/// Run a job in the app (on the UI side) and wait for its one-line `OK …` / `ERR …` answer.
fn ask_app(tx: &Sender<IpcMsg>, waker: &Waker, job: &Job, force: bool) -> Result<String, String> {
    let (reply, answer) = oneshot::channel();
    tx.send(IpcMsg::Job { job: job.clone(), force, reply }).map_err(|_| "the app closed".to_string())?;
    waker();
    let line = answer.blocking_recv().map_err(|_| "the app closed".to_string())?;
    match line.trim().strip_prefix("OK") {
        Some(info) => Ok(info.trim().to_string()),
        None => Err(line.trim().trim_start_matches("ERR").trim().to_string()),
    }
}

async fn respond(stream: &mut tokio::net::TcpStream, resp: Resp) {
    let allow = if resp.allow_post { "Allow: POST\r\n" } else { "" };
    let content_type = if resp.body.starts_with(['{', '[']) { "application/json" } else { "text/plain; charset=utf-8" };
    let head = format!("HTTP/1.1 {} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{allow}Cache-Control: no-store\r\nConnection: close\r\n\r\n", resp.status, reason(resp.status), resp.body.len());
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(resp.body.as_bytes()).await;
    let _ = stream.flush().await;
}

async fn handle(mut stream: tokio::net::TcpStream, tx: Sender<IpcMsg>, waker: Waker) {
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.unwrap_or(0) == 0 {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());

    let (mut host, mut origin, mut length, mut expect_continue) = (None, None, None::<usize>, false);
    let mut seen = request_line.len();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await.unwrap_or(0);
        seen += n;
        if n == 0 || seen > MAX_HEADERS {
            drop(reader);
            return respond(&mut stream, Resp::new(400, "bad request")).await;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim().to_string();
            match name.trim().to_ascii_lowercase().as_str() {
                "host" => host = Some(value),
                "origin" => origin = Some(value),
                "content-length" => length = value.parse().ok(),
                "expect" => expect_continue = value.eq_ignore_ascii_case("100-continue"),
                _ => {}
            }
        }
    }

    let len = length.unwrap_or(0);
    if len > MAX_BODY {
        drop(reader);
        return respond(&mut stream, Resp::new(413, "that is too much to print")).await;
    }
    let mut bytes = vec![0u8; len];
    if len > 0 {
        if expect_continue {
            let _ = reader.get_mut().write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await;
        }
        if reader.read_exact(&mut bytes).await.is_err() {
            return;
        }
    }
    drop(reader);

    let body = String::from_utf8_lossy(&bytes).into_owned();
    let resp = tokio::task::spawn_blocking(move || {
        let mut call = |job: &Job, force: bool| ask_app(&tx, &waker, job, force);
        route(&method, &path, host.as_deref(), origin.as_deref(), &body, &mut call)
    })
    .await
    .unwrap_or_else(|_| Resp::new(500, "internal error"));
    respond(&mut stream, resp).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn post(host: Option<&str>, origin: Option<&str>, body: &str) -> Resp {
        let mut call = |job: &Job, _: bool| Ok::<String, String>(format!("printed a {}", job.kind));
        route("POST", "/mcp", host, origin, body, &mut call)
    }

    const LOCAL: Option<&str> = Some("127.0.0.1:7391");

    #[test]
    fn a_tool_call_over_http_prints_and_answers_in_json() {
        let r = post(LOCAL, None, r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"print_text","arguments":{"text":"hi"}}}"#);
        assert_eq!(r.status, 200);
        let v: Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["result"]["content"][0]["text"], "printed a text");
        let r = post(LOCAL, None, r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#);
        assert_eq!(serde_json::from_str::<Value>(&r.body).unwrap()["result"]["serverInfo"]["name"], "tronic-printer");
    }

    #[test]
    fn notifications_are_accepted_without_a_body_and_batches_answer_each_request() {
        let r = post(LOCAL, None, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        assert_eq!((r.status, r.body.as_str()), (202, ""));
        let r = post(LOCAL, None, r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":2,"method":"tools/list"}]"#);
        let v: Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!((r.status, v.as_array().map(Vec::len)), (200, Some(2)));
    }

    #[test]
    fn web_pages_and_foreign_hosts_are_refused() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        assert_eq!(post(LOCAL, Some("https://evil.example"), body).status, 403);
        assert_eq!(post(LOCAL, Some("null"), body).status, 403);
        assert_eq!(post(Some("rebind.evil.example:7391"), None, body).status, 403);
        assert_eq!(post(LOCAL, Some("http://localhost:3000"), body).status, 200, "a page served locally is fine");
        assert_eq!(post(Some("[::1]:7391"), None, body).status, 200);
        assert_eq!(post(None, None, body).status, 200, "HTTP/1.0 clients may send no Host");
    }

    #[test]
    fn only_post_on_the_mcp_path_is_served() {
        let mut call = |_: &Job, _: bool| Ok::<String, String>(String::new());
        let get = route("GET", "/mcp", LOCAL, None, "", &mut call);
        assert_eq!((get.status, get.allow_post), (405, true));
        assert_eq!(route("POST", "/other", LOCAL, None, "{}", &mut call).status, 404);
        assert_eq!(route("POST", "/mcp?x=1", LOCAL, None, r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#, &mut call).status, 200);
    }

    #[test]
    fn garbage_gets_a_json_rpc_error_not_a_crash() {
        let r = post(LOCAL, None, "{not json");
        assert_eq!(serde_json::from_str::<Value>(&r.body).unwrap()["error"]["code"], -32700);
    }
}
