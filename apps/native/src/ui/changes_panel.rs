use super::*;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub was_running: bool,
    pub folder: PathBuf,
    pub diff: Option<Result<changes::Folder, String>>,
    pub folded: HashSet<usize>,
    pub expanded: HashSet<usize>,
    /// Open context folds keyed by (file, hunk, first-hidden-row index), matching Electron per-segment folds.
    pub contexts: HashSet<(usize, usize, usize)>,
    /// Markdown files currently shown as line diffs instead of word passages (`FileViewer.tsx::asLines`).
    pub as_lines: HashSet<usize>,
    /// Selected earlier turn index per file path (`FileViewer.tsx::showing`).
    pub file_turn: HashMap<String, usize>,
    /// Per-file toggle to show what that earlier turn changed rather than the rebuilt file (`FileViewer.tsx::asChange`).
    pub file_turn_as_change: HashSet<String>,
    pub files: bool,
    /// Collapsed folder paths in the file tree (`prefix/name/`).
    pub closed_folders: HashSet<String>,
    pub records: Vec<(String, serde_json::Value)>,
    pub asking: Option<String>,
    pub result: Option<String>,
    /// The header's More menu.
    pub menu: bool,
    /// Walk me through it (ChangesPanel.tsx, 0.771): asked, shown, or why not. Ended by closing it, another commit
    /// picked, the change read again, or a turn that may change it.
    pub walk: Option<Walk>,
}

pub(super) enum Walk {
    Asking,
    Shown(locust_core::walkthrough::Walkthrough),
    Failed(String),
}

/// The walk's answer: the folder it was asked in, and the walkthrough or why not.
pub(super) type WalkAnswer = (PathBuf, Result<locust_core::walkthrough::Walkthrough, String>);

/// The conversation on screen's teammate goes through the change shown, read-only, off the window's thread.
fn walk_through(s: &mut State) {
    let Some((teammate, _)) = selected_conversation(s) else { return };
    let teammate = teammate.clone();
    let items = s.ledger.as_ref().map(|pane| pane.items.as_slice()).unwrap_or(&[]);
    let asks = crate::commit_panel::draft_asks_from(items);
    // What it said when it finished: the conversation's latest final answer.
    let said = items.iter().rev().find_map(|item| match item {
        ledger::Item::Assistant { text, r#final: true, .. } => Some(text.clone()),
        _ => None,
    });
    let folder = s.changes.folder.clone();
    let sha = match &s.changes.diff {
        Some(Ok(diff)) if diff.showing != "all" => Some(diff.showing.clone()),
        _ => None,
    };
    s.changes.walk = Some(Walk::Asking);
    s.walk_job.request((folder, sha, asks, said, teammate), |(folder, sha, asks, said, teammate)| {
        let answer = locust_core::walkthrough::walk(&folder, sha.as_deref(), &asks, said.as_deref(), &teammate);
        (folder, answer)
    });
}
fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}
fn root(s: &State) -> Option<PathBuf> {
    selected_conversation(s)
        .and_then(|(t, _)| t.folder.as_ref())
        .map(PathBuf::from)
}
pub(super) fn controls(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut out = row().gap(2.0);
    if let Some(folder) = root(s) {
        // A teammate on its own branch has Review changes instead (Electron's header).
        if !crate::review_changes::applies(s)
            && s.changes_repo
                .as_ref()
                .is_some_and(|(p, yes)| p == &folder && *yes)
        {
            out = out.child(
                crate::look::controls::head_icon(&crate::look::icons::DIFF, "Changes", "Changes in this folder")
                    .on_press(|s: &mut State| open(s))
                    .build(l.k),
            )
        }
        out = out.child(
            crate::look::controls::head_icon(&crate::look::icons::ARROW_LEFT, "Undo", "Undo a turn's changes")
                .on_press(|s: &mut State| open(s))
                .build(l.k),
        );
    }
    out
}
pub(super) fn open(s: &mut State) {
    if let Some(folder) = root(s) {
        s.changes = Panel {
            open: true,
            folder,
            files: true,
            ..Panel::default()
        };
        refresh(s, None)
    }
}
fn refresh(s: &mut State, sha: Option<&str>) {
    let folder = s.changes.folder.clone();
    s.changes.diff = None;
    let sha = sha.map(str::to_owned);
    let store = native::store();
    s.changes_job
        .request((folder, sha, store), |(folder, sha, store)| {
            let diff = changes::read(&folder, sha.as_deref());
            let records = store
                .map(|store| checkpoints::records(&store, &folder).map_err(|e| e.to_string()))
                .unwrap_or_else(|| Ok(vec![]));
            (folder, diff, records)
        });
    s.changes.folded.clear();
    s.changes.expanded.clear();
    s.changes.contexts.clear();
    s.changes.as_lines.clear();
    s.changes.file_turn.clear();
    s.changes.file_turn_as_change.clear();
    s.changes.closed_folders.clear();
    s.changes.asking = None;
    s.changes.walk = None;
}
/// Undo's answer: the folder, the turn, and the receipt or why it could not.
pub(super) type UndoAnswer = (PathBuf, String, Result<serde_json::Value, String>);

/// Put the turn's files back, off the window's thread (checkpoints::undo reads and writes the folder). One at a time.
pub(super) fn undo_turn(s: &mut State, run: String) {
    let folder = s.changes.folder.clone();
    undo_turn_in(s, folder, run);
}

/// Undo in a named folder: the Changes panel's, or the folder of the turn whose foot was pressed (TurnUndo).
pub(super) fn undo_turn_in(s: &mut State, folder: PathBuf, run: String) {
    if s.undo_job.is_pending() {
        return;
    }
    let store = native::store();
    s.undo_job.request((folder, run, store), |(folder, run, store)| {
        let answer = store.ok_or_else(|| "No native store".to_owned()).and_then(|store| checkpoints::undo(&store, &folder, &run).map_err(|e| e.to_string()));
        (folder, run, answer)
    });
}

