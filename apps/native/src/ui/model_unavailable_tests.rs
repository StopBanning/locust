//! `a-free-model-that-is-down-offers-the-next.test.ts`, drawn: a free model whose provider is down (or refused by
//! OpenCode's service) offers the next free model with the down-provider sentence, and pressing it switches the route
//! and hands the prompt back to the composer.
use super::*;
use rune::{Env, Runtime};
use serde_json::json;

fn drawn_failure(payload: serde_json::Value) -> Runtime<State> {
    let ev = json!({ "type": "run.failed", "payload": payload });
    let mut s = State::default();
    s.models.insert(
        "tm_1".into(),
        vec![engine::Models {
            runtime: "opencode".into(),
            model: None,
            effort: None,
            teammate: None,
            models: vec![
                engine::Model { id: "opencode/glm-5.3-flash-free".into(), name: "GLM 5.3 Flash Free".into(), efforts: vec![] },
                engine::Model { id: "opencode/beam-free".into(), name: "Beam Free".into(), efforts: vec![] },
            ],
        }],
    );
    let mut snap = ledger::Snapshot::default();
    snap.failed = true;
    snap.next_model = locust_core::failure_words::offers_next_model([&ev], false);
    snap.next_model_down = locust_core::failure_words::offers_next_model_down([&ev], false);
    snap.sign_in = locust_core::failure_words::offers_sign_in([&ev], false);
    snap.turn_metas.push(ledger::Metadata {
        mission_id: "m1".into(),
        prompt: "Check the build status".into(),
        runtime: "opencode".into(),
        model: "opencode/glm-5.3-flash-free".into(),
        ..Default::default()
    });
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate {
            id: "tm_1".into(),
            name: "Codec".into(),
            role: "Engineer".into(),
            runtime: Some("opencode".into()),
            model: Some("opencode/glm-5.3-flash-free".into()),
            account: None,
            busy: false,
            runs_in: "local".into(),
            folder: None,
        },
        conversation: Conversation::default(),
        items: vec![],
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: snap,
        due: f64::INFINITY,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = Runtime::new(
        s,
        |s, _| col().w_full().h_full().children(view(s, &Palette::new(true))),
        (800.0, 200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.05);
    rt
}

fn texts(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

// case: reads OpenCode's words for a model or endpoint that is unavailable
#[test]
fn a_down_free_model_draws_the_down_sentence_and_switches_to_the_next_free_model() {
    let mut rt = drawn_failure(json!({ "message": "OpenCode stopped: Upstream request failed: Model is unavailable." }));
    let t = texts(&rt);
    assert!(t.contains(locust_core::failure_words::MODEL_DOWN_SENTENCE), "{t}");
    assert!(!t.contains(locust_core::failure_words::AT_ITS_LIMIT_SENTENCE), "{t}");
    assert!(t.contains("Switch to Beam Free"), "{t}");
    let (x, y) = rt.find("next-model-action").expect("no Switch to next free model button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    let pick = rt.state.picks.get("tm_1").expect("route did not switch");
    assert_eq!(pick.runtime.as_deref(), Some("opencode"));
    assert_eq!(pick.model, "opencode/beam-free");
    assert_eq!(rt.state.draft.text, "Check the build status");
}

// case: and nothing else: a retirement, a busy server or a plain failure is not it
#[test]
fn a_non_unavailable_failure_does_not_draw_the_down_model_offer() {
    let rt = drawn_failure(json!({ "message": "OpenCode stopped: Model exo-free has been deprecated." }));
    assert!(rt.find("next-model").is_none());
    let rt_limit = drawn_failure(json!({ "message": "OpenCode stopped.", "process": { "stderr": "rate limit exceeded" } }));
    let t = texts(&rt_limit);
    assert!(t.contains(locust_core::failure_words::AT_ITS_LIMIT_SENTENCE), "{t}");
    assert!(!t.contains(locust_core::failure_words::MODEL_DOWN_SENTENCE), "{t}");
}
