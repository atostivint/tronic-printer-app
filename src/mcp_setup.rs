//! The words and snippets for plugging the printer into an AI assistant over MCP. Shown in the
//! app (Terminal tab) and printed by `tronic-mcp --config`, so it uses nothing but `std`.

// The app shows the short snippets; `tronic-mcp --config` prints the whole text.
#![allow(dead_code)]

/// `tronic-mcp` (`tronic-mcp.exe` on Windows), next to the program that is running now.
pub fn mcp_exe() -> String {
    let name = format!("tronic-mcp{}", std::env::consts::EXE_SUFFIX);
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(&name)))
        .map(|p| p.display().to_string())
        .unwrap_or(name)
}

pub const DEFAULT_PORT: u16 = 7391;

/// 0 until first asked; then the port in use (changed live from the app's settings).
static PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// The port of the app's own HTTP MCP endpoint: `TRONIC_MCP_PORT` if set, else the one saved in
/// the app's settings, else 7391.
pub fn http_port() -> u16 {
    use std::sync::atomic::Ordering::Relaxed;
    let cached = PORT.load(Relaxed);
    if cached != 0 {
        return cached;
    }
    let env = std::env::var("TRONIC_MCP_PORT").ok().and_then(|p| p.parse::<u16>().ok()).filter(|p| *p != 0);
    let port = env.or_else(|| crate::settings::load().port).unwrap_or(DEFAULT_PORT);
    PORT.store(port, Relaxed);
    port
}

/// Use another port from now on (the caller saves it and rebinds the server).
pub fn set_http_port(port: u16) {
    PORT.store(port, std::sync::atomic::Ordering::Relaxed);
}

/// Where an MCP host that takes a URL should connect: the running app itself.
pub fn http_url() -> String {
    format!("http://127.0.0.1:{}/mcp", http_port())
}

/// The one-liner for Claude Code, over HTTP.
pub fn claude_code_http_command() -> String {
    format!("claude mcp add --transport http tronic-printer {}", http_url())
}

/// The JSON block for hosts that take a URL (Cursor, VS Code, Windsurf and others).
pub fn json_http_config() -> String {
    format!("{{\n  \"mcpServers\": {{\n    \"tronic-printer\": {{\n      \"url\": \"{}\"\n    }}\n  }}\n}}", http_url())
}

/// The one-liner for Claude Code.
pub fn claude_code_command(exe: &str) -> String {
    format!("claude mcp add tronic-printer -- \"{exe}\"")
}

/// The JSON block for Claude Desktop, Cursor, Windsurf, VS Code and most other MCP hosts.
pub fn json_config(exe: &str) -> String {
    let escaped = exe.replace('\\', "\\\\").replace('"', "\\\"");
    format!("{{\n  \"mcpServers\": {{\n    \"tronic-printer\": {{\n      \"command\": \"{escaped}\",\n      \"args\": []\n    }}\n  }}\n}}")
}

/// Everything `tronic-mcp --config` prints.
pub fn instructions(exe: &str) -> String {
    format!(
        "Tronic Printer MCP server\n\
         =========================\n\
         \n\
         An AI assistant can print on your Tronic Mini Pocket Printer through this server. Keep the\n\
         Tronic Printer app open and connected (the light turns green): the server talks to it.\n\
         \n\
         Tools: print_text, print_code (QR and barcodes), print_image, printer_status.\n\
         \n\
         Over HTTP (no helper program)\n\
         -----------------------------\n\
         While the app is open it is itself an MCP server at {url}\n\
         (this computer only). Any host that takes a URL can use it, and your own apps can POST\n\
         JSON-RPC to it.\n\
         \n\
         Claude Code:  {claude_http}\n\
         \n\
         Cursor, VS Code, Windsurf and others:\n\
         \n\
         {json_http}\n\
         \n\
         Over stdio (the helper program)\n\
         -------------------------------\n\
         \n\
         Claude Code\n\
         -----------\n\
         {claude}\n\
         \n\
         Claude Desktop\n\
         --------------\n\
         Settings > Developer > Edit Config, add this to claude_desktop_config.json, then restart\n\
         Claude Desktop:\n\
         \n\
         {json}\n\
         \n\
         Cursor, Windsurf, VS Code, Cline and other MCP hosts\n\
         ----------------------------------------------------\n\
         Add a stdio server with the same JSON as above (the file is usually mcp.json or\n\
         settings.json), or just give them this command: \"{exe}\"\n\
         \n\
         Try it\n\
         ------\n\
         Ask the assistant: \"print a QR code for https://example.com\" or \"what is the printer status?\"\n",
        url = http_url(),
        claude_http = claude_code_http_command(),
        json_http = json_http_config(),
        claude = claude_code_command(exe),
        json = json_config(exe),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_snippets_name_the_server_and_carry_the_path() {
        let exe = r"C:\Tools\Tronic Printer\tronic-mcp.exe";
        let cmd = claude_code_command(exe);
        assert!(cmd.starts_with("claude mcp add tronic-printer -- "));
        assert!(cmd.contains("\"C:\\Tools\\Tronic Printer\\tronic-mcp.exe\""), "the path is quoted for spaces: {cmd}");
        let json = json_config(exe);
        assert!(json.contains("C:\\\\Tools\\\\Tronic Printer\\\\tronic-mcp.exe"), "backslashes are escaped in JSON: {json}");
        assert!(json.contains("\"mcpServers\"") && json.contains("\"tronic-printer\""));
        let all = instructions(exe);
        assert!(all.contains("Claude Code") && all.contains("Claude Desktop") && all.contains("print_text"));
    }

    #[test]
    fn the_http_snippets_point_at_the_apps_own_endpoint() {
        assert_eq!(http_url(), format!("http://127.0.0.1:{}/mcp", http_port()));
        assert!(claude_code_http_command().starts_with("claude mcp add --transport http tronic-printer http://127.0.0.1:"));
        assert!(json_http_config().contains(&format!("\"url\": \"{}\"", http_url())));
        assert!(instructions("x").contains(&http_url()));
    }

    #[test]
    fn the_server_is_found_next_to_the_running_program() {
        assert!(mcp_exe().ends_with(&format!("tronic-mcp{}", std::env::consts::EXE_SUFFIX)));
    }
}
