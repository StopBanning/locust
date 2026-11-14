//! REVIEW CHANGES (Electron `ReviewChanges.tsx`, 0.439/0.440): a teammate's own branch, seen as a whole or turn by
//! turn, against where it left the person's branch, and Land it: the branch onto the person's as one commit of
//! theirs. Every reason it cannot land is read before the button is offered, so the card says the reason in place of
//! the button. A conflict is handed to the teammate as a turn in its own conversation.
//!
//! The git work (`worktrees::Manager`) runs off the window's thread, one request at a time.

use super::*;
use locust_core::worktrees::{BranchReview, LandBlock, LandPreview, LandResult, Manager};
use std::path::PathBuf;

pub(super) struct Review {
    pub teammate_id: String,
    pub name: String,
    pub folder: PathBuf,
    pub review: Option<BranchReview>,
    pub preview: Option<LandPreview>,
    pub problem: Option<String>,
    /// The whole change (None), or one turn by its sha.
    pub showing: Option<String>,
    pub turn_text: Option<(String, String)>,
    /// What the last landing, or the last ask to resolve, did.
    pub note: Option<String>,
    pub editing: bool,
    pub message: TextEdit,
    pub working: bool,
    pub was_running: bool,
}

pub(super) enum Ask {
    Read,
    Turn(String),
    Land(String),
    Resolve,
}

pub(super) enum Answer {
    Read(Result<BranchReview, String>, Result<LandPreview, String>),
    Turn(String, Result<String, String>),
    Land(Result<LandResult, String>),
    Resolve(Result<Vec<String>, String>),
}

/// The teammate on screen, when it works on its own branch (the roster's `worktree`), with its project folder.
fn own_branch_teammate(s: &State) -> Option<(String, String, PathBuf)> {
    // The picked teammate, with or without a conversation yet (Electron's pickedTeammate).
    let teammate = selected_conversation(s).map(|(t, _)| t).or_else(|| s.teammates.get(s.selected))?;
    let folder = teammate.folder.as_ref()?;
    if teammate.runtime.as_deref() == Some("antigravity") {
        return None;
    }
    let store = s.data.as_ref()?;
    let team = locust_core::people::team(store).ok()?;
    let wants = team["teammates"]
        .as_array()?
        .iter()
        .any(|t| t["teammateId"] == teammate.id.as_str() && t["worktree"] == true);
    wants.then(|| (teammate.id.clone(), teammate.name.clone(), PathBuf::from(folder)))
}

/// Whether the teammate on screen has a branch to review: the Changes icon gives way to this one (Electron's header).
pub(super) fn applies(s: &State) -> bool {
    own_branch_teammate(s).is_some()
}

