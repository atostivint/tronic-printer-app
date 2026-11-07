//! Minimal blocking client for a self-hosted XBackBone instance.
//!
//! What XBackBone really offers (verified in the repository, see the report):
//!
//! * Legacy line (3.x, latest 3.8.2): `POST {base}/upload`, multipart with the
//!   fields `token` and `file`; JSON reply `{message, version, url, raw_url}`
//!   (HTTP 201). Raw file: `{base}/{user}/{code}/raw[.ext]`, forced download:
//!   `/download`. There is NO token-based way to list uploads: the dashboard
//!   (`/home`) is HTML behind a cookie session.
//! * Rewrite (`master`, Laravel, unreleased): `POST {base}/api/v1/upload`,
//!   `Authorization: Bearer <token>`, multipart `file` (+ optional `name`),
//!   reply `{"data": {raw_url, preview_ext_url, deletion_url, ...}}`; and
//!   `DELETE /api/v1/resources/{code}`. Again no listing endpoint.
//!
//! Therefore there is no `list` function: the gallery has to remember the
//! URLs itself (uploads made from the app, or URLs pasted by the user) and
//! use [`download`], which accepts a share URL or a raw URL.
#![allow(dead_code)]

use std::fmt;
use std::io::Read;
use std::time::Duration;

use serde_json::Value;

/// Hard cap for both uploads and downloads.
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// Which server API to talk to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Api {
    /// Try the legacy 3.x endpoint, fall back to the v1 API when it does not exist.
    #[default]
    Auto,
    /// XBackBone 3.x: `POST /upload`, token in the form.
    Legacy,
    /// XBackBone rewrite: `POST /api/v1/upload`, bearer token.
    V1,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    /// Normalised: scheme + host[:port][/subpath], no trailing slash.
    base_url: String,
    token: String,
    pub api: Api,
}

// The token must never show up in logs.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("base_url", &self.base_url)
            .field("token", &"<hidden>")
            .field("api", &self.api)
            .finish()
    }
}

impl Config {
    pub fn new(base_url: &str, token: &str) -> Result<Config, XbbError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(XbbError::BadConfig("the token is empty".into()));
        }
        if token.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(XbbError::BadConfig("the token contains spaces".into()));
        }
        Ok(Config { base_url: normalize_base_url(base_url)?, token: token.to_string(), api: Api::Auto })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Plain http: the token travels in clear text. The UI may want to warn.
    pub fn is_insecure(&self) -> bool {
        self.base_url.starts_with("http://")
    }

    pub fn upload_url(&self, api: Api) -> String {
        match api {
            Api::V1 => format!("{}/api/v1/upload", self.base_url),
            _ => format!("{}/upload", self.base_url),
        }
    }
}

/// `"example.org/xbb/ "` -> `"https://example.org/xbb"`.
pub fn normalize_base_url(input: &str) -> Result<String, XbbError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(XbbError::BadConfig("the server address is empty".into()));
    }
    let with_scheme = if s.contains("://") { s.to_string() } else { format!("https://{s}") };
    let u = url::Url::parse(&with_scheme)
        .map_err(|_| XbbError::BadConfig("the server address is not a valid URL".into()))?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err(XbbError::BadConfig("the server address must start with http:// or https://".into()));
    }
    if u.host_str().is_none() {
        return Err(XbbError::BadConfig("the server address has no host".into()));
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err(XbbError::BadConfig("do not put a login in the address".into()));
    }
    let mut out = format!("{}://{}", u.scheme(), u.host_str().unwrap());
    if let Some(p) = u.port() {
        out.push_str(&format!(":{p}"));
    }
    // keep a sub-path install (https://host/xbb), drop query/fragment/trailing slashes
    let mut path = u.path().trim_end_matches('/').to_string();
    for suffix in ["/upload", "/api/v1/upload"] {
        if let Some(p) = path.strip_suffix(suffix) {
            path = p.to_string();
        }
    }
    out.push_str(&path);
    Ok(out)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uploaded {
    /// Public share page.
    pub url: String,
    /// Direct file URL when the server gave one.
    pub raw_url: Option<String>,
    /// Pre-authenticated deletion link (v1 API only).
    pub deletion_url: Option<String>,
}

