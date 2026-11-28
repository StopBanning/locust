use super::*;
use rune::{Env, Runtime};

// case: a copy says it leaves the machine's agents alone, instead of a switch that would not
#[test]
fn settings_agent_updates_explain_the_copy_and_hide_its_switch() {
    let drawn = |held| {
        let mut s = State::default();
        s.settings_panel.agents = Some(Vec::new());
        s.agent_updates.decided = true;
        s.agent_updates.read = true;
        s.agent_updates.held_here = held;
        let mut rt = Runtime::new(
            s,
            |s, _| section(s, &Palette::new(true)),
            (1200.0, 1500.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements()
    };
    let copy = drawn(true);
    assert!(copy.iter().any(|el| el.key.as_deref() == Some("agents-held-here")
        && el.text.as_deref().is_some_and(|t| t.starts_with("This copy is not the installed Locust, so it never updates the agents on this machine on its own."))));
    assert!(
        !copy
            .iter()
            .any(|el| el.key.as_deref() == Some("agents-keep-current"))
    );
    let installed = drawn(false);
    assert!(
        installed
            .iter()
            .any(|el| el.key.as_deref() == Some("agents-keep-current"))
    );
    assert!(
        !installed
            .iter()
            .any(|el| el.key.as_deref() == Some("agents-held-here"))
    );
}
