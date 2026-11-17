//! What the composer's route chip wears before its words (Composer.tsx, the-runtimes-wear-their-marks.test.tsx): the
//! runtime's mark where the ready dot was, greyed when that runtime cannot take work; a plain dot for a model of the
//! person's own, which no maker's mark is true of; and "No AI agent", with no mark, when nothing can take work.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChipMark {
    /// The runtime's own mark; muted when it cannot take work (signed out, not installed, still checking).
    Mark { muted: bool },
    /// A model of the person's own: the ready dot, no maker's mark.
    Dot { ready: bool },
    /// Nothing on this machine can take work.
    NoAgent,
}

/// `agents`: each runtime found and whether it can take work; None until the first look has finished, when the chip
/// keeps the mark rather than flash "No AI agent".
pub(crate) fn chip_mark(runtime: &str, model: &str, agents: Option<&[(String, bool)]>) -> ChipMark {
    let ready = agents.is_none_or(|all| all.iter().any(|(id, ready)| id == runtime && *ready));
    if agents.is_some_and(|all| !all.iter().any(|(_, ready)| *ready)) {
        return ChipMark::NoAgent;
    }
    if locust_core::spend::is_own_model(runtime, model) {
        return ChipMark::Dot { ready };
    }
    ChipMark::Mark { muted: !ready }
}

/// The runtimes this window has found and whether each can take work, from Settings' look at the machine.
pub(crate) fn found_agents(s: &State) -> Option<Vec<(String, bool)>> {
    s.settings_panel.agents.as_ref().map(|found| found.iter().map(|f| (f.id.clone(), f.ready)).collect())
}

/// The El the chip wears for `mark`, or None for "No AI agent".
pub(crate) fn chip_mark_el(l: &crate::look::controls::Look, runtime: &str, mark: &ChipMark) -> Option<El> {
    match mark {
        ChipMark::Mark { muted } => crate::look::controls::runtime_mark(l, runtime, 13.0, if *muted { l.c("text-muted") } else { l.c("text-secondary") }, None, *muted)
            .map(|m| m.key(format!("chip-mark-{runtime}{}", if *muted { "-muted" } else { "" }))),
        ChipMark::Dot { ready } => Some(
            row()
                .w(7.0)
                .h(7.0)
                .radius(3.5)
                .fill(if *ready { l.c("green") } else { l.c("text-muted") })
                .key("chip-dot")
                .a11y_hidden(),
        ),
        ChipMark::NoAgent => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(list: &[(&str, bool)]) -> Vec<(String, bool)> {
        list.iter().map(|(id, ready)| (id.to_string(), *ready)).collect()
    }

    #[test]
    fn wears_the_runtimes_mark_where_the_ready_dot_was() {
        assert_eq!(chip_mark("claude", "sonnet", Some(&found(&[("claude", true)]))), ChipMark::Mark { muted: false });
        // Before the first look, the mark stays rather than flash "No AI agent".
        assert_eq!(chip_mark("claude", "sonnet", None), ChipMark::Mark { muted: false });
    }

    #[test]
    fn greys_the_mark_when_that_runtime_cannot_take_work_the_one_thing_the_dot_said() {
        assert_eq!(chip_mark("claude", "sonnet", Some(&found(&[("codex", true), ("claude", false)]))), ChipMark::Mark { muted: true });
        assert_eq!(chip_mark("cursor", "auto", Some(&found(&[("codex", true)]))), ChipMark::Mark { muted: true }, "not found at all");
    }

    #[test]
    fn shows_no_makers_mark_on_a_model_of_the_persons_own_and_none_when_nothing_is_connected() {
        assert_eq!(chip_mark("opencode", "own-1a2b3c4d/acme-70b", Some(&found(&[("opencode", true)]))), ChipMark::Dot { ready: true });
        assert_eq!(chip_mark("claude", "sonnet", Some(&found(&[("claude", false)]))), ChipMark::NoAgent);
        assert_eq!(chip_mark("claude", "sonnet", Some(&[])), ChipMark::NoAgent);
    }
}