/// What Undo did, in words: how many files were put back, and each one left alone with why.
fn undone_words(receipt: &serde_json::Value) -> String {
    let n = receipt["restored"].as_array().map_or(0, Vec::len);
    let mut said = match n {
        0 => "Nothing was put back.".to_owned(),
        1 => "Undone: 1 file put back as it was before this turn.".to_owned(),
        _ => format!("Undone: {n} files put back as they were before this turn."),
    };
    for f in receipt["leftAlone"].as_array().into_iter().flatten() {
        said.push_str(&format!("\n{} left as it is: {}", f["path"].as_str().unwrap_or(""), f["why"].as_str().unwrap_or("")));
    }
    said
}

pub(super) type ReadResult = (
    PathBuf,
    Result<changes::Folder, String>,
    Result<Vec<(String, serde_json::Value)>, String>,
);
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    let folder = root(s);
    if folder != s.changes_repo_root {
        s.changes_repo_root = folder.clone();
        s.changes_repo = None;
        s.changes_repo_job.cancel();
        if let Some(ref folder) = folder {
            s.changes_repo_job.request(folder.clone(), |folder| {
                let yes = changes::repository(&folder);
                (folder, yes)
            });
        }
    }
    if let Some(answer) = s.changes_repo_job.take() {
        s.changes_repo = Some(answer.clone());
        // Electron hides Changes when the bound folder is not a repository.
        if s.changes.open && answer.0 == s.changes.folder && !answer.1 {
            s.changes.open = false;
        }
        changed = true;
    }
    if s.changes.open && folder.as_ref() != Some(&s.changes.folder) {
        s.changes.open = false;
        changed = true;
    }
    if let Some((folder, diff, records)) = s.changes_job.take()
        && s.changes.folder == folder
    {
        s.changes.diff = Some(diff);
        match records {
            Ok(r) => s.changes.records = r,
            Err(e) => s.changes.result = Some(e),
        }
        changed = true;
    }
    if let Some((folder, run, answer)) = s.undo_job.take() {
        if let Ok(receipt) = &answer {
            crate::turn_undo::heard(s, &run, receipt);
        }
        s.changes.result = Some(match answer {
            Ok(receipt) => undone_words(&receipt),
            Err(e) => e,
        });
        s.changes.asking = None;
        // Read again, so the turn shows its receipt and the diff the folder now has.
        if s.changes.open && s.changes.folder == folder {
            let said = s.changes.result.clone();
            refresh(s, None);
            s.changes.result = said;
        }
        changed = true;
    }
    if let Some((folder, answer)) = s.walk_job.take() {
        // A walk ended meanwhile (closed, another commit, read again) is not brought back.
        if s.changes.folder == folder && matches!(s.changes.walk, Some(Walk::Asking)) {
            s.changes.walk = Some(match answer {
                Ok(walk) => Walk::Shown(walk),
                Err(why) => Walk::Failed(why),
            });
        }
        changed = true;
    }
    let running = turn_running(s);
    if running && !s.changes.was_running && !matches!(s.changes.walk, Some(Walk::Asking)) {
        s.changes.walk = None;
    }
    if s.changes.open && s.changes.was_running && !running {
        refresh(s, None);
        changed = true;
    }
    s.changes.was_running = running;
    changed
}

