use super::*;
use locust_core::approval::Decision;
use rune::{Env, Runtime};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

fn teammate(id: &str, name: &str, role: &str) -> Teammate {
    Teammate { id: id.into(), name: name.into(), role: role.into(), runtime: None, model: None,
        account: None, busy: true, runs_in: "ask".into(), folder: None }
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).expect(key);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..6 { rt.frame(0.05); }
}

fn shows(rt: &Runtime<State>, words: &str) -> bool {
    rt.elements().iter().any(|el| !el.exiting && el.text.as_deref() == Some(words))
}

fn button_state(tree: &Value, label: &str) -> Option<String> {
    if tree["name"] == label {
        return tree["description"].as_str().map(str::to_owned);
    }
    tree["children"].as_array()?.iter().find_map(|child| button_state(child, label))
}

// Flyout regression. teammate_faces.rs already covers Electron’s compact-rail accessibility case.
#[test]
fn attention_rail_names_only_the_teammate_waiting_for_the_person() {
    // The running app uses one shared approval channel. Keep this fixture's card out of other window tests.
    const CHILD: &str = "LOCUST_ATTENTION_RAIL_FIXTURE";
    if std::env::var_os(CHILD).is_none() {
        let test = concat!(module_path!(), "::attention_rail_names_only_the_teammate_waiting_for_the_person");
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
            .env(CHILD, "1")
            .output().unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(result.status.success() && stdout.contains("1 passed"), "{stdout}\n{}", String::from_utf8_lossy(&result.stderr));
        return;
    }
    let (mission, run) = ("mission_attention_rail", "run_attention_rail");
    let channel = native::approvals_channel();
    let deadline = std::time::Duration::from_secs(10);
    // Feed the real in-process channel at the same boundary the permission bridge uses.
    // The cloud runner disallows binding Unix sockets; no permission transport is faked here.
    let cancel = Arc::new(AtomicBool::new(false));
    struct Cancel(Arc<AtomicBool>);
    impl Drop for Cancel {
        fn drop(&mut self) { self.0.store(true, Ordering::Relaxed); }
    }
    let _release = Cancel(cancel.clone());
    let request = std::thread::spawn(move || channel.ask_claude(
        run, mission, "permission", "Write", &json!({"file_path": "C:/work/a.txt", "content": "OK"}), "C:/work",
        &|| cancel.load(Ordering::Relaxed),
    ));
    let engine = locust_core::engine_api::api();
    let until = std::time::Instant::now() + deadline;
    let pending = loop {
        if let Some(card) = engine.waiting_for(mission).into_iter().next() { break card; }
        assert!(std::time::Instant::now() < until, "the real approval channel did not raise a card");
        std::thread::sleep(Duration::from_millis(5));
    };

    let mut s = State::default();
    s.demo = true;
    s.layout_pref = "compact".into();
    s.teammates = vec![
        teammate("tm_wren", "Wren", "Code & Migrations"),
        teammate("tm_sable", "Sable", "Ops & Scheduling"),
    ];
    s.mission_owners.insert(mission.into(), "tm_wren".into());
    let mut rt = Runtime::new(s, crate::view, (900., 720.), Env::new(true, 1.));
    for _ in 0..6 { rt.frame(0.05); }

    let tree = rt.accessibility_json();
    assert_eq!(button_state(&tree, "Message Wren · Code & Migrations").as_deref(), Some("waiting on you"));
    assert_eq!(button_state(&tree, "Message Sable · Ops & Scheduling").as_deref(), Some("working"));

    press(&mut rt, "rail-tm_wren");
    assert!(rt.find("rail-flyout").is_some() && rt.state.rail_pinned, "the real face opened its flyout");
    assert!(shows(&rt, "Code & Migrations · waiting on you"));
    press(&mut rt, "rail-tm_sable");
    assert!(shows(&rt, "Ops & Scheduling · working"));
    assert!(!shows(&rt, "Ops & Scheduling · waiting on you"));
    assert_eq!(engine.waiting_for(mission)[0].approval_id, pending.approval_id, "the flyout is only a notice");

    press(&mut rt, "rail-tm_wren");
    engine.decide(&pending.approval_id, Decision::Deny, None).unwrap();
    assert_eq!(request.join().unwrap().unwrap()["behavior"], "deny");
    assert!(engine.waiting_for(mission).is_empty());

    rt.update(|s, _| { home_panel::sync_board_asks(s); });
    for _ in 0..6 { rt.frame(0.05); }
    assert!(shows(&rt, "Code & Migrations · working"), "the same flyout returned to working");
    assert!(!shows(&rt, "Code & Migrations · waiting on you"));
}
