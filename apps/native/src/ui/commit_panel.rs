//! COMMIT: the folder's changes, committed from the conversation that made them (Electron's
//! renderer/src/components/CommitChanges.tsx). In the conversation's header only while the folder has something
//! uncommitted and nothing is running: the files a commit would take, a message drafted from what was asked, and
//! Commit, or Commit and push, or Open a pull request when the folder can. folder_commit.rs does the git, as the person.
use super::*;
use crate::folder_commit::{self, CommitResult, FolderChanges, FolderCommits, Nothing, Then};
use std::{path::PathBuf, sync::OnceLock};

/// Files listed before "and N more" (SHOWN_FILES).
const SHOWN_FILES: usize = 6;

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub folder: PathBuf,
    /// The last read of the folder, for the folder it was read from.
    pub changes: Option<(PathBuf, FolderChanges)>,
    pub message: TextEdit,
    /// Every file listed, not the first six.
    pub all_shown: bool,
    pub working: Option<Then>,
    pub result: Option<CommitResult>,
    pub was_running: bool,
    /// The teammate is writing the message from the changes.
    pub drafting: bool,
    /// Why the teammate's message is not in the box, when it is not.
    pub draft_said: Option<String>,
}

pub(super) type ReadAnswer = (PathBuf, FolderChanges);
pub(super) type CommitAnswer = (PathBuf, CommitResult);
pub(super) type DraftAnswer = (PathBuf, Result<String, String>);

/// One for the app: it keeps `gh auth status` for five minutes, as Electron's does.
fn commits() -> &'static FolderCommits {
    static ONE: OnceLock<FolderCommits> = OnceLock::new();
    ONE.get_or_init(FolderCommits::default)
}

fn root(s: &State) -> Option<PathBuf> {
    selected_conversation(s).and_then(|(t, _)| t.folder.as_ref()).map(PathBuf::from)
}

/// What the person asked in this conversation, oldest first: the draft is made from it.
/// Electron App.tsx takes the last five prompts and strips attachment preambles (`splitAttachments`).
fn asks(s: &State) -> Vec<String> {
    draft_asks_from(
        s.ledger.as_ref().map(|pane| pane.items.as_slice()).unwrap_or(&[]),
    )
}

/// Pure: last five user asks, attachment preambles stripped — same shape Electron passes CommitChanges.
pub(super) fn draft_asks_from(items: &[ledger::Item]) -> Vec<String> {
    let all: Vec<String> = items
        .iter()
        .filter_map(|item| match item {
            ledger::Item::User(text) => {
                let (body, _) = crate::composer::attachments::split_attachments(text);
                Some(body)
            }
            _ => None,
        })
        .collect();
    let skip = all.len().saturating_sub(5);
    all.into_iter().skip(skip).collect()
}

/// Whether the header Commit control is shown (Electron `shown`).
pub(super) fn control_shown(open: bool, running: bool, changes: Option<&FolderChanges>) -> bool {
    open || (!running && changes.is_some_and(|c| !matches!(c, FolderChanges::None(_))))
}

/// Whether poll should start a fresh folder read (stale folder, or a run just ended).
pub(super) fn should_reread(stale: bool, pending: bool, was_running: bool, running: bool) -> bool {
    (stale && !pending) || (was_running && !running)
}

fn read(s: &mut State, folder: PathBuf) {
    s.commit_read_job.request(folder, |folder| {
        let changes = commits().changes(&folder);
        (folder, changes)
    });
}

