//! The window's Shift+Tab and `/plan`-style commands move the mode a turn runs in (`composer_mode`, which the send
//! reads), over the modes the teammate's route can run. They used to write a field nothing read.

use super::*;
use locust_core::engine::Teammate;

fn with(runtime: &str, auto_on: bool) -> State {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_wren".into(), name: "Wren".into(), role: "Code".into(), runtime: Some(runtime.into()), model: None,
        account: None, busy: false, runs_in: "ask".into(), folder: None,
    });
    s.selected = 0;
    s.settings_panel.settings.auto_mode = auto_on;
    s
}

fn cycle(s: &mut State, times: usize) -> Vec<String> {
    (0..times).map(|_| { cycle_mode(s); s.composer_mode.clone().unwrap_or_default() }).collect()
}

#[test]
fn shift_tab_moves_the_mode_a_turn_runs_in_and_skips_auto_while_its_switch_is_off() {
    let mut s = with("claude", false);
    assert_eq!(cycle(&mut s, 4), ["accept-edits", "plan", "approve-each", "ask"]);
    let mut on = with("claude", true);
    assert_eq!(cycle(&mut on, 4), ["accept-edits", "plan", "approve-each", "auto"]);
    // Landing on Auto never turns the saved switch on (the M33 defect).
    let mut off = with("claude", false);
    cycle(&mut off, 8);
    assert!(!off.settings_panel.settings.auto_mode);
}

#[test]
fn shift_tab_cycles_only_the_modes_the_route_can_run() {
    // Cursor runs Ask, Edit and Auto (and Plan, which follows Ask); never Approve each.
    let mut s = with("cursor", false);
    assert_eq!(cycle(&mut s, 3), ["accept-edits", "plan", "ask"]);
}

#[test]
fn a_mode_command_sets_the_mode_a_turn_runs_in() {
    let mut s = with("claude", false);
    let plan = composer::slash::slash_commands().into_iter().find(|c| matches!(c.action, composer::SlashAction::Mode("plan"))).unwrap();
    run_slash_command(&mut s, &plan);
    assert_eq!(s.composer_mode.as_deref(), Some("plan"));
}
