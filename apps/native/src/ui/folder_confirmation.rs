//! First-send question at the common dispatch boundary. Save before dispatch.
use super::*;

const FILE: &str = "folder-cautions.json";

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Question {
    folder: String,
    teammate: String,
    why: String,
}

fn target(s: &State) -> Option<(String, String)> {
    let (mate, conversation) = selected_conversation(s)?;
    if conversation.conversation_id.is_some() {
        return None;
    }
    // The Electron door keeps its own workspace caution. The native engine uses the
    // teammate's project first; ask about that project rather than an unrelated Home folder.
    if s.engine != Some(Kind::Native) {
        return Some((mate.id.clone(), s.home.folder_path.clone()?));
    }
    let folder = s.data.as_ref().and_then(|store| native::folder_for(mate, store.electron()).ok())
        .or_else(|| {
            mate.folder.as_deref().map(std::path::PathBuf::from)
                .filter(|p| p.is_absolute() && p.is_dir())
        })
        .or_else(|| s.home.folder_path.as_deref().map(std::path::PathBuf::from))?;
    let folder = folder.canonicalize().unwrap_or(folder).to_string_lossy().into_owned();
    Some((mate.id.clone(), folder))
}

fn confirmed(store: &store::Store) -> Result<Vec<String>, String> {
    let value = store.seeded_json(FILE, serde_json::json!({"schemaVersion":1,"confirmed":[]}))?;
    Ok(kept_folders(&value))
}

/// workspace.ts readConfirmedFolders: only absolute folders count; anything else in the file is read as nothing.
fn kept_folders(value: &serde_json::Value) -> Vec<String> {
    value["confirmed"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).filter(|p| std::path::Path::new(p).is_absolute()).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn same_folder(a: &str, b: &str) -> bool {
    let real = |p: &str| std::fs::canonicalize(p).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_owned());
    same_folder_on(&real(a), &real(b), cfg!(windows))
}

/// workspace.ts sameFolder: the same folder only (never a child), a trailing separator ignored, and on Windows either
/// slash and any case.
fn same_folder_on(a: &str, b: &str, windows: bool) -> bool {
    let normalize = |p: &str| {
        let p = if windows { p.replace('\\', "/").to_lowercase() } else { p.to_owned() };
        let trimmed = p.trim_end_matches('/');
        if trimmed.is_empty() { "/".to_owned() } else { trimmed.to_owned() }
    };
    normalize(a) == normalize(b)
}

pub(super) fn hold_send(s: &mut State) -> bool {
    let Some((teammate, folder)) = target(s) else {
        s.folder_question = None;
        return false;
    };
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok();
    let canonical = |path: String| std::fs::canonicalize(&path)
        .map(|real| real.to_string_lossy().into_owned()).unwrap_or(path);
    let home = home.map(canonical);
    let system = std::env::var("SystemRoot").ok().map(canonical);
    let Some(why) = locust_core::thread_view::folder_caution(
        &folder,
        home.as_deref(),
        cfg!(windows),
        system.as_deref(),
    ) else {
        s.folder_question = None;
        return false;
    };
    let kept = s
        .data
        .as_ref()
        .and_then(|store| confirmed(store).ok())
        .unwrap_or_default();
    if kept.iter().any(|p| same_folder(p, &folder)) {
        s.folder_question = None;
        return false;
    }
    s.folder_question = Some(Question {
        folder,
        teammate,
        why,
    });
    true
}

fn current(s: &State) -> Option<&Question> {
    let question = s.folder_question.as_ref()?;
    let (mate, folder) = target(s)?;
    (question.teammate == mate && question.folder == folder).then_some(question)
}