/// The header icon: only a teammate on its own branch has one to review.
pub(super) fn control(s: &State, p: &Palette) -> Option<El> {
    let (id, name, _) = own_branch_teammate(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    let open = s.review.as_ref().is_some_and(|r| r.teammate_id == id);
    let mut icon = crate::look::controls::head_icon(
        &crate::look::icons::DIFF,
        &format!("Review {name}'s changes"),
        &format!("Review changes: what {name} has saved on its own branch, whole or turn by turn"),
    )
    .on_press_tx(|s: &mut State, tx| toggle(s, tx));
    if open {
        icon = icon.state(rune_kit::State::Pressed);
    }
    Some(icon.build(l.k).key("review-changes-open"))
}

fn toggle(s: &mut State, _tx: &mut Tx) {
    let Some((id, name, folder)) = own_branch_teammate(s) else { return };
    if s.review.as_ref().is_some_and(|r| r.teammate_id == id) {
        s.review = None;
        return;
    }
    s.review = Some(Review {
        teammate_id: id,
        name,
        folder,
        review: None,
        preview: None,
        problem: None,
        showing: None,
        turn_text: None,
        note: None,
        editing: false,
        message: TextEdit::default(),
        working: false,
        was_running: turn_running(s),
    });
    ask(s, Ask::Read);
}

fn ask(s: &mut State, what: Ask) {
    let Some(r) = &mut s.review else { return };
    let reading = matches!(what, Ask::Read | Ask::Turn(_));
    // The job keeps only the newest request: a read sent while a landing or a resolve is in flight would take its
    // answer's place, and the window would never say what the press did. That answer reads the branch again itself.
    if reading && r.working {
        return;
    }
    if !reading {
        r.working = true;
    }
    let (folder, id, name) = (r.folder.clone(), r.teammate_id.clone(), r.name.clone());
    s.review_job.request((folder, id, name, what), |(folder, id, name, what)| {
        let manager = Manager::new(&folder);
        match what {
            Ask::Read => Answer::Read(manager.review(&id), manager.land_preview(&id, &name)),
            Ask::Turn(sha) => {
                let diff = manager.turn_diff(&id, &sha);
                Answer::Turn(sha, diff)
            }
            Ask::Land(message) => Answer::Land(manager.land(&id, &name, &message)),
            Ask::Resolve => Answer::Resolve(manager.start_resolving(&id)),
        }
    });
}

/// A run of this teammate's is live: its turn is saved when it ends, so land waits for it.
fn busy(s: &State, teammate_id: &str) -> bool {
    crate::own_copies::busy(s, teammate_id)
}

/// The conversation's mode can edit files: a conflict can only be resolved in one that can.
fn can_edit(s: &State) -> bool {
    let shown = s
        .composer_mode
        .clone()
        .or_else(|| selected_conversation(s).map(|(t, _)| t.runs_in.clone()))
        .unwrap_or_default();
    shown != "ask" && shown != "plan"
}

/// Send a message in the teammate's conversation, as the person would, keeping what they were writing.
fn ask_teammate(s: &mut State, tx: &mut Tx, prompt: String) {
    let kept = s.draft.text.clone();
    s.draft.replace_text(prompt);
    send(s, tx);
    if !kept.is_empty() {
        s.draft.replace_text(kept);
    }
}

pub(super) fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let mut changed = false;
    // The panel belongs to the teammate on screen.
    if let Some(r) = &s.review
        && own_branch_teammate(s).map(|(id, ..)| id) != Some(r.teammate_id.clone())
    {
        s.review = None;
        return true;
    }
    if let Some(answer) = s.review_job.take()
        && let Some(r) = &mut s.review
    {
        changed = true;
        match answer {
            Answer::Read(review, preview) => {
                match review {
                    Ok(review) => r.review = Some(review),
                    Err(why) => r.problem = Some(why),
                }
                match preview {
                    Ok(preview) => {
                        r.message = TextEdit::new(&preview.draft);
                        r.preview = Some(preview);
                    }
                    Err(why) => r.problem = Some(why),
                }
            }
            Answer::Turn(sha, diff) => match diff {
                Ok(diff) => r.turn_text = Some((sha, diff)),
                Err(why) => r.problem = Some(why),
            },
            Answer::Land(result) => {
                r.working = false;
                match result {
                    Ok(LandResult::Landed { sha, onto, files, branch_reset }) => {
                        r.note = Some(landed_sentence(&r.name, &onto, &sha, files.len(), branch_reset));
                        r.editing = false;
                        reread(s);
                    }
                    Ok(LandResult::Blocked(block)) => {
                        if let Some(preview) = &mut r.preview {
                            preview.block = Some(block);
                        }
                        r.editing = false;
                    }
                    Ok(LandResult::Refused(message)) => r.problem = Some(format!("{message} Your checkout is as it was.")),
                    Err(_) => r.problem = Some("The landing could not be made. Your checkout is as it was.".into()),
                }
            }
            Answer::Resolve(files) => {
                r.working = false;
                match files {
                    Ok(files) => {
                        let onto = r.preview.as_ref().and_then(|p| p.onto.clone()).unwrap_or_else(|| "your branch".into());
                        let name = r.name.clone();
                        let files = if files.is_empty() { "the files that conflict".to_owned() } else { files.join(", ") };
                        ask_teammate(s, tx, resolve_prompt(&onto, &files));
                        if let Some(r) = &mut s.review {
                            r.note = Some(format!("Asked {name} to resolve {files}. Land it when that turn ends."));
                        }
                        reread(s);
                    }
                    Err(why) => r.problem = Some(why),
                }
            }
        }
    }
    // A turn that ends is saved on the branch: read it again without a press.
    let running = turn_running(s);
    if let Some(r) = &mut s.review {
        if r.was_running && !running {
            reread(s);
            changed = true;
        }
        if let Some(r) = &mut s.review {
            r.was_running = running;
        }
    }
    changed
}

fn reread(s: &mut State) {
    // A landing or resolve in flight reads the branch again when it answers.
    if s.review.as_ref().is_some_and(|r| r.working) {
        return;
    }
    if let Some(r) = &mut s.review {
        r.showing = None;
        r.turn_text = None;
        r.problem = None;
        r.preview = None;
    }
    ask(s, Ask::Read);
}

