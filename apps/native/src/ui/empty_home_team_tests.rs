//! Electron's `an-empty-home-offers-a-team.test.tsx`, on Home as the window draws it: three teams to start with, each
//! with who is on it, New teammate beside them as the team's own chip; the team itself once there is one; and a
//! teammates file that would not read said so, with no team offered to start over with.
use super::*;
use rune::{Env, Runtime};

fn home(team: Vec<Teammate>, roster_unreadable: bool) -> Runtime<State> {
    let mut s = State::default();
    home_scene(&mut s);
    s.home.splash_open = false;
    s.home.splash_entries.clear();
    s.teammates = team;
    s.org.teammates_org_unreadable = roster_unreadable;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.0);
    rt
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn keyed(rt: &Runtime<State>, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key))
}

#[test]
fn offers_three_teams_each_with_who_is_on_it() {
    // case: offers three teams, each with who is on it
    let rt = home(vec![], false);
    let said = words(&rt);
    assert!(said.contains("Start with a team"), "{said}");
    for title in ["Build software", "Research & money", "Write & design"] {
        assert!(said.contains(title), "{title}: {said}");
    }
    assert_eq!(keyed(&rt, "home-template-names-software").and_then(|e| e.text).as_deref(), Some("Wren · Juno · Atlas"));
    for names in ["Wren · Juno · Atlas", "Sable · Penny · Rook", "Quill · Iris · Moss"] {
        assert!(said.contains(names), "{names}: {said}");
    }
    assert!(said.contains("They run on the model you pick."), "{said}");
}

#[test]
fn keeps_new_teammate_beside_them_as_the_chat_bars_own_chip() {
    // case: keeps New teammate beside them, as the chat bar's own chip
    let rt = home(vec![], false);
    let chip = keyed(&rt, "home-templates-new-teammate").expect("New teammate is offered beside the teams");
    let top = keyed(&rt, "home-templates-top").expect("the teams' heading row").rect;
    assert!(chip.rect[1] >= top[1] && chip.rect[1] + chip.rect[3] <= top[1] + top[3] + 0.5, "beside the heading");
    // The same chip the team's own head carries, not a card of its own.
    let chip_words = rt.elements().into_iter().filter(|e| !e.exiting && e.rect[0] >= chip.rect[0] && e.rect[0] < chip.rect[0] + chip.rect[2] && e.rect[1] >= chip.rect[1] && e.rect[1] < chip.rect[1] + chip.rect[3]).filter_map(|e| e.text).collect::<Vec<_>>();
    assert!(chip_words.iter().any(|t| t == "+  New teammate"), "{chip_words:?}");
    let team = home(vec![Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None }], false);
    assert!(words(&team).contains("+  New teammate"), "the team's head carries the same chip");
}

#[test]
fn is_the_team_itself_once_there_is_one() {
    // case: is the team itself once there is one
    let rt = home(vec![Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None }], false);
    let said = words(&rt);
    assert!(said.contains("Your team"), "{said}");
    assert!(!said.contains("Start with a team"), "{said}");
}

#[test]
fn says_so_and_offers_no_team_to_start_over_with() {
    // case: says so, and offers no team to start over with
    let rt = home(vec![], true);
    let said = words(&rt);
    assert!(said.contains("Your teammates file could not be read, so your team is not shown."), "{said}");
    assert!(said.contains("Nothing was changed."), "{said}");
    assert!(!said.contains("Start with a team"), "{said}");
    assert!(keyed(&rt, "home-template-software").is_none());
}
