//! One file's change, drawn as Electron's `DiffView.tsx` draws it for every place that shows a change: the Changes
//! panel, the review of a teammate's branch, the activity drawer, the approval card. The hunks come first two at a
//! time with the rest behind a button that names what remains, unchanged lines fold to a count, what changed within a
//! line is marked, and the last thing in the box is always a statement of completeness (`changes::completeness`),
//! ending, when the record is shorter than the change, with what the runtime counted in all. Silence after the last
//! row would read as "that was everything".
use super::*;
use std::collections::HashSet;

/// What is open in the diffs that are not the Changes panel's: the files expanded past two hunks and the folds of
/// unchanged lines pressed open, each under the diff's own id.
#[derive(Default)]
pub(super) struct Folds {
    expanded: HashSet<String>,
    contexts: HashSet<(String, usize, usize)>,
}

/// Where a diff keeps what is open in it: the Changes panel keeps its own, by file; any other diff, by id.
#[derive(Clone)]
pub(super) enum Place {
    Panel(usize),
    Free(String),
}

impl Place {
    /// A diff in a drawer, a review or a card, identified by where it is and what it says, so two changes to one
    /// path do not share their folds.
    pub(super) fn of(whose: &str, f: &changes::File) -> Place {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", f.hunks).hash(&mut hasher);
        Place::Free(format!("{whose}/{}/{:x}", f.path, hasher.finish()))
    }
    fn expanded(&self, s: &State) -> bool {
        match self {
            Place::Panel(i) => s.changes.expanded.contains(i),
            Place::Free(id) => s.diff_folds.expanded.contains(id),
        }
    }
    fn expand(&self, s: &mut State) {
        match self {
            Place::Panel(i) => {
                s.changes.expanded.insert(*i);
            }
            Place::Free(id) => {
                s.diff_folds.expanded.insert(id.clone());
            }
        }
    }
    fn fold_open(&self, s: &State, hunk: usize, start: usize) -> bool {
        match self {
            Place::Panel(i) => s.changes.contexts.contains(&(*i, hunk, start)),
            Place::Free(id) => s.diff_folds.contexts.contains(&(id.clone(), hunk, start)),
        }
    }
    fn open_fold(&self, s: &mut State, hunk: usize, start: usize) {
        match self {
            Place::Panel(i) => {
                s.changes.contexts.insert((*i, hunk, start));
            }
            Place::Free(id) => {
                s.diff_folds.contexts.insert((id.clone(), hunk, start));
            }
        }
    }
}

fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into()).font(type_::LABEL).color(p.secondary).padding(7.0)
}

fn unchanged(hidden: usize) -> String {
    if hidden == 1 { "1 unchanged line".into() } else { format!("{hidden} unchanged lines") }
}

/// The file's hunks and its foot. `notes` is the folder and whether a note can be written on a row (review notes);
/// `copy` is the Copy control the caller draws for the code as it reads after the change, if it offers one.
/// `reported` is what the runtime counted across the whole change, before the record bounded it.
pub(super) fn diff_view(
    s: &State,
    place: Place,
    f: &changes::File,
    truncated: bool,
    reported: Option<(usize, usize)>,
    notes: (Option<&str>, bool),
    copy: Option<El>,
    p: &Palette,
) -> El {
    let mut out = col().w_full().gap(2.0);
    let shown = if place.expanded(s) { f.hunks.len() } else { 2.min(f.hunks.len()) };
    let lang = crate::reply_actions::language_of_path(&f.path);
    for (j, h) in f.hunks.iter().take(shown).enumerate() {
        out = out.child(text(format!("{} {}", h.range(), h.heading)).color(p.muted));
        let (before, after) = h.sides();
        let bh = rune_kit::syntax::highlight(lang.as_deref(), &before);
        let ah = rune_kit::syntax::highlight(lang.as_deref(), &after);
        let mut offsets = (0, 0);
        let mut hidden = 0;
        let keep_mask = changes::kept_context(&h.rows);
        let marks = changes::paired_spans(&h.rows);
        for (k, r) in h.rows.iter().enumerate() {
            let offset = if r.kind == '-' { offsets.0 } else { offsets.1 };
            if r.kind != '+' {
                offsets.0 += r.text.len() + 1
            }
            if r.kind != '-' {
                offsets.1 += r.text.len() + 1
            }
            let run_start = {
                let mut start = k;
                while start > 0 && !keep_mask[start - 1] {
                    start -= 1;
                }
                start
            };
            let keep = keep_mask[k] || place.fold_open(s, j, run_start);
            if !keep {
                hidden += 1;
                continue;
            }
            if hidden > 0 {
                let start = k - hidden;
                let at = place.clone();
                out = out.child(button(unchanged(hidden), p).on_press(move |s: &mut State| at.open_fold(s, j, start)));
                hidden = 0;
            }
            let line = crate::changes_panel::code_row(r, &r.text, offset, if r.kind == '-' { &bh } else { &ah }, marks[k].as_deref(), p);
            out = crate::diff_notes_ui::append_noted_row(out, s, notes.0, notes.1, &f.path, r, line, p);
        }
        if hidden > 0 {
            let start = h.rows.len() - hidden;
            let at = place.clone();
            out = out.child(button(unchanged(hidden), p).on_press(move |s: &mut State| at.open_fold(s, j, start)));
        }
    }
    let more = shown < f.hunks.len();
    let statement = changes::completeness(f, shown, truncated && !more);
    let mut foot = row().w_full().gap(10.0).center_y();
    if let Some(copy) = copy {
        foot = foot.child(copy);
    }
    if more {
        let at = place.clone();
        foot = foot.child(button(statement, p).on_press(move |s: &mut State| at.expand(s)));
    } else {
        let mut said = statement;
        if let (true, Some((added, removed))) = (truncated, reported) {
            said = format!("{said} · the runtime counted {} in all", changes::line_counts(added, removed));
        }
        foot = foot.child(text(said).font(type_::LABEL).color(p.muted).wrap().grow(1.0).shrink());
    }
    out.child(foot)
}
