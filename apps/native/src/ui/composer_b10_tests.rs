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

// case: shows the route handed to, over the teammate's saved model
#[test]
fn thread_composer_shows_the_handed_to_route_until_a_new_pick_overrides_it() {
    let (root, mut s) = fresh();
    s.home.home_open = false;
    let id = s.teammates[s.selected].id.clone();
    s.teammates[s.selected].model = Some("gpt-6-luna".into());
    s.teammates[s.selected].account = Some("codex-account".into());
    let header = s.teammates[s.selected].clone();
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some("mission_handed".into()),
            turns: vec![],
        },
        items: vec![],
        dir: root.0.clone(),
        worker: None,
        snapshot: ledger::Snapshot {
            tip: "mission_handed".into(),
            context: Some(locust_core::context_usage::Reading {
                used: 164_000,
                window: 200_000,
                percent: 82,
            }),
            turn_metas: vec![ledger::Metadata {
                mission_id: "mission_handed".into(),
                runtime: "claude".into(),
                model: "claude-sonnet-5".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        cold_tip: None,
        due: f64::INFINITY,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.,
    });
    s.models.insert(
        id.clone(),
        vec![engine::Models {
            runtime: "claude".into(),
            model: Some("claude-haiku-5-5".into()),
            effort: None,
            teammate: None,
            models: vec![engine::Model {
                id: "claude-sonnet-5".into(),
                name: "Claude Sonnet 5".into(),
                efforts: vec!["low".into(), "high".into()],
            }],
        }],
    );
    let cmd = |name: &str, description: &str| locust_core::runtime_commands::CommandInfo {
        name: name.into(),
        description: description.into(),
        argument_hint: String::new(),
    };
    s.runtime_commands.by_runtime.insert(
        "claude".into(),
        vec![
            cmd("context", "Claude-only stats"),
            cmd("compact", "Summarize the conversation"),
        ],
    );
    s.runtime_commands
        .by_runtime
        .insert("codex".into(), vec![cmd("review", "Codex-only review")]);
    let mut rt = window(s);
    let route = |rt: &rune::Runtime<State>| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("model-chip"))
            .and_then(|e| e.label.or(e.text))
            .unwrap()
    };
    assert!(route(&rt).contains("Sonnet"), "actual label {}", route(&rt));
    type_words(&mut rt, "/con");
    click_word(&mut rt, "/context");
    assert_eq!(rt.state.draft.text, "/context ");
    assert!(rt.find("slash-menu").is_none());
    rt.update(|s, _| s.model_menu = Some(id.clone()));
    frames(&mut rt);
    assert!(words(&rt).contains("Conversation’s route"));
    assert!(words(&rt).contains("Pick a model and account to change effort for this turn."));
    assert!(rt.find("effort-scale").is_none());
    rt.update(|s, _| {
        s.model_menu = None;
        s.runtime_commands.ring_open = true;
        s.picks.insert(
            id,
            TurnPick {
                model: "gpt-6-luna".into(),
                effort: None,
                runtime: None, // A real picker row on the teammate's saved runtime.
                account: Some(None),
            },
        );
        s.draft.replace_text("/rev");
    });
    frames(&mut rt);
    assert!(route(&rt).contains("Luna"), "actual label {}", route(&rt));
    assert!(words(&rt).contains("Codex-only review"));
    assert!(rt.find("chip-mark-codex").is_some());
    assert!(words(&rt).contains("Your next message goes to another model."));
    assert_eq!(
        rt.state.teammates[rt.state.selected].model.as_deref(),
        Some("gpt-6-luna")
    );
    // Rewriting an earlier message doesn't follow the handed-to route; the engine starts a new branch.
    rt.update(|s, _| {
        s.picks.clear();
        s.runtime_commands.ring_open = false;
        s.edit.editing = Some(crate::edit_message::Editing {
            conversation: "mission_handed".into(),
            tip: "mission_handed".into(),
            turn: "mission_handed".into(),
            before: None,
            set_aside: vec!["mission_handed".into()],
            folder: None,
            offer: None,
            put_back: false,
        });
        s.draft.replace_text("Rewritten words.");
    });
    frames(&mut rt);
    assert!(route(&rt).contains("Luna"));
    assert!(rt.find("chip-mark-codex").is_some());
}
