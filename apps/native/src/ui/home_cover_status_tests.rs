//! Electron `homeCoverStatus.test.ts` is the rule (core/src/home_cover_tests.rs); these draw it on Home's glass, as
//! App.tsx passes it to the cover: the same facts the teammate cards say.
use super::*;

fn mates(s: &mut State, list: &[(&str, &str, bool)]) {
    crate::team_scene(s);
    s.teammates.truncate(list.len());
    let mut owners = serde_json::Map::new();
    for (mate, (id, name, busy)) in s.teammates.iter_mut().zip(list) {
        mate.id = (*id).into();
        mate.name = (*name).into();
        mate.busy = *busy;
        mate.runtime = Some("codex".into());
        owners.insert(format!("m_{id}"), serde_json::json!(id));
    }
    s.people_panel.team = serde_json::json!({ "missionOwners": owners });
}

fn glass(s: State) -> (String, String) {
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let line = all.iter().find(|e| e.key.as_deref() == Some("home-cover-line")).and_then(|e| e.text.clone()).expect("the glass line");
    let label = all.iter().find_map(|e| e.label.clone().filter(|l| l.starts_with("Locust. "))).expect("the cover's label");
    (line, label)
}

#[test]
fn the_glass_names_who_waits_on_you_then_who_is_working() {
    let mut s = State::default();
    mates(&mut s, &[("tm_codex", "Codex", true), ("tm_casper", "Casper", true)]);
    s.home.waiting_for.insert("m_tm_casper".into(), "A decision".into());
    let (line, label) = glass(s);
    assert_eq!(line, "CASPER WAITING ON YOU · CODEX WORKING");
    assert_eq!(label, "Locust. Casper waiting on you · Codex working");
}

#[test]
fn the_glass_keeps_its_claim_when_nothing_runs_or_waits() {
    let mut s = State::default();
    mates(&mut s, &[("tm_codex", "Codex", false)]);
    let (line, label) = glass(s);
    assert_eq!(line, COVER_CLAIM.to_uppercase());
    assert_eq!(label, format!("Locust. {COVER_CLAIM}"));
}
