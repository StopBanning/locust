//! Start from a GitHub issue (Electron `IssueList.tsx` and App's `openIssueList` / `chooseIssue`): `/issues` in the
//! message box lists the folder's open issues, searchable, with arrows and Enter; choosing one fills the box with its
//! brief. Nothing here sends. The words and the `gh` calls are `locust_core::github_issues`.

use super::*;
use locust_core::github_issues::{self, GhIn, ListedIssue, StartedIssue};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    folder: PathBuf,
    draft_key: String,
}

struct StagedIssue {
    target: Target,
    brief: String,
}

#[derive(Default)]
pub(super) struct Picker {
    pub open: bool,
    pub issues: Vec<ListedIssue>,
    pub query: TextEdit,
    pub at: usize,
    /// Said when the folder has no open issues, or the list or an issue could not be read.
    pub empty: Option<String>,
    /// The issue the next send starts from.
    pub pending: Option<StartedIssue>,
    picker_target: Option<Target>,
    staged: Option<StagedIssue>,
    /// Whether the folder on screen is one `gh` can read, once asked.
    pub ready: Option<(PathBuf, bool)>,
}

pub(super) enum Answer {
    Ready(PathBuf, bool),
    Listed(Target, Vec<ListedIssue>),
    Opened(Target, String, Option<(String, StartedIssue)>),
}

static GH: Mutex<Option<Arc<GhIn>>> = Mutex::new(None);

fn gh() -> Option<Arc<GhIn>> {
    let mut held = GH.lock().unwrap_or_else(|e| e.into_inner());
    // Tests never run the real gh: without a fake of their own there is no list.
    if cfg!(test) {
        return held.clone();
    }
    Some(held.get_or_insert_with(|| Arc::new(github_issues::real_gh)).clone())
}

#[cfg(test)]
pub(super) fn use_gh(gh: Arc<GhIn>) {
    *GH.lock().unwrap_or_else(|e| e.into_inner()) = Some(gh);
}

fn folder(s: &State) -> Option<PathBuf> {
    selected_conversation(s).and_then(|(t, _)| t.folder.as_ref()).or_else(|| s.teammates.get(s.selected).and_then(|t| t.folder.as_ref())).map(PathBuf::from)
}

fn target(s: &State) -> Option<Target> {
    Some(Target { folder: folder(s)?, draft_key: conversation_draft_key(s)? })
}

fn picker_is_current(s: &State, expected: &Target) -> bool {
    s.issues.open && s.issues.picker_target.as_ref() == Some(expected) && target(s).as_ref() == Some(expected)
}

/// `/issues` is offered only for a folder `gh` can read as a GitHub repository.
pub(super) fn offered(s: &State) -> bool {
    matches!((&s.issues.ready, folder(s)), (Some((at, true)), Some(now)) if *at == now)
}

/// Ask once whether the folder on screen is a GitHub one, off the window's thread.
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if s.issues.open && s.issues.picker_target.as_ref() != target(s).as_ref() {
        s.issues.open = false;
        changed = true;
    }
    if let Some(answer) = s.issues_job.take() {
        changed = true;
        match answer {
            Answer::Ready(at, ready) if folder(s).as_ref() == Some(&at) => s.issues.ready = Some((at, ready)),
            Answer::Listed(at, issues) if picker_is_current(s, &at) => {
                s.issues.issues = issues;
                s.issues.at = 0;
            }
            Answer::Opened(at, was, opened) if picker_is_current(s, &at) && s.draft.text == was => {
                if let Some((brief, issue)) = opened {
                    s.draft.replace_text(&brief);
                    s.draft.focused = true;
                    s.issues.pending = Some(issue);
                    s.issues.staged = Some(StagedIssue { target: at, brief });
                    s.issues.open = false;
                } else {
                    s.issues.empty = Some("That issue could not be read. The box is unchanged.".into());
                }
            }
            _ => {} // A stale answer cannot write into the current box.
        }
    }
    if let Some(now) = folder(s)
        && s.issues.ready.as_ref().is_none_or(|(at, _)| *at != now)
        && !s.issues_job.is_pending()
        && let Some(gh) = gh()
    {
        s.issues.ready = Some((now.clone(), false));
        s.issues_job.request((gh, now), |(gh, now)| {
            let ready = github_issues::ready(gh.as_ref(), &now);
            Answer::Ready(now, ready)
        });
    }
    changed
}

/// The list, as `/issues` opens it.
pub(super) fn open_list(s: &mut State) {
    if !offered(s) {
        return;
    }
    let (Some(at), Some(gh)) = (target(s), gh()) else { return };
    s.issues.open = true;
    s.issues.picker_target = Some(at.clone());
    s.issues.empty = None;
    s.issues.query = TextEdit::default();
    s.issues.query.focused = true;
    s.issues.issues.clear();
    s.issues_job.request((gh, at), |(gh, at)| {
        let issues = github_issues::list(gh.as_ref(), &at.folder);
        Answer::Listed(at, issues)
    });
}

