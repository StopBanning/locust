//! Electron's `references-never-widen-a-mode.test.ts`: a reference folder is lent to a run for reading, never as a way
//! to widen what its mode lets it change.
use super::*;
use crate::runtime::opencode_config::{legacy_config, with_references};

const FOLDER: &str = "C:/references/design kit";

fn args(mode: Mode, references: &[&str]) -> Vec<String> {
    print_args(&PrintOptions {
        mode,
        bridge: None,
        browser: None,
        guard_settings: None,
        skills: &[],
        references,
        connectors: &[],
        account_connectors: false,
        model: None,
        effort: None,
        resume: None,
        fork: false,
    })
    .unwrap()
}

fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

// case: Claude adds references without changing its tools, with scoped Edit protection
#[test]
fn claude_adds_references_without_changing_its_tools_with_scoped_edit_protection() {
    let ask = args(Mode::Ask, &[FOLDER]);
    assert_eq!(after(&ask, "--add-dir"), Some(FOLDER));
    assert_eq!(after(&ask, "--tools"), after(&args(Mode::Ask, &[]), "--tools"), "the read-only tools are as they were");
    assert!(!ask.contains(&"--disallowedTools".to_owned()));
    let editing = args(Mode::AcceptEdits, &[FOLDER]);
    assert!(editing.contains(&"--add-dir".to_owned()));
    assert!(editing.contains(&"Edit(//c/references/design kit/**)".to_owned()), "{editing:?}");
    assert_eq!(after(&editing, "--tools"), after(&args(Mode::AcceptEdits, &[]), "--tools"));
    assert_eq!(args(Mode::Auto, &[FOLDER]), args(Mode::Auto, &[]), "Auto already reaches everything: nothing is added");
}

// case: OpenCode adds only named reference reads while every write denial and bash rule stays intact
#[test]
fn opencode_adds_only_named_reference_reads_and_every_write_and_bash_rule_stays() {
    let before = legacy_config(Mode::Ask, None);
    let after = with_references(before.clone(), Mode::Ask, &[FOLDER]);
    let reach = &after["permission"]["external_directory"];
    assert_eq!(reach["*"], "deny");
    for key in [FOLDER.to_owned(), format!("{FOLDER}/**"), format!("{FOLDER}/*"), "C:\\references\\design kit\\*".to_owned()] {
        assert_eq!(reach[&key], "allow", "{key}");
    }
    for key in ["edit", "write", "patch", "bash"] {
        assert_eq!(after["permission"][key], before["permission"][key], "{key}");
    }
    assert_eq!((after["permission"]["edit"].as_str(), after["permission"]["bash"].as_str()), (Some("deny"), Some("ask")));
}

// case: OpenCode Edit and Auto permissions are unchanged, not silently narrowed
#[test]
fn opencode_edit_and_auto_permissions_are_unchanged() {
    for mode in [Mode::AcceptEdits, Mode::Auto] {
        let plain = legacy_config(mode, None);
        assert_eq!(with_references(plain.clone(), mode, &[FOLDER]), plain, "{mode:?}");
    }
}

// case: Codex never receives add-dir, including in full access
#[test]
fn codex_never_receives_add_dir() {
    // Codex runs as an app server whose arguments take no folder at all: a reference reaches it only by name in the brief.
    let argv = crate::runtime::codex::app_server_args(Some("0.153.0"));
    assert!(!argv.iter().any(|a| a == "--add-dir" || a == FOLDER), "{argv:?}");
}

// case: Copilot and Antigravity keep their read-only posture when adding a directory
#[test]
fn copilot_and_antigravity_keep_their_read_only_posture_when_adding_a_directory() {
    let copilot = crate::runtime::copilot::prompt_args("Read the reference.", Mode::Ask, None, None, None, None, &[FOLDER], None);
    assert!(copilot.contains(&"--deny-tool=write,shell".to_owned()));
    assert_eq!(after(&copilot, "--add-dir"), Some(FOLDER));
    let editing = crate::runtime::copilot::prompt_args("Read the reference.", Mode::AcceptEdits, None, None, None, None, &[FOLDER], None);
    assert!(!editing.contains(&FOLDER.to_owned()), "outside Ask an extra root would be writable: not added");
    let agy = crate::runtime::antigravity::print_args(Mode::Ask, None, None, None, &[FOLDER], None);
    assert_eq!(after(&agy, "--add-dir"), Some(FOLDER));
    assert!(!agy.contains(&"--dangerously-skip-permissions".to_owned()) && !agy.contains(&"--mode".to_owned()));
}