fn keep_answer(s: &mut State) -> bool {
    let Some(question) = current(s).cloned() else {
        s.folder_question = None;
        return false;
    };
    let answer = (|| {
        let store = s.data.as_ref().ok_or("The data folder is unavailable.")?;
        // A file that cannot be read is read as no answers and written over, as Electron's is: never a question
        // that can never be answered.
        let mut kept = confirmed(store).unwrap_or_default();
        if !kept.iter().any(|p| same_folder(p, &question.folder)) {
            kept.push(question.folder);
        }
        store.save_json(
            FILE,
            &serde_json::json!({"schemaVersion":1,"confirmed":kept}),
        )
    })();
    match answer {
        Ok(()) => {
            s.folder_question = None;
            true
        }
        Err(error) => {
            s.composer_note = Some(format!(
                "Your answer could not be saved. Your message is still here; nothing was sent. ({error})"
            ));
            false
        }
    }
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let question = current(s)?;
    let title = format!(
        "Start this conversation in {}?",
        folders::folder_name(&question.folder)
    );
    Some(col().key("folder-question").w_full().gap(8.0).padding(12.0).role("group").label(title.clone())
        .child(text(format!("{title} This is {}, and a teammate here reads it as its project, sign-in files included.", question.why)).font(type_::BODY).color(p.text).w_full().wrap())
        .child(row().gap(12.0)
            .child(text("Choose another folder").key("folder-question-choose").font(type_::LABEL).color(p.text).role("button").on_press_tx(|s: &mut State, tx| {
                s.folder_question = None;
                crate::restored_folders_card::choose_workspace(s, tx);
            }))
            .child(text("Start here").key("folder-question-start").font(type_::LABEL).color(p.text).role("button").on_press_tx(|s: &mut State, tx| {
                if keep_answer(s) { crate::composer_wiring::submit_composer(s, tx, false); }
            }))))
}

