// Carries three packages/runtime-adapters tests on the Claude Code command line (Electron 4ce2f37a):
// a-run-without-connectors-leaves-them-out, connector-allow-rules and the-browser-shares-one-flag-with-the-bridge.
// Electron's sandboxes are Rust's modes: read-only is Ask, workspace-write is Accept edits, full-access is Auto.
use super::claude::{BRIDGE_TOOL, PrintOptions, claude_connector_rules, print_args};
use crate::approval::Mode;

const BRIDGE: &str = "C:\\temp\\locust-permission\\mcp.json";
const BROWSER: &str = "C:\\temp\\locust-browser\\mcp.json";

fn args(o: PrintOptions) -> Vec<String> {
    print_args(&o).unwrap()
}
fn after<'a>(args: &'a [String], flag: &str) -> &'a str {
    &args[args.iter().position(|a| a == flag).unwrap_or_else(|| panic!("{flag} in {args:?}")) + 1]
}
fn count(args: &[String], flag: &str) -> usize {
    args.iter().filter(|a| *a == flag).count()
}

// a-run-without-connectors-leaves-them-out.test.ts

// case: stay out when the run is told so, on every model and in every mode, and Locust's own servers stay in
#[test]
fn the_persons_connectors_stay_out_on_every_model_and_in_every_mode_and_locusts_own_servers_stay_in() {
    for mode in [Mode::Ask, Mode::AcceptEdits, Mode::Auto] {
        for model in [None, Some("haiku"), Some("sonnet"), Some("opus")] {
            let a = args(PrintOptions { mode, bridge: Some(BRIDGE), model, account_connectors: false, ..Default::default() });
            assert!(a.iter().any(|x| x == "--strict-mcp-config"), "{model:?} {mode:?}");
        }
    }
    let a = args(PrintOptions { mode: Mode::AcceptEdits, bridge: Some(BRIDGE), ..Default::default() });
    assert_eq!(after(&a, "--mcp-config"), BRIDGE);
    assert_eq!(after(&a, "--permission-prompt-tool"), "mcp__locust__approve");

    // Auto needs no permission socket. This exercises the actual host launch even in a socket-less test container.
    #[cfg(unix)]
    check_the_launch_with_no_connectors();
}

