//! settings-says-what-locust-can-stop.test.tsx: Settings > AI agents says which agents ask first, in the table's own
//! words, drawn headless.
use super::*;
use locust_core::what_locust_can_stop::{StopReach, WHAT_LOCUST_CAN_STOP};
use rune::{Env, Runtime};

fn page() -> Runtime<State> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Runtimes);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 9000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn keys(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.key).collect()
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

#[test]
fn is_one_folded_row_per_ai_agent_named_with_how_much_a_card_can_stop() {
    let rt = page();
    let keys = keys(&rt);
    let said = texts(&rt);
    for r in WHAT_LOCUST_CAN_STOP {
        assert!(keys.contains(&format!("stops-{}", r.runtime)), "{}", r.runtime);
        assert!(said.iter().any(|t| t == locust_core::carry_on::display_name(r.runtime)), "{}", r.runtime);
        assert!(!keys.contains(&format!("stops-{}-open", r.runtime)), "folded: {}", r.runtime);
    }
    let rows = keys.iter().filter(|k| k.starts_with("stops-") && !k.ends_with("-reach") && !k.ends_with("-open")).count();
    assert_eq!(rows, WHAT_LOCUST_CAN_STOP.len());
    // Green only where each action can be stopped.
    let green = Palette::new(true).green;
    let greens = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.ends_with("-reach")) && e.text_color == Some(green)).count();
    assert_eq!(greens, WHAT_LOCUST_CAN_STOP.iter().filter(|r| r.reach == StopReach::EachAction).count());
}

#[test]
fn opens_to_the_same_sentences_the_document_holds() {
    let mut rt = page();
    for r in WHAT_LOCUST_CAN_STOP {
        let (x, y) = rt.find(&format!("stops-{}", r.runtime)).expect("the row");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
    }
    let said = texts(&rt);
    for r in WHAT_LOCUST_CAN_STOP {
        for sentence in [Some(r.asks), Some(r.without), r.always].into_iter().flatten() {
            assert!(said.iter().any(|t| t == sentence), "{}: {sentence}", r.runtime);
        }
    }
    assert_eq!(said.iter().filter(|t| *t == "Always").count(), WHAT_LOCUST_CAN_STOP.iter().filter(|r| r.always.is_some()).count());
}