pub(super) fn line_count_marks(added: usize, removed: usize, p: &Palette) -> El {
    if added > 0 && removed == 0 {
        return text(changes::line_counts(added, 0)).color(p.green);
    }
    if removed > 0 && added == 0 {
        return text(changes::line_counts(0, removed)).color(p.red);
    }
    if added > 0 && removed > 0 {
        // Same zero-free words as `line_counts`, split so + is green and − is red.
        return row()
            .gap(0.0)
            .child(text(format!("+{added}")).color(p.green))
            .child(text(" "))
            .child(text(format!("−{removed}")).color(p.red));
    }
    row()
}
pub(super) fn code_row(
    r: &changes::Row,
    line: &str,
    offset: usize,
    highlights: &[rune_kit::syntax::Highlight],
    marks: Option<&[changes::Span]>,
    p: &Palette,
) -> El {
    use rune_kit::syntax::{Paint, Syntax};
    // diff.ts `pairedSpans`: the bytes of `line` that changed from the line it replaced, drawn on Electron's word
    // background (`--lc-diff-add-word`, `--lc-diff-del-word`).
    let marked = marks.map_or((0, 0), |spans| {
        let mut at = 0;
        let mut range = (0, 0);
        for span in spans {
            if span.changed {
                range = (at, at + span.text.len())
            }
            at += span.text.len();
        }
        range
    });
    let word = match (r.kind, p.dark) {
        ('+', true) => color("#1f4029"),
        ('+', false) => color("#c5e5cb"),
        (_, true) => color("#4a2429"),
        (_, false) => color("#f2c9c4"),
    };
    let piece = |spans: El, a: usize, b: usize, c: Color| -> El {
        let mut spans = spans;
        let cuts = [a, marked.0.clamp(a, b), marked.1.clamp(a, b), b];
        for w in 0..3 {
            let (x, y) = (cuts[w], cuts[w + 1]);
            if x >= y || !line.is_char_boundary(x) || !line.is_char_boundary(y) {
                continue;
            }
            let t = text(line[x..y].to_owned()).color(c);
            spans = spans.child(if w == 1 { t.fill(word).radius(2.0) } else { t });
        }
        spans
    };
    let row_fill = if r.kind == '+' {
        p.green
    } else if r.kind == '-' {
        p.red
    } else {
        p.secondary
    };
    let sign = match r.kind {
        '+' => "+",
        '-' => "−",
        _ => "",
    };
    let sign_tone = match r.kind {
        '+' => p.green,
        '-' => p.red,
        _ => p.secondary,
    };
    let mut spans = row().gap(0.0);
    let mut cursor = 0;
    for (range, paint) in highlights {
        let a = range
            .start
            .max(offset)
            .saturating_sub(offset)
            .min(line.len());
        let b = range.end.saturating_sub(offset).min(line.len());
        if a >= b || a < cursor || !line.is_char_boundary(a) || !line.is_char_boundary(b) {
            continue;
        }
        if cursor < a {
            spans = piece(spans, cursor, a, p.text)
        }
        let c = match paint {
            Paint::Color(c) => *c,
            Paint::Kind(Syntax::Comment) => p.muted,
            Paint::Kind(Syntax::String) => p.green,
            Paint::Kind(Syntax::Number) => p.amber,
            Paint::Kind(Syntax::Keyword | Syntax::Type | Syntax::Function) => {
                if p.dark {
                    color("#b6a1e8")
                } else {
                    color("#6c42a8")
                }
            }
            _ => p.text,
        };
        spans = piece(spans, a, b, c);
        cursor = b;
    }
    if cursor < line.len() {
        spans = piece(spans, cursor, line.len(), p.text)
    }
    row()
        .w_full()
        .gap(8.0)
        .px(6.0)
        .py(2.0)
        .fill(if r.kind == ' ' {
            p.bg
        } else {
            row_fill.alpha(if p.dark { 0.12 } else { 0.09 })
        })
        .child(
            text(format!(
                "{:>4} {:>4}",
                r.old.map(|n| n.to_string()).unwrap_or_default(),
                r.new.map(|n| n.to_string()).unwrap_or_default(),
            ))
            .color(p.muted),
        )
        .child(text(sign.to_owned()).color(sign_tone))
        .child(spans)
}
/// ChangesPanel.tsx's head (`.lc-viewer__head`): the files toggle, the branch and the change's line counts, then More
/// (show or hide files, collapse, expand, refresh) and Close, as icon buttons with their names.
fn head(s: &State, p: &Palette) -> El {
    let panel = &s.changes;
    let l = crate::look::controls::Look::new(p.dark);
    let shortcut = if cfg!(target_os = "macos") { "Cmd+Shift+Y" } else { "Ctrl+Shift+Y" };
    let files = crate::look::controls::head_icon(
        &crate::look::icons::COLUMNS,
        "Show files",
        &format!("Show the files ({shortcut})"),
    )
    .on_press(|s: &mut State| s.changes.files = !s.changes.files)
    .build(l.k)
    .checked(panel.files)
    .key("changes-files");
    let files = if panel.files { files.fill(l.c("bg-selected")).radius(7.0) } else { files };
    let branch = match &panel.diff {
        Some(Ok(diff)) => format!("{} → {}", diff.base, diff.branch),
        _ => "Changes".into(),
    };
    let mut row_ = row()
        .w_full()
        .center_y()
        .gap(10.0)
        .pb(6.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-hairline"))
        .child(files)
        .child(text(branch).font(rune::TextStyle::new(12.0, 400.0).mono()).color(l.c("text-secondary")).shrink().truncate());
    if let Some(Ok(diff)) = &panel.diff
        && !diff.files.is_empty()
    {
        let (a, d) = diff.files.iter().map(|f| f.counts()).fold((0, 0), |(x, y), (a, d)| (x + a, y + d));
        row_ = row_.child(line_count_marks(a, d, p));
    }
    // Walk me through it: for the conversation on screen, when there is a change, and no walk shown or under way.
    if let Some((teammate, _)) = selected_conversation(s)
        && matches!(&panel.diff, Some(Ok(diff)) if !diff.files.is_empty())
        && matches!(panel.walk, None | Some(Walk::Failed(_)))
    {
        let tip = format!("{} goes through the change as its author would in a review", teammate.name);
        row_ = row_.child(rune::spacer()).child(
            row()
                .gap(5.0)
                .center_y()
                .py(4.0)
                .px(8.0)
                .radius(7.0)
                .hover(l.c("bg-selected"))
                .child(crate::look::controls::glyph(&l, &crate::look::icons::MESSAGE, 12.0, Some(l.c("text-secondary"))))
                .child(text("Walk me through it").font(l.ty.control).color(l.c("text-secondary")))
                .key("changes-walk")
                .role("button")
                .label("Walk me through it")
                .description(tip)
                .on_press(walk_through),
        );
    } else {
        row_ = row_.child(rune::spacer());
    }
    row_
        .child(
            crate::look::controls::head_icon(&crate::look::icons::DOTS, "More", "More")
                .on_press(|s: &mut State| s.changes.menu = !s.changes.menu)
                .build(l.k)
                .key("changes-more"),
        )
        .child(
            crate::look::controls::head_icon(&crate::look::icons::CLOSE, "Close", "Close")
                .on_press(|s: &mut State| {
                    s.changes.open = false;
                    s.changes.menu = false;
                })
                .build(l.k)
                .key("changes-close"),
        )
}

/// The More menu, in Electron's words and order.
fn more_menu(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let item = |words: &'static str, run: fn(&mut State)| {
        row()
            .w_full()
            .py(7.0)
            .px(10.0)
            .radius(7.0)
            .hover(l.c("bg-selected"))
            .child(text(words).font(l.ty.control).color(l.c("text-primary")))
            .role("menuitem")
            .label(words)
            .on_press(move |s: &mut State| {
                run(s);
                s.changes.menu = false;
            })
    };
    let files_words = if s.changes.files { "Hide files" } else { "Show files" };
    row().w_full().justify_end().child(
        col()
            .key("changes-menu")
            .w(200.0)
            .padding(4.0)
            .radius(11.0)
            .fill(l.c("bg-raised-2"))
            .border(1.0, l.c("border-strong"))
            .box_shadow(l.t.shadows("shadow-menu").first().copied().unwrap_or_default())
            .role("menu")
            .child(item(files_words, |s| s.changes.files = !s.changes.files))
            .child(item("Collapse all files", |s| {
                if let Some(Ok(d)) = &s.changes.diff {
                    s.changes.folded = (0..d.files.len()).collect()
                }
            }))
            .child(item("Expand all files", |s| s.changes.folded.clear()))
            .child(item("Refresh", |s| refresh(s, None))),
    )
}

