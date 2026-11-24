//! Electron `renderer/src/claudes-cloud-says-what-it-works-on.test.tsx`: Colin's first send went from `.claude`, which is
//! not on GitHub; the panel now says so before a send, with his folders that are on GitHub.

use super::*;
use rune::{Cx, Env, Runtime};

fn words(panel: Panel) -> String {
    fn render(s: &Panel, _: &mut Cx<Panel>) -> El {
        view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(panel, render, (480.0, 640.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n")
}

fn dot_claude(claude: Option<ClaudeSide>) -> Panel {
    Panel {
        place: Some(CloudWhere { folder_name: Some(".claude".into()), repo: None, branch: None }),
        folders: Some(vec![CloudFolder {
            id: "f1".into(),
            name: "locust-astra".into(),
            repo: "owner/repo".into(),
            path: "/work/locust-astra".into(),
            environment_ready: false,
        }]),
        claude,
        codex_tasks: 0,
    }
}

#[test]
// case: says a folder not on GitHub starts with no project, and offers the folders that are
fn says_a_folder_not_on_github_starts_with_no_project_and_offers_the_folders_that_are() {
    let html = words(dot_claude(Some(ClaudeSide { picked: true, sessions: vec![] })));
    assert!(html.contains("Claude’s cloud works on a copy of a"), "{html}");
    assert!(html.contains("starts with no project"), "{html}");
    assert!(!html.contains("Codex Cloud, not on this computer"), "{html}");
    assert!(html.contains("locust-astra"), "{html}");
    // Codex's environment is not Claude's concern.
    assert!(!html.contains("no cloud environment"), "{html}");
}

#[test]
fn codex_cloud_still_says_its_own_words_and_the_missing_environment() {
    let html = words(dot_claude(None));
    assert!(html.contains(".claude is not on GitHub. A cloud task runs on a copy of a GitHub repository in Codex Cloud, not on this computer"), "{html}");
    assert!(!html.contains("starts with no project"), "{html}");
    assert!(html.contains("owner/repo · no cloud environment"), "{html}");
    assert!(html.contains("Codex Cloud"), "{html}");
}

#[test]
fn codex_tasks_here_keep_codexs_words_even_with_claude_picked() {
    let mut panel = dot_claude(Some(ClaudeSide { picked: true, sessions: vec![] }));
    panel.codex_tasks = 1;
    let html = words(panel);
    assert!(html.contains("Codex Cloud, not on this computer"), "{html}");
    assert!(html.contains("owner/repo · no cloud environment"), "{html}");
}

#[test]
fn a_folder_on_github_says_nothing_of_it_and_names_its_repository() {
    let mut panel = dot_claude(None);
    panel.place = Some(CloudWhere { folder_name: Some("locust".into()), repo: Some("owner/locust".into()), branch: Some("main".into()) });
    let html = words(panel);
    assert!(!html.contains("is not on GitHub"), "{html}");
    assert!(html.contains("owner/locust · main"), "{html}");
}

#[test]
fn while_folders_are_checked_and_when_none_is_on_github_it_says_so() {
    let mut panel = dot_claude(Some(ClaudeSide { picked: true, sessions: vec![] }));
    panel.folders = None;
    assert!(words(panel.clone()).contains("Checking your other folders…"));
    panel.folders = Some(vec![]);
    assert!(words(panel).contains("None of the folders Locust knows is on GitHub yet. Push this project to GitHub, or open one that is."));
}

// Electron `renderer/src/a-cloud-environment-is-optional-and-validated.test.tsx` (W6: "In the cloud start form add an
// optional field"). Its third case renders the Composer, which has no cloud start form here yet.

const FIELD: &str = "cloud-environment";

/// The field's words held in the app's state, as a start form would hold them.
fn field(value: &str) -> Runtime<State> {
    fn render(s: &State, _: &mut Cx<State>) -> El {
        let value = s.question_answers.typed.get(FIELD).cloned().unwrap_or_default();
        environment_field(&value, |s: &mut State| s.question_answers.typed.entry(FIELD.into()).or_default(), &Palette::new(true))
    }
    let mut s = State::default();
    s.question_answers.typed.entry(FIELD.into()).or_default().text = value.into();
    let mut rt = Runtime::new(s, render, (480.0, 240.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn field_words(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect::<Vec<_>>().join("\n")
}

#[test]
// case: The cloud form names the optional environment and explains when it is saved.
fn the_cloud_form_names_the_optional_environment_and_explains_when_it_is_saved() {
    let rt = field("");
    let html = field_words(&rt);
    assert!(html.contains("Run on your organization's environment (ID)"), "{html}");
    assert!(html.contains("Optional"), "{html}");
    assert!(html.contains("Saved for this folder"), "{html}");
    // aria-invalid="false"
    assert!(rt.find("cloud-environment-help").is_some() && rt.find("cloud-environment-invalid").is_none());
}

#[test]
// case: The cloud form marks invalid environment ids and explains how to correct them.
fn the_cloud_form_marks_invalid_environment_ids_and_explains_how_to_correct_them() {
    let rt = field("bad & calc");
    let html = field_words(&rt);
    // aria-invalid="true"
    assert!(rt.find("cloud-environment-invalid").is_some() && rt.find("cloud-environment-help").is_none());
    assert!(html.contains("Use ccpool_ followed by 1 to 80"), "{html}");
    let rt = field("ccpool_test");
    assert!(rt.find("cloud-environment-help").is_some(), "a real id is valid");
}
