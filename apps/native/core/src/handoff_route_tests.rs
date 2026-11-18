//! Electron's `a-handoff-keeps-the-route-handed-to.test.ts`, its first case: after a handoff the chat box shows the
//! route handed to, over the teammate's saved model (`route_at_start::composer_route_for`), with Electron's values.
//! Its second case reads Electron's `App.tsx` source.
use crate::route_at_start::{PickerRoute, TeammateRoute, composer_route_for};
use std::collections::HashMap;

fn route(runtime: &str, model: &str) -> PickerRoute {
    PickerRoute { runtime: runtime.into(), model: model.into(), account: None }
}

#[test]
fn shows_the_route_handed_to_over_the_teammates_saved_model() {
    let saved = TeammateRoute { runtime: "codex".into(), model: "gpt-6-luna".into(), mode: "ask".into(), account: None, effort: None };
    let handed_to = route("claude", "claude-sonnet-5");
    let shown = composer_route_for(&route("codex", "gpt-6-luna"), Some(("tm_wren", Some(&saved))), &HashMap::new(), Some("mission_handed"), Some(&handed_to));
    assert_eq!(shown, handed_to);
}
