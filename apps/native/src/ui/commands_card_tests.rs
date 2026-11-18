//! Electron's `a-commands-card-says-unknown-once.test.tsx` (both cases) and `a-commands-card-says-what-it-is-for.test.tsx`
//! (3 of 5 cases, plus `command_plain::plain_command`'s `careful` flag), drawn headless through `approval_card`.
//! Not checked in Rust: `approval_card.rs` draws `In short:` in `p.text` rather than amber when `careful` is true
//! (`plain_action` returns `Option<String>`), and Electron's `phone-page.ts` is not in native.
use super::*;
use locust_core::approval::{Kind, Request};
use locust_core::command_plain::plain_command;
use rune::{Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(approval_card(s, &Palette::new(true)))
}
fn asked(detail: &str) -> Request {
    Request {
        approval_id: "ap_1".into(),
        mission_id: "mission_1".into(),
        runtime: "copilot".into(),
        kind: Kind::Command,
        summary: "Run a command".into(),
        detail: detail.into(),
        cwd: Some("C:\\work".into()),
        requested_at: "2026-10-10T22:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        claude_input: None,
        questions: vec![],
    }
}
fn drawn(req: Request) -> String {
    let mut state = State::default();
    state.pending_approval = Some(req);
    let mut rt = Runtime::new(state, view, (800.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|n| n.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn are_one_row_when_locust_can_tell_nothing_about_them() {
    let said = drawn(asked("echo LOCUST-ACP-OK > approved.txt"));
    assert!(said.contains("Effects: Unknown. Locust cannot tell what a command sends or undo what it does; it can do anything the sandbox allows."), "{said}");
    assert!(!said.contains("Data sent:"));
    assert!(!said.contains("Reversible:"));
    assert_eq!(said.matches("Unknown").count(), 1);
}

#[test]
fn stay_two_rows_whenever_either_row_knows_something() {
    let network = drawn(asked("curl https://example.com -o page.html"));
    assert!(network.contains("Data sent: This command can reach the network."), "{network}");
    assert!(network.contains("Reversible:"));
    assert!(!network.contains("Effects:"));

    let mut said_req = asked("echo LOCUST-ACP-OK > approved.txt");
    said_req.reversible_says = Some("Nothing on this machine is changed by a fetch.".into());
    let said = drawn(said_req);
    assert!(said.contains("Reversible: Nothing on this machine is changed by a fetch."), "{said}");
    assert!(said.contains("Data sent:"));

    let mut edit_req = asked("cart.py");
    edit_req.kind = Kind::FileChange;
    edit_req.summary = "Edit a file".into();
    let edit = drawn(edit_req);
    assert!(edit.contains("Data sent: Nothing. The change is written to this machine and sent nowhere."), "{edit}");
    assert!(edit.contains("Reversible:"));
    assert!(!edit.contains("Effects:"));
}

#[test]
fn says_what_a_forced_push_does_under_the_command() {
    let mut req = asked("git push --force origin main");
    req.runtime = "claude".into();
    req.cwd = Some("C:\\work\\shop".into());
    let said = drawn(req);
    assert!(said.contains("In short: Replaces main on origin with this computer’s version: commits there that are not on this computer are lost."), "{said}");
    let exact = said.find("Exact:").expect("Exact");
    let in_short = said.find("In short:").expect("In short");
    let where_at = said.find("Where:").expect("Where");
    assert!(exact < in_short && in_short < where_at);
    assert!(plain_command("git push --force origin main").unwrap().careful);
}

#[test]
fn says_it_plainly_for_one_that_only_looks() {
    let said = drawn(asked("git status"));
    assert!(said.contains("In short: Shows which files have changed. It only looks."), "{said}");
    assert!(!plain_command("git status").unwrap().careful);
}

#[test]
fn has_no_such_row_for_a_command_it_cannot_read_all_of_or_for_a_file_change() {
    assert!(!drawn(asked("npm test && ./deploy.sh")).contains("In short:"));
    let mut edit = asked("rm -rf dist");
    edit.kind = Kind::FileChange;
    edit.summary = "Edit a file".into();
    assert!(!drawn(edit).contains("In short:"));
}
