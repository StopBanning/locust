//! a-decision-is-answered-by-number.test.tsx (4 cases): a decision card numbers its options with the keys that choose
//! them; number keys answer only off a text field, and a typed letter goes to the chat box; no option looks chosen
//! before anyone chooses; each number is drawn as a key, and the card is raised as a pending card, never amber.
use super::*;

const ASK: &str = "<locust-ask>\nWhich file is right?\n- Keep notes.md :: the newer one\n- Keep NOTES.txt\n</locust-ask>";

fn asked() -> State {
    let mut s = State::default();
    s.teammates.push(crate::Teammate { id: "tm".into(), name: "Wren".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: Some("C:/work".into()), runs_in: "ask".into(), account: None, busy: false });
    s.selected = 0;
    s.conversations.insert("tm".into(), Conversation { conversation_id: None, turns: vec![("You".into(), "Which one?".into()), ("Wren".into(), ASK.into())] });
    s
}

fn drawn(s: State) -> Vec<rune::ElementInfo> {
    let mut rt = rune::Runtime::new(s, |s, _| col().w(700.0).child(card(s, &Palette::new(true)).expect("a decision card")), (800.0, 600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn key(text: &str) -> KeyEvent {
    KeyEvent { key: Key::Text(text.into()), mods: Default::default() }
}

// case: numbers its options with the keys that choose them
#[test]
fn numbers_its_options_with_the_keys_that_choose_them() {
    let els = drawn(asked());
    let at = |t: &str| els.iter().position(|e| e.text.as_deref() == Some(t)).unwrap_or_else(|| panic!("{t}"));
    assert!(at("1") < at("Keep notes.md") && at("Keep notes.md") < at("2") && at("2") < at("Keep NOTES.txt"));
    for n in ["1", "2"] {
        assert!(els.iter().any(|e| e.key.as_deref() == Some(&format!("decision-option-{n}")) && e.role == "button"));
    }
}

// case: takes number keys only off a text field, and hands a typed letter to the chat box
#[test]
fn number_keys_only_off_a_text_field_and_a_letter_goes_to_the_chat_box() {
    let mut tx = Tx::default();
    // A letter, with nothing focused: the chat box takes the focus and the letter.
    let mut s = asked();
    assert!(handle_key(&mut s, &key("k"), &mut tx));
    assert!(s.draft.focused && s.draft.text == "k", "{:?}", s.draft.text);
    // A space is not a reply.
    let mut s = asked();
    assert!(!handle_key(&mut s, &key(" "), &mut tx));
    // Typing in the chat box, a number is a number.
    let mut s = asked();
    s.draft.focused = true;
    assert!(!handle_key(&mut s, &key("1"), &mut tx));
    // A key past the options chooses nothing.
    let mut s = asked();
    assert!(!handle_key(&mut s, &key("3"), &mut tx));
    // With a modifier held it is a shortcut, not an answer.
    let mut s = asked();
    assert!(!handle_key(&mut s, &KeyEvent { key: Key::Text("1".into()), mods: rune::Mods { cmd: true, ..Default::default() } }, &mut tx));
}

// case: takes the focus itself, so no option looks chosen before anyone chooses
#[test]
fn no_option_looks_chosen_before_anyone_chooses() {
    let els = drawn(asked());
    assert!(els.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("decision-option-"))).all(|e| !e.focused));
}

// case: draws each number as a key, and sits raised but never amber
#[test]
fn each_number_is_a_key_and_the_card_is_raised_never_amber() {
    let els = drawn(asked());
    let l = crate::look::controls::Look::new(true);
    let one = els.iter().position(|e| e.text.as_deref() == Some("1")).unwrap();
    let badge = &els[els[one].parent.expect("the key around the number")];
    assert_eq!(badge.fill, l.c("bg-window"));
    let card = els.iter().find(|e| e.key.as_deref() == Some("decision-card")).unwrap();
    assert_eq!(card.fill, l.c("bg-pending"));
    assert_ne!(card.fill, l.c("amber-tint"));
    assert!(!l.t.shadows("shadow-pending").is_empty(), "the pending shadow is read from tokens.css");
}
