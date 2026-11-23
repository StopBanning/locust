//! LIVE2 #6: the "Engine: native" builder's label is not shown on conversation screens.
use super::*;
use rune::{Env, Runtime};

#[test]
fn conversation_screen_hides_the_engine_builders_label() {
    for kind in [Kind::Native, Kind::Door] {
        let mut s = State::default();
        crate::scenes::team_scene(&mut s);
        s.engine = Some(kind);
        let mut rt = Runtime::new(s, crate::view, (1200.0, 800.0), Env::new(true, 1.0));
        rt.frame(0.0);

        let elements = rt.elements();
        assert!(
            !elements.iter().any(|e| e.key.as_deref() == Some("engine-label")),
            "the builder's engine-label must not be drawn on screen for {kind:?}"
        );
        assert!(
            !elements
                .iter()
                .filter_map(|e| e.text.as_deref())
                .any(|t| t.contains("Engine: native") || t.contains("Engine: Locust door")),
            "the builder's Engine label text must not appear on the conversation screen for {kind:?}"
        );
    }
    // Keep the Kind::label() hook available off-screen for tests that inspect state.
    assert_eq!(Kind::Native.label(), "Engine: native");
    assert_eq!(Kind::Door.label(), "Engine: Locust door");
}