fn choose(s: &mut State, number: u64) {
    let (Some(at), Some(gh)) = (target(s), gh()) else { return };
    if !picker_is_current(s, &at) || !s.issues.issues.iter().any(|issue| issue.number == number) {
        return;
    }
    s.issues_job.request((gh, at, s.draft.text.clone(), number), |(gh, at, was, number)| {
        let opened = github_issues::open_with_images(gh.as_ref(), &at.folder, number, |url| {
            if cfg!(test) { None } else { github_issues::real_image_get(url) }
        })
        .map(|(brief, issue, _images)| (brief, issue));
        Answer::Opened(at, was, opened)
    });
}

fn choose_active(s: &mut State) {
    let list = shown(s);
    if let Some(number) = list.get(s.issues.at.min(list.len().saturating_sub(1))).map(|i| i.number) {
        choose(s, number);
    }
}

fn shown(s: &State) -> Vec<&ListedIssue> {
    let needle = s.issues.query.text.trim().to_lowercase();
    s.issues
        .issues
        .iter()
        .filter(|i| needle.is_empty() || format!("{} {} {} {}", i.number, i.title, i.author.as_deref().unwrap_or(""), i.labels.join(" ")).to_lowercase().contains(&needle))
        .collect()
}

/// Arrows, Enter and Escape, while the list is open.
pub(super) fn key(s: &mut State, event: &KeyEvent) -> bool {
    if !s.issues.open {
        return false;
    }
    let count = shown(s).len();
    match event.key {
        Key::Escape => s.issues.open = false,
        Key::Down if count > 0 => s.issues.at = (s.issues.at + 1) % count,
        Key::Up if count > 0 => s.issues.at = (s.issues.at + count - 1) % count,
        Key::Enter => choose_active(s),
        _ => return false,
    }
    true
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    if !s.issues.open {
        let staged = s.issues.staged.as_ref()?;
        let issue = s.issues.pending.as_ref()?;
        if target(s).as_ref() != Some(&staged.target) || s.draft.text != staged.brief {
            return None;
        }
        return Some(row().key(format!("issue-draft-{}", issue.number)).role("status")
            .child(text(format!("#{} · {}", issue.number, issue.title)).font(type_::LABEL).color(p.secondary)));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let list = shown(s);
    let mut body = col()
        .w_full()
        .gap(4.0)
        .padding(8.0)
        .radius(10.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .key("issue-list")
        .label("Start from an issue")
        .child(
            text_input(&s.issues.query, type_::BODY)
                .key("issue-search")
                .placeholder("Search issues", p.muted)
                .label("Search issues")
                .w_full()
                .padding(6.0)
                .radius(6.0)
                .fill(p.field)
                .color(p.text)
                .on_edit(|s: &mut State| {
                    s.issues.at = 0;
                    &mut s.issues.query
                })
                // Enter in the search field chooses the highlighted issue (the field holds the keyboard).
                .on_submit(|s: &mut State, _tx: &mut Tx| choose_active(s)),
        );
    if let Some(empty) = &s.issues.empty
        && !s.issues.issues.is_empty()
    {
        body = body.child(text(empty.clone()).font(type_::BODY).color(p.secondary));
    }
    if list.is_empty() {
        let words = if s.issues.issues.is_empty() { s.issues.empty.clone().unwrap_or_else(|| "No open issues.".into()) } else { "No issue matches that.".into() };
        return Some(body.child(text(words).font(type_::BODY).color(p.secondary).key("issue-empty")));
    }
    let active = s.issues.at.min(list.len() - 1);
    for (index, issue) in list.iter().enumerate() {
        let meta: Vec<String> = issue.author.iter().cloned().chain(std::iter::once(github_issues::issue_age(&issue.created_at, now))).chain(issue.labels.iter().cloned()).filter(|p| !p.is_empty()).collect();
        let number = issue.number;
        let mut line = col()
            .w_full()
            .gap(2.0)
            .px(8.0)
            .py(5.0)
            .radius(6.0)
            .key(format!("issue-{number}"))
            .role("button")
            .child(
                row()
                    .gap(6.0)
                    .child(text(format!("#{number}")).font(type_::LABEL.mono()).color(p.muted))
                    .child(text(issue.title.clone()).font(type_::LABEL).color(p.text).truncate()),
            )
            .on_press(move |s: &mut State| choose(s, number));
        if !meta.is_empty() {
            line = line.child(text(meta.join(" · ")).font(type_::LABEL).color(p.muted));
        }
        if index == active {
            line = line.fill(p.selected);
        }
        body = body.child(line);
    }
    Some(body)
}

#[cfg(test)]
#[path = "next_steps_issue_handoff_tests.rs"]
mod issue_handoff_tests;

#[cfg(test)]
#[path = "issue_picker_target_change_tests.rs"]
mod issue_picker_target_change_tests;