impl Uploaded {
    /// Best URL to download the picture again.
    pub fn download_url(&self) -> &str {
        self.raw_url.as_deref().unwrap_or(&self.url)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XbbError {
    BadConfig(String),
    BadToken,
    AccountDisabled,
    Unreachable(String),
    Timeout,
    TooBig,
    NotAnImage,
    NotFound,
    QuotaExceeded,
    Maintenance,
    RedirectedUseHttps,
    /// Server answered with an error; carries its own one-line message.
    Server(u16, String),
    BadReply(String),
}

impl fmt::Display for XbbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XbbError::BadConfig(m) => write!(f, "XBackBone settings: {m}"),
            XbbError::BadToken => write!(f, "XBackBone refused the token (check it in your profile)"),
            XbbError::AccountDisabled => write!(f, "XBackBone account is disabled"),
            XbbError::Unreachable(h) => write!(f, "XBackBone is unreachable ({h})"),
            XbbError::Timeout => write!(f, "XBackBone did not answer in time"),
            XbbError::TooBig => write!(f, "The picture is too big (limit 16 MB)"),
            XbbError::NotAnImage => write!(f, "That file is not a picture (png, jpeg, gif, webp or bmp)"),
            XbbError::NotFound => write!(f, "XBackBone does not know that file (private or deleted?)"),
            XbbError::QuotaExceeded => write!(f, "XBackBone disk quota exceeded"),
            XbbError::Maintenance => write!(f, "XBackBone is under maintenance"),
            XbbError::RedirectedUseHttps => write!(f, "XBackBone redirected the upload: use the exact https:// address"),
            XbbError::Server(c, m) => write!(f, "XBackBone error {c}: {m}"),
            XbbError::BadReply(m) => write!(f, "Unexpected XBackBone reply: {m}"),
        }
    }
}

impl std::error::Error for XbbError {}

/// Map an HTTP error status (and the JSON `message`, if any) of an upload.
pub fn map_upload_status(status: u16, message: Option<&str>) -> XbbError {
    let msg = message.unwrap_or("").trim();
    match status {
        301 | 302 | 303 | 307 | 308 => XbbError::RedirectedUseHttps,
        401 | 403 if msg.to_ascii_lowercase().contains("disabled") => XbbError::AccountDisabled,
        401 | 403 => XbbError::BadToken,
        404 if msg.to_ascii_lowercase().contains("token") => XbbError::BadToken,
        404 | 405 => XbbError::NotFound,
        413 => XbbError::TooBig,
        400 if msg.to_ascii_lowercase().contains("too large") => XbbError::TooBig,
        503 => XbbError::Maintenance,
        507 => XbbError::QuotaExceeded,
        c => XbbError::Server(c, if msg.is_empty() { "no details".into() } else { one_line(msg) }),
    }
}

fn one_line(s: &str) -> String {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 120 { t.chars().take(120).collect::<String>() + "..." } else { t }
}

/// Parse the JSON reply of an upload (legacy flat object, or v1 `{"data": {...}}`).
pub fn parse_upload_reply(body: &str) -> Result<Uploaded, XbbError> {
    let v: Value = serde_json::from_str(body).map_err(|_| XbbError::BadReply("not JSON".into()))?;
    let obj = v.get("data").filter(|d| d.is_object()).unwrap_or(&v);
    let s = |k: &str| obj.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    let raw_url = s("raw_url");
    let url = s("url").or_else(|| s("preview_ext_url")).or_else(|| s("preview_url")).or_else(|| raw_url.clone());
    match url {
        Some(url) => Ok(Uploaded { url, raw_url, deletion_url: s("deletion_url") }),
        None => {
            let m = v.get("message").and_then(Value::as_str).unwrap_or("no url in reply");
            Err(XbbError::BadReply(one_line(m)))
        }
    }
}

fn json_message(body: &str) -> Option<String> {
    serde_json::from_str::<Value>(body).ok()?.get("message")?.as_str().map(str::to_string)
}

