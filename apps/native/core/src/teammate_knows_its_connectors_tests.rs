//! Electron's `teammate-knows-its-connectors.test.ts` (3 cases): a teammate is told which connectors it can call, by
//! name. Colin had to tell a teammate to "try rh local": the obvious name on his machine was a broken entry, so it
//! checked that one and reported the connector dead while a working one sat beside it.
use super::connectors_section;

/// Jimothy's brief for "Check my positions." on `runtime`, with this machine's Cursor reading set to `connectors`.
fn compose(runtime: &str, connectors: Option<&str>) -> String {
    let root = std::env::temp_dir().join(format!("locust-knows-connectors-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::write(
        store.electron().join("teammates.json"),
        serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_j", "name": "Jimothy", "role": "Custom", "roleTitle": "finance"}], "missionOwners": {}, "settings": {}}).to_string(),
    )
    .unwrap();
    crate::cursor_config::READY_FOR_TEST.with(|held| *held.borrow_mut() = connectors.map(str::to_owned));
    let briefed = crate::people::turn_brief(&store, "tm_j", &root.join("shop"), "Check my positions.", None, Some(runtime), &[], 0);
    crate::cursor_config::READY_FOR_TEST.with(|held| *held.borrow_mut() = None);
    let _ = std::fs::remove_dir_all(&root);
    briefed.unwrap().0
}

// case: names them when the machine has any
#[test]
fn names_them_when_the_machine_has_any() {
    let line = "Connectors you can call on this machine, by name: robinhood-local.";
    let prompt = compose("cursor", Some(line));
    assert!(prompt.contains("robinhood-local"), "{prompt}");
    // Its own paragraph, word for word, in the standing part ahead of the formats every run answers in.
    let at = prompt.find(&format!("\n\n{line}\n\n")).expect("the line is a paragraph of its own");
    assert!(at < prompt.find("If you reach a real fork").unwrap(), "{prompt}");
    // And the words the machine's reading is said in (runtime-adapters cursorReadyConnectorLine).
    let read = crate::cursor_config_allow::parse_cursor_mcp_list("robinhood-trading: requires_authentication\nrobinhood-local: ready\n");
    assert_eq!(
        crate::cursor_config_allow::cursor_ready_connector_line(&read).as_deref(),
        Some("Connectors you can call on this machine, by name: robinhood-local. Use these exact names -- another server with a similar name may exist and not work.")
    );
}

// case: says nothing when there are none, rather than an empty heading
#[test]
fn says_nothing_when_there_are_none_rather_than_an_empty_heading() {
    assert!(!compose("cursor", None).contains("Connectors you can call"));
    // Asked of Cursor's own reading only: another runtime has no connector story here, so a Cursor reading is never
    // told to it.
    assert!(!compose("codex", Some("Connectors you can call on this machine, by name: x.")).contains("Connectors you can call"));
    assert_eq!(connectors_section(None, || Some("x".into())), None);
}

// case: keeps the person words last, so the ask still reads closest to the asking
#[test]
fn keeps_the_person_words_last_so_the_ask_still_reads_closest_to_the_asking() {
    // The line is STANDING -- it describes the machine, not the turn -- so it belongs in the cached prefix with memory
    // and the block formats.
    let prompt = compose("cursor", Some("Connectors you can call on this machine, by name: x."));
    assert!(prompt.contains("Connectors you can call on this machine, by name: x."), "{prompt}");
    assert!(prompt.trim_end().ends_with("Check my positions."), "{prompt}");
    // Said once, after the line: the ask is not repeated ahead of the standing part.
    assert_eq!(prompt.matches("Check my positions.").count(), 1, "{prompt}");
    assert!(prompt.find("Check my positions.") > prompt.find("by name: x."), "{prompt}");
}
