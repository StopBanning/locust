//! Opening a non-shell tool drawer (such as an MCP connector call) still shows its single-line arguments even when the
//! tool row has no summary, because only shell rows already print their single-line command on the heading line.
use super::*;

fn draw_item(item: ledger::Item, open: &[&str]) -> Vec<String> {
    let mut s = State::default();
    for key in open {
        s.activity_open.insert((*key).into(), true);
    }
    let mut rt = rune::Runtime::new(
        (s, item),
        |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"),
        (900.0, 400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

#[test]
fn opened_non_shell_tool_drawer_shows_single_line_arguments() {
    let args = r#"{"query":"locust"}"#;
    let item = ledger::Item::Tool {
        name: "mcp__github__search_code".into(),
        status: ledger::Status::Done,
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail {
            id: "mcp_call_1".into(),
            arguments: args.into(),
            output: "2 matches".into(),
            ..Default::default()
        }),
    };

    // Folded: the MCP heading names the tool and server, not its arguments.
    let folded = draw_item(item.clone(), &[]);
    assert!(
        folded.iter().any(|t| t == "search_code · github · done"),
        "expected MCP heading in {folded:?}"
    );
    assert!(
        !folded.iter().any(|t| t == args),
        "arguments should stay folded until the drawer is opened: {folded:?}"
    );

    // Opened: the single-line arguments must appear inside the drawer alongside the output.
    let opened = draw_item(item, &["conv/mcp_call_1"]);
    assert!(
        opened.iter().any(|t| t == args),
        "opened non-shell tool drawer must display its single-line arguments: {opened:?}"
    );
    assert!(
        opened.iter().any(|t| t == "2 matches"),
        "opened non-shell tool drawer must display its output: {opened:?}"
    );
}