#[cfg(test)]
#[path = "settings_folder_once_tests.rs"]
mod settings_folder_once_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> (State, std::path::PathBuf) {
        let root =
            std::path::PathBuf::from(std::env::var_os("LOCUST_TEST_ROOT").unwrap()).join(format!(
                "folder-question-{}",
                locust_core::invariants::unique_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let mut s = State::default();
        crate::composer_base_scene(&mut s, false);
        s.demo = false;
        s.data = Some(store::Store::new(root.join("native"), root.join("electron")).unwrap());
        s.selected = 0;
        s.ledger = None;
        for mate in &mut s.teammates {
            mate.folder = None;
        }
        for conversation in s.conversations.values_mut() {
            conversation.conversation_id = None;
        }
        s.home.folder_path = Some(format!("{}/.claude", std::env::var("HOME").unwrap()));
        s.draft.replace_text("Read this first.");
        s.attached = vec!["README.md".into()];
        (s, root)
    }
    fn rt(s: State) -> rune::Runtime<State> {
        let mut rt =
            rune::Runtime::new(
                s,
                |s, _| {
                    col()
                        .w_full()
                        .children(view(s, &Palette::new(true)))
                        .child(text("Submit").key("submit-test").on_press_tx(
                            |s: &mut State, tx| {
                                crate::composer_wiring::submit_composer(s, tx, false)
                            },
                        ))
                        .child(
                            text("Direct")
                                .key("direct-test")
                                .on_press_tx(|s: &mut State, tx| send(s, tx)),
                        )
                },
                (1200., 800.),
                rune::Env::new(true, 1.),
            );
        rt.frame(0.);
        rt
    }
    fn click(rt: &mut rune::Runtime<State>, key: &str) {
        let (x, y) = rt.find(key).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.);
    }
    #[test]
    fn names_the_folder_and_why_with_choose_another_folder_first_and_start_here() {
        let (mut s, root) = state();
        let folder = r"C:\Users\person\.claude".to_owned();
        s.home.folder_path = Some(folder.clone());
        s.folder_question = Some(Question {
            folder,
            teammate: s.teammates[0].id.clone(),
            why: ".claude, where the agents keep their own settings".into(),
        });
        let rt = rt(s);
        let words = rt.accessibility_json().to_string();
        assert!(
            words.contains("Start this conversation in .claude?"),
            "{words}"
        );
        assert!(words.contains("This is .claude, where the agents keep their own settings, and a teammate here reads it as its project, sign-in files included."));
        let elements = rt.elements();
        let choose = elements
            .iter()
            .position(|e| e.key.as_deref() == Some("folder-question-choose"))
            .unwrap();
        let start = elements
            .iter()
            .position(|e| e.key.as_deref() == Some("folder-question-start"))
            .unwrap();
        assert!(choose < start);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn the_first_submit_keeps_the_entire_draft_and_direct_dispatch_also_waits() {
        let (s, root) = state();
        assert!(can_send(&s));
        let mut rt = rt(s);
        click(&mut rt, "submit-test");
        assert!(rt.state.folder_question.is_some());
        assert!(rt.state.flight.is_none());
        assert_eq!(rt.state.draft.text, "Read this first.");
        assert_eq!(rt.state.attached, ["README.md"]);
        assert!(rt.state.sent_history.is_empty());
        rt.state.folder_question = None;
        click(&mut rt, "direct-test");
        assert!(rt.state.folder_question.is_some());
        assert!(rt.state.flight.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn an_answer_is_durable_for_exactly_that_folder_and_not_its_children() {
        let (mut s, root) = state();
        assert!(hold_send(&mut s));
        assert!(keep_answer(&mut s));
        let (mut reopened, other) = state();
        reopened.data = s.data.clone();
        assert!(!hold_send(&mut reopened));
        reopened
            .home
            .folder_path
            .as_mut()
            .unwrap()
            .push_str("/skills");
        assert!(hold_send(&mut reopened));
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(other).unwrap();
    }
    #[test]
    fn changing_folder_or_teammate_invalidates_the_question_without_saving() {
        let (mut s, root) = state();
        assert!(hold_send(&mut s));
        let original = s.home.folder_path.clone();
        s.home.folder_path = Some("/projects/shop".into());
        assert!(current(&s).is_none());
        assert!(!keep_answer(&mut s));
        assert!(confirmed(s.data.as_ref().unwrap()).unwrap().is_empty());
        assert!(!hold_send(&mut s));
        s.home.folder_path = original;
        assert!(hold_send(&mut s));
        s.selected = 1;
        assert!(!keep_answer(&mut s));
        assert!(confirmed(s.data.as_ref().unwrap()).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn a_failed_save_keeps_the_question_and_sends_nothing() {
        let (mut s, root) = state();
        assert!(hold_send(&mut s));
        let path = s.data.as_ref().unwrap().root().join(FILE);
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(!keep_answer(&mut s));
        assert!(s.folder_question.is_some());
        assert!(s.flight.is_none());
        assert_eq!(s.draft.text, "Read this first.");
        assert!(
            s.composer_note
                .as_ref()
                .unwrap()
                .contains("nothing was sent")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn the_start_button_saves_the_answer_before_submission_rechecks_readiness() {
        let (mut s, root) = state();
        assert!(hold_send(&mut s));
        s.selected_ready = false; // No agent may be started by this headless click.
        let mut rt = rt(s);
        click(&mut rt, "folder-question-start");
        assert!(rt.state.folder_question.is_none());
        assert_eq!(confirmed(rt.state.data.as_ref().unwrap()).unwrap().len(), 1);
        assert_eq!(rt.state.draft.text, "Read this first.");
        assert_eq!(rt.state.attached, ["README.md"]);
        assert!(rt.state.flight.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn choosing_another_folder_keeps_the_draft_and_does_not_confirm_the_old_one() {
        let (mut s, root) = state();
        assert!(hold_send(&mut s));
        let mut rt = rt(s);
        click(&mut rt, "folder-question-choose");
        assert!(rt.state.folder_question.is_none());
        assert!(
            confirmed(rt.state.data.as_ref().unwrap())
                .unwrap()
                .is_empty()
        );
        assert_eq!(rt.state.draft.text, "Read this first.");
        assert_eq!(rt.state.attached, ["README.md"]);
        assert!(rt.state.flight.is_none());
        assert!(hold_send(&mut rt.state)); // Cancelled chooser has granted nothing.
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn an_existing_conversation_does_not_ask_again() {
        let (mut s, root) = state();
        let id = s.teammates[0].id.clone();
        s.conversations.get_mut(&id).unwrap().conversation_id = Some("already-started".into());
        assert!(!hold_send(&mut s));
        assert!(s.folder_question.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
