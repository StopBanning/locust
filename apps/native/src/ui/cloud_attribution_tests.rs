use super::*;

struct Remove(std::path::PathBuf);
impl Drop for Remove {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn historical_cloud_thread(status: &str, problem: Option<&str>) -> (rune::Runtime<State>, Remove) {
    let root = std::env::temp_dir().join(format!("locust-cloud-words-{}", locust_core::invariants::unique_nanos()));
    let _ = std::fs::create_dir_all(&root);
    let remove = Remove(root.clone());
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    std::fs::write(store.electron().join("cloud-turns.json"), serde_json::json!({
        "tasks": {
            "t_ab12cd34ef56": {
                "taskId":"t_ab12cd34ef56", "codespace":"locust-hearth-x7q9",
                "repo":"acme/hearth", "branch":"main",
            }
        },
        "turns":{"mission_1":{"missionId":"mission_1", "taskId":"t_ab12cd34ef56", "state":status, "problem":problem}}
    }).to_string()).unwrap();

    let mut state = super::tests_live_state();
    state.data = Some(store);
    let pane = state.ledger.as_mut().unwrap();
    pane.snapshot.tip = "mission_2".into();
    pane.snapshot.turn_metas = vec![
        ledger::Metadata { mission_id: "mission_1".into(), ..Default::default() },
        ledger::Metadata { mission_id: "mission_2".into(), ..Default::default() },
    ];
    pane.items = vec![
        ledger::Item::User("First question".into()),
        ledger::Item::Assistant {item_id: "a1".into(), text: "First work".into(), r#final: true},
        ledger::Item::User("Second question".into()),
        ledger::Item::Assistant {item_id: "a2".into(), text: "Second work".into(), r#final: true},
    ];
    let mut rt = rune::Runtime::new(state, |s, _| message_column(
        &s.ledger.as_ref().unwrap().items, "Wren", &Palette::new(true), s
    ), (1000.0, 1800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    (rt, remove)
}

#[test]
// case: says where it ran, between the words and the work
fn messages_place_the_cloud_marker_after_the_right_turns_words() {
    let (rt, _remove) = historical_cloud_thread("working", None);
    let el = rt.elements();
    let y = |wanted| el.iter().find(|e| e.text.as_deref() == Some(wanted))
        .unwrap_or_else(|| panic!("missing {wanted}")).rect[1];
    assert_eq!(el.iter().filter(|e| e.text.as_deref() == Some("On your cloud computer")).count(), 1);
    assert!(y("First question") < y("On your cloud computer"));
    assert!(y("On your cloud computer") < y("First work"));
    assert!(y("First work") < y("Second question"));
    y("hearth · main · 2 CPUs, 8 GB");
}


#[test]
// case: says so when nothing changed there, or when what changed could not be read
fn messages_put_the_cloud_readout_after_the_turn_it_describes() {
    for (state, problem, expected) in [
        ("none", None, "No files changed on your cloud computer"),
        ("failed", Some("GitHub did not answer."), "What it changed on your cloud computer could not be read. GitHub did not answer."),
    ] {
        let (mut rt, _remove) = historical_cloud_thread(state, problem);
        let el = rt.elements();
        let row = |wanted| el.iter().find(|e| e.text.as_deref() == Some(wanted)).unwrap().rect[1];
        assert!(row("First work") < row(expected));
        assert!(row(expected) < row("Second question"));

        // The same durable result is shown after the last turn too, not while its ledger still runs.
        rt.update(|s, _| {
            let pane = s.ledger.as_mut().unwrap();
            pane.snapshot.tip = "mission_1".into();
            pane.snapshot.turn_metas.truncate(1);
            pane.items.truncate(2);
            pane.snapshot.running = false;
        });
        rt.frame(0.1);
        let el = rt.elements();
        let row = |wanted| el.iter().find(|e| !e.exiting && e.text.as_deref() == Some(wanted)).unwrap().rect[1];
        assert!(row("First work") < row(expected));
        rt.update(|s, _| s.ledger.as_mut().unwrap().snapshot.running = true);
        rt.frame(0.2);
        assert!(!rt.elements().iter().any(|e| !e.exiting && e.text.as_deref() == Some(expected)));
    }
}

#[test]
// case: reads a path in the task's copy there as this folder's own
fn messages_cloud_copy_paths_read_relative_to_the_original_folder() {
    let folder = Some(r"C:\work\hearth");
    assert_eq!(locust_core::path_words::relative_path("/workspaces/.locust/t_ab12cd34ef56/src/dates.ts", folder), "src/dates.ts");
    assert_eq!(locust_core::path_words::relative_path("/workspaces/.locust/t_ab12cd34ef56", folder), ".");
}
