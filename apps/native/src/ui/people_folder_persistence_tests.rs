use super::*;
use locust_core::store::Store;
use rune::{Env, Runtime};

fn edit_wren(store: Store, project: &str) -> Runtime<State> {
    let mut state = State::default();
    crate::team_scene(&mut state);
    state.data = Some(store);
    state.home.folder_path = Some(project.to_owned());
    state.engine = Some(Kind::Native);
    state.models.insert(LEDGER_TARGET.into(), Vec::new());
    let team = people::team(state.data.as_ref().unwrap()).unwrap();
    apply_team(&mut state, native::parse_roster(&team.to_string()).unwrap());
    open_edit(&mut state, "tm_wren");
    let mut window = Runtime::new(
        state,
        |s, _| edit_form(s, &Palette::new(true)),
        (1100., 2400.),
        Env::new(true, 1.),
    );
    window.frame(0.);
    window
}

fn visible(window: &Runtime<State>, words: &str) -> bool {
    window
        .elements()
        .iter()
        .any(|e| e.text.as_deref() == Some(words))
}

// case: shows the folder it was pointed at, and the way back
#[test]
fn people_folder_reset_survives_reopening_without_saving_other_edits() {
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Temp(std::env::temp_dir().join(format!(
        "locust-folder-reset-{}-{}",
        std::process::id(),
        locust_core::invariants::unique_nanos(),
    )));
    std::fs::create_dir(&root.0).unwrap();
    let open_store = || Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let make_folder = |name| {
        let folder = root.0.join(name);
        std::fs::create_dir(&folder).unwrap();
        folder.to_str().unwrap().to_owned()
    };
    let project = make_folder("project");
    let old = make_folder("claude");
    let other = make_folder("other");
    let store = open_store();
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [
                    {"teammateId": "tm_wren", "name": "Wren", "role": "Code & Migrations", "hue": "lime",
                     "createdAt": "2026-09-05T05:00:00.000Z", "folder": old},
                    {"teammateId": "tm_ada", "name": "Ada", "role": "Custom", "hue": "blue",
                     "createdAt": "2026-09-05T05:00:00.000Z", "folder": other}
                ],
                "missionOwners": {"mission_one": "tm_wren"}
            }),
        )
        .unwrap();
    let mut window = edit_wren(store, &project);
    assert!(visible(&window, &old));
    assert!(visible(&window, "Use the project folder"));
    assert!(visible(&window, "Change"));
    assert!(!visible(&window, "The project folder"));

    window.update(|s, _| s.people_panel.fields[0].replace_text("Unsubmitted name"));
    window.frame(0.1);
    let (x, y) = window.find("team-folder-reset").unwrap();
    window.pointer_down(x, y);
    window.pointer_up(x, y);
    window.frame(0.2);
    assert!(visible(&window, "The project folder"));
    assert!(visible(&window, "Choose folder"));
    assert!(window.find("team-folder-reset").is_none());
    assert!(window.state.people_panel.form_open);
    assert_eq!(window.state.people_panel.fields[0].text, "Unsubmitted name");
    assert_eq!(
        window.state.home.folder_path.as_deref(),
        Some(project.as_str())
    );
    assert_eq!(
        window
            .state
            .teammates
            .iter()
            .find(|t| t.id == "tm_wren")
            .unwrap()
            .folder,
        None
    );
    drop(window);

    let reopened = edit_wren(open_store(), &project);
    assert!(visible(&reopened, "The project folder"));
    assert!(!visible(&reopened, &old));
    let team = people::team(reopened.state.data.as_ref().unwrap()).unwrap();
    assert!(team["teammates"][0].get("folder").is_none());
    assert_eq!(team["teammates"][0]["name"], "Wren");
    assert_eq!(team["teammates"][1]["folder"], other);
    assert_eq!(team["missionOwners"]["mission_one"], "tm_wren");
}