pub(super) fn landed_sentence(name: &str, onto: &str, sha: &str, files: usize, branch_reset: bool) -> String {
    format!(
        "Landed on {onto} as {}: {files} {}, one commit of yours.{}",
        &sha[..sha.len().min(7)],
        if files == 1 { "file" } else { "files" },
        if branch_reset { format!(" {name}'s branch now starts from there.") } else { String::new() }
    )
}

pub(super) fn resolve_prompt(onto: &str, files: &str) -> String {
    format!(
        "Merging {onto} into your branch has begun, and {files} conflict. Resolve them: keep what both sides meant, remove every conflict marker, and change nothing else."
    )
}

fn listed(files: &[String]) -> String {
    if files.len() <= 3 { files.join(", ") } else { format!("{} and {} more", files[..2].join(", "), files.len() - 2) }
}

fn file_count(count: usize) -> String {
    format!("{count} {}", if count == 1 { "file" } else { "files" })
}

/// Electron's landBlockSentence: why it cannot land now, said with what would change it. `busy` is Electron's own
/// block for a teammate whose turn is running.
pub(super) fn block_sentence(block: Option<&LandBlock>, busy: bool, name: &str, onto: Option<&str>) -> Option<String> {
    let target = onto.unwrap_or("your branch");
    if busy {
        return Some(format!("{name} is working. Land it when the turn ends."));
    }
    Some(match block? {
        LandBlock::Nothing => format!("Nothing on {name}'s branch is new to {target}."),
        LandBlock::Detached => "Your checkout is not on a branch, so there is nowhere to land it. Check out a branch, then read it again.".into(),
        LandBlock::OldGit(version) => format!(
            "Landing needs git 2.38 or newer, to check for conflicts without touching your files; this machine has {}.",
            version.as_deref().unwrap_or("an older one")
        ),
        LandBlock::Unsaved(files) => format!("{name}'s folder has changes no turn saved yet ({}). Send {name} a message and that turn saves them.", listed(files)),
        LandBlock::Markers(files) => format!("Conflict markers are still in {} on {name}'s branch.", listed(files)),
        LandBlock::MarkersUnchecked => format!("Locust could not check {name}'s branch for conflict markers, so it will not land it yet. Read it again."),
        LandBlock::YourChanges(files) => format!(
            "You have unsaved changes in {}, which this landing would overwrite. Commit them or set them aside, then read it again.",
            listed(files)
        ),
        LandBlock::Conflicts(files) => format!("{} {} changed on {target} too, in the same places.", listed(files), if files.len() == 1 { "was" } else { "were" }),
        LandBlock::Merging => format!("{name}'s merge with {target} is not finished."),
    })
}

fn resolvable(block: Option<&LandBlock>) -> bool {
    matches!(block, Some(LandBlock::Conflicts(_) | LandBlock::Markers(_) | LandBlock::Merging))
}

fn note(words: String, tone: Color, p: &Palette) -> El {
    let _ = p;
    text(words).font(type_::BODY).color(tone).wrap()
}

fn primary(label: String, key: &str, p: &Palette) -> El {
    text(label).font(type_::LABEL).color(p.bg).px(12.0).py(6.0).radius(6.0).fill(p.text).key(key.to_owned()).role("button")
}

fn plain(label: &str, key: &str, p: &Palette) -> El {
    text(label.to_owned()).font(type_::LABEL).color(p.secondary).px(12.0).py(6.0).radius(6.0).border(1.0, p.hairline).key(key.to_owned()).role("button")
}

