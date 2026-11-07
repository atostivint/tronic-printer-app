//! `tronic-mcp`: an MCP server (stdio) that lets an AI assistant print on the Tronic Mini Pocket
//! Printer through the running Tronic Printer app.
//!
//! ```text
//! tronic-mcp --config      how to add it to Claude Code, Claude Desktop, Cursor and others
//! tronic-mcp               speak MCP on stdin/stdout (what an assistant runs)
//! ```

#[path = "../client.rs"]
mod client;
#[path = "../jobspec.rs"]
mod jobspec;
#[path = "../mcp.rs"]
mod mcp;
#[path = "../mcp_setup.rs"]
mod mcp_setup;
// Only the saved port is read here.
#[allow(dead_code)]
#[path = "../settings.rs"]
mod settings;

use std::io::{BufRead, Write};

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--config" | "--help" | "-h" | "help") => {
            print!("{}", mcp_setup::instructions(&mcp_setup::mcp_exe()));
            return;
        }
        Some("--version") => {
            println!("tronic-mcp {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        _ => {}
    }

    // Standard output belongs to the protocol: anything else (logs) goes to standard error.
    let mut call = |job: &jobspec::Job, force: bool| client::request("job", force, job.encode().as_bytes());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if let Some(response) = mcp::handle_line(&line, &mut call) {
            if writeln!(stdout, "{response}").and_then(|_| stdout.flush()).is_err() {
                break;
            }
        }
    }
}
