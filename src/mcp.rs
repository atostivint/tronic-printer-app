//! The MCP (Model Context Protocol) side of `tronic-mcp`: JSON-RPC over lines of text. An
//! assistant lists the tools, calls one, and this turns the call into a print job for the
//! running Tronic Printer app. Everything here is pure: the job is handed to a `call` function,
//! so the whole protocol is tested without a printer.

use serde_json::{Value, json};

use crate::jobspec::{Job, THEMES};

/// The MCP revision answered when the client does not ask for one we know.
pub const PROTOCOL: &str = "2024-11-05";
const SUPPORTED: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Prints on a small 58 mm thermal printer (384 dots wide, black and white, paper costs money). \
Prefer short tickets. print_text for text, print_code for a QR code or barcode, print_image for a picture file on this PC, \
printer_status to check the printer is connected before printing. The Tronic Printer app must be open and connected.";

fn reply(id: Option<Value>, result: Value) -> Option<String> {
    let id = id?;
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string())
}

fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Option<String> {
    Some(json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message.into() } }).to_string())
}

/// "receipt [store, address, date]: A shop receipt…" for every theme but plain.
fn theme_help() -> String {
    THEMES.iter().filter(|t| t.0 != "plain").map(|(id, fields, what)| if fields.is_empty() { format!("{id}: {what}") } else { format!("{id} [{}]: {what}", fields.replace(',', ", ")) }).collect::<Vec<_>>().join("; ")
}

