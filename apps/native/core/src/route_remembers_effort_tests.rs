//! A teammate's remembered route keeps its effort (Electron `main/route-remembers-effort.test.ts`).
//!
//! Colin, 2026-09-16: "my effort levels are resetting." Two places dropped it: the host built the route it remembered
//! from three fields, and the store rebuilt it from three again.
use super::*;
use crate::native::{Send, hooks, send_from};

struct Temp(std::path::PathBuf);
impl Temp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-route-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(p.join("electron/mission-ledger")).unwrap();
        std::fs::create_dir_all(p.join("work")).unwrap();
        Self(p)
    }
    fn store(&self) -> Store {
        Store::new(self.0.join("native"), self.0.join("electron")).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wren(store: &Store, route: Option<Value>) -> String {
    let mut entry = json!({"teammateId": "", "name": "Wren", "hue": "lime", "role": "Custom", "roleTitle": "Code & Migrations"});
    if let Some(route) = route {
        entry["route"] = route;
    }
    save_teammate_id(store, entry).unwrap()
}

fn route_of(store: &Store, id: &str) -> Value {
    team(store).unwrap()["teammates"].as_array().unwrap().iter().find(|t| t["teammateId"] == id).unwrap()["route"].clone()
}

// case: keeps the effort it was started with, and clears it when the next start has none
#[test]
fn keeps_the_effort_it_was_started_with_and_clears_it_when_the_next_start_has_none() {
    let t = Temp::new("effort");
    let store = t.store();
    let id = wren(&store, None);
    remember_route(&store, &id, &json!({"runtime": "codex", "model": "gpt-5.2", "mode": "auto", "effort": "low"})).unwrap();
    assert_eq!(route_of(&store, &id), json!({"runtime": "codex", "model": "gpt-5.2", "mode": "auto", "effort": "low"}));
    // A fresh store reads the file, not memory.
    assert_eq!(route_of(&t.store(), &id)["effort"], "low");
    // A start on a runtime with no levels records no effort -- and must not inherit the old one.
    remember_route(&store, &id, &json!({"runtime": "cursor", "model": "composer-2.5", "mode": "auto"})).unwrap();
    assert_eq!(route_of(&store, &id), json!({"runtime": "cursor", "model": "composer-2.5", "mode": "auto"}));
}

#[test]
fn a_route_that_is_not_one_changes_nothing() {
    let t = Temp::new("invalid");
    let store = t.store();
    let id = wren(&store, None);
    let kept = json!({"runtime": "codex", "model": "gpt-5.2", "mode": "ask", "effort": "high"});
    remember_route(&store, &id, &kept).unwrap();
    for bad in [
        json!({"runtime": "nope", "model": "m", "mode": "ask"}),
        json!({"runtime": "codex", "model": "", "mode": "ask"}),
        json!({"runtime": "codex", "model": "x".repeat(201), "mode": "ask"}),
        json!({"runtime": "codex", "model": "m", "mode": "yolo"}),
        json!({"runtime": "codex", "model": "m", "mode": "ask", "effort": ""}),
        json!({"runtime": "codex", "model": "m", "mode": "ask", "effort": "e".repeat(41)}),
        json!({"runtime": "codex", "model": "m", "mode": "ask", "account": "work"}),
        json!("codex"),
    ] {
        remember_route(&store, &id, &bad).unwrap();
        assert_eq!(route_of(&store, &id), kept, "{bad}");
    }
    // An added account's id and a plan run are routes.
    let account = format!("account_{}", "a".repeat(32));
    let planned = json!({"runtime": "claude", "model": "m", "mode": "plan", "account": account});
    remember_route(&store, &id, &planned).unwrap();
    assert_eq!(route_of(&store, &id), planned);
    // Nobody by that id: nothing written, nothing refused.
    remember_route(&store, "tm_nobody", &kept).unwrap();
    assert_eq!(route_of(&store, &id), planned);
}

fn teammate(t: &Temp, id: &str, runtime: &str) -> crate::engine::Teammate {
    crate::engine::Teammate {
        id: id.into(),
        name: "Wren".into(),
        role: "Code & Migrations".into(),
        runtime: Some(runtime.into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "auto".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    }
}

fn fake_codex() -> Option<(std::path::PathBuf, Vec<String>)> {
    Some(("python3".into(), vec![concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py").into(), "success".into()]))
}

fn start(t: &Temp, id: &str, effort: Option<&str>) {
    let started = send_from(
        Send {
            teammate: teammate(t, id, "codex"),
            conversation: None,
            prompt: "Say hello.".into(),
            model: Some("gpt-5.2".into()),
            effort: effort.map(str::to_owned),
        },
        crate::native::Origin { mode: Some(crate::approval::Mode::Ask), ..Default::default() },
    );
    assert!(started.is_ok(), "{started:?}");
}

#[test]
fn a_start_remembers_its_route_with_its_effort_on_a_teammate_that_has_none() {
    let t = Temp::new("start");
    let _g = hooks::install(t.store(), fake_codex());
    let id = wren(&t.store(), None);
    start(&t, &id, Some("low"));
    assert_eq!(route_of(&t.store(), &id), json!({"runtime": "codex", "model": "gpt-5.2", "mode": "ask", "effort": "low"}));
}

#[test]
fn a_start_leaves_a_teammate_s_own_route_as_it_was_set() {
    // index.ts: a conversation keeps its own model; the default changes only where it is set, the teammate's profile.
    let t = Temp::new("kept");
    let _g = hooks::install(t.store(), fake_codex());
    let own = json!({"runtime": "codex", "model": "gpt-5", "mode": "ask", "effort": "high"});
    let id = wren(&t.store(), Some(own.clone()));
    start(&t, &id, Some("low"));
    assert_eq!(route_of(&t.store(), &id), own);
}

#[test]
fn a_comparison_column_is_not_a_start_of_the_teammate_s_own() {
    // index.ts: a compare's column runs on a route picked for that column; the teammate keeps none from it.
    let t = Temp::new("slot");
    let _g = hooks::install(t.store(), fake_codex());
    let id = wren(&t.store(), None);
    let started = send_from(
        Send {
            teammate: teammate(&t, &id, "codex"),
            conversation: None,
            prompt: "Say hello.".into(),
            model: Some("gpt-5.2".into()),
            effort: Some("low".into()),
        },
        crate::native::Origin {
            mode: Some(crate::approval::Mode::Ask),
            slot: Some(crate::native::SlotOrigin { key: "a".into(), ..Default::default() }),
            ..Default::default()
        },
    );
    assert!(started.is_ok(), "{started:?}");
    assert_eq!(route_of(&t.store(), &id), Value::Null);
}
