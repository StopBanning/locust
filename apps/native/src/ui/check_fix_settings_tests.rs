//! a-turn-sent-back-to-fix-is-said-as-locusts.test.tsx, "the folder's choice, beside its check", drawn headless on
//! Settings > Project folder.
use super::*;
use crate::settings_pages::SettingsPageId;
use rune::{ElementInfo, Env, Runtime};

const FOLDER: &str = "/tmp/locust-check-fix-folder";

fn page(saved: &str, fix_tries: Option<u32>) -> Vec<ElementInfo> {
    let mut s = State::default();
    s.home.folder_path = Some(FOLDER.into());
    let ws = workspace_settings::workspace_id_for(std::path::Path::new(FOLDER));
    if !saved.is_empty() {
        s.settings_panel.settings.check_commands.insert(ws.clone(), saved.into());
    }
    if let Some(tries) = fix_tries {
        s.settings_panel.settings.check_fixes.insert(ws, tries);
    }
    s.settings_panel.open = true;
    s.settings_panel.page = Some(SettingsPageId::Workspace);
    let view = |s: &State, _: &mut Cx<State>| super::view(s, &Palette::new(true));
    let mut rt = Runtime::new(s, view, (1200.0, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}
fn keyed<'a>(all: &'a [ElementInfo], key: &str) -> &'a ElementInfo {
    all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}"))
}
fn said(all: &[ElementInfo]) -> String {
    all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn is_off_until_chosen_the_failure_is_shown_with_the_button() {
    let all = page("npm test", None);
    assert!(said(&all).contains("When it fails"));
    assert_eq!(keyed(&all, "settings-check-fix-off").label.as_deref(), Some("Show me, chosen"));
    assert_eq!(keyed(&all, "settings-check-fix-on").label.as_deref(), Some("Send it back to fix"));
    assert!(keyed(&all, "settings-check-fix-note").text.as_deref().unwrap().contains("with a button to send it to the teammate"));
    assert!(!said(&all).contains("at most"));
}

#[test]
fn when_on_says_how_many_fixes_at_most_and_that_each_is_a_turn() {
    let all = page("npm test", Some(3));
    assert_eq!(keyed(&all, "settings-check-fix-on").label.as_deref(), Some("Send it back to fix, chosen"));
    assert_eq!(keyed(&all, "settings-check-fix-3").label.as_deref(), Some("3 fixes, chosen"));
    assert_eq!(keyed(&all, "settings-check-fix-1").label.as_deref(), Some("1 fix"));
    assert!(keyed(&all, "settings-check-fix-note").text.as_deref().unwrap().contains("3 times at most, and it stops sooner when a fix changes nothing. Each fix is one more turn."));
    let one = page("npm test", Some(1));
    assert!(keyed(&one, "settings-check-fix-note").text.as_deref().unwrap().contains("1 time at most"));
}

#[test]
fn cannot_be_turned_on_before_there_is_a_check_to_run() {
    let all = page("", None);
    let on = keyed(&all, "settings-check-fix-on");
    assert!(on.opacity < 0.5, "drawn as off");
    assert!(keyed(&all, "settings-check-fix-note").text.as_deref().unwrap().starts_with("Save a check first."));
    // And pressing it changes nothing.
    let mut s = State::default();
    s.home.folder_path = Some(FOLDER.into());
    s.settings_panel.open = true;
    s.settings_panel.page = Some(SettingsPageId::Workspace);
    let view = |s: &State, _: &mut Cx<State>| super::view(s, &Palette::new(true));
    let mut rt = Runtime::new(s, view, (1200.0, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let (x, y) = rt.find("settings-check-fix-on").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.state.settings_panel.settings.check_fixes.is_empty());
}