/// A Markdown document's change as passages: old words struck (del mark) and new words marked (add mark),
/// matching Electron's `DocumentChange.tsx`.
fn document_change_view(file_index: usize, f: &changes::File, p: &Palette) -> El {
    let passages = locust_core::document_change::document_changes(f);
    if passages.is_empty() {
        return text("No words changed: only spacing or layout did.")
            .color(p.muted)
            .wrap()
            .key(format!("changes-doc-{file_index}"));
    }
    let del_bg = if p.dark { color("#4a2429") } else { color("#f2c9c4") };
    let add_bg = if p.dark { color("#1f4029") } else { color("#c5e5cb") };
    let mut list = col()
        .w_full()
        .gap(6.0)
        .px(8.0)
        .py(4.0)
        .key(format!("changes-doc-{file_index}"))
        .label("What changed, in the document's words");
    for (idx, passage) in passages.into_iter().enumerate() {
        let (what, mut words) = match passage {
            locust_core::document_change::ChangedPassage::Changed {
                before,
                removed,
                added,
                after,
            } => {
                let mut w = row().gap(0.0).shrink();
                if !before.is_empty() {
                    w = w.child(text(before).color(p.text));
                }
                if !removed.is_empty() {
                    w = w.child(
                        text(removed)
                            .color(p.red)
                            .fill(del_bg)
                            .radius(2.0)
                            .px(2.0)
                            .key(format!("changes-doc-{file_index}-del-{idx}")),
                    );
                }
                if !added.is_empty() {
                    w = w.child(
                        text(added)
                            .color(p.green)
                            .fill(add_bg)
                            .radius(2.0)
                            .px(2.0)
                            .key(format!("changes-doc-{file_index}-add-{idx}")),
                    );
                }
                if !after.is_empty() {
                    w = w.child(text(after).color(p.text));
                }
                ("Changed", w)
            }
            locust_core::document_change::ChangedPassage::Added { text: body } => {
                let w = row().gap(0.0).shrink().child(
                    text(body)
                        .color(p.green)
                        .fill(add_bg)
                        .radius(2.0)
                        .px(2.0)
                        .key(format!("changes-doc-{file_index}-add-{idx}")),
                );
                ("Added", w)
            }
            locust_core::document_change::ChangedPassage::Removed { text: body } => {
                let w = row().gap(0.0).shrink().child(
                    text(body)
                        .color(p.red)
                        .fill(del_bg)
                        .radius(2.0)
                        .px(2.0)
                        .key(format!("changes-doc-{file_index}-del-{idx}")),
                );
                ("Removed", w)
            }
        };
        words = words.center_y();
        list = list.child(
            row()
                .w_full()
                .gap(8.0)
                .py(2.0)
                .center_y()
                .child(
                    text(what)
                        .font(rune::TextStyle::new(11.0, 500.0).mono())
                        .color(p.muted),
                )
                .child(words),
        );
    }
    list
}

/// The version strip for a file changed across turns (`FileViewer.tsx`: `CHANGED IN N TURNS`, `1..N`, `Now`).
fn version_strip(
    file_index: usize,
    file_path: &str,
    turns: &[locust_core::file_versions::FileTurn],
    showing: Option<usize>,
    p: &Palette,
) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let n = turns.len();
    let label = format!("CHANGED IN {n} {}", if n == 1 { "TURN" } else { "TURNS" });
    let mut strip = row()
        .w_full()
        .gap(4.0)
        .px(8.0)
        .py(4.0)
        .center_y()
        .key(format!("changes-versions-{file_index}"))
        .child(
            text(label)
                .font(rune::TextStyle::new(11.0, 500.0).mono())
                .color(p.muted),
        )
        .child(rune::spacer());
    for (t_idx, turn) in turns.iter().enumerate() {
        let active = showing == Some(t_idx);
        let path_for_click = file_path.to_owned();
        let chip = button(format!("{}", t_idx + 1), p)
            .key(format!("changes-version-{file_index}-{t_idx}"))
            .role("button")
            .label(format!("Turn {}", t_idx + 1))
            .description(turn.prompt.clone())
            .on_press(move |s: &mut State| {
                s.changes.file_turn_as_change.remove(&path_for_click);
                if s.changes.file_turn.get(&path_for_click) == Some(&t_idx) {
                    s.changes.file_turn.remove(&path_for_click);
                } else {
                    s.changes.file_turn.insert(path_for_click.clone(), t_idx);
                }
            });
        strip = strip.child(if active {
            chip.fill(l.c("bg-selected")).radius(6.0)
        } else {
            chip
        });
    }
    let now_active = showing.is_none();
    let path_for_now = file_path.to_owned();
    let now_chip = button("Now", p)
        .key(format!("changes-version-{file_index}-now"))
        .role("button")
        .label("Now")
        .description("The file as it is on disk now")
        .on_press(move |s: &mut State| {
            s.changes.file_turn.remove(&path_for_now);
            s.changes.file_turn_as_change.remove(&path_for_now);
        });
    strip.child(if now_active {
        now_chip.fill(l.c("bg-selected")).radius(6.0)
    } else {
        now_chip
    })
}

