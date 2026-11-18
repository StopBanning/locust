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
