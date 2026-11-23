//! The steps fold (missionView.ts `buildThread`, the `pieces` loop): what a teammate said breaks the turn, and the
//! steps taken between are one line. A thought that says something starts a new line (0.492); a thought with no words
//! folds into the line it sits in and still shows there (0.571, Antigravity thinks before every call).
use crate::ledger::{Item, Status};

/// One steps line: the indices (into the thread's items) of the thoughts and calls it holds, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepsGroup {
    pub items: Vec<usize>,
}

fn says_something(item: &Item) -> bool {
    matches!(item, Item::Thought { text, .. } if !text.trim().is_empty())
}

fn is_step(item: &Item) -> bool {
    matches!(item, Item::Thought { .. } | Item::Tool { .. })
}

/// Group a turn's items into steps lines. While `running`, a call still going is the live line's and is left out.
pub fn fold_steps(items: &[Item], running: bool) -> Vec<StepsGroup> {
    let mut out = Vec::new();
    let mut group: Vec<usize> = Vec::new();
    let flush = |group: &mut Vec<usize>, out: &mut Vec<StepsGroup>| {
        let shown: Vec<usize> = group
            .iter()
            .copied()
            .filter(|&i| !(running && matches!(&items[i], Item::Tool { status: Status::Running, .. })))
            .collect();
        if !shown.is_empty() {
            out.push(StepsGroup { items: shown });
        }
        group.clear();
    };
    for (index, item) in items.iter().enumerate() {
        match item {
            Item::Assistant { .. } | Item::User(_) | Item::Handoff { .. } | Item::CarryOn { .. } => flush(&mut group, &mut out),
            _ if says_something(item) => {
                flush(&mut group, &mut out);
                group.push(index);
            }
            _ if is_step(item) => group.push(index),
            _ => {}
        }
    }
    flush(&mut group, &mut out);
    out
}

/// What the thread draws for a turn, in the order it happened (missionView.ts `buildThread`, 0.491): each thing said,
/// then the steps taken before the next thing said as lines, the way Claude Code draws a turn, while it runs and after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    /// An item drawn as itself: a message, the person's words, a note, the turn's foot.
    Item(usize),
    /// A steps line; `run` is the index the run of steps it belongs to began at (its place, kept as it grows).
    Steps { run: usize, group: StepsGroup },
}

/// A message with nothing on screen once its blocks are taken out, and nothing handed over or refused (`shownText`
/// empty, no `handedNow`, no `refused`): it does not split the steps around it, and is not drawn.
pub fn says_nothing_on_screen(item: &Item, running: bool) -> bool {
    let Item::Assistant { text, r#final, .. } = item else { return false };
    let settled = *r#final || !running;
    crate::protocol_tags::shown_text(text).trim().is_empty()
        && !(settled && !crate::protocol_tags::parse_file_blocks(text).is_empty())
        && !(settled && !crate::protocol_tags::refused_file_lines(text).is_empty())
}

/// A row the host added after the run ended: in no steps line, only in the foot's files.
fn after_end(item: &Item) -> bool {
    matches!(item, Item::Tool { detail, .. } if detail.after_end)
}

/// The turn's items as the thread draws them. A message stands where it BEGAN (the ledger holds it at its first piece),
/// so narration restated after the next call stays before it; a message with nothing on screen neither splits the steps
/// nor draws; a row born after the run ended is left to the foot; and a call still going is the live line's.
pub fn thread_pieces(items: &[Item], running: bool) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    let flush = |run: &mut Vec<usize>, out: &mut Vec<Piece>| {
        if let Some(&start) = run.first() {
            let held: Vec<Item> = run.iter().map(|&i| items[i].clone()).collect();
            for group in fold_steps(&held, running) {
                out.push(Piece::Steps { run: start, group: StepsGroup { items: group.items.iter().map(|&i| run[i]).collect() } });
            }
        }
        run.clear();
    };
    for (index, item) in items.iter().enumerate() {
        if after_end(item) || says_nothing_on_screen(item, running) {
            continue;
        }
        if is_step(item) {
            run.push(index);
            continue;
        }
        flush(&mut run, &mut out);
        out.push(Piece::Item(index));
    }
    flush(&mut run, &mut out);
    out
}

/// The line's own words when one of its thoughts leads with a headline (`stepsLine`: `headline ??= thoughtHeadline(..)`,
/// the first one wins, and it is the whole line, 0.610). None leaves the counts line to the caller.
pub fn group_headline(items: &[Item], group: &StepsGroup) -> Option<String> {
    group.items.iter().find_map(|&i| match &items[i] {
        Item::Thought { text, .. } => crate::thread_words::thought_headline(text),
        _ => None,
    })
}

/// A turn that only planned (`touchedNothing`): it had no activity at all, so its plan is the answer and draws as a
/// plain list; with any call or thought in the turn the plan's outcomes are drawn.
pub fn touched_nothing(items: &[Item]) -> bool {
    !items.iter().any(is_step)
}

/// How many of a plan's steps finished (`doneCount`).
pub fn plan_done_count(steps: &[crate::ledger::PlanStep]) -> usize {
    steps.iter().filter(|step| step.state == "done").count()
}
