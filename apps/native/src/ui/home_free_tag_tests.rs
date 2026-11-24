//! Electron's `home-free-model-keeps-its-tag.test.tsx`, on Home's teammate cards as the window draws them: a free
//! model's Free is its own tag, kept whole while the name gives way; a paid model is the line it was; a teammate on
//! its account's default is named by its runtime, as the chip and Team say it.
use super::*;
use rune::{Env, Runtime};

fn mate(id: &str, runtime: &str, model: Option<&str>) -> Teammate {
    Teammate {
        id: id.into(),
        name: "Penny".into(),
        role: "Money".into(),
        runtime: Some(runtime.into()),
        model: model.map(Into::into),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

fn home(team: Vec<Teammate>) -> Runtime<State> {
    let mut s = State::default();
    home_scene(&mut s);
    s.home.splash_open = false;
    s.home.splash_entries.clear();
    s.teammates = team;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 2000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.0);
    rt
}

fn keyed(rt: &Runtime<State>, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key))
}

#[test]
fn keeps_the_free_tag_whole_and_lets_the_name_give_way() {
    // case: keeps the Free tag whole and lets the name give way
    let rt = home(vec![mate("tm_penny", "opencode", Some("opencode/nemotron-3.5-lightning-free"))]);
    let name = keyed(&rt, "home-card-model-tm_penny").expect("the model is named");
    assert_eq!(name.text.as_deref(), Some("Nemotron 3.5 Lightning"));
    let tag = keyed(&rt, "home-card-free-tm_penny").expect("Free is its own tag");
    assert_eq!(tag.text.as_deref(), Some("Free"));
    assert!(tag.rect[0] >= name.rect[0] + name.rect[2] - 0.5, "after the name");
    let card = keyed(&rt, "home-card-tm_penny").expect("the card").rect;
    assert!(tag.rect[0] + tag.rect[2] <= card[0] + card[2] + 0.5, "kept inside the card, whole");
}

#[test]
fn draws_a_paid_model_as_the_line_it_was() {
    // case: draws a paid model as the line it was
    let rt = home(vec![mate("tm_penny", "claude", Some("opus"))]);
    assert_eq!(keyed(&rt, "home-card-model-tm_penny").and_then(|e| e.text).as_deref(), Some("Opus 5.5"));
    assert!(keyed(&rt, "home-card-free-tm_penny").is_none());
}

#[test]
fn names_a_teammate_on_its_accounts_default_by_the_runtime_as_the_chip_and_team_do() {
    // case: names a teammate on its account's default by the runtime, as the chip and Team do
    let rt = home(vec![
        mate("tm_wren", "codex", Some("account-default")),
        mate("tm_ash", "antigravity", Some("account-default")),
        mate("tm_kit", "codex", None),
    ]);
    assert_eq!(keyed(&rt, "home-card-model-tm_wren").and_then(|e| e.text).as_deref(), Some("Codex"));
    assert_eq!(keyed(&rt, "home-card-model-tm_ash").and_then(|e| e.text).as_deref(), Some("Antigravity"));
    assert_eq!(keyed(&rt, "home-card-model-tm_kit").and_then(|e| e.text).as_deref(), Some("Codex"));
}