/// LandCard: the reason it cannot land, or Land on <branch>, or the commit message and Land it.
fn land_card(s: &State, r: &Review, p: &Palette) -> Option<El> {
    let preview = r.preview.as_ref()?;
    let busy = busy(s, &r.teammate_id);
    if !busy && matches!(preview.block, Some(LandBlock::Nothing)) {
        return None;
    }
    let onto = preview.onto.clone().unwrap_or_else(|| "your branch".into());
    let mut card = col().w_full().gap(8.0).padding(12.0).radius(8.0).border(1.0, p.hairline).key("review-land");
    if let Some(sentence) = block_sentence(preview.block.as_ref(), busy, &r.name, preview.onto.as_deref()) {
        card = card.child(note(sentence, if busy { p.muted } else { p.amber }, p).key("review-land-block"));
        if !busy && resolvable(preview.block.as_ref()) {
            if can_edit(s) {
                let word = if matches!(preview.block, Some(LandBlock::Conflicts(_))) {
                    format!("Ask {} to resolve", r.name)
                } else {
                    format!("Ask {} to finish resolving", r.name)
                };
                let mut b = primary(word, "review-resolve", p);
                if !r.working {
                    b = b.on_press(|s: &mut State| ask(s, Ask::Resolve));
                }
                card = card.child(row().child(b));
            } else {
                card = card.child(note(
                    format!("{0} is in a mode that cannot edit files. Switch the mode to Edit, then ask {0} to resolve it.", r.name),
                    p.muted,
                    p,
                ));
            }
        }
        return Some(card);
    }
    let files = preview.files.len();
    if r.editing {
        let empty = r.message.text.trim().is_empty();
        let mut land = primary(if r.working { "Landing…".into() } else { "Land it".into() }, "review-land-it", p);
        if !r.working && !empty {
            land = land.on_press(|s: &mut State| {
                let message = s.review.as_ref().map(|r| r.message.text.clone()).unwrap_or_default();
                ask(s, Ask::Land(message));
            });
        }
        let mut cancel = plain("Cancel", "review-land-cancel", p);
        if !r.working {
            cancel = cancel.on_press(|s: &mut State| {
                if let Some(r) = &mut s.review {
                    r.editing = false;
                }
            });
        }
        card = card
            .child(text(format!("The commit on {onto}")).font(type_::LABEL.mono()).color(p.muted))
            .child(
                text_input(&r.message, type_::BODY)
                    .key("review-land-message")
                    .label(format!("The commit on {onto}"))
                    .w_full()
                    .min_h(88.0)
                    .padding(8.0)
                    .fill(p.bg)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.review.as_mut().expect("review").message),
            )
            .child(note(
                format!("{}, one commit made as you. Your commit hooks run; if one refuses, your checkout is left as it was.", file_count(files)),
                p.muted,
                p,
            ))
            .child(row().gap(8.0).child(land).child(cancel));
    } else {
        card = card.child(
            row()
                .gap(10.0)
                .center_y()
                .child(primary(format!("Land on {onto}"), "review-land-start", p).on_press(|s: &mut State| {
                    if let Some(r) = &mut s.review {
                        r.editing = true;
                    }
                }))
                .child(note(format!("{} as one commit", file_count(files)), p.muted, p)),
        );
    }
    Some(card)
}

/// The diff of one file, as the Changes panel draws its rows (every hunk, every line: a review reads it all),
/// carrying review-note controls when `may_note` holds for the teammate on screen.
fn file_diff(
    s: &State,
    folder: Option<&str>,
    may_note: bool,
    f: &changes::File,
    p: &Palette,
) -> El {
    let (a, d) = f.counts();
    let mut head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text(f.path.clone()).font(rune::TextStyle::new(12.0, 500.0).mono()).color(p.text).shrink().truncate());
    if f.status != "MODIFIED" {
        head = head.child(text(f.status.to_lowercase()).font(type_::LABEL).color(p.muted));
    }
    let mut out = col().w_full().gap(2.0).child(head.child(crate::changes_panel::line_count_marks(a, d, p)));
    let lang = std::path::Path::new(&f.path).extension().and_then(|e| e.to_str());
    for h in &f.hunks {
        out = out.child(text(format!("{} {}", h.range(), h.heading)).font(type_::LABEL).color(p.muted));
        let (before, after) = h.sides();
        let (bh, ah) = (rune_kit::syntax::highlight(lang, &before), rune_kit::syntax::highlight(lang, &after));
        let mut offsets = (0, 0);
        for r in &h.rows {
            let offset = if r.kind == '-' { offsets.0 } else { offsets.1 };
            if r.kind != '+' {
                offsets.0 += r.text.len() + 1;
            }
            if r.kind != '-' {
                offsets.1 += r.text.len() + 1;
            }
            let line = crate::changes_panel::code_row(r, &r.text, offset, if r.kind == '-' { &bh } else { &ah }, p);
            out = crate::diff_notes_ui::append_noted_row(
                out,
                s,
                folder,
                may_note,
                &f.path,
                r,
                line,
                p,
            );
        }
    }
    out
}