/// Renders the Changes panel's file tree grouped by folder, folders first (`ChangesPanel.tsx::FileTree`).
fn render_file_tree(
    folder: &changes::TreeFolder,
    files: &[changes::File],
    closed: &HashSet<String>,
    prefix: &str,
    depth: usize,
    p: &Palette,
    out: &mut Vec<El>,
) {
    let l = crate::look::controls::Look::new(p.dark);
    for (name, child) in &folder.folders {
        let key = format!("{prefix}{name}/");
        let open = !closed.contains(&key);
        let chevron = if open {
            &crate::look::icons::CHEVRON_DOWN
        } else {
            &crate::look::icons::CHEVRON_RIGHT
        };
        let toggle_key = key.clone();
        out.push(
            row()
                .pl(8.0 + depth as f32 * 12.0)
                .gap(6.0)
                .py(3.0)
                .center_y()
                .child(crate::look::controls::glyph(&l, chevron, 11.0, Some(l.c("text-muted"))))
                .child(text(name.clone()).font(type_::LABEL).color(p.secondary))
                .role("button")
                .label(format!("Folder {name}"))
                .on_press(move |s: &mut State| {
                    if !s.changes.closed_folders.remove(&toggle_key) {
                        s.changes.closed_folders.insert(toggle_key.clone());
                    }
                }),
        );
        if open {
            render_file_tree(child, files, closed, &key, depth + 1, p, out);
        }
    }
    for &i in &folder.files {
        let f = &files[i];
        let (a, d) = f.counts();
        let badge = kind(&f.path);
        let display_name = f.path.rsplit('/').next().unwrap_or(&f.path);
        out.push(
            row()
                .pl(8.0 + depth as f32 * 12.0)
                .gap(8.0)
                .center_y()
                .child(button(format!("{badge}  {display_name}"), p).on_press(
                    move |s: &mut State| {
                        s.changes.folded.remove(&i);
                    },
                ))
                .child(line_count_marks(a, d, p)),
        );
    }
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let panel = &s.changes;
    let mut head = col().w_full().gap(6.0).child(self::head(s, p));
    if panel.menu {
        head = head.child(more_menu(s, p));
    }
    let mut body = col().w_full().gap(8.0);
    if let Some(result) = &panel.result {
        body = body.child(text(result.clone()).color(p.amber).wrap())
    }
    for (id, record) in &panel.records {
        if record["shared"] == true {
            continue;
        }
        if let Some(receipt) = record.get("undone") {
            let n = receipt["restored"].as_array().map_or(0, Vec::len);
            body = body.child(
                text(if n == 0 {
                    "Nothing was put back.".into()
                } else if n == 1 {
                    "Undone: 1 file put back as it was before this turn.".into()
                } else {
                    format!("Undone: {n} files put back as they were before this turn.")
                })
                .color(p.muted)
                .wrap(),
            );
            continue;
        }
        let count = checkpoints::changed(record).len();
        if count == 0 {
            continue;
        }
        let run = id.clone();
        let mut undo = col()
            .w_full()
            .gap(6.0)
            .padding(8.0)
            .fill(p.raised)
            .child(text(format!("Turn {id}")).color(p.muted).wrap());
        if panel.asking.as_deref() == Some(id) {
            let not = record["notKept"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            undo=undo.child(text(format!("Put back the {} this turn changed, as they were before it? A file changed since is left as it is.{}",if count==1{"1 file".into()}else{format!("{count} files")},if not.is_empty(){String::new()}else{format!(" Too large to have been kept: {not}.")})).color(p.text).wrap())
            .child(row().w_full().gap(6.0).child(button(if s.undo_job.is_pending() { "Undoing\u{2026}" } else { "Undo" },p).key(format!("changes-undo-{id}")).on_press(move |s:&mut State| undo_turn(s, run.clone()))            ).child(button("Keep them",p).key("changes-undo-keep").on_press(|s:&mut State|s.changes.asking=None)));
        } else {
            undo = undo.child(
                button("Undo these changes", p)
                    .key(format!("changes-undo-ask-{id}"))
                    .on_press(move |s: &mut State| s.changes.asking = Some(run.clone())),
            )
        }
        body = body.child(undo);
    }
    match &panel.diff {
        Some(Err(e)) => body = body.child(text(e.clone()).color(p.muted).wrap()),
        Some(Ok(diff)) => {
            if diff.files.is_empty() {
                body = body.child(
                    text(changes::empty_diff_words(diff.commits.len()).to_owned())
                        .color(p.muted)
                        .wrap(),
                )
            }
            if !diff.commits.is_empty() {
                body =
                    body.child(button("All changes", p).on_press(|s: &mut State| refresh(s, None)));
                for (sha, subject) in &diff.commits {
                    let sha = sha.clone();
                    body = body.child(
                        button(subject.clone(), p)
                            .on_press(move |s: &mut State| refresh(s, Some(&sha))),
                    )
                }
            }
            let teammate = selected_conversation(s).map_or_else(|| "Your teammate".to_owned(), |(t, _)| t.name.clone());
            match &panel.walk {
                None => {}
                Some(Walk::Asking) => {
                    body = body.child(text(format!("{teammate} is walking through the change\u{2026}")).color(p.muted).wrap().key("changes-walkthrough").role("status"));
                }
                Some(Walk::Failed(why)) => {
                    body = body.child(text(why.clone()).color(p.muted).wrap().key("changes-walkthrough").role("status"));
                }
                Some(Walk::Shown(walk)) => {
                    let l = crate::look::controls::Look::new(p.dark);
                    body = body.child(
                        col()
                            .w_full()
                            .gap(6.0)
                            .padding(10.0)
                            .radius(8.0)
                            .fill(l.c("hover-surface"))
                            .key("changes-walkthrough")
                            .role("status")
                            .label("Walkthrough")
                            .child(
                                row()
                                    .w_full()
                                    .center_y()
                                    .child(text(format!("{teammate}\u{2019}s walkthrough")).font(l.ty.control).color(l.c("text-secondary")))
                                    .child(rune::spacer())
                                    .child(
                                        crate::look::controls::head_icon(&crate::look::icons::CLOSE, "Close the walkthrough", "Close the walkthrough")
                                            .control_size(rune_kit::ControlSize::Small)
                                            .on_press(|s: &mut State| s.changes.walk = None)
                                            .build(l.k)
                                            .key("changes-walk-close"),
                                    ),
                            )
                            .child(text(walk.summary.clone()).color(l.c("text-primary")).wrap()),
                    );
                }
            }
            if panel.files {
                let tree = changes::tree_of(&diff.files);
                let mut rows = Vec::new();
                render_file_tree(&tree, &diff.files, &panel.closed_folders, "", 0, p, &mut rows);
                for item in rows {
                    body = body.child(item);
                }
            }
            let shown = match &panel.walk {
                Some(Walk::Shown(walk)) => Some(walk),
                _ => None,
            };
            let paths: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
            for i in locust_core::walkthrough::reading_order(&paths, shown) {
                let f = &diff.files[i];
                let (a, d) = f.counts();
                let open = !panel.folded.contains(&i);
                // The file's head (`.lc-changes__file`): a chevron that folds it, its path in the code face, how it
                // changed, its line counts, and a copy button at the right.
                let l = crate::look::controls::Look::new(p.dark);
                let chevron = if open { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
                let mut file_head = row()
                    .w_full()
                    .gap(8.0)
                    .py(6.0)
                    .px(8.0)
                    .center_y()
                    .radius(7.0)
                    .fill(l.c("hover-surface"))
                    .child(crate::look::controls::glyph(&l, chevron, 12.0, Some(l.c("text-muted"))))
                    .child(text(f.path.clone()).font(rune::TextStyle::new(12.0, 500.0).mono()).color(l.c("text-primary")).shrink().truncate())
                    .child(text(f.status.to_lowercase()).font(l.ty.body).color(l.c("text-muted")))
                    .child(line_count_marks(a, d, p))
                    .child(rune::spacer())
                    .key(format!("changes-file-{i}"))
                    .role("button")
                    .label(format!("{} · {}", f.path, f.status))
                    .on_press(move |s: &mut State| {
                        if !s.changes.folded.remove(&i) {
                            s.changes.folded.insert(i);
                        }
                    });
                if open && f.status != "DELETED" {
                    let folder = panel.folder.clone();
                    let path = f.path.clone();
                    file_head = file_head.child(
                        crate::look::controls::head_icon(&crate::look::icons::COPY, &format!("Copy {}", f.path), "Copy the file")
                            .control_size(rune_kit::ControlSize::Small)
                            .on_press_tx(move |s: &mut State, tx| {
                                let target = reply_actions::FileTarget { path: path.clone(), start: 1, end: 1 };
                                match reply_actions::read_file(&folder, &target) {
                                    Ok(body) => reply_actions::request_copy(tx, body),
                                    Err(e) => s.changes.result = Some(e),
                                }
                            })
                            .build(l.k),
                    );
                }
                let mut file = col().w_full().gap(4.0).child(file_head);
                // Walked through: its place in the reading and its note, with what to check there.
                if let Some(walk) = shown
                    && let Some((order, note)) = locust_core::walkthrough::note_on(walk, &f.path)
                {
                    let l = crate::look::controls::Look::new(p.dark);
                    let mut words = col().gap(3.0).shrink().child(text(note.why.clone()).color(l.c("text-primary")).wrap());
                    if let Some(check) = &note.check {
                        words = words.child(
                            row()
                                .gap(6.0)
                                .child(text("Check").font(l.ty.control).color(l.c("text-secondary")))
                                .child(text(check.clone()).color(l.c("text-secondary")).wrap().shrink()),
                        );
                    }
                    file = file.child(
                        row()
                            .w_full()
                            .gap(8.0)
                            .px(8.0)
                            .key(format!("changes-why-{i}"))
                            .child(
                                text(order.to_string())
                                    .font(rune::TextStyle::new(11.0, 500.0).mono())
                                    .color(l.c("text-muted"))
                                    .label(format!("Read it {order} of {}", walk.files.len())),
                            )
                            .child(words),
                    );
                }
                if open {
                    let items = s.ledger.as_ref().map(|pane| pane.items.as_slice()).unwrap_or(&[]);
                    let workspace = panel.folder.to_str().filter(|path| !path.is_empty());
                    let turns = locust_core::file_versions::file_turns_from_items(items, &f.path, workspace);
                    let showing_turn = panel
                        .file_turn
                        .get(&f.path)
                        .copied()
                        .filter(|&idx| idx < turns.len());
                    if !turns.is_empty() {
                        file = file.child(version_strip(i, &f.path, &turns, showing_turn, p));
                    }
                    let is_doc = locust_core::text_files::viewer_mode(&f.path)
                        == locust_core::text_files::ViewerMode::Markdown;
                    if let Some(t_idx) = showing_turn {
                        let version = &turns[t_idx];
                        let turn_name = format!("turn {}", t_idx + 1);
                        let current_text = (!panel.folder.as_os_str().is_empty())
                            .then(|| std::fs::read_to_string(panel.folder.join(&f.path)).ok())
                            .flatten();
                        let rebuilt = current_text
                            .as_deref()
                            .map(|now| locust_core::file_versions::file_as_it_was(now, &turns, t_idx));
                        let as_change = panel.file_turn_as_change.contains(&f.path);
                        match rebuilt {
                            Some(locust_core::file_versions::Reversed::Ok(ref body)) if !as_change => {
                                let note = match body {
                                    None => format!("The file was not there after {turn_name}."),
                                    Some(_) => format!("The file as it was after {turn_name}."),
                                };
                                let path_toggle = f.path.clone();
                                file = file.child(
                                    row()
                                        .w_full()
                                        .gap(8.0)
                                        .px(8.0)
                                        .center_y()
                                        .child(
                                            text(note)
                                                .color(p.muted)
                                                .key(format!("changes-version-note-{i}")),
                                        )
                                        .child(
                                            button(format!("What {turn_name} changed"), p)
                                                .key(format!("changes-version-as-change-{i}"))
                                                .on_press(move |s: &mut State| {
                                                    s.changes.file_turn_as_change.insert(path_toggle.clone());
                                                }),
                                        ),
                                );
                                if let Some(body) = body {
                                    file = file.child(
                                        text(body.clone())
                                            .color(p.text)
                                            .wrap()
                                            .px(8.0)
                                            .py(4.0)
                                            .key(format!("changes-version-body-{i}")),
                                    );
                                }
                            }
                            other => {
                                let mut note_row = row()
                                    .w_full()
                                    .gap(8.0)
                                    .px(8.0)
                                    .center_y()
                                    .child(text(format!("What {turn_name} changed.")).color(p.muted));
                                match other {
                                    Some(locust_core::file_versions::Reversed::Ok(_)) => {
                                        let path_toggle = f.path.clone();
                                        note_row = note_row.child(
                                            button(format!("The file as it was after {turn_name}"), p)
                                                .key(format!("changes-version-as-was-{i}"))
                                                .on_press(move |s: &mut State| {
                                                    s.changes.file_turn_as_change.remove(&path_toggle);
                                                }),
                                        );
                                    }
                                    Some(locust_core::file_versions::Reversed::Err(why)) => {
                                        note_row = note_row.child(
                                            text(format!(
                                                "It cannot be shown as it was, because {why}. The file itself is under Now."
                                            ))
                                            .color(p.muted)
                                            .wrap()
                                            .key(format!("changes-version-refused-{i}")),
                                        );
                                    }
                                    None => {
                                        note_row = note_row.child(
                                            text("The file itself is under Now.").color(p.muted),
                                        );
                                    }
                                }
                                file = file.child(note_row);
                                if is_doc && !panel.as_lines.contains(&i) {
                                    file = file.child(document_change_view(i, &version.file, p));
                                } else {
                                    for h in &version.file.hunks {
                                        file = file.child(
                                            text(format!("{} {}", h.range(), h.heading)).color(p.muted),
                                        );
                                        for r in &h.rows {
                                            file = file.child(code_row(r, &r.text, 0, &[], None, p));
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        let show_words = is_doc && !panel.as_lines.contains(&i);
                        if show_words {
                            file = file.child(document_change_view(i, f, p));
                            file = file.child(
                                button("Show the lines that changed", p)
                                    .key(format!("changes-doc-lines-{i}"))
                                    .on_press(move |s: &mut State| {
                                        s.changes.as_lines.insert(i);
                                    }),
                            );
                        } else {
                            if is_doc {
                                file = file.child(
                                    button("Show it as words", p)
                                        .key(format!("changes-doc-words-{i}"))
                                        .on_press(move |s: &mut State| {
                                            s.changes.as_lines.remove(&i);
                                        }),
                                );
                            }
                            let copy = f.after().map(|after| reply_actions::copy_button(s.copy_feedback.as_ref(), after, p));
                            file = file.child(crate::diff_view::diff_view(
                                s,
                                crate::diff_view::Place::Panel(i),
                                f,
                                diff.truncated,
                                None,
                                (s.changes.folder.to_str(), crate::diff_notes_ui::may_note(s)),
                                copy,
                                p,
                            ));
                        }
                    }
                }
                body = body.child(file);
            }
            if diff.truncated {
                body = body.child(
                    text("The rest is too large to show here. Your own git has all of it.")
                        .color(p.amber)
                        .wrap(),
                );
            }
        }
        None => body = body.child(text("Reading the folder’s changes…").color(p.muted)),
    }
    col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .padding(16.0)
        .gap(12.0)
        .child(head)
        .child(body.grow(1.0).min_h(0.0).scroll(ScrollSpec {
            stick_bottom: false,
        }))
}
fn kind(path: &str) -> &'static str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return "▫";
    };
    if stem.is_empty() {
        return "▫";
    }
    match ext.to_lowercase().as_str() {
        "js" => "JS",
        "mjs" => "JS",
        "cjs" => "JS",
        "jsx" => "JS",
        "ts" => "TS",
        "mts" => "TS",
        "cts" => "TS",
        "tsx" => "TS",
        "py" => "PY",
        "rs" => "RS",
        "go" => "GO",
        "java" => "JV",
        "kt" => "KT",
        "swift" => "SW",
        "rb" => "RB",
        "php" => "PH",
        "cs" => "C#",
        "c" => "C",
        "h" => "H",
        "cpp" => "C+",
        "hpp" => "H+",
        "html" => "<>",
        "htm" => "<>",
        "vue" => "V",
        "svelte" => "S",
        "css" => "#",
        "scss" => "#",
        "sass" => "#",
        "json" => "{}",
        "jsonl" => "{}",
        "yml" => "YM",
        "yaml" => "YM",
        "toml" => "TM",
        "sh" => "$",
        "bash" => "$",
        "ps1" => "$",
        "bat" => "$",
        "cmd" => "$",
        "sql" => "SQ",
        "md" | "mdx" | "txt" | "rst" | "csv" | "log" => "▤",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp" | "avif" => "▧",
        _ => "▫",
    }
}
pub(super) fn scene(s: &mut State) {
    team_scene(s);
    s.changes = Panel {
        open: true,
        files: true,
        diff: Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "locust/native-changes".into(),
            files: changes::parse(
                "diff --git a/src/cart.ts b/src/cart.ts\n--- a/src/cart.ts\n+++ b/src/cart.ts\n@@ -1,4 +1,4 @@\n /* The cart total. */\n-export const total = (items: number[]) => sum(items) * 2\n+export const total = (items: number[]) => sum(items)\n const label = \"done\"\n return total\n",
            ),
            ..changes::Folder::default()
        })),
        ..Panel::default()
    };
}
pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}

pub(super) fn undo_scene(s: &mut State) {
    scene(s);
    s.changes.records = vec![(
        "mission_example".into(),
        serde_json::json!({"before":{"src/cart.ts":"old","notes.md":"old"},"after":{"src/cart.ts":"new","notes.md":"new"},"notKept":["video.mp4"]}),
    )];
    s.changes.asking = Some("mission_example".into());
}
pub(super) fn undo_light_scene(s: &mut State) {
    undo_scene(s);
    s.appearance = Some(false);
}

#[cfg(test)]
mod tests {
    // Syntax colour is the toolkit's job, so this check of a diff's two sides lives with the window.
    #[test]
    fn a_change_is_coloured_as_two_whole_sides() {
        let f = crate::changes::parse("--- a/cart.ts\n+++ b/cart.ts\n@@ -1,4 +1,4 @@\n /* cart,\n    priced */\n-export const total = 2\n+export const total = 1\n const label = \"done\"\n").remove(0);
        let h = &f.hunks[0];
        let (b, a) = h.sides();
        assert!(b.contains("total = 2"));
        assert!(a.contains("total = 1"));
        for code in [a, b] {
            let highlights = rune_kit::syntax::highlight(Some("ts"), &code);
            assert!(!highlights.is_empty());
            for (range, _) in highlights.iter() {
                assert!(code.get(range.clone()).is_some())
            }
        }
    }

    // ---- Electron renderer/src/the-changes-panel-lists-every-file.test.tsx ----------------------------------------

    const SAMPLE_DIFF: &str = "\
diff --git a/src/cart.py b/src/cart.py
--- a/src/cart.py
+++ b/src/cart.py
@@ -1,2 +1,3 @@
 def total(items):
-    return 0
+    return sum(items)
+
diff --git a/src/shop/price.py b/src/shop/price.py
--- a/src/shop/price.py
+++ b/src/shop/price.py
@@ -1 +1 @@
-PRICE = 1
+PRICE = 2
diff --git a/README.md b/README.md
new file mode 100644
--- /dev/null
+++ b/README.md
@@ -0,0 +1 @@
+# Shop";

    // case: counts each file’s added and removed lines
    #[test]
    fn counts_each_files_added_and_removed_lines() {
        let files = crate::changes::parse(SAMPLE_DIFF);
        let got: Vec<(&str, (usize, usize))> = files.iter().map(|f| (f.path.as_str(), f.counts())).collect();
        assert_eq!(
            got,
            vec![
                ("src/cart.py", (2, 1)),
                ("src/shop/price.py", (1, 1)),
                ("README.md", (1, 0)),
            ]
        );
    }

    // case: groups them by folder, each folder’s own files under it
    #[test]
    fn groups_them_by_folder_each_folders_own_files_under_it() {
        let files = crate::changes::parse(SAMPLE_DIFF);
        let root = crate::changes::tree_of(&files);
        let paths_of = |idxs: &[usize]| -> Vec<&str> { idxs.iter().map(|&i| files[i].path.as_str()).collect() };
        assert_eq!(paths_of(&root.files), vec!["README.md"]);
        let src = root.folder("src").expect("src folder");
        assert_eq!(paths_of(&src.files), vec!["src/cart.py"]);
        let src_subfolders: Vec<&str> = src.folders.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(src_subfolders, vec!["shop"]);
        assert_eq!(paths_of(&src.folder("shop").expect("shop folder").files), vec!["src/shop/price.py"]);

        // And the panel draws the folder tree in the headless window, folders before root files.
        let mut s = super::State::default();
        s.changes = super::Panel {
            open: true,
            files: true,
            diff: Some(Ok(crate::changes::Folder {
                base: "main".into(),
                branch: "feature".into(),
                showing: "all".into(),
                files,
                ..crate::changes::Folder::default()
            })),
            ..super::Panel::default()
        };
        let mut rt = rune::Runtime::new(
            s,
            |s, _| super::view(s, &super::Palette::new(true)),
            (1000.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let mut texts = String::new();
        for t in rt.elements().into_iter().filter_map(|e| e.text) {
            texts.push_str(&t);
            texts.push('\n');
        }
        assert!(texts.contains("src\n"), "src folder heading missing: {texts}");
        assert!(texts.contains("shop\n"), "shop folder heading missing: {texts}");
        assert!(texts.contains("price.py"), "price.py missing: {texts}");
        assert!(texts.contains("README.md"), "README.md missing: {texts}");
        assert!(
            texts.find("src\n").unwrap() < texts.find("README.md").unwrap(),
            "folders must be drawn before root files: {texts}"
        );
    }
}

#[cfg(test)]
#[path = "changes_panel_diff_tests.rs"]
mod changes_panel_diff_tests;
#[cfg(test)]
#[path = "changes_walkthrough_tests.rs"]
mod changes_walkthrough_tests;
#[cfg(test)]
#[path = "changes_document_tests.rs"]
mod changes_document_tests;
#[cfg(test)]
#[path = "changes_versions_tests.rs"]
mod changes_versions_tests;

#[cfg(test)]
#[path = "file_versions_strip_tests.rs"]
mod file_versions_strip_tests;

#[cfg(test)]
#[path = "changes_files_highlight_tests.rs"]
mod changes_files_highlight_tests;
