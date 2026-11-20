//! Electron `renderer/src/mode-says-one-thing.test.ts`, on the Rust surfaces: each surface that names a mode uses its
//! register from `locust_core::mode_facts`, so a teammate's card says read-only for Ask and Plan and whole machine for
//! Auto (modeLabel, Screens.tsx), the teammate form says the summary (modeSummary, NewTeammateDialog.tsx), and the
//! setup card names the mode as a control does (App.tsx). The idle sentence (modeSentence) is held by
//! `idle_teammate`'s own tests, the picker rows (consequence) by `thread_view`'s. The registers' cases are in
//! `core/src/mode_says_one_thing_tests.rs`.

use super::*;
use rune::{Env, Runtime};

fn text_in(rt: &Runtime<State>, key: &str) -> String {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn team() -> State {
    let mate = |id: &str, mode: Option<&str>| {
        let mut route = json!({"runtime": "codex", "model": "synthetic-model"});
        if let Some(mode) = mode {
            route["mode"] = json!(mode);
        }
        json!({"teammateId": id, "name": id, "role": "Engineer", "route": route})
    };
    let mut s = State::default();
    s.people_panel.open = true;
    s.people_panel.team = json!({"teammates": [
        mate("tm_ask", Some("ask")),
        mate("tm_plan", Some("plan")),
        mate("tm_auto", Some("auto")),
        mate("tm_edit", Some("accept-edits")),
        mate("tm_none", None),
    ]});
    s
}

fn draw(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 2400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

// case: says read-only for every mode that is read-only, on every surface
#[test]
fn a_teammates_card_says_read_only_for_ask_and_plan_and_whole_machine_for_auto() {
    let rt = draw(team());
    assert_eq!(text_in(&rt, "team-mode-tm_ask"), "ask · read-only");
    assert_eq!(text_in(&rt, "team-mode-tm_plan"), "plan · read-only");
    assert_eq!(text_in(&rt, "team-mode-tm_auto"), "auto · whole machine");
    assert_eq!(text_in(&rt, "team-mode-tm_edit"), "edit");
    assert_eq!(text_in(&rt, "team-mode-tm_none"), "not set yet");
}

// case: names each mode the same way everywhere it is named
#[test]
fn the_teammate_form_says_the_summary_and_the_setup_card_names_the_mode_as_a_control_does() {
    for (id, said) in [
        ("tm_plan", "Plan · answers with the steps it would take, changes nothing"),
        ("tm_auto", "Auto · may edit anything on this machine"),
    ] {
        let mut s = team();
        edit(&mut s, Some(id));
        assert_eq!(text_in(&draw(s), "team-permissions"), said);
    }
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Engineer".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "approve-each".into(),
        folder: None,
    });
    crate::setup_card::open_for_teammate(&mut s, "tm_codec");
    assert_eq!(
        s.setup_card.as_ref().map(|c| c.model_line.as_str()),
        Some("Codex CLI / synthetic-model · Approve each action")
    );
}
