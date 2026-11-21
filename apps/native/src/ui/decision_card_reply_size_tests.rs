//! Carries apps/desktop/src/main/a-question-card-grows-with-the-reply.test.ts, second case: the question card and the
//! person's own message are set in the reply size's own sizes (shell.css `.lc-decision__question`, `__note`, `__label`,
//! `__foot` and `.lc-bubble`), drawn at each size; and the Settings line that chooses it (Screens.tsx "Reply text
//! size"), pressed as a person would.
use super::*;

const ASK: &str = "<locust-ask>\nWhich file is right?\n- Keep notes.md :: the newer one\n- Keep NOTES.txt\n</locust-ask>";

fn asked(size: &str) -> State {
    let mut s = State::default();
    s.teammates.push(crate::Teammate { id: "tm".into(), name: "Wren".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: Some("C:/work".into()), runs_in: "ask".into(), account: None, busy: false });
    s.selected = 0;
    s.conversations.insert("tm".into(), Conversation { conversation_id: None, turns: vec![("You".into(), "Which one?".into()), ("Wren".into(), ASK.into())] });
    s.settings_panel.settings.reply_size = size.into();
    s
}

/// The height of each line the sizes reach, drawn at one reply size: question, note, label, foot, bubble.
fn heights(size: &str) -> [f32; 6] {
    let mut rt = rune::Runtime::new(
        asked(size),
        |s, _| {
            let p = Palette::new(true);
            let items = vec![ledger::Item::User("Which one?".into())];
            col().w(700.0).child(card(s, &p).expect("a decision card")).child(crate::messages::message_column(&items, "Locust", &p, s))
        },
        (800.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let all = rt.elements();
    let rect = |t: &str| all.iter().find(|e| e.text.as_deref() == Some(t)).unwrap_or_else(|| panic!("no {t}")).rect;
    let height = |t: &str| rect(t)[3];
    [
        height("Which file is right?"),
        height("the newer one"),
        height("Keep notes.md"),
        height("Or reply below — you are not limited to these"),
        height("Which one?"),
        // The bubble is as wide as its words, so its width says the size whatever the line height.
        rect("Which one?")[2],
    ]
}

// case: is what the card and the bubble are set in
#[test]
fn is_what_the_card_and_the_bubble_are_set_in() {
    let (standard, large, largest) = (heights("standard"), heights("large"), heights("largest"));
    let names = ["question", "note", "label", "foot", "bubble", "bubble's width"];
    for i in 0..6 {
        assert!(standard[i] > 0.0, "{}", names[i]);
        assert!(large[i] > standard[i], "{}: {} then {}", names[i], standard[i], large[i]);
        assert!(largest[i] > large[i], "{}: {} then {}", names[i], large[i], largest[i]);
    }
    // An unknown size draws as the standard one.
    assert_eq!(heights("huge"), standard);
}

fn settings_window(store: Option<locust_core::store::Store>) -> rune::Runtime<State> {
    let mut s = State::default();
    s.data = store;
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Appearance);
    let mut rt = rune::Runtime::new(s, |s, _| crate::settings_panel::view(s, &Palette::new(true)), (1200.0, 4000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn press(rt: &mut rune::Runtime<State>, key: &str) {
    rt.frame(0.016);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..40 {
        rt.frame(0.016);
    }
}

fn words(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n")
}

#[test]
fn reply_text_size_is_chosen_in_appearance_said_for_each_choice_and_kept() {
    let (_dir, store) = crate::org::tests::temp_store();
    let mut rt = settings_window(Some(store.clone()));
    let said = words(&rt);
    assert!(said.contains("Reply text size"), "{said}");
    assert!(said.contains("Standard. The size a reply has always been set at."), "{said}");
    assert!(said.contains("How big to set a reply"), "{said}");
    for (key, size, lede) in [
        ("settings-reply-size-largest", "largest", "Largest. For reading at a distance, or a long reply you want to sit with."),
        ("settings-reply-size-large", "large", "Large. A step up without turning the thread into a slide."),
        ("settings-reply-size-standard", "standard", "Standard. The size a reply has always been set at."),
    ] {
        press(&mut rt, key);
        assert_eq!(rt.state.settings_panel.settings.reply_size, size);
        assert_eq!(locust_core::workspace_settings::load(&store).unwrap().reply_size, size, "kept");
        assert!(words(&rt).contains(lede), "{size}");
        // The choice is the one drawn chosen.
        let p = Palette::new(true);
        let fill = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap().fill;
        for other in ["standard", "large", "largest"] {
            let drawn = fill(&format!("settings-reply-size-{other}"));
            assert_eq!(drawn == p.selected, other == size, "{other} while {size} is chosen");
        }
    }
}