#[cfg(unix)]
fn check_the_launch_with_no_connectors() {
    use crate::engine::Teammate;
    use crate::native::{self, Origin, Send, SlotOrigin};
    use crate::store::Store;
    use serde_json::json;
    use std::{fs, time::{Duration, Instant}};

    const FAKE: &str = r#"
import json, sys
with open("claude-arguments.json", "w") as log:
    json.dump(sys.argv[1:], log)
sys.stdin.readline()
print(json.dumps({"type":"result","subtype":"success","is_error":False,"result":"OK"}), flush=True)
"#;
    let root = crate::routine_copy::tests::temp("locust-no-claude-connectors-");
    let store = Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let folder = root.0.join("work");
    fs::create_dir(&folder).unwrap();
    let argv = folder.join("claude-arguments.json");
    let _host = native::hooks::install(store.clone(), Some(("python3".into(), vec!["-c".into(), FAKE.into()])));
    for (case, ticked, sealed, strict) in [
        ("teammate opted out", json!([]), false, true),
        ("teammate opted in", json!(["claude.ai Gmail"]), false, false),
        ("sealed despite the tick", json!(["claude.ai Gmail"]), true, true),
    ] {
        store.save_json("teammates.json", &json!({
            "schemaVersion": 1,
            "teammates": [{"teammateId":"tm_wren","name":"Wren","connectors":ticked}],
            "missionOwners": {},
            "settings":{"autoMode":true}
        })).unwrap();
        if argv.exists() { fs::remove_file(&argv).unwrap(); }
        let begun = native::send_from(Send {
            teammate: Teammate {
                id: "tm_wren".into(),
                name: "Wren".into(),
                role: String::new(),
                runtime: Some("claude".into()),
                model: None,
                account: None,
                busy: false,
                runs_in: "auto".into(),
                folder: Some(folder.to_string_lossy().into_owned()),
            },
            conversation: None,
            prompt: "Say OK.".into(),
            model: None,
            effort: None,
        }, Origin {
            slot: sealed.then(|| SlotOrigin { key: "slot:no-connectors".into(), sealed, ..Default::default() }),
            ..Default::default()
        }).unwrap_or_else(|e| panic!("{case}: {e}"));
        let until = Instant::now() + crate::load_waits::UNDER_LOAD;
        while native::owns(&begun.conversation_id) {
            assert!(Instant::now() < until, "{case}: the fake Claude did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
        let flags: Vec<String> = serde_json::from_slice(&fs::read(&argv).expect(case)).unwrap();
        assert_eq!(after(&flags, "--permission-mode"), "bypassPermissions", "{case}: {flags:?}");
        assert_eq!(count(&flags, "--strict-mcp-config"), usize::from(strict), "{case}: {flags:?}");
        assert_eq!(count(&flags, "--mcp-config"), 0, "{case}: {flags:?}");
        assert_eq!(count(&flags, "--allowedTools"), 0, "{case}: {flags:?}");
    }
}

// Electron's second half, "where nothing was said", has no Rust form: the option is a plain yes or no, and no is the
// default (Rust gives no teammate the person's connectors).
#[test]
fn the_persons_connectors_come_along_for_a_teammate_that_has_them_on() {
    let a = args(PrintOptions { mode: Mode::AcceptEdits, bridge: Some(BRIDGE), account_connectors: true, ..Default::default() });
    assert!(!a.iter().any(|x| x == "--strict-mcp-config"), "{a:?}");
}

// connector-allow-rules.test.ts

fn joined(mode: Mode, connectors: &[&str]) -> String {
    args(PrintOptions { mode, connectors, ..Default::default() }).join(" ")
}

#[test]
fn the_connectors_a_teammate_was_given_become_one_named_allow_rule_each() {
    assert!(joined(Mode::AcceptEdits, &["claude.ai Robinhood", "claude.ai Gmail"]).contains("--allowedTools mcp__claude_ai_Robinhood__*,mcp__claude_ai_Gmail__*"));
}

#[test]
fn no_allow_rule_is_sent_when_no_connector_was_given() {
    for mode in [Mode::Ask, Mode::AcceptEdits] {
        assert!(!joined(mode, &[]).contains("--allowedTools"), "{mode:?}");
    }
}

#[test]
fn an_allow_rule_names_one_connector_and_not_its_neighbours() {
    let a = joined(Mode::Ask, &["claude.ai Robinhood"]);
    assert!(a.contains("mcp__claude_ai_Robinhood__*"), "{a}");
    assert!(!a.contains("Gmail"));
    assert!(!a.contains("--allowedTools mcp__*"));
}

#[test]
fn no_allow_rule_is_sent_in_auto_where_nothing_is_asked() {
    assert!(!joined(Mode::Auto, &["claude.ai Robinhood"]).contains("--allowedTools"));
}

#[test]
fn a_connector_name_that_sanitises_to_nothing_widens_no_scope() {
    let a = joined(Mode::AcceptEdits, &["", "   ", "!!!"]);
    assert!(!a.contains("--allowedTools"), "{a}");
    assert!(!a.contains("mcp____"), "{a}");
}

// Rust runs Codex through its app server, which takes no connector flag of any kind.
#[test]
fn codex_is_given_no_connector_rule() {
    let codex = super::codex::app_server_args(None).join(" ");
    assert!(!codex.contains("allowedTools") && !codex.contains("mcp__"), "{codex}");
}

// the-browser-shares-one-flag-with-the-bridge.test.ts

#[test]
fn the_browser_is_named_in_the_bridges_one_mcp_config_and_the_bridge_keeps_its_prompt_tool() {
    for mode in [Mode::Ask, Mode::AcceptEdits] {
        let a = args(PrintOptions { mode, bridge: Some(BRIDGE), browser: Some(BROWSER), ..Default::default() });
        assert_eq!(count(&a, "--mcp-config"), 1, "{mode:?}");
        let at = a.iter().position(|x| x == "--mcp-config").unwrap();
        assert_eq!(a[at + 1..at + 3], [BRIDGE, BROWSER], "{mode:?}");
        assert_eq!(after(&a, "--permission-prompt-tool"), BRIDGE_TOOL, "{mode:?}");
    }
}

#[test]
fn the_browser_joins_the_connectors_rules_in_one_allowed_tools() {
    let a = args(PrintOptions { mode: Mode::AcceptEdits, bridge: Some(BRIDGE), browser: Some(BROWSER), connectors: &["claude.ai Gmail"], ..Default::default() });
    assert_eq!(count(&a, "--allowedTools"), 1);
    assert_eq!(after(&a, "--allowedTools"), "mcp__claude_ai_Gmail__*,mcp__locust_browser__*");
    let alone = args(PrintOptions { mode: Mode::AcceptEdits, bridge: Some(BRIDGE), browser: Some(BROWSER), ..Default::default() });
    assert!(alone.iter().any(|x| x == "mcp__locust_browser__*"), "{alone:?}");
}

#[test]
fn the_browser_is_the_only_mcp_config_in_auto_with_no_bridge_and_no_rule() {
    let a = args(PrintOptions { mode: Mode::Auto, bridge: Some(BRIDGE), browser: Some(BROWSER), ..Default::default() });
    assert_eq!(count(&a, "--mcp-config"), 1);
    assert_eq!(after(&a, "--mcp-config"), BROWSER);
    assert!(!a.iter().any(|x| x == "--permission-prompt-tool" || x == "--allowedTools"), "{a:?}");
}

#[test]
fn a_run_without_the_browser_is_unchanged() {
    let a = args(PrintOptions { mode: Mode::Ask, bridge: Some(BRIDGE), ..Default::default() });
    assert_eq!(count(&a, "--mcp-config"), 1);
    assert_eq!(after(&a, "--mcp-config"), BRIDGE);
    assert!(!a.iter().any(|x| x == "mcp__locust_browser__*"));
}

// Rust's own (connectors.ts toolPrefixFor): a claude.ai name's runs of underscores are one, and its ends trimmed; any
// other name is kept as it sanitises.
#[test]
fn a_claude_ai_names_runs_of_underscores_are_one_and_another_names_are_kept() {
    assert_eq!(super::claude::allow_rule_for("claude.ai Google Drive (beta)").as_deref(), Some("mcp__claude_ai_Google_Drive_beta__*"));
    assert_eq!(super::claude::allow_rule_for("My  Server").as_deref(), Some("mcp__My__Server__*"));
}

// ask-asks-before-a-connector.test.ts: claudeConnectorRules, the gate in front of the command builder.
#[test]
fn connector_rules_are_none_in_ask_or_plan_every_call_goes_to_the_card() {
    for mode in [Mode::Ask, Mode::Plan] {
        assert!(claude_connector_rules(mode, false, &["robinhood", "github"]).is_empty(), "{mode:?}");
    }
}

#[test]
fn connector_rules_are_the_named_ones_in_edit_as_before() {
    assert_eq!(claude_connector_rules(Mode::AcceptEdits, false, &["github"]), &["github"]);
}

#[test]
fn connector_rules_are_none_when_the_person_asked_to_be_asked_about_every_one_or_none_are_named() {
    assert!(claude_connector_rules(Mode::AcceptEdits, true, &["github"]).is_empty());
    assert!(claude_connector_rules(Mode::ApproveEach, false, &["github"]).is_empty());
    assert!(claude_connector_rules(Mode::AcceptEdits, false, &[]).is_empty());
}

// ask-asks-before-a-connector.test.ts, a teammate with no connector ticked: Electron's connectorsOn is
// native::claude_args_for_turn (the app's Claude Code command, native.rs), which loads the person's connectors only
// when the teammate's roster entry ticks one, whatever the mode; the roster read itself is driven end to end in
// claude_host_grid_tests::native_claude_loads_only_ticked_connectors_and_honors_approval_mode.
#[test]
fn a_teammate_runs_without_the_persons_connectors_until_one_is_ticked() {
    // case: runs without the person’s connectors, and with them once one is ticked
    let turn = |mode: Mode, connectors: &[&str]| {
        crate::native::claude_args_for_turn(PrintOptions { mode, bridge: Some(BRIDGE), connectors, ..Default::default() }, false).unwrap()
    };
    for mode in [Mode::Ask, Mode::AcceptEdits, Mode::Auto] {
        let none = turn(mode, &[]);
        assert!(none.iter().any(|x| x == "--strict-mcp-config"), "{mode:?}: {none:?}");
        // Ticked: the person's connectors come along, even in Ask, where none is pre-approved.
        let ticked = turn(mode, &["claude.ai Gmail"]);
        assert!(!ticked.iter().any(|x| x == "--strict-mcp-config"), "{mode:?}: {ticked:?}");
    }
}
