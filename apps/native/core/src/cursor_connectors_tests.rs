//! cursor-connectors.test.ts: what the Cursor CLI says about its connectors, and which a teammate is told it has
//! (cursor_config_allow.rs, which the Cursor driver already reads them through).
use super::*;

fn read(rows: &[&str]) -> Vec<CursorConnector> {
    parse_cursor_mcp_list(&rows.join("\n"))
}

#[test]
fn reads_a_server_and_its_status() {
    // case: reads a server and its status
    assert_eq!(
        parse_cursor_mcp_list("robinhood-trading: requires_authentication"),
        [CursorConnector { name: "robinhood-trading".into(), status: "requires_authentication".into(), needs_authentication: true }]
    );
}

#[test]
fn keeps_a_working_server_and_does_not_call_it_unauthenticated() {
    // case: keeps a working server, and does not call it unauthenticated
    let found = read(&["github: connected", "robinhood-trading: requires_authentication", "slack: authenticated"]);
    assert_eq!(found.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["github", "robinhood-trading", "slack"]);
    assert_eq!(found.iter().map(|c| c.needs_authentication).collect::<Vec<_>>(), [false, true, false]);
}

#[test]
fn ignores_blank_lines_and_prose() {
    // case: ignores blank lines and prose
    assert!(read(&["", "No MCP servers configured.", "   "]).is_empty());
}

#[test]
fn says_nothing_when_every_connector_is_fine() {
    // case: says nothing when every connector is fine
    assert_eq!(cursor_connector_sentence(&parse_cursor_mcp_list("github: connected")), None);
    assert_eq!(cursor_connector_sentence(&[]), None);
}

#[test]
fn names_the_server_and_stops_short_of_prescribing_a_fix() {
    // case: names the server and stops short of prescribing a fix
    let said = cursor_connector_sentence(&parse_cursor_mcp_list("robinhood-trading: requires_authentication")).unwrap();
    assert!(said.contains("robinhood-trading"));
    assert!(said.contains("Cursor app does not cover the CLI"));
    assert!(!said.contains("mcp login"));
    let lower = said.to_lowercase();
    assert!(!lower.contains("run this") && !lower.contains("in a terminal"), "{said}");
}

#[test]
fn names_every_waiting_server_when_there_are_several() {
    // case: names every waiting server when there are several
    let said = cursor_connector_sentence(&read(&["a: requires_authentication", "b: connected", "c: requires_authentication"])).unwrap();
    assert!(said.contains("a and c"), "{said}");
    assert!(!said.contains("b,"), "{said}");
}

#[test]
fn names_the_ready_ones_so_nobody_has_to_know_a_config_file() {
    // case: names the ready ones, so nobody has to know a config file
    let said = cursor_ready_connector_line(&read(&["robinhood-local: ready", "github: connected"])).unwrap();
    assert!(said.contains("robinhood-local"));
    assert!(said.contains("github"));
    assert!(said.contains("Use these exact names"));
}

#[test]
fn never_names_one_that_cannot_be_called() {
    // case: never names one that cannot be called
    let said = cursor_ready_connector_line(&read(&["robinhood-trading: requires_authentication", "robinhood-local: ready"])).unwrap();
    assert!(said.contains("robinhood-local"));
    assert!(!said.contains("robinhood-trading"));
}

#[test]
fn says_nothing_at_all_when_nothing_is_ready() {
    // case: says nothing at all when nothing is ready
    assert_eq!(cursor_ready_connector_line(&parse_cursor_mcp_list("a: requires_authentication")), None);
    assert_eq!(cursor_ready_connector_line(&parse_cursor_mcp_list("a: disabled")), None);
    assert_eq!(cursor_ready_connector_line(&[]), None);
}