/// Image kind from magic bytes (the server's content-type is not trusted).
pub fn sniff_image_ext(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("gif")
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else if b.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

/// Turn a share URL into the URL of the raw file when it is one of ours
/// (`{base}/{user}/{code}[.ext]` -> `{base}/{user}/{code}/raw`). Anything else
/// (already raw, other host, v1 `raw_url`) is returned unchanged.
pub fn raw_url_for(cfg: &Config, share_url: &str) -> Result<String, XbbError> {
    let u = url::Url::parse(share_url.trim()).map_err(|_| XbbError::BadConfig("not a valid picture address".into()))?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err(XbbError::BadConfig("the picture address must be http(s)".into()));
    }
    let base = url::Url::parse(&cfg.base_url).expect("normalised");
    let same_origin = u.host_str() == base.host_str() && u.port_or_known_default() == base.port_or_known_default();
    if same_origin {
        let prefix = base.path().trim_end_matches('/');
        if let Some(rest) = u.path().strip_prefix(prefix) {
            let segs: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
            if segs.len() == 2 && segs[0] != "api" {
                let code = segs[1].split('.').next().unwrap_or(segs[1]);
                return Ok(format!("{}/{}/{}/raw", cfg.base_url, segs[0], code));
            }
        }
    }
    Ok(u.to_string())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout(Duration::from_secs(60))
        .redirects(0)
        .user_agent("tronic-printer")
        .build()
}

fn map_transport(e: &ureq::Transport, host: &str) -> XbbError {
    use ureq::ErrorKind::*;
    match e.kind() {
        Io if e.to_string().to_ascii_lowercase().contains("timed out") => XbbError::Timeout,
        Dns | ConnectionFailed | Io | ProxyConnect | InvalidProxyUrl => XbbError::Unreachable(host.to_string()),
        InsecureRequestHttpsOnly => XbbError::BadConfig("https is required".into()),
        _ => XbbError::Unreachable(format!("{host}: {}", one_line(&e.to_string()))),
    }
}

fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| "server".into())
}

fn read_capped(resp: ureq::Response, cap: usize) -> Result<Vec<u8>, XbbError> {
    let mut out = Vec::new();
    resp.into_reader()
        .take(cap as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| if e.to_string().to_ascii_lowercase().contains("timed out") { XbbError::Timeout } else { XbbError::Unreachable("connection lost".into()) })?;
    if out.len() > cap { Err(XbbError::TooBig) } else { Ok(out) }
}

