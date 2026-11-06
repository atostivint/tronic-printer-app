//! The words and snippets for plugging the printer into an AI assistant over MCP. Shown in the
//! app (Terminal tab) and printed by `tronic-mcp --config`, so it uses nothing but `std`.

// The app shows the short snippets; `tronic-mcp --config` prints the whole text.
#![allow(dead_code)]

/// `tronic-mcp.exe`, next to the program that is running now.
pub fn mcp_exe() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("tronic-mcp.exe")))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "tronic-mcp.exe".into())
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
    fn the_server_is_found_next_to_the_running_program() {
        assert!(mcp_exe().ends_with("tronic-mcp.exe"));
    }
}
