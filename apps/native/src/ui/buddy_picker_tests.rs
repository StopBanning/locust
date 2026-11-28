use super::*;
use serde_json::{Value, json};

fn open(id: &str, screen: Option<bool>, terminal: bool) -> rune::Runtime<State> {
    let mut s = State::default();
    s.settings_panel.settings.terminal_faces = terminal;
    let mut pet = json!({"source":"codex","id":id});
    if let Some(screen) = screen {
        pet["screen"] = json!(screen);
    }
    s.people_panel.team = json!({"teammates": [{
        "teammateId":"tm_buddy", "name":"Buddy", "hue":"blue", "role":"Docs & QA",
        "createdAt":"2026-10-05T00:00:00.000Z",
        "avatar":{"headwear":0,"accessory":0,"mouth":0,"pet":pet}
    }]});
    crate::people_panel::edit(&mut s, Some("tm_buddy"));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::people_panel::edit_form(s, &Palette::new(true)),
        (1100.0, 2400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"]
        .as_array()?
        .iter()
        .find_map(|child| node(child, key))
}

// case: offers his face as drawn or the screen, the screen until asked otherwise
#[test]
fn buddy_offers_his_drawn_face_and_defaults_to_the_screen() {
    let rt = open("codex-buddy", None, true);
    assert_eq!(
        node(&rt.accessibility_json(), "team-look-face").unwrap()["name"],
        "Face"
    );
    let radios = tests::radios_in(&rt, "team-look-face");
    assert_eq!(
        radios
            .iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["As drawn", "Screen"]
    );
    assert_eq!(tests::checked_in(&rt, "team-look-face"), ["Screen"]);
    assert_eq!(
        tests::checked_in(&open("codex-buddy", Some(false), true), "team-look-face"),
        ["As drawn"]
    );
}

// case: offers the screen only while Terminal faces is on, as a bot’s
#[test]
fn buddy_screen_choice_waits_for_terminal_faces() {
    let rt = open("codex-buddy", None, false);
    assert_eq!(tests::checked_in(&rt, "team-look-face"), ["As drawn"]);
    let screen = &tests::radios_in(&rt, "team-look-face")[1];
    assert_eq!(screen["name"], "Screen");
    assert!(
        screen["states"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "disabled")
    );
    assert_eq!(
        screen["description"],
        "Turn Terminal faces on in Settings > Appearance to give a teammate a screen."
    );
}

// case: offers nothing for a pet with no screen measured, as before
#[test]
fn unmeasured_pet_has_no_screen_choice() {
    let rt = open("other-cat", None, true);
    assert!(node(&rt.accessibility_json(), "team-look-face").is_none());
}

// case: keeps the choice with the teammate, and nothing else of the kind
#[test]
fn buddy_keeps_only_an_explicit_boolean_screen_choice_in_the_avatar() {
    use locust_core::look_picker::{avatar_from_json, avatar_json};
    let buddy = |pet| json!({"headwear":0,"accessory":0,"mouth":0,"pet":pet});
    let chosen = json!({"source":"gallery","id":"codex-buddy","screen":false});
    let implicit = json!({"source":"gallery","id":"codex-buddy"});
    assert!(
        avatar_from_json(&buddy(
            json!({"source":"gallery","id":"codex-buddy","screen":"no"})
        ))
        .is_none()
    );
    for pet in [chosen, implicit] {
        let avatar = avatar_from_json(&buddy(pet.clone())).unwrap();
        assert_eq!(avatar_json(&avatar)["pet"], pet);
    }
}
