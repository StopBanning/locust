//! `the-frame-pass.test.ts` (A1 lives in `live_line_tests`/`waiting_clock_tests`).
use crate::runtime_order::{RuntimeEntry, keep_what_was_known, runtime_list_order};

fn runtime(id: &str, name: &str) -> RuntimeEntry {
    RuntimeEntry { id: id.into(), display_name: name.into(), installed: true, version: None, status: "ready".into() }
}
fn tag(id: &str) -> String {
    match id {
        "codex" | "claude" | "cursor" | "opencode" | "copilot" => "READY",
        "gemini" | "omniroute" => "PLANNED",
        "antigravity" => "EXPERIMENTAL",
        "offline" => "SIGN IN",
        _ => "NOT INSTALLED",
    }
    .into()
}
fn at(id: &str, status: &str, version: Option<&str>, installed: bool) -> RuntimeEntry {
    RuntimeEntry { id: id.into(), display_name: id.into(), installed, version: version.map(Into::into), status: status.into() }
}
fn ids(v: &[RuntimeEntry]) -> Vec<&str> {
    v.iter().map(|e| e.id.as_str()).collect()
}

// case: bands them ready, experimental, sign-in, not installed, planned
#[test]
fn bands_them_ready_experimental_sign_in_not_installed_planned() {
    let all = [runtime("gemini", "Gemini CLI"), runtime("codex", "Codex CLI"), runtime("antigravity", "Antigravity"), runtime("absent", "Absent CLI"), runtime("offline", "Offline CLI"), runtime("omniroute", "OmniRoute")];
    let order = runtime_list_order(&all, |e| tag(&e.id));
    assert_eq!(ids(&order), ["codex", "antigravity", "offline", "absent", "gemini", "omniroute"]);
}

// case: never leaves something that does not exist above something that does
#[test]
fn never_leaves_something_that_does_not_exist_above_something_that_does() {
    let all: Vec<_> = ["codex", "claude", "cursor", "gemini", "opencode", "copilot", "omniroute", "antigravity", "offline", "absent"].iter().map(|i| runtime(i, i)).collect();
    let tags: Vec<_> = runtime_list_order(&all, |e| tag(&e.id)).iter().map(|e| tag(&e.id)).collect();
    let last_ready = tags.iter().rposition(|t| t == "READY").unwrap();
    let first_planned = tags.iter().position(|t| t == "PLANNED").unwrap();
    assert!(first_planned > last_ready);
}

// case: is stable between launches, so the list does not shuffle
#[test]
fn is_stable_between_launches_so_the_list_does_not_shuffle() {
    let order = runtime_list_order(&[runtime("cursor", "Cursor Agent"), runtime("claude", "Claude Code"), runtime("codex", "Codex CLI")], |_| "READY".into());
    let names: Vec<_> = order.iter().map(|e| e.display_name.as_str()).collect();
    assert_eq!(names, ["Claude Code", "Codex CLI", "Cursor Agent"]);
}

// case: keeps a known result while the next probe is still out
#[test]
fn keeps_a_known_result_while_the_next_probe_is_still_out() {
    let kept = keep_what_was_known(&[at("claude", "ready", Some("2.1.270"), true)], &[at("claude", "probe-failed", None, true)]);
    assert_eq!((kept[0].status.as_str(), kept[0].version.as_deref()), ("ready", Some("2.1.270")));
}

// case: keeps a version the new answer simply did not print
#[test]
fn keeps_a_version_the_new_answer_simply_did_not_print() {
    let kept = keep_what_was_known(&[at("codex", "ready", Some("0.153.0"), true)], &[at("codex", "ready", None, true)]);
    assert_eq!(kept[0].version.as_deref(), Some("0.153.0"));
}

// case: takes every DEFINITE answer at once, including the unwelcome ones
#[test]
fn takes_every_definite_answer_at_once_including_the_unwelcome_ones() {
    assert_eq!(keep_what_was_known(&[at("claude", "ready", Some("2.1.270"), true)], &[at("claude", "auth-required", Some("2.1.270"), true)])[0].status, "auth-required");
    assert_eq!(keep_what_was_known(&[at("cursor", "ready", Some("1.0"), true)], &[at("cursor", "not-installed", None, false)])[0].status, "not-installed");
}

// case: lets a runtime that was out come back when it answers
#[test]
fn lets_a_runtime_that_was_out_come_back_when_it_answers() {
    let kept = keep_what_was_known(&[at("opencode", "probe-failed", None, true)], &[at("opencode", "ready", Some("1.18.27"), true)]);
    assert_eq!((kept[0].status.as_str(), kept[0].version.as_deref()), ("ready", Some("1.18.27")));
}

// case: passes through a runtime it has never seen
#[test]
fn passes_through_a_runtime_it_has_never_seen() {
    assert_eq!(keep_what_was_known(&[], &[at("gemini", "ready", Some("0.58.0"), true)])[0].version.as_deref(), Some("0.58.0"));
}
