use super::*;
use locust_core::{approval::{Decision, Request}, approval_rules::RuleEffect, engine_api::EngineApi};
use rune::{Env, Runtime};
use std::path::PathBuf;

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn routes(tune: bool) -> Vec<CompareRoute> {
    ["Current", "How it works", "What it knows"].into_iter().enumerate().map(|(i, label)| {
        let model = if tune { "opencode/example-free".into() } else { format!("opencode/example-free-{i}") };
        let mut route = CompareRoute::new("opencode", &model);
        if tune {
            route.label = Some(label.into());
            route.instructions = Some(["Be concise.", "Check your work.", "Name your assumptions."][i].into());
        }
        route
    }).collect()
}

fn add_compare(store: &locust_core::compare::CompareStore, s: &mut State, tune: bool) -> String {
    let cmp = store.create(CreateCompareInput {
        teammate_id: tune.then(|| "tm_wren".into()),
        prompt: "Explain the cause.".into(),
        routes: routes(tune),
        changes: false,
        changes_in: None,
        blind: true,
        base: None,
        rerun_of: None,
    }).unwrap();
    for (slot, suffix) in [(CompareSlotId::A, "a"), (CompareSlotId::B, "b"), (CompareSlotId::C, "c")] {
        let id = format!("{}_{}", cmp.compare_id, suffix);
        store.add_turn(&cmp.compare_id, slot, &id).unwrap();
        s.compare.snapshots.insert(id.clone(), ledger::Snapshot {
            tip: id,
            items: vec![ledger::Item::Assistant { item_id: "answer".into(), text: "Done.".into(), r#final: true }],
            ..Default::default()
        });
    }
    s.compare.compares.push(store.get(&cmp.compare_id).unwrap().unwrap());
    cmp.compare_id
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..14 { rt.frame(0.05); }
}

fn window(tune: bool) -> (Dirs, locust_core::compare::CompareStore, Runtime<State>) {
    let dir = Dirs(std::env::temp_dir().join(format!("locust-compare-rubric-{}", locust_core::invariants::unique_nanos())));
    std::fs::create_dir_all(dir.0.join("electron/mission-ledger")).unwrap();
    let store = crate::store::Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    let compare_store = locust_core::compare::CompareStore::from_store(&store).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    let model = |id: &str, name: &str| locust_core::engine::Model { id: id.into(), name: name.into(), efforts: vec![] };
    s.models.insert("opencode".into(), vec![locust_core::engine::Models {
        teammate: None, runtime: "opencode".into(), model: None, effort: None,
        models: vec![model("opencode/judge-free", "Judge Free"), model("opencode/judge-two", "Judge Two"),
            model("opencode/judge-three", "Judge Three")],
    }]);
    s.compare.active_id = Some(add_compare(&compare_store, &mut s, tune));
    let mut rt = Runtime::new(s, crate::view, (1450., 1500.), Env::new(true, 1.));
    settle(&mut rt);
    (dir, compare_store, rt)
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.as_deref()).collect::<Vec<_>>().join("\n")
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} on the comparison"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

// case: says it reads the setups without their names, and that one run of each can be luck
#[test]
fn compare_judge_explains_blind_setups_and_why_to_repeat_them() {
    let (_dir, _store, rt) = window(true);
    let text = words(&rt);
    assert!(text.contains("It reads the answers without the setups’ names"), "{text}");
    assert!(text.contains("One run of each setup can be luck. Once the judge has scored them, run the three again, and keep the setup whose scores hold up."));
}

// case: says neither of a comparison of models
#[test]
fn compare_judge_explains_blind_models_without_the_setup_note() {
    let (_dir, _store, rt) = window(false);
    let text = words(&rt);
    assert!(text.contains("It reads the answers without the models’ names"), "{text}");
    assert!(!text.contains("One run of each setup can be luck"));
}

// Records the app's actual judge requests. A fake engine here means no tools, account or network is used.
#[derive(Default)]
struct FakeEngine(Mutex<Vec<(native::Send, Origin)>>);
impl EngineApi for FakeEngine {
    fn send(&self, job: native::Send, origin: Origin) -> Result<locust_core::engine::Started, String> {
        let mut calls = self.0.lock().unwrap();
        calls.push((job, origin));
        Ok(locust_core::engine::Started { conversation_id: format!("fixture_judge_{}", calls.len()), mode: "ask".into() })
    }
    fn stop(&self, _: &str) -> Result<String, String> { Err("no live run".into()) }
    fn owns(&self, _: &str) -> bool { false }
    fn running(&self) -> Vec<String> { vec![] }
    fn waiting(&self) -> Vec<Request> { vec![] }
    fn decide(&self, _: &str, _: Decision, _: Option<String>) -> Result<(), String> { Err("no approval".into()) }
    fn answer(&self, _: &str, _: Vec<(String, Vec<String>)>) -> Result<(), String> { Err("no question".into()) }
    fn save_rule_and_decide(&self, _: &Request, _: RuleEffect) -> Result<(), String> { Err("no rule".into()) }
    fn carry(&self, _: &str, _: &str, _: Option<&str>) -> Result<(String, Option<String>, String), String> { Err("no carry".into()) }
    fn shut_down(&self) {}
    fn wait_for_shutdown(&self) {}
    fn room_notices(&self) -> Vec<locust_core::room_tasks::RoomNotice> { vec![] }
}

// JudgeAsk's textarea and handlers, exercised by the actual window, not by seeding its criteria state.
#[test]
fn compare_judges_use_the_points_typed_in_this_comparison() {
    const CHILD: &str = "LOCUST_COMPARE_RUBRIC_FIXTURE";
    if std::env::var_os(CHILD).is_none() {
        let test = concat!(module_path!(), "::compare_judges_use_the_points_typed_in_this_comparison");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"]).env(CHILD, "1").output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success() && stdout.contains("1 passed"), "{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
        return;
    }
    let engine = Arc::new(FakeEngine::default());
    locust_core::engine_api::install(engine.clone()).unwrap();
    let (_dir, store, mut rt) = window(true);
    let id = rt.state.compare.active_id.clone().unwrap();

    press(&mut rt, "compare-judge-three-toggle");
    press(&mut rt, "compare-ask-judge");
    assert!(engine.0.lock().unwrap().is_empty(), "blank points sent no turns");
    let note = "Three judges vote on the points you write: say what a good answer does, one point a line.";
    assert!(words(&rt).contains(note));

    let available = rt.state.models["opencode"].clone();
    rt.update(|s, _| s.models.get_mut("opencode").unwrap()[0].models.truncate(1));
    press(&mut rt, "compare-ask-judge");
    assert!(engine.0.lock().unwrap().is_empty(), "three judges cannot silently become one");
    assert!(words(&rt).contains("Pick three different models to judge."));
    rt.update(|s, _| { s.models.insert("opencode".into(), available); });
    settle(&mut rt);

    let points = "Names a price for five seats\nNames the assumptions";
    press(&mut rt, "compare-judge-criteria");
    rt.paste(points);
    settle(&mut rt);
    assert_eq!(rt.state.compare.criteria.get(&id).map(|edit| edit.text.as_str()), Some(points), "the points field accepted actual typing");
    let mut other = None;
    rt.update(|s, _| { other = Some(add_compare(&store, s, false)); s.compare.active_id = other.clone(); });
    settle(&mut rt);
    assert!(!words(&rt).contains("Names a price for five seats"), "another comparison did not inherit the points");
    press(&mut rt, "compare-ask-judge");
    assert!(engine.0.lock().unwrap().is_empty(), "another comparison did not send the points");
    let other = other.unwrap();
    rt.update(|s, _| s.compare.active_id = Some(id.clone()));
    settle(&mut rt);

    press(&mut rt, "compare-ask-judge");
    let captured = engine.0.lock().unwrap();
    assert_eq!(captured.len(), 3, "the real Ask press sends to all three distinct judges");
    let names: HashSet<_> = captured.iter().map(|(job, _)| job.model.as_deref().unwrap()).collect();
    assert_eq!(names, HashSet::from(["opencode/judge-free", "opencode/judge-two", "opencode/judge-three"]));
    for (job, origin) in captured.iter() {
        assert!(job.prompt.contains("1. Names a price for five seats") && job.prompt.contains("2. Names the assumptions"), "each judge got the typed points: {:?}", job.prompt);
        assert!(job.prompt.contains("Answer A:") && !job.prompt.contains("How it works"), "only answer letters go to the judge");
        assert_eq!(origin.mode, Some(locust_core::approval::Mode::Ask));
    }
    drop(captured);
    let saved = store.get(&id).unwrap().unwrap();
    assert!(saved.kept.is_none());
    let judge = saved.judge.expect("the button recorded its judge");
    assert_eq!(judge.criteria.as_deref(), Some(points));
    assert_eq!(judge.panel.len(), 2);

    // A draft requested in the other comparison must not land in this one's textarea.
    rt.update(|s, _| s.compare.active_id = Some(other.clone()));
    settle(&mut rt);
    press(&mut rt, "compare-draft-points");
    let draft = rt.state.compare.draft_mission_id.clone().expect("Draft the points asked the engine");
    let ledger_dir = rt.state.data.as_ref().unwrap().ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger_dir).unwrap();
    use serde_json::json;
    let rows = [
        json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":draft,"runtime":"opencode","model":"opencode/judge-free","prompt":"Draft","createdAt":"2026-10-21T12:00:00Z"}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-10-21T12:00:01Z","type":"message.delta",
            "payload":{"itemId":"draft","text":"POINT 1: Explains the cause.","operation":"replace","final":true}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":2,"occurredAt":"2026-10-21T12:00:02Z","type":"run.completed","payload":{}}}),
    ];
    std::fs::write(ledger_dir.join(format!("{draft}.jsonl")), rows.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
    rt.update(|s, _| {
        s.compare.active_id = Some(id.clone());
        poll(s);
    });
    assert_eq!(rt.state.compare.criteria.get(&id).map(|e| e.text.as_str()), Some(points));
    assert_eq!(rt.state.compare.criteria.get(&other).map(|e| e.text.as_str()), Some("Explains the cause."), "drafted points belong to the comparison that asked");
}
