use super::*;
use crate::tests::TempDir;
use rune::{Env, Runtime};
use serde_json::{Value, json};
use std::path::Path;

const AT: &str = "2026-11-23T11:00:00Z";
const BEAM: &str = "opencode/beam-free";
const FLEDGE: &str = "opencode/fledge-alpha-free";
const NEXT: &str = "opencode/ling-3.0-flash-fin-free";
const PROMPT: &str = "Review the attached notes.";

fn catalog() -> Vec<engine::Models> {
    vec![engine::Models {
        teammate: None,
        model: None,
        effort: None,
        runtime: "opencode".into(),
        models: [BEAM, FLEDGE, NEXT]
            .into_iter()
            .map(|id| engine::Model {
                id: id.into(),
                name: id.into(),
                efforts: vec![],
            })
            .collect(),
    }]
}

fn record(dir: &Path, id: &str, model: &str, kind: &str, payload: Value, at: &str) {
    let prompt = locust_core::attachments::with_attachments(PROMPT, &["notes.md".into()]);
    let created = json!({
        "schemaVersion": 24, "recordType": "mission.created", "occurredAt": at,
        "metadata": {"missionId": id, "prompt": prompt, "runtime": "opencode", "model": model, "createdAt": at}
    });
    let end = json!({
        "schemaVersion": 24, "recordType": "mission.event", "occurredAt": at,
        "event": {
            "id": "end", "sequence": 1, "type": kind, "sourceAdapter": "opencode", "occurredAt": at,
            "payload": payload
        }
    });
    std::fs::write(
        dir.join(format!("{id}.jsonl")),
        format!("{created}\n{end}\n"),
    )
    .unwrap();
}

fn case(payload: Value, model: &str) -> (TempDir, Runtime<State>) {
    let dir = TempDir::new();
    record(&dir.0, "m_model", model, "run.failed", payload, AT);
    let mut worker = ledger::Session::new(dir.0.clone(), "m_model".into());
    let snapshot = worker.poll().unwrap();
    let mut s = State::default();
    team_scene(&mut s);
    let mut header = s.teammates[0].clone();
    header.runtime = Some("opencode".into());
    header.model = Some(model.into());
    s.models.insert(header.id.clone(), catalog());
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some("m_model".into()),
            turns: vec![],
        },
        items: snapshot.items.clone(),
        dir: dir.0.clone(),
        worker: Some(worker),
        snapshot,
        cold_tip: None,
        due: f64::INFINITY,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = Runtime::new(
        s,
        |s, _| col().children(view(s, &Palette::new(true))),
        (800.0, 200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    (dir, rt)
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements()
        .into_iter()
        .filter_map(|el| el.text)
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn retired_free_offer_puts_the_words_and_files_back_without_sending() {
    let (_dir, mut rt) = case(
        json!({"message": "Model not found: opencode/beam-free"}),
        BEAM,
    );
    let said = words(&rt);
    assert!(
        said.contains("OpenCode no longer offers this model."),
        "{said}"
    );
    assert!(said.contains("Switch to Ling 3.0 Flash Fin Free"), "{said}");

    let (x, y) = rt.find("limit-model-switch").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    let teammate = &rt.state.ledger.as_ref().unwrap().header.id;
    assert_eq!(rt.state.picks[teammate].model, NEXT);
    assert_eq!(rt.state.draft.text, PROMPT);
    assert_eq!(rt.state.attached, ["notes.md"]);
    assert!(rt.state.flight.is_none() && !rt.state.sending.is_pending());
}

#[test]
fn failed_free_offer_says_down_or_limit_and_refuses_an_unrelated_failure() {
    let (_dir, down) = case(json!({"message": "Endpoint is unavailable."}), BEAM);
    assert!(words(&down).contains("provider is down for now."));
    assert!(words(&down).contains("Switch to Ling 3.0 Flash Fin Free"));

    let (_dir, limit) = case(
        json!({"message": "Run failed", "process": {"stderr": "HTTP 429"}}),
        BEAM,
    );
    assert!(words(&limit).contains("This model is at its limit."));

    let (_dir, other) = case(json!({"message": "local file not found: notes.md"}), BEAM);
    assert!(other.find("limit-model-switch").is_none());
    let (_dir, paid) = case(
        json!({"message": "Endpoint is unavailable."}),
        "provider/paid",
    );
    assert!(paid.find("limit-model-switch").is_none());
}

#[test]
fn main_history_refresh_filters_app_state_catalogues_and_restores_a_model_that_answers() {
    let dir = TempDir::new();
    let store = store::Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    let _guard = native::hooks::install(store.clone(), None);
    let history = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&history).unwrap();
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store);
    s.models.insert("shelf".into(), catalog());
    s.models.insert("opencode".into(), catalog());
    let visible =
        |s: &State, key: &str, id: &str| s.models[key][0].models.iter().any(|m| m.id == id);
    assert!(!visible(&s, "shelf", FLEDGE));
    assert!(visible(&s, "shelf", BEAM));

    let refresh = |s: &mut State, mission: &str| {
        refresh_recent(s);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !s.ledger_summaries.contains_key(mission) {
            assert!(
                std::time::Instant::now() < deadline,
                "window never loaded {mission}"
            );
            poll(s, 0.0, &mut Tx::default());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    record(
        &history,
        "m_retired",
        BEAM,
        "run.failed",
        json!({"message": "Model not found: opencode/beam-free"}),
        AT,
    );
    refresh(&mut s, "m_retired");
    assert!(!visible(&s, "shelf", BEAM));
    assert!(!visible(&s, "opencode", BEAM));

    record(
        &history,
        "m_answered",
        BEAM,
        "run.completed",
        json!({}),
        "2026-11-23T12:00:00Z",
    );
    refresh(&mut s, "m_answered");
    assert!(visible(&s, "shelf", BEAM));
    assert!(visible(&s, "opencode", BEAM));
    assert!(!visible(&s, "shelf", FLEDGE));
}
