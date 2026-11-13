//! A connector call reads as a connector call (Electron `missionView.ts` `mcpToolParts`, `isBareConnectorTool`). The
//! first connector tool a teammate reached was drawn whole as `mcp__claude_ai_Robinhood__get_watchlists`. The shape
//! is `mcp__<server>__<tool>`, and a row already has two slots for exactly those: what was acted on, and what acted.

/// A connector call's server (the product a person connected, not its transport) and tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpParts {
    pub server: String,
    pub tool: String,
}

/// The name a person connected, not the transport that carries it.
fn pretty_server(raw: &str) -> String {
    let raw = raw.strip_prefix("claude_ai_").unwrap_or(raw);
    crate::invariants::regex(r"[_-]+").replace_all(raw, " ").trim().to_owned()
}

/// `mcp__server__tool` (split on the first double underscore), `Server__tool` with no prefix (a name on the left, no
/// path or sentence), or Codex's `mcp_tool.server.tool`; None for every ordinary tool.
pub fn mcp_tool_parts(name: &str) -> Option<McpParts> {
    if let Some(c) = crate::invariants::regex(r"^mcp__(.+?)__(.+)$").captures(name) {
        return Some(McpParts { server: pretty_server(&c[1]), tool: c[2].to_owned() });
    }
    if let Some(c) = crate::invariants::regex(r"^([A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)*)__([A-Za-z][A-Za-z0-9_-]*)$").captures(name) {
        return Some(McpParts { server: pretty_server(&c[1]), tool: c[2].to_owned() });
    }
    let c = crate::invariants::regex(r"^mcp[_.]?(?:tool)?[_.](.+)$").captures(name)?;
    let rest = &c[1];
    let (server, tool) = rest.split_once('.')?;
    Some(McpParts { server: pretty_server(server), tool: tool.to_owned() })
}

/// A connector call whose name carries nothing to split (Antigravity names it `mcp`): a short closed list.
pub fn is_bare_connector_tool(name: &str) -> bool {
    matches!(name.trim().to_lowercase().as_str(), "mcp" | "mcp_tool" | "mcptool" | "use_mcp_tool" | "call_mcp_tool" | "run_mcp_tool")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(server: &str, tool: &str) -> Option<McpParts> {
        Some(McpParts { server: server.into(), tool: tool.into() })
    }

    // Electron's renderer/src/mcp-tool-rows.test.ts.
    #[test]
    fn splits_into_the_server_and_the_tool() {
        assert_eq!(mcp_tool_parts("mcp__claude_ai_Robinhood__get_watchlists"), parts("Robinhood", "get_watchlists"));
    }

    #[test]
    fn drops_the_transport_keeps_the_product() {
        assert_eq!(mcp_tool_parts("mcp__claude_ai_Gmail__send_message").unwrap().server, "Gmail");
        assert_eq!(mcp_tool_parts("mcp__robinhood-trading__get_portfolio").unwrap().server, "robinhood trading");
    }

    #[test]
    fn keeps_a_tool_name_that_has_underscores_of_its_own() {
        assert_eq!(mcp_tool_parts("mcp__Figma__get_design_context"), parts("Figma", "get_design_context"));
    }

    #[test]
    fn reads_codexs_own_join_too() {
        assert_eq!(mcp_tool_parts("mcp_tool.robinhood.get_portfolio").unwrap().server, "robinhood");
    }

    #[test]
    fn leaves_every_ordinary_tool_alone() {
        for name in ["Read", "Bash", "src/billing/adapter.ts", "mcp_tool"] {
            assert_eq!(mcp_tool_parts(name), None, "{name}");
        }
    }

    #[test]
    fn reads_a_connector_tool_with_no_prefix_and_a_bare_one() {
        assert_eq!(mcp_tool_parts("Google_Drive__create_file"), parts("Google Drive", "create_file"));
        assert!(is_bare_connector_tool("mcp") && is_bare_connector_tool("use_mcp_tool"));
        assert!(!is_bare_connector_tool("mcp_server_config.json"));
    }
}
