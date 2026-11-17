//! Electron's `shift-tab-never-turns-auto-on.test.ts` (M33): Shift+Tab cycles the modes this route can run, and Auto is
//! among them only when the workspace's Auto switch is already on.

use super::shift_tab_mode;

const ALL: [&str; 5] = ["ask", "accept-edits", "plan", "approve-each", "auto"];

#[test]
fn skips_auto_while_the_auto_switch_is_off() {
    let mut seen = Vec::new();
    let mut mode = "ask".to_owned();
    for _ in 0..8 {
        mode = shift_tab_mode(&mode, &ALL, false).unwrap();
        seen.push(mode.clone());
    }
    assert!(!seen.iter().any(|m| m == "auto"), "{seen:?}");
}

#[test]
fn includes_it_once_the_switch_is_on() {
    assert_eq!(shift_tab_mode("approve-each", &ALL, true).as_deref(), Some("auto"));
}

#[test]
fn moves_off_auto_to_the_first_mode_when_auto_is_no_longer_allowed() {
    assert_eq!(shift_tab_mode("auto", &ALL, false).as_deref(), Some("ask"));
}

#[test]
fn goes_nowhere_with_fewer_than_two_modes_to_cycle() {
    assert_eq!(shift_tab_mode("ask", &["ask", "auto"], false), None);
}