/// Build a multipart/form-data body. Returns (content-type, body).
pub fn multipart(fields: &[(&str, &str)], filename: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = format!("----tronic{:016x}{:08x}", bytes.len() as u64 ^ 0x9E37_79B9_7F4A_7C15, fields.len());
    let mut body = Vec::with_capacity(bytes.len() + 512);
    for (k, v) in fields {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n").as_bytes());
    }
    let safe: String = filename.chars().filter(|c| !matches!(c, '"' | '\r' | '\n' | '\\')).collect();
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{safe}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// Make sure the file name has an image extension (XBackBone derives the
/// stored extension from it).
fn ensure_extension(filename: &str, bytes: &[u8]) -> Result<String, XbbError> {
    let ext = sniff_image_ext(bytes).ok_or(XbbError::NotAnImage)?;
    let stem = filename.rsplit(['/', '\\']).next().unwrap_or(filename).trim();
    let stem = stem.rsplit_once('.').map(|(s, _)| s).unwrap_or(stem);
    let stem = if stem.is_empty() { "picture" } else { stem };
    Ok(format!("{stem}.{ext}"))
}

fn upload_once(cfg: &Config, api: Api, name: &str, bytes: &[u8]) -> Result<Uploaded, XbbError> {
    let endpoint = cfg.upload_url(api);
    let (ctype, body) = if api == Api::V1 { multipart(&[], name, bytes) } else { multipart(&[("token", &cfg.token)], name, bytes) };
    let mut req = agent().post(&endpoint).set("Content-Type", &ctype).set("Accept", "application/json");
    if api == Api::V1 {
        req = req.set("Authorization", &format!("Bearer {}", cfg.token));
    }
    match req.send_bytes(&body) {
        Ok(resp) => {
            let text = String::from_utf8_lossy(&read_capped(resp, 1 << 20)?).into_owned();
            parse_upload_reply(&text)
        }
        Err(ureq::Error::Status(code, resp)) => {
            let text = String::from_utf8_lossy(&read_capped(resp, 1 << 16).unwrap_or_default()).into_owned();
            Err(map_upload_status(code, json_message(&text).as_deref()))
        }
        Err(ureq::Error::Transport(t)) => Err(map_transport(&t, &host_of(&endpoint))),
    }
}

/// Upload a picture. The name is only a hint; the extension comes from the bytes.
pub fn upload(cfg: &Config, filename: &str, bytes: &[u8]) -> Result<Uploaded, XbbError> {
    if bytes.len() > MAX_BYTES {
        return Err(XbbError::TooBig);
    }
    let name = ensure_extension(filename, bytes)?;
    match cfg.api {
        Api::Legacy => upload_once(cfg, Api::Legacy, &name, bytes),
        Api::V1 => upload_once(cfg, Api::V1, &name, bytes),
        Api::Auto => match upload_once(cfg, Api::Legacy, &name, bytes) {
            // legacy route missing (v1-only server answers 404/405 without our JSON "message")
            Err(XbbError::NotFound) => upload_once(cfg, Api::V1, &name, bytes),
            other => other,
        },
    }
}

/// Download a picture from a share URL or a raw URL (no token is sent: XBackBone
/// serves public uploads without authentication). Capped at 16 MB, must be a picture.
pub fn download(cfg: &Config, url: &str) -> Result<Vec<u8>, XbbError> {
    let target = raw_url_for(cfg, url)?;
    let host = host_of(&target);
    // Follow a couple of redirects by hand-free means: allow up to 3 here.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout(Duration::from_secs(60))
        .redirects(3)
        .user_agent("tronic-printer")
        .build();
    match agent.get(&target).call() {
        Ok(resp) => {
            let ctype = resp.content_type().to_ascii_lowercase();
            if !(ctype.starts_with("image/") || ctype == "application/octet-stream" || ctype.is_empty()) {
                return Err(XbbError::NotAnImage);
            }
            if resp.header("Content-Length").and_then(|v| v.parse::<usize>().ok()).is_some_and(|n| n > MAX_BYTES) {
                return Err(XbbError::TooBig);
            }
            let bytes = read_capped(resp, MAX_BYTES)?;
            sniff_image_ext(&bytes).ok_or(XbbError::NotAnImage)?;
            Ok(bytes)
        }
        Err(ureq::Error::Status(404, _)) | Err(ureq::Error::Status(403, _)) => Err(XbbError::NotFound),
        Err(ureq::Error::Status(c, _)) => Err(XbbError::Server(c, "download failed".into())),
        Err(ureq::Error::Transport(t)) => Err(map_transport(&t, &host)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_base_url() {
        assert_eq!(normalize_base_url(" https://x.org/ ").unwrap(), "https://x.org");
        assert_eq!(normalize_base_url("x.org").unwrap(), "https://x.org");
        assert_eq!(normalize_base_url("http://x.org:8080//").unwrap(), "http://x.org:8080");
        assert_eq!(normalize_base_url("https://x.org/xbb/upload").unwrap(), "https://x.org/xbb");
        assert_eq!(normalize_base_url("https://x.org/xbb/?a=1#f").unwrap(), "https://x.org/xbb");
        assert!(normalize_base_url("").is_err());
        assert!(normalize_base_url("ftp://x.org").is_err());
        assert!(normalize_base_url("https://user:pw@x.org").is_err());
    }

    #[test]
    fn config_hides_token_and_builds_urls() {
        let c = Config::new("x.org/xbb/", " tok123 ").unwrap();
        assert!(!format!("{c:?}").contains("tok123"));
        assert_eq!(c.upload_url(Api::Legacy), "https://x.org/xbb/upload");
        assert_eq!(c.upload_url(Api::V1), "https://x.org/xbb/api/v1/upload");
        assert!(!c.is_insecure());
        assert!(Config::new("http://x.org", "t").unwrap().is_insecure());
        assert!(Config::new("x.org", "  ").is_err());
        assert!(Config::new("x.org", "a b").is_err());
    }

    #[test]
    fn parses_legacy_reply() {
        // shape produced by UploadController::saveMedia (3.x)
        let r = r#"{"message":"OK","version":"3.8.2","url":"https://x.org/aBcDe/FgHiJ.png","raw_url":"https://x.org/aBcDe/FgHiJ/raw.png"}"#;
        let u = parse_upload_reply(r).unwrap();
        assert_eq!(u.url, "https://x.org/aBcDe/FgHiJ.png");
        assert_eq!(u.download_url(), "https://x.org/aBcDe/FgHiJ/raw.png");
        assert_eq!(u.deletion_url, None);
    }

    #[test]
    fn parses_v1_reply() {
        // shape produced by ResourceResource (rewrite)
        let r = r#"{"data":{"id":1,"filename":"a.png","raw_url":"https://x.org/r/abc.png","preview_ext_url":"https://x.org/abc.png","deletion_url":"https://x.org/abc/delete/t"}}"#;
        let u = parse_upload_reply(r).unwrap();
        assert_eq!(u.url, "https://x.org/abc.png");
        assert_eq!(u.raw_url.as_deref(), Some("https://x.org/r/abc.png"));
        assert_eq!(u.deletion_url.as_deref(), Some("https://x.org/abc/delete/t"));
    }

    #[test]
    fn rejects_bad_replies() {
        assert!(matches!(parse_upload_reply("<html>"), Err(XbbError::BadReply(_))));
        assert!(matches!(parse_upload_reply(r#"{"message":"nope"}"#), Err(XbbError::BadReply(_))));
    }

    #[test]
    fn maps_errors() {
        assert_eq!(map_upload_status(404, Some("Token specified not found.")), XbbError::BadToken);
        assert_eq!(map_upload_status(404, None), XbbError::NotFound);
        assert_eq!(map_upload_status(401, Some("Account disabled.")), XbbError::AccountDisabled);
        assert_eq!(map_upload_status(401, Some("x")), XbbError::BadToken);
        assert_eq!(map_upload_status(400, Some("File too large (post_max_size too low?).")), XbbError::TooBig);
        assert_eq!(map_upload_status(413, None), XbbError::TooBig);
        assert_eq!(map_upload_status(507, Some("User disk quota exceeded.")), XbbError::QuotaExceeded);
        assert_eq!(map_upload_status(503, Some("Endpoint under maintenance.")), XbbError::Maintenance);
        assert_eq!(map_upload_status(301, None), XbbError::RedirectedUseHttps);
        let m = map_upload_status(409, Some("Custom url code already exists.")).to_string();
        assert!(m.contains("409") && !m.contains('\n'));
    }

    #[test]
    fn sniffs_images() {
        assert_eq!(sniff_image_ext(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]), Some("png"));
        assert_eq!(sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_image_ext(b"GIF89a.."), Some("gif"));
        assert_eq!(sniff_image_ext(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_image_ext(b"<!DOCTYPE html>"), None);
        assert_eq!(sniff_image_ext(b"<svg"), None);
    }

    #[test]
    fn converts_share_urls_to_raw() {
        let c = Config::new("https://x.org", "t").unwrap();
        assert_eq!(raw_url_for(&c, "https://x.org/aBcDe/FgHiJ.png").unwrap(), "https://x.org/aBcDe/FgHiJ/raw");
        assert_eq!(raw_url_for(&c, "https://x.org/aBcDe/FgHiJ").unwrap(), "https://x.org/aBcDe/FgHiJ/raw");
        assert_eq!(raw_url_for(&c, "https://x.org/aBcDe/FgHiJ/raw.png").unwrap(), "https://x.org/aBcDe/FgHiJ/raw.png");
        assert_eq!(raw_url_for(&c, "https://other.net/a/b.png").unwrap(), "https://other.net/a/b.png");
        assert!(raw_url_for(&c, "file:///etc/passwd").is_err());
        let sub = Config::new("https://x.org/xbb", "t").unwrap();
        assert_eq!(raw_url_for(&sub, "https://x.org/xbb/u/c.jpg").unwrap(), "https://x.org/xbb/u/c/raw");
    }

    #[test]
    fn multipart_shape() {
        let (ct, body) = multipart(&[("token", "T")], "a\"b.png", b"DATA");
        let b = ct.split("boundary=").nth(1).unwrap();
        let s = String::from_utf8_lossy(&body);
        assert!(s.contains("name=\"token\"\r\n\r\nT\r\n"));
        assert!(s.contains("name=\"file\"; filename=\"ab.png\""));
        assert!(s.ends_with(&format!("--{b}--\r\n")));
    }

    #[test]
    fn upload_rejects_non_images_and_big_files_offline() {
        let c = Config::new("https://x.invalid", "t").unwrap();
        assert_eq!(upload(&c, "a.txt", b"hello").unwrap_err(), XbbError::NotAnImage);
        let mut big = vec![0u8; MAX_BYTES + 1];
        big[..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        assert_eq!(upload(&c, "a.jpg", &big).unwrap_err(), XbbError::TooBig);
        assert_eq!(ensure_extension("dir\\meme.PNG", b"GIF89a").unwrap(), "meme.gif");
    }
}