pub(super) fn panel(s: &State, p: &Palette) -> Option<El> {
    let r = s.review.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);
    let title = match &r.review {
        None => "Reading the branch…".to_owned(),
        Some(review) => match &review.against {
            Some(against) => format!("{} against {against}", review.branch),
            None => review.branch.clone(),
        },
    };
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(crate::look::controls::glyph(&l, &crate::look::icons::DIFF, 14.0, Some(l.c("text-muted"))))
        .child(
            col()
                .gap(2.0)
                .grow(1.0)
                .shrink()
                .child(text(format!("Review {}'s changes", r.name)).font(type_::LABEL).color(p.text))
                .child(text(title).font(type_::LABEL.mono()).color(p.muted).truncate()),
        )
        .child(
            crate::look::controls::head_icon(&crate::look::icons::REFRESH, "Read it again", "Read the branch again")
                .on_press(|s: &mut State| reread(s))
                .build(l.k)
                .key("review-reread"),
        )
        .child(
            crate::look::controls::head_icon(&crate::look::icons::CLOSE, "Close", "Close")
                .on_press(|s: &mut State| s.review = None)
                .build(l.k)
                .key("review-close"),
        );
    let mut body = col().w_full().gap(10.0);
    if let Some(problem) = &r.problem {
        body = body.child(note(problem.clone(), p.amber, p).key("review-problem"));
    }
    if let Some(review) = &r.review {
        if let Some(done) = &r.note {
            body = body.child(note(done.clone(), p.green, p).key("review-note"));
        }
        let turns = &review.turns;
        body = body.child(note(
            if turns.is_empty() {
                format!("Nothing new on {}. Each turn {} finishes is saved there as it ends.", review.branch, r.name)
            } else {
                format!(
                    "{} {} saved on {} since it left {}. Nothing here is in your branch until you land it.",
                    turns.len(),
                    if turns.len() == 1 { "turn" } else { "turns" },
                    review.branch,
                    review.against.as_deref().unwrap_or("your checkout")
                )
            },
            p.secondary,
            p,
        ).key("review-summary"));
        if !turns.is_empty()
            && let Some(card) = land_card(s, r, p)
        {
            body = body.child(card);
        }
        if !review.uncommitted.is_empty() {
            let n = review.uncommitted.len();
            body = body.child(note(
                format!(
                    "Not saved on the branch yet ({n}): {}{}. A turn may still be running, or these were edited by hand.",
                    review.uncommitted.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
                    if n > 5 { format!(" and {} more", n - 5) } else { String::new() }
                ),
                p.amber,
                p,
            ));
        }
        let text_now = match &r.showing {
            None => review.diff.clone(),
            Some(sha) => r.turn_text.as_ref().filter(|(at, _)| at == sha).map(|(_, d)| d.clone()),
        };
        let files = text_now.as_deref().map(changes::parse).unwrap_or_default();
        if !turns.is_empty() {
            let whole_files = match &review.diff {
                Some(diff) => changes::parse(diff).len(),
                None => turns.iter().flat_map(|t| t.files.iter()).collect::<std::collections::HashSet<_>>().len(),
            };
            let tab = |key: String, label: El, meta: String, active: bool| {
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .px(8.0)
                    .py(5.0)
                    .radius(6.0)
                    .fill(if active { l.c("bg-selected") } else { l.c("bg-raised") })
                    .hover(l.c("hover-surface"))
                    .child(label.grow(1.0).shrink())
                    .child(text(meta).font(type_::LABEL.mono()).color(p.muted))
                    .key(key)
                    .role("tab")
            };
            let mut list = col().w_full().gap(2.0).child(
                tab("review-whole".into(), text("The whole change").font(type_::LABEL).color(p.text), file_count(whole_files), r.showing.is_none())
                    .on_press(|s: &mut State| {
                        if let Some(r) = &mut s.review {
                            r.showing = None;
                        }
                    }),
            );
            for (index, turn) in turns.iter().enumerate() {
                let ago = crate::people_panel::ago_label(&turn.at).map(|a| format!(" · {a}")).unwrap_or_default();
                let meta = if turn.merge {
                    format!("merge with {}{ago}", review.against.as_deref().unwrap_or("your branch"))
                } else {
                    format!("{}{ago}", file_count(turn.files.len()))
                };
                let sha = turn.sha.clone();
                list = list.child(
                    tab(
                        format!("review-turn-{}", index + 1),
                        row()
                            .gap(6.0)
                            .child(text(format!("{}", index + 1)).font(type_::LABEL.mono()).color(p.muted))
                            .child(text(turn.subject.clone()).font(type_::LABEL).color(p.text).truncate()),
                        meta,
                        r.showing.as_deref() == Some(turn.sha.as_str()),
                    )
                    .on_press(move |s: &mut State| {
                        let cached = s.review.as_ref().is_some_and(|r| r.turn_text.as_ref().is_some_and(|(at, _)| at == &sha));
                        if let Some(r) = &mut s.review {
                            r.showing = Some(sha.clone());
                        }
                        if !cached {
                            ask(s, Ask::Turn(sha.clone()));
                        }
                    }),
                );
            }
            body = body.child(list);
            if r.showing.is_none() && review.diff.is_none() {
                body = body.child(note("The whole change is too large to show at once. Pick a turn to see it turn by turn.".into(), p.muted, p));
            }
            if text_now.as_deref().is_some_and(|t| t.trim().is_empty()) {
                body = body.child(note("No difference in text: the change is only to file modes or binary files.".into(), p.muted, p));
            }
        }
        let may_note = crate::diff_notes_ui::may_note(s)
            && selected_conversation(s).is_some_and(|(t, _)| t.id == r.teammate_id);
        for f in &files {
            body = body.child(file_diff(s, r.folder.to_str(), may_note, f, p));
        }
    }
    Some(
        col()
            .key("review-changes")
            .label("Review changes")
            .w(440.0)
            .h_full()
            .padding(14.0)
            .gap(10.0)
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(head)
            .child(body.grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_it_cannot_land_is_said_in_electrons_words() {
        let f = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        let say = |block: LandBlock| block_sentence(Some(&block), false, "Wren", Some("main")).unwrap();
        assert_eq!(say(LandBlock::Nothing), "Nothing on Wren's branch is new to main.");
        assert_eq!(block_sentence(None, true, "Wren", Some("main")).unwrap(), "Wren is working. Land it when the turn ends.");
        assert_eq!(say(LandBlock::Detached), "Your checkout is not on a branch, so there is nowhere to land it. Check out a branch, then read it again.");
        assert_eq!(say(LandBlock::OldGit(Some("2.30.1".into()))), "Landing needs git 2.38 or newer, to check for conflicts without touching your files; this machine has 2.30.1.");
        assert_eq!(say(LandBlock::OldGit(None)), "Landing needs git 2.38 or newer, to check for conflicts without touching your files; this machine has an older one.");
        assert_eq!(say(LandBlock::Unsaved(f(&["a.py", "b.py", "c.py", "d.py"]))), "Wren's folder has changes no turn saved yet (a.py, b.py and 2 more). Send Wren a message and that turn saves them.");
        assert_eq!(say(LandBlock::Markers(f(&["cart.py"]))), "Conflict markers are still in cart.py on Wren's branch.");
        assert_eq!(say(LandBlock::MarkersUnchecked), "Locust could not check Wren's branch for conflict markers, so it will not land it yet. Read it again.");
        assert_eq!(say(LandBlock::YourChanges(f(&["cart.py"]))), "You have unsaved changes in cart.py, which this landing would overwrite. Commit them or set them aside, then read it again.");
        assert_eq!(say(LandBlock::Conflicts(f(&["cart.py", "tax.py"]))), "cart.py, tax.py were changed on main too, in the same places.");
        assert_eq!(block_sentence(Some(&LandBlock::Merging), false, "Wren", None).unwrap(), "Wren's merge with your branch is not finished.");
        assert_eq!(block_sentence(None, false, "Wren", Some("main")), None);
        assert_eq!(landed_sentence("Wren", "main", "8ec6c8c1234", 1, false), "Landed on main as 8ec6c8c: 1 file, one commit of yours.");
    }
}

#[cfg(test)]
mod race_tests {
    use super::*;

    /// The job keeps only the newest request: a turn ending while a landing is in flight must not send a read that
    /// would take the landing's answer's place.
    #[test]
    fn no_read_is_sent_while_a_landing_is_in_flight() {
        let mut s = State::default();
        s.review = Some(Review {
            teammate_id: "tm_codec".into(),
            name: "Codec".into(),
            folder: std::env::temp_dir(),
            review: None,
            preview: Some(LandPreview { branch: "locust/codec".into(), onto: Some("main".into()), files: vec![], draft: String::new(), block: None }),
            problem: None,
            showing: None,
            turn_text: None,
            note: None,
            editing: true,
            message: TextEdit::default(),
            working: true,
            was_running: true,
        });
        reread(&mut s);
        ask(&mut s, Ask::Turn("abc".into()));
        assert!(!s.review_job.is_pending(), "a read was sent while the landing was in flight");
        assert!(s.review.as_ref().unwrap().preview.is_some(), "the card was cleared under the landing");
    }
}
