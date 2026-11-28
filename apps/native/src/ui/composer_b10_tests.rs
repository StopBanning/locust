use super::*;
use std::path::PathBuf;

pub(super) struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fresh() -> (Temp, State) {
    let root = Temp(std::env::temp_dir().join(format!(
        "locust-composer-b10-{}",
        locust_core::invariants::unique_nanos()
    )));
    for dir in ["native", "electron", "project"] {
        std::fs::create_dir_all(root.0.join(dir)).unwrap();
    }
    let store = store::Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let project = root.0.join("project");
    native::set_open_folder(&store, &project).unwrap();
    let mut s = State::default();
    crate::composer_base_scene(&mut s, false);
    s.demo = false;
    s.data = Some(store);
    s.home.folder_path = Some(project.to_string_lossy().into_owned());
    s.conversations
        .get_mut(&s.teammates[s.selected].id)
        .unwrap()
        .turns
        .clear();
    (root, s)
}

fn window(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, crate::view, (1350., 850.), rune::Env::new(true, 1.));
    frames(&mut rt);
    rt
}

fn frames(rt: &mut rune::Runtime<State>) {
    for _ in 0..12 {
        rt.frame(0.05);
    }
}

fn click_at(rt: &mut rune::Runtime<State>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    frames(rt);
}

fn click(rt: &mut rune::Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    click_at(rt, x, y);
}

fn click_word(rt: &mut rune::Runtime<State>, word: &str) {
    let [x, y, w, h] = rt
        .elements()
        .into_iter()
        .find(|e| {
            e.text.as_deref() == Some(word)
                && !e.exiting
                && e.visible[2] > 0.0
                && e.visible[3] > 0.0
        })
        .unwrap_or_else(|| panic!("missing visible word {word}"))
        .visible;
    click_at(rt, x + w / 2., y + h / 2.);
}

fn type_words(rt: &mut rune::Runtime<State>, words: &str) {
    click(rt, "composer");
    rt.paste(words);
    frames(rt);
}

fn words(rt: &rune::Runtime<State>) -> String {
    rt.accessibility_json().to_string()
}

#[test]
fn composer_slash_stop_remembers_the_run_that_is_still_starting() {
    let (_root, mut s) = fresh();
    let id = s.teammates[s.selected].id.clone();
    s.flight = Some(Flight {
        teammate_id: id.clone(),
        conversation_id: None,
        header: None,
        after_tip: None,
        message: Some("This turn is on its way.".into()),
    });
    let mut rt = window(s);
    type_words(&mut rt, "/stop");
    click_word(&mut rt, "/stop");
    assert_eq!(rt.state.draft.text, "");
    assert_eq!(
        rt.state.stop_when_named.as_deref(),
        Some(format!("pending:{id}").as_str())
    );
    assert_eq!(rt.state.status, "Stopping…");
}

#[test]
fn composer_slash_model_opens_the_real_route_picker() {
    let (_root, mut s) = fresh();
    s.engine = None; // No local engine door is configured in the isolated runner.
    let id = s.teammates[s.selected].id.clone();
    let mut rt = window(s);
    type_words(&mut rt, "/model");
    click_word(&mut rt, "/model");
    assert!(rt.state.draft.text.is_empty());
    assert_eq!(rt.state.model_menu.as_deref(), Some(id.as_str()));
    assert!(rt.state.flight.is_none());
}

fn personal_settings() -> PathBuf {
    // The same shape as the Electron .claude caution, inside our isolated test HOME.
    let folder = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".claude");
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn folder_confirmation_follows_the_teammates_project_not_the_open_folder() {
    let (root, mut s) = fresh();
    let guarded = personal_settings();
    s.teammates[s.selected].folder = Some(guarded.to_string_lossy().into_owned());
    let _fake = native::hooks::install(
        s.data.clone().unwrap(),
        Some((
            "python3".into(),
            vec![
                format!(
                    "{}/scripts/fixtures/m1-app-server.py",
                    env!("CARGO_MANIFEST_DIR")
                ),
                "success".into(),
            ],
        )),
    );
    let mut rt = window(s);
    type_words(&mut rt, "Read this first.");
    click(&mut rt, "send");
    let visible = words(&rt);
    assert!(
        visible.contains("Start this conversation in .claude?"),
        "{visible}"
    );
    assert!(visible.contains("Choose another folder") && visible.contains("Start here"));
    assert!(rt.state.flight.is_none());
    assert_eq!(rt.state.draft.text, "Read this first.");

    // A stale answer from another route must not approve either project or send the preserved text.
    rt.update(|s, _| {
        s.teammates[s.selected].folder = Some(root.0.join("project").to_string_lossy().into_owned())
    });
    frames(&mut rt);
    assert!(rt.find("folder-question-start").is_none());
    assert!(rt.state.sent_history.is_empty());

    // The exact same pinned folder can be confirmed. Hold readiness off so this click can never launch an agent.
    rt.update(|s, _| {
        s.teammates[s.selected].folder = Some(guarded.to_string_lossy().into_owned());
        s.selected_ready = false;
    });
    frames(&mut rt);
    click(&mut rt, "folder-question-start");
    assert!(rt.find("folder-question-start").is_none());
    let saved = std::fs::read_to_string(root.0.join("native/folder-cautions.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["confirmed"], serde_json::json!([guarded]));
    assert!(rt.state.flight.is_none());
    assert_eq!(rt.state.draft.text, "Read this first.");
}

#[test]
fn folder_confirmation_does_not_ask_for_a_caution_folder_a_pinned_project_will_not_use() {
    let (root, mut s) = fresh();
    let guarded = personal_settings();
    s.home.folder_path = Some(guarded.to_string_lossy().into_owned());
    s.teammates[s.selected].folder = Some(root.0.join("project").to_string_lossy().into_owned());
    let _fake = native::hooks::install(
        s.data.clone().unwrap(),
        Some((
            "python3".into(),
            vec![
                format!(
                    "{}/scripts/fixtures/m1-app-server.py",
                    env!("CARGO_MANIFEST_DIR")
                ),
                "success".into(),
            ],
        )),
    );
    let mut rt = window(s);
    type_words(&mut rt, "Read this first.");
    click(&mut rt, "send");
    assert!(rt.find("folder-question-start").is_none());
    assert!(rt.state.folder_question.is_none());
    assert_eq!(
        rt.state.flight.as_ref().and_then(|f| f.message.as_deref()),
        Some("Read this first.")
    );
}

#[cfg(unix)]
#[test]
fn folder_confirmation_also_checks_the_real_project_behind_an_alias() {
    let (root, mut s) = fresh();
    let guarded = personal_settings();
    let alias = root.0.join("project-alias");
    std::os::unix::fs::symlink(&guarded, &alias).unwrap();
    s.teammates[s.selected].folder = Some(alias.to_string_lossy().into_owned());
    let _fake = native::hooks::install(
        s.data.clone().unwrap(),
        Some((
            "python3".into(),
            vec![
                format!(
                    "{}/scripts/fixtures/m1-app-server.py",
                    env!("CARGO_MANIFEST_DIR")
                ),
                "success".into(),
            ],
        )),
    );
    let mut rt = window(s);
    type_words(&mut rt, "Read this first.");
    click(&mut rt, "send");
    assert!(words(&rt).contains("Start this conversation in .claude?"));
    assert!(rt.state.flight.is_none());
}