/// Read when the conversation is shown and when a run ends: never on a timer.
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    let folder = root(s);
    let running = turn_running(s);
    let stale = s.commit.changes.as_ref().map(|(f, _)| Some(f.clone())) != Some(folder.clone());
    if let Some(folder) = folder.clone()
        && should_reread(stale, s.commit_read_job.is_pending(), s.commit.was_running, running)
    {
        read(s, folder);
    }
    s.commit.was_running = running;
    if let Some((read_from, changes)) = s.commit_read_job.take()
        && folder.as_ref() == Some(&read_from)
    {
        s.commit.changes = Some((read_from, changes));
        changed = true;
    }
    if let Some((drafted_in, answer)) = s.commit_draft_job.take() {
        s.commit.drafting = false;
        if drafted_in == s.commit.folder {
            match answer {
                Ok(message) => s.commit.message = TextEdit::new(&message),
                Err(why) => s.commit.draft_said = Some(why),
            }
        }
        changed = true;
    }
    if let Some((done_in, result)) = s.commit_job.take() {
        s.commit.working = None;
        if done_in == s.commit.folder {
            s.commit.result = Some(result);
            read(s, done_in);
        }
        changed = true;
    }
    if s.commit.open && folder.as_ref() != Some(&s.commit.folder) {
        s.commit.open = false;
        changed = true;
    }
    changed
}

fn label(p: &Palette, word: impl Into<String>) -> El {
    text(word.into()).font(type_::LABEL).color(p.secondary).padding(7.0)
}

/// The header's Commit button: only while there is something to commit and nothing is running.
pub(super) fn control(s: &State, p: &Palette) -> El {
    let count = match s.commit.changes.as_ref().map(|(_, c)| c) {
        Some(FolderChanges::Changes { files, .. }) => files.len(),
        _ => 0,
    };
    let shown = control_shown(
        s.commit.open,
        turn_running(s),
        s.commit.changes.as_ref().map(|(_, c)| c),
    );
    if !shown {
        return row();
    }
    let title = if count > 0 {
        format!("{count} {} changed in this folder and not committed", if count == 1 { "file" } else { "files" })
    } else {
        "Commit this folder's changes".to_owned()
    };
    crate::look::controls::header_button(&crate::look::controls::Look::new(p.dark), &crate::look::icons::COMMIT, "Commit")
        .label(title.clone())
        .on_press(|s: &mut State| {
            if s.commit.open {
                if s.commit.working.is_none() {
                    s.commit.open = false;
                }
                return;
            }
            let Some(folder) = root(s) else { return };
            let teammate = selected_conversation(s).map(|(t, _)| t.name.clone());
            let draft = folder_commit::commit_draft(&asks(s), teammate.as_deref());
            s.commit.open = true;
            s.commit.folder = folder.clone();
            s.commit.result = None;
            s.commit.all_shown = false;
            s.commit.draft_said = None;
            s.commit.message = TextEdit::new(&draft);
            read(s, folder);
        })
}

/// "Write it from the changes": the conversation's teammate writes the message from the diff, read-only.
fn draft(s: &mut State) {
    let Some((teammate, _)) = selected_conversation(s) else { return };
    let teammate = teammate.clone();
    let asks = asks(s);
    let folder = s.commit.folder.clone();
    s.commit.drafting = true;
    s.commit.draft_said = None;
    s.commit_draft_job.request((folder, asks, teammate), |(folder, asks, teammate)| {
        let answer = crate::commit_draft::write(&folder, &asks, &teammate);
        (folder, answer)
    });
}

fn go(s: &mut State, then: Then) {
    let Some((_, FolderChanges::Changes { files, .. })) = s.commit.changes.as_ref() else { return };
    // The files the person was shown: the commit takes those, or is refused if the folder changed since.
    let shown: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let message = s.commit.message.text.clone();
    let folder = s.commit.folder.clone();
    s.commit.working = Some(then);
    s.commit_job.request((folder, message, then, shown), |(folder, message, then, shown)| {
        let result = commits().commit(&folder, &message, then, Some(&shown), None);
        (folder, result)
    });
}

