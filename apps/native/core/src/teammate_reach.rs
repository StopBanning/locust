//! A partial discovery sweep cannot prove that no agent can run (status.ts runtimeReach).
use crate::agents::{Found, Probe};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reach {
    pub usable: Option<bool>,
    pub installed: Option<bool>,
}
pub fn runtime_reach(runtimes: &[Found], known: bool) -> Reach {
    if !known {
        return Reach::default();
    }
    let usable = runtimes.iter().any(|r| r.ready && r.status == Probe::Ready);
    Reach {
        usable: (usable || !runtimes.iter().any(|r| r.checking)).then_some(usable),
        installed: Some(runtimes.iter().any(|r| r.installed)),
    }
}
/// No label means discovery has not established a block: keep the teammate's other state.
pub fn blocked_label(own: Option<&Found>, reach: Reach) -> Option<&'static str> {
    if reach.usable == Some(false) {
        return Some(if reach.installed == Some(false) {
            "No AI agent installed"
        } else {
            "Sign-in needed"
        });
    }
    let own = own.filter(|r| !r.checking && !(r.ready && r.status == Probe::Ready))?;
    Some(if !own.installed {
        "AI agent not installed"
    } else {
        match own.status {
            Probe::Unsupported => "This AI agent version is not supported yet",
            Probe::ProbeFailed | Probe::Offline => "AI agent not answering — checking again",
            _ => "Sign-in needed",
        }
    })
}
/// Shared roster presentation; status, words and presence must describe the same discovery answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusView {
    pub status: &'static str,
    pub label: String,
    pub tone: &'static str,
    pub presence: &'static str,
}
pub fn status_view(own: Option<&Found>, reach: Reach, running: bool, approvals: usize, role: &str) -> StatusView {
    let (status, label, tone, presence) = if let Some(label) = blocked_label(own, reach) {
        let not_answering = reach.usable != Some(false) && own.is_some_and(|r| r.installed && matches!(r.status, Probe::ProbeFailed | Probe::Offline));
        ("blocked", label.to_owned(), if not_answering { "amber" } else { "red" }, "blocked")
    } else if approvals > 0 {
        ("approval-needed", format!("{role} · waiting on you"), "amber", "approval")
    } else if running {
        ("working", format!("{role} · working"), "live", "working")
    } else {
        ("idle", format!("{role} · idle"), "muted", "none")
    };
    StatusView { status, label, tone, presence }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Auth;
    fn runtime(id: &str, usable: bool, installed: bool) -> Found {
        Found {
            id: id.into(),
            display_name: id.into(),
            installed,
            version: installed.then(|| "1.0.0".into()),
            auth: if usable {
                Auth::Authenticated
            } else {
                Auth::Unknown
            },
            ready: usable,
            status: if usable {
                Probe::Ready
            } else if installed {
                Probe::AuthRequired
            } else {
                Probe::Missing
            },
            unsupported_note: None,
            through_cli: false,
            checking: false,
        }
    }
    fn pending() -> Found {
        Found {
            status: Probe::ProbeFailed,
            checking: true,
            ..runtime("opencode", false, true)
        }
    }
    fn own_view(own: &Found, others: &[Found]) -> Option<&'static str> {
        let all: Vec<_> = std::iter::once(own.clone())
            .chain(others.iter().cloned())
            .collect();
        blocked_label(Some(own), runtime_reach(&all, true))
    }
    #[test]
    fn has_no_global_blocked_verdict_while_the_only_possible_agent_is_still_checking() {
        assert_eq!(
            runtime_reach(&[pending(), runtime("claude", false, false)], true),
            Reach {
                usable: None,
                installed: Some(true)
            }
        );
        assert_eq!(blocked_label(None, runtime_reach(&[pending()], true)), None);
    }
    #[test]
    fn keeps_its_pending_own_agent_idle_even_when_another_agent_is_ready() {
        assert_eq!(own_view(&pending(), &[runtime("claude", true, true)]), None);
    }
    #[test]
    fn keeps_a_checked_unusable_own_agent_blocked_while_another_check_is_pending() {
        assert_eq!(
            own_view(&runtime("claude", false, false), &[pending()]),
            Some("AI agent not installed")
        );
        assert_eq!(
            own_view(&runtime("claude", false, true), &[pending()]),
            Some("Sign-in needed")
        );
    }
    #[test]
    fn returns_to_idle_when_a_late_ready_answer_replaces_a_checked_failure() {
        let failed = Found {
            checking: false,
            ..pending()
        };
        assert!(own_view(&failed, &[]).is_some());
        assert_eq!(own_view(&pending(), &[]), None);
        assert_eq!(own_view(&runtime("opencode", true, true), &[]), None);
    }
    #[test]
    fn is_idle_not_blocked_while_discovery_has_not_answered() {
        assert_eq!(runtime_reach(&[], false), Reach::default());
        assert_eq!(blocked_label(None, runtime_reach(&[], false)), None);
    }
    #[test]
    fn is_blocked_once_discovery_says_nothing_can_run_and_says_why() {
        assert_eq!(
            blocked_label(
                None,
                runtime_reach(
                    &[
                        runtime("opencode", false, false),
                        runtime("claude", false, false)
                    ],
                    true
                )
            ),
            Some("No AI agent installed")
        );
        assert_eq!(
            blocked_label(
                None,
                runtime_reach(&[runtime("opencode", false, true)], true)
            ),
            Some("Sign-in needed")
        );
    }
    #[test]
    fn is_idle_once_one_agent_can_run() {
        assert_eq!(
            blocked_label(
                None,
                runtime_reach(
                    &[
                        runtime("opencode", true, true),
                        runtime("claude", false, false)
                    ],
                    true
                )
            ),
            None
        );
    }
    // a-signed-out-teammate-is-not-idle.test.ts: the remaining six Electron cases.
    fn view(own: Option<&Found>, usable: bool, running: bool, approvals: usize, role: &str) -> StatusView {
        status_view(own, Reach { usable: Some(usable), installed: None }, running, approvals, role)
    }
    #[test]
    fn reads_as_blocked_not_idle_when_its_runtime_is_signed_out() {
        let v = view(Some(&runtime("claude", false, true)), true, false, 0, "Docs & QA");
        assert_eq!(v.status, "blocked");
        assert_eq!(v.label, "Sign-in needed");
        assert!(!v.label.contains("idle"));
        assert_eq!(v.tone, "red");
        assert_eq!(v.presence, "blocked");
    }
    #[test]
    fn still_reads_as_idle_when_its_own_runtime_is_fine() {
        let v = view(Some(&runtime("codex", true, true)), true, false, 0, "Docs & QA");
        assert_eq!(v.status, "idle");
        assert_eq!(v.presence, "none");
    }
    #[test]
    fn keeps_a_mission_waiting_on_an_approval_as_its_own_state_not_blocked() {
        let v = view(Some(&runtime("codex", true, true)), true, true, 1, "Builder");
        assert_eq!(v.status, "approval-needed");
        assert_eq!(v.presence, "approval");
        assert_eq!(v.tone, "amber");
    }
    #[test]
    fn is_blocked_when_nothing_at_all_is_signed_in_whatever_it_is_set_to() {
        assert_eq!(view(None, false, false, 0, "Docs & QA").status, "blocked");
    }
    #[test]
    fn is_not_told_to_sign_in() {
        for status in [Probe::ProbeFailed, Probe::Offline] {
            let own = Found { status, ..runtime("claude", false, true) };
            let v = view(Some(&own), true, false, 0, "Code & Migrations");
            assert!(!v.label.to_lowercase().contains("sign-in"));
            assert!(v.label.contains("not answering"));
        }
    }
    #[test]
    fn still_is_when_the_runtime_says_it_is_signed_out() {
        let own = Found { auth: Auth::Unauthenticated, status: Probe::AuthRequired, ..runtime("claude", false, true) };
        assert_eq!(view(Some(&own), true, false, 0, "Code & Migrations").label, "Sign-in needed");
    }

}
