//! What a stopped run left behind (Electron `missionView.ts` cancellationSummary and `CancellationCard.tsx`), read
//! from the turn's own record rather than from file bookkeeping: the tool calls that reported finishing, the ones open
//! when it stopped, and the latest plan's steps, counted so they add up. "Finished" is what the runtime reported, never
//! a claim about the disk, and nothing claims a revert.

use crate::ledger::{Item, Status};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub settled: Vec<String>,
    pub interrupted: Vec<String>,
    pub never_started: usize,
    /// (finished, cut off, total), from the latest plan.
    pub plan: Option<(usize, usize, usize)>,
    /// The runtime reports a tool only once it finished (OpenCode), so one running when it stopped is in neither list.
    pub tools_reported_when_done: bool,
}

/// The turn's items, as the thread holds them; `runtime` the agent that ran it; `workspace` the folder.
pub fn summary(items: &[Item], runtime: &str, workspace: Option<&str>) -> Summary {
    let normal = |path: &str| path.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let root = workspace.map(normal);
    let shown = |name: &str| -> String {
        let norm = name.replace('\\', "/");
        if let Some(ws) = workspace {
            let ws = ws.replace('\\', "/");
            if let Some(rest) = norm.strip_prefix(&format!("{}/", ws.trim_end_matches('/'))) {
                return rest.to_owned();
            }
        }
        norm
    };
    let is_root = |name: &str| {
        root.as_ref().is_some_and(|r| {
            let n = normal(name);
            n == *r || Some(n.as_str()) == r.rsplit('/').next() || n == "." || n.is_empty()
        })
    };
    let key = |name: &str| shown(name).to_lowercase();
    let (mut settled, mut interrupted) = (Vec::new(), Vec::new());
    let mut seen = std::collections::HashSet::new();
    let mut take = |name: &str, into: &mut Vec<String>| {
        if is_root(name) || !seen.insert(key(name)) {
            return;
        }
        into.push(shown(name));
    };
    // What the teammate had in hand: a helper's calls are under its own row.
    let tools: Vec<(&Status, String)> = items
        .iter()
        .filter_map(|item| match item {
            Item::Tool { status, summary, name, detail } if !detail.helper => Some((status, if summary.is_empty() { name.clone() } else { summary.clone() })),
            _ => None,
        })
        .collect();
    for (status, name) in tools.iter().filter(|(s, _)| !matches!(s, Status::Running)) {
        let _ = status;
        take(name, &mut settled);
    }
    for (_, name) in tools.iter().filter(|(s, _)| matches!(s, Status::Running)) {
        take(name, &mut interrupted);
    }
    // Only the latest reported plan advances the plan: reading a folder or a changed file cannot.
    let latest = items.iter().rev().find_map(|item| match item {
        Item::Plan { steps, .. } if !steps.is_empty() => Some(steps),
        _ => None,
    });
    let (finished, cut_off, total) = latest.map_or((0, 0, 0), |steps| {
        (steps.iter().filter(|s| s.state == "done").count(), steps.iter().filter(|s| s.state == "running").count(), steps.len())
    });
    Summary {
        settled,
        interrupted,
        never_started: total.saturating_sub(finished + cut_off),
        plan: (total > 0).then_some((finished, cut_off, total)),
        tools_reported_when_done: runtime == "opencode",
    }
}

/// No tool call settled, none was cut off, none was planned: what "nothing happened" means on the card.
pub fn stopped_before_any_tool(s: &Summary) -> bool {
    s.settled.is_empty() && s.interrupted.is_empty() && s.never_started == 0 && s.plan.is_none()
}

pub const BEFORE_ANY_TOOL: &str = "Stopped before it used any tools, so nothing is half-done.";
pub const UNREPORTED_COMMAND: &str = "An unreported OpenCode command may have been running when stopped. OpenCode reports tools only after they finish; check the folder before sending again.";
pub const NONE_OPEN: &str = "No tool call was open when you stopped it.";
pub const NOTHING_FINISHED: &str = "Nothing reported finishing.";
pub const UNKNOWN_EFFECT: &str = "Started and never reported back \u{2014} whether it took effect is unknown.";
pub const IN_THE_RECORD: &str = "Everything up to this point is in the durable record. Nothing is rolled back \u{2014} this build does not snapshot the workspace, so undoing a change is yours to do.";
pub const MAY_STILL_FINISH: &str = "A command that had already started may still finish on its own. If one was running, check the workspace rather than assuming it stopped when you did.";

pub fn plan_line(s: &Summary) -> Option<String> {
    let (finished, cut_off, total) = s.plan?;
    Some(format!("{finished} finished \u{b7} {cut_off} cut off \u{b7} {} never started ({total} step{})", s.never_started, if total == 1 { "" } else { "s" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{PlanStep, ToolDetail};

    fn tool(summary: &str, status: Status) -> Item {
        Item::Tool { name: "Bash".into(), status, summary: summary.into(), detail: Box::new(ToolDetail::default()) }
    }
    fn plan(states: &[&str]) -> Item {
        Item::Plan { id: "p".into(), steps: states.iter().map(|s| PlanStep { text: "step".into(), state: (*s).into() }).collect(), finished: false }
    }

    // Electron's a-stopped-card-counts-the-plan-and-says-what-is-unknown.test.tsx.
    #[test]
    fn counts_plan_states_even_when_unrelated_tools_settled_and_uses_the_latest_plan() {
        let items = vec![plan(&["done", "pending"]), tool("ls", Status::Done), tool("cat a.txt", Status::Done), tool("echo", Status::Done), plan(&["done", "running", "pending"])];
        let s = summary(&items, "codex", None);
        assert_eq!(s.plan, Some((1, 1, 3)));
        assert_eq!(s.never_started, 1);
        assert_eq!(plan_line(&s).as_deref(), Some("1 finished \u{b7} 1 cut off \u{b7} 1 never started (3 steps)"));
        assert_eq!(s.settled.len(), 3, "extra tool calls are never plan steps");
    }

    #[test]
    fn shows_a_finished_plan_without_tools_and_invents_none_for_a_command_without_one() {
        let done = summary(&[plan(&["done", "done"])], "codex", None);
        assert_eq!((done.plan, done.never_started), (Some((2, 0, 2)), 0));
        let none = summary(&[tool("sleep 30", Status::Running)], "codex", None);
        assert_eq!((none.plan, none.interrupted.clone()), (None, vec!["sleep 30".to_owned()]));
        assert!(!stopped_before_any_tool(&none));
        assert!(stopped_before_any_tool(&summary(&[], "claude", None)));
    }

    #[test]
    fn excludes_the_workspace_root_and_its_relative_spellings_without_hiding_a_file_of_the_same_basename() {
        let items = vec![tool("/work/app", Status::Done), tool("app", Status::Done), tool(".", Status::Done), tool("/work/app/notes.txt", Status::Done), tool("notes.txt", Status::Done), tool("/work/app/src/app", Status::Done)];
        let s = summary(&items, "codex", Some("/work/app"));
        assert_eq!(s.settled, vec!["notes.txt".to_owned(), "src/app".to_owned()]);
    }

    #[test]
    fn an_opencode_run_says_an_unreported_command_may_have_been_running() {
        assert!(summary(&[], "opencode", None).tools_reported_when_done);
        assert!(!summary(&[], "codex", None).tools_reported_when_done);
    }
}
