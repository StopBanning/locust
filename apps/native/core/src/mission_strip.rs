//! THE MISSION STRIP: one scope, one tense (App.tsx `lc-workroom__mission`; three-placements.test.ts part 1).
//!
//! Design ruling, 2026-09-14, after the line truncated on Colin's screen as "… 208 out · co…": it had grown to seven
//! facts because it is the most visible surface, not the right one. Off came the noun `Mission ·` (eight characters on
//! every render to label the line once), the permission sentence (the composer's mode chip states it where it can be
//! changed), and, in the first-impressions pass after 0.349, the eight-hex id and "restored from the local ledger".
//! What stays: the model, by the name the chip gives it, and the phase only when it is news.

use crate::teammate_tooltip::route_model_name;

/// The strip after the runtime: the model (none for the account's own default, which says nothing the runtime beside
/// it does not), the phase when it is news (running, failed, cancelled, interrupted; a finished run says nothing),
/// then this run's own cost tail (`benchmark::header_cost_tail`, which starts with " · " when there is one).
pub fn mission_strip(runtime: &str, model: Option<&str>, phase: &str, cost_tail: &str) -> String {
    let model = match model {
        None | Some("") | Some("account-default") => String::new(),
        Some(model) => route_model_name(runtime, model),
    };
    let phase = match phase {
        "completed" | "" => String::new(),
        news => format!(" · {news}"),
    };
    let line = format!("{model}{phase}{cost_tail}");
    line.strip_prefix(" · ").map(str::to_owned).unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_the_noun_that_labelled_the_line_once_and_cost_eight_characters_forever() {
        for phase in ["running", "completed", "failed"] {
            let strip = mission_strip("claude", Some("sonnet"), phase, " · $0.12");
            assert!(!strip.contains("Mission"), "{strip}");
            assert!(strip.starts_with("Sonnet 5.5"), "the model opens the line: {strip}");
        }
    }

    #[test]
    fn dropped_the_permission_prose_which_the_composer_already_states_where_it_can_be_changed() {
        // The strip takes no mode or sandbox at all: there is nothing for it to say about permission.
        let strip = mission_strip("codex", Some("gpt-5.5-codex"), "running", "");
        for prose in ["sandbox", "read-only", "Read only", "workspace", "Ask", "Auto", "approve"] {
            assert!(!strip.contains(prose), "{prose:?} in {strip}");
        }
    }

    #[test]
    fn names_the_model_always_and_the_phase_only_when_it_is_news() {
        assert_eq!(mission_strip("claude", Some("sonnet"), "completed", ""), "Sonnet 5.5");
        assert_eq!(mission_strip("claude", Some("sonnet"), "running", ""), "Sonnet 5.5 · running");
        assert_eq!(mission_strip("claude", Some("sonnet"), "failed", " · $0.12"), "Sonnet 5.5 · failed · $0.12");
        assert_eq!(mission_strip("claude", Some("sonnet"), "cancelled", ""), "Sonnet 5.5 · cancelled");
        assert_eq!(mission_strip("claude", Some("sonnet"), "interrupted", ""), "Sonnet 5.5 · interrupted");
        // The account's own default says nothing the runtime beside it does not; the phase then opens the line.
        assert_eq!(mission_strip("claude", Some("account-default"), "running", ""), "running");
        assert_eq!(mission_strip("claude", Some("account-default"), "completed", ""), "");
        assert_eq!(mission_strip("claude", None, "completed", " · $0.12"), "$0.12");
        // No id, and nothing about where the record came from.
        let strip = mission_strip("claude", Some("opus"), "completed", "");
        assert!(!strip.contains("restored from the local ledger") && !strip.contains("completed"), "{strip}");
    }
}