/// The panel under the header (the dialog Electron opens below its Commit button).
pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    if !s.commit.open {
        return None;
    }
    let mut card = col().w_full().gap(8.0).padding(12.0).fill(p.field).radius(8.0).label("Commit");
    let said = |words: &str, tone: Color| text(words.to_owned()).font(type_::BODY).color(tone).w_full().wrap();
    if let Some(result) = &s.commit.result {
        let refused = matches!(result, CommitResult::Refused(_));
        for (i, line) in folder_commit::outcome_words(result).into_iter().enumerate() {
            let failed_after = i > 0 && (line.starts_with("The push") || line.starts_with("GitHub did not"));
            let tone = if refused || failed_after { p.red } else { p.text };
            card = card.child(said(&line, tone));
        }
        let (word, back): (&str, fn(&mut State)) = if refused {
            ("Back", |s: &mut State| s.commit.result = None)
        } else {
            ("Done", |s: &mut State| s.commit.open = false)
        };
        return Some(card.child(row().gap(8.0).child(label(p, word).on_press(back))));
    }
    match s.commit.changes.as_ref().filter(|(f, _)| f == &s.commit.folder).map(|(_, c)| c) {
        None | Some(FolderChanges::None(Nothing::NotARepository)) => {
            card = card.child(said("This folder is not a git repository.", p.secondary));
        }
        Some(FolderChanges::None(Nothing::Clean)) => {
            card = card.child(said("Nothing in this folder is waiting to be committed.", p.secondary));
        }
        Some(FolderChanges::Blocked { why, .. }) => {
            card = card.child(said(folder_commit::blocked_sentence(*why), p.secondary));
        }
        Some(FolderChanges::Changes { branch, files, remote }) => {
            let n = files.len();
            card = card.child(
                row()
                    .gap(8.0)
                    .child(text(format!("Commit {n} {}", if n == 1 { "file" } else { "files" })).font(type_::TITLE).color(p.text))
                    .child(text(format!("on {branch}")).font(type_::LABEL).color(p.secondary)),
            );
            let listed = if s.commit.all_shown { files.len() } else { files.len().min(SHOWN_FILES) };
            for file in &files[..listed] {
                card = card.child(
                    row()
                        .gap(8.0)
                        .child(text(file.status.letter()).font(type_::LABEL).color(p.secondary).label(file.status.word()))
                        .child(text(file.path.clone()).font(type_::BODY).color(p.text)),
                );
            }
            if !s.commit.all_shown && files.len() > SHOWN_FILES {
                card = card.child(
                    label(p, format!("and {} more", files.len() - SHOWN_FILES)).on_press(|s: &mut State| s.commit.all_shown = true),
                );
            }
            let busy = s.commit.working.is_some() || s.commit.drafting;
            let mut label_row = row().gap(8.0).child(text("Message").font(type_::LABEL).color(p.secondary));
            if let Some((teammate, _)) = selected_conversation(s) {
                let word = if s.commit.drafting {
                    format!("{} is writing it…", teammate.name)
                } else {
                    "Write it from the changes".to_owned()
                };
                let link = label(p, word);
                label_row = label_row.child(if busy { link } else { link.on_press(draft) });
            }
            card = card.child(label_row);
            if let Some(why) = &s.commit.draft_said {
                card = card.child(said(why, p.red));
            }
            card = card.child(
                text_input(&s.commit.message, type_::BODY)
                    .key("commit-message")
                    .disabled(busy)
                    .label("Message")
                    .w_full()
                    .min_h(72.0)
                    .padding(8.0)
                    .fill(p.bg)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.commit.message),
            );
            let empty = s.commit.message.text.trim().is_empty();
            let action = |word: &str, working: &str, then: Then, mine: bool| -> El {
                let shown_word = if s.commit.working == Some(then) { working } else { word };
                let mut b = label(p, shown_word);
                if mine {
                    b = b.color(p.text);
                }
                if busy || empty {
                    b
                } else {
                    b.on_press(move |s: &mut State| go(s, then))
                }
            };
            let mut actions = row().gap(8.0).child(action("Commit", "Committing…", Then::Commit, true));
            if remote.is_some() {
                actions = actions.child(action("Commit and push", "Pushing…", Then::Push, false));
            }
            if remote.as_ref().is_some_and(|r| r.pull_requests) {
                actions = actions.child(action("Open a pull request", "Opening…", Then::PullRequest, false));
            }
            card = card.child(actions);
            let mut note = "Made as you, with your own git name and hooks.".to_owned();
            if let Some(r) = remote.as_ref().filter(|r| r.pull_requests && &r.default_branch == branch) {
                note.push_str(&format!(" A pull request goes from a new branch; {} stays where it is.", r.default_branch));
            }
            card = card.child(said(&note, p.secondary));
        }
    }
    Some(card)
}