pub fn tools() -> Value {
    let theme_ids: Vec<&str> = THEMES.iter().map(|t| t.0).collect();
    let copies = json!({ "type": "integer", "minimum": 1, "maximum": 5, "description": "How many copies (default 1)." });
    let force = json!({ "type": "boolean", "description": "Allow more than 100 cm of paper for one ticket." });
    json!([
        {
            "name": "print_text",
            "description": "Print text on the thermal printer. Long text wraps to the 58 mm paper; about 30 characters fit on a line at the default size.",
            "inputSchema": { "type": "object", "required": ["text"], "properties": {
                "text": { "type": "string", "description": "The text to print. New lines are kept." },
                "font": { "type": "string", "enum": ["sans", "bold", "mono", "serif", "script"], "description": "Default sans. Use mono for tables or code." },
                "size": { "type": "string", "enum": ["small", "medium", "large", "xl"], "description": "Default medium." },
                "align": { "type": "string", "enum": ["left", "center", "right"] },
                "invert": { "type": "boolean", "description": "White text on a black background." },
                "theme": { "type": "string", "description": format!("Dress the text as a document instead of plain text (font, size and align are then ignored). The text is the main content; some themes read it line by line (receipt: 'Coffee 2.50'). Built-in themes ({}), with their optional fields: {}. The user may have added more as files in the app's themes folder: an unknown id is answered with the full list.", theme_ids.join(", "), theme_help()) },
                "fields": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Optional extras for the theme, by key, for example {\"store\": \"FRESH MART\"} for receipt. Left out, each has a sensible default." },
                "copies": copies, "force": force } }
        },
        {
            "name": "print_code",
            "description": "Print a QR code or a barcode (Code 128, Code 39, EAN-13) with the text under it.",
            "inputSchema": { "type": "object", "required": ["text"], "properties": {
                "text": { "type": "string", "description": "What to encode: a link or text for QR; digits for EAN-13 (12 or 13 of them)." },
                "type": { "type": "string", "enum": ["qr", "code128", "code39", "ean13"], "description": "Default qr." },
                "error_correction": { "type": "string", "enum": ["L", "M", "Q", "H"], "description": "QR only. Default M." },
                "caption": { "type": "boolean", "description": "Print the text under the code (default true)." },
                "copies": copies, "force": force } }
        },
        {
            "name": "print_image",
            "description": "Print a picture file (png, jpg, bmp, gif, webp) from this PC, scaled to the paper width and dithered to black and white.",
            "inputSchema": { "type": "object", "required": ["path"], "properties": {
                "path": { "type": "string", "description": "Absolute path of the picture on this PC." },
                "look": { "type": "string", "enum": ["photo", "meme", "line_art", "soft"], "description": "photo (default) for photographs, meme for high contrast, line_art for drawings and text, soft for gradients." },
                "brightness": { "type": "number", "minimum": -1, "maximum": 1, "description": "-1 darker to 1 lighter. Default 0." },
                "contrast": { "type": "number", "minimum": -1, "maximum": 1, "description": "-1 flatter to 1 punchier. Default 0." },
                "invert": { "type": "boolean" },
                "rotate": { "type": "integer", "enum": [0, 90, 180, 270], "description": "Degrees clockwise." },
                "flip": { "type": "boolean", "description": "Mirror left to right." },
                "copies": copies, "force": force } }
        },
        {
            "name": "printer_status",
            "description": "Whether the Tronic Printer app is running and connected to the printer, its battery, model and whether it is busy.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn text_arg(args: &Value, key: &str) -> Option<String> {
    match args.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// The job (and whether `force` was asked) for a tool call, or why the call is wrong.
pub fn job_for(tool: &str, args: &Value) -> Result<(Job, bool), String> {
    let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
    let (mut job, required, body_key) = match tool {
        "print_text" => (Job::new("text"), true, "text"),
        "print_code" => (Job::new("code"), true, "text"),
        "print_image" => (Job::new("image"), true, "path"),
        "printer_status" => return Ok((Job::new("status"), false)),
        other => return Err(format!("unknown tool '{other}'")),
    };
    let body = text_arg(args, body_key);
    match body {
        Some(b) if !b.trim().is_empty() => job = job.body(b),
        _ if required => return Err(format!("'{body_key}' is required")),
        _ => {}
    }
    for key in ["font", "size", "align", "type", "error_correction", "look", "brightness", "contrast", "rotate", "copies", "invert", "flip", "caption", "theme"] {
        if let Some(v) = text_arg(args, key) {
            job = job.param(key, v);
        }
    }
    if tool == "print_text" {
        if let Some(Value::Object(fields)) = args.get("fields") {
            for (k, v) in fields {
                if let Some(v) = text_arg(&json!({ "v": v }), "v") {
                    job = job.param(k, v);
                }
            }
        }
    }
    Ok((job, force))
}

/// Handle one line from the client. `call` runs a job against the app and returns its answer
/// (`Err` when the app could not be reached or refused). `None` means there is nothing to send
/// back (a notification, or a reply to us).
pub fn handle_line(line: &str, call: &mut dyn FnMut(&Job, bool) -> Result<String, String>) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return error(None, -32700, format!("parse error: {e}")),
    };
    let id = msg.get("id").cloned();
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        // A response to something we never asked: ignore it.
        return None;
    };
    match method {
        "initialize" => {
            let asked = msg.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL);
            let version = if SUPPORTED.contains(&asked) { asked } else { PROTOCOL };
            reply(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "tronic-printer", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS
                }),
            )
        }
        "ping" => reply(id, json!({})),
        "tools/list" => reply(id, json!({ "tools": tools() })),
        "tools/call" => {
            let name = msg.pointer("/params/name").and_then(Value::as_str).unwrap_or("");
            let args = msg.pointer("/params/arguments").cloned().unwrap_or_else(|| json!({}));
            let (text, is_error) = match job_for(name, &args) {
                Err(e) => (e, true),
                Ok((job, force)) => match call(&job, force) {
                    Ok(answer) => (if answer.is_empty() { "done".to_string() } else { answer }, false),
                    Err(e) => (e, true),
                },
            };
            reply(id, json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
        }
        m if m.starts_with("notifications/") => None,
        other => match id {
            Some(_) => error(id, -32601, format!("method not found: {other}")),
            None => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str) -> Option<Value> {
        let mut seen = Vec::new();
        run_with(line, &mut seen)
    }

    fn run_with(line: &str, seen: &mut Vec<(Job, bool)>) -> Option<Value> {
        let mut call = |job: &Job, force: bool| {
            seen.push((job.clone(), force));
            if job.kind == "status" { Ok("connected=true battery=87%".to_string()) } else { Ok("printing 1 × 3.2 cm".to_string()) }
        };
        handle_line(line, &mut call).map(|s| serde_json::from_str(&s).unwrap())
    }

    #[test]
    fn initialize_answers_with_the_clients_revision_when_known_and_ours_otherwise() {
        let r = run(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap();
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "tronic-printer");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let r = run(r#"{"jsonrpc":"2.0","id":"a","method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#).unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL);
        assert_eq!(r["id"], "a", "string ids are echoed back");
    }

    #[test]
    fn notifications_and_stray_replies_get_no_answer() {
        assert!(run(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        assert!(run(r#"{"jsonrpc":"2.0","id":3,"result":{}}"#).is_none());
        assert!(run("").is_none());
    }

    #[test]
    fn the_four_tools_are_listed_with_schemas() {
        let r = run(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let names: Vec<&str> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["print_text", "print_code", "print_image", "printer_status"]);
        for t in r["result"]["tools"].as_array().unwrap() {
            assert_eq!(t["inputSchema"]["type"], "object");
            assert!(t["description"].as_str().unwrap().len() > 20);
        }
    }

    #[test]
    fn a_text_call_becomes_a_job_with_its_settings() {
        let mut seen = Vec::new();
        let r = run_with(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"print_text","arguments":{"text":"Hello\nWorld","font":"mono","invert":true,"copies":2,"force":true}}}"#,
            &mut seen,
        )
        .unwrap();
        assert_eq!(r["result"]["isError"], false);
        assert_eq!(r["result"]["content"][0]["text"], "printing 1 × 3.2 cm");
        let (job, force) = &seen[0];
        assert!(*force);
        assert_eq!((job.kind.as_str(), job.body.as_str(), job.get("font"), job.get("copies")), ("text", "Hello\nWorld", Some("mono"), Some("2")));
        assert!(job.flag("invert"));
        let (job, _) = job_for("print_text", &json!({ "text": "Tea 1.00", "theme": "receipt", "fields": { "store": "CAFE", "n": 3 } })).unwrap();
        assert_eq!((job.get("theme"), job.get("store"), job.get("n")), (Some("receipt"), Some("CAFE"), Some("3")));
        let print_text = tools().as_array().unwrap().iter().find(|t| t["name"] == "print_text").cloned().unwrap();
        let theme = &print_text["inputSchema"]["properties"]["theme"];
        assert!(theme["description"].as_str().unwrap().contains("parchment"), "built-in ids are listed");
        assert!(theme["description"].as_str().unwrap().contains("receipt [store, address, date]"));
    }

    #[test]
    fn image_and_code_calls_map_their_own_arguments() {
        let (job, _) = job_for("print_image", &json!({ "path": r"C:\pics\dog.png", "look": "meme", "brightness": 0.25, "rotate": 90 })).unwrap();
        assert_eq!((job.kind.as_str(), job.body.as_str(), job.get("look")), ("image", r"C:\pics\dog.png", Some("meme")));
        assert_eq!(job.number("brightness"), Some(0.25));
        assert_eq!(job.number("rotate"), Some(90.0));
        let (job, _) = job_for("print_code", &json!({ "text": "https://example.com", "type": "qr", "caption": false })).unwrap();
        assert_eq!((job.kind.as_str(), job.get("type")), ("code", Some("qr")));
        assert!(!job.flag("caption") && job.get("caption") == Some("false"));
    }

    #[test]
    fn mistakes_come_back_as_tool_errors_the_assistant_can_read() {
        let r = run(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"print_text","arguments":{}}}"#).unwrap();
        assert_eq!(r["result"]["isError"], true);
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("'text' is required"));
        let r = run(r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"print_hologram","arguments":{}}}"#).unwrap();
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("unknown tool"));
        let mut call = |_: &Job, _: bool| Err::<String, String>("the Tronic Printer app is not running: open it and press Connect".to_string());
        let out = handle_line(r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"printer_status"}}"#, &mut call).unwrap();
        let r: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(r["result"]["isError"], true);
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("not running"));
    }

    #[test]
    fn unknown_methods_and_garbage_get_json_rpc_errors() {
        let r = run(r#"{"jsonrpc":"2.0","id":8,"method":"resources/list"}"#).unwrap();
        assert_eq!(r["error"]["code"], -32601);
        let r = run("{not json").unwrap();
        assert_eq!(r["error"]["code"], -32700);
        assert!(run(r#"{"jsonrpc":"2.0","method":"something/else"}"#).is_none(), "an unknown notification is ignored");
    }
}