/// Synthetic: four changed files on main of a GitHub repository with gh signed in, so every button shows.
pub(super) fn scene(s: &mut State) {
    use crate::folder_commit::{ChangeStatus, ChangedFile, Remote};
    team_scene(s);
    let folder = PathBuf::from("/tmp/locust-scene/pebble");
    let file = |path: &str, status| ChangedFile { path: path.into(), status };
    s.commit = Panel {
        open: true,
        folder: folder.clone(),
        changes: Some((
            folder,
            FolderChanges::Changes {
                branch: "main".into(),
                files: vec![
                    file("src/cart.ts", ChangeStatus::Modified),
                    file("src/cart.test.ts", ChangeStatus::Added),
                    file("README.md", ChangeStatus::Modified),
                    file("old/notes.md", ChangeStatus::Deleted),
                ],
                remote: Some(Remote { name: "origin".into(), upstream: true, default_branch: "main".into(), pull_requests: true }),
            },
        )),
        message: TextEdit::new("Fix the cart total\n\n- Fix the cart total\n- and add a test\n\nLocust-Teammate: Codec"),
        ..Panel::default()
    };
}
pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}
/// After a commit and pull request: what the panel says.
pub(super) fn done_scene(s: &mut State) {
    scene(s);
    s.commit.result = Some(CommitResult::Done {
        sha: "4f2c9a1b7e0d3c5a8f6e2d1c0b9a8f7e6d5c4b3a".into(),
        branch: "locust/fix-the-cart-total".into(),
        files: 4,
        left_out: Vec::new(),
        new_branch: Some("locust/fix-the-cart-total".into()),
        pushed: Some(true),
        pull_request_url: Some("https://github.com/someone/pebble/pull/7".into()),
        after: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder_commit::{ChangeStatus, ChangedFile, Nothing};

    #[test]
    fn commit_control_only_while_not_running_and_folder_has_changes() {
        let changes = FolderChanges::Changes {
            branch: "main".into(),
            files: vec![ChangedFile { path: "a".into(), status: ChangeStatus::Modified }],
            remote: None,
        };
        assert!(control_shown(false, false, Some(&changes)));
        assert!(!control_shown(false, true, Some(&changes)));
        assert!(control_shown(true, true, Some(&changes)));
        assert!(!control_shown(false, false, Some(&FolderChanges::None(Nothing::Clean))));
        assert!(!control_shown(false, false, None));
        assert!(control_shown(
            false,
            false,
            Some(&FolderChanges::Blocked { why: folder_commit::Blocked::Merging, files: vec![] }),
        ));
    }

    #[test]
    fn poll_rereads_when_a_run_ends_and_when_the_folder_is_stale() {
        assert!(should_reread(true, false, false, false));
        assert!(!should_reread(true, true, false, false));
        assert!(should_reread(false, false, true, false));
        assert!(!should_reread(false, false, true, true));
        assert!(!should_reread(false, false, false, false));
    }

    #[test]
    fn draft_asks_are_the_last_five_with_attachments_stripped() {
        let preamble = crate::composer::attachments::with_attachments("Fix it", &["NOTES.md".into()]);
        let items: Vec<ledger::Item> = (0..6)
            .map(|i| ledger::Item::User(if i == 5 { preamble.clone() } else { format!("ask {i}") }))
            .collect();
        assert_eq!(
            draft_asks_from(&items),
            vec![
                "ask 1".to_owned(),
                "ask 2".to_owned(),
                "ask 3".to_owned(),
                "ask 4".to_owned(),
                "Fix it".to_owned(),
            ]
        );
    }
}
