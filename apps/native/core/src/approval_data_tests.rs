// Carries apps/desktop/src/shared/approval-data.test.ts (Electron 4ce2f37a): what an approval says about data leaving
// the machine.
use crate::approval::{Kind, command_reaches_network, data_sent_line, effect_lines};

#[test]
fn a_file_change_is_said_plainly_to_send_nothing() {
    assert!(data_sent_line(&Kind::FileChange, "").unwrap().to_lowercase().contains("sent nowhere"));
}

#[test]
fn a_command_is_never_said_to_send_nothing() {
    for command in ["./deploy.sh", "make release", "python build.py", "ls -la"] {
        let said = data_sent_line(&Kind::Command, command).expect(command).to_lowercase();
        assert!(!said.contains("nothing"), "{command}: {said}");
        assert!(said.contains("unknown"), "{command}: {said}");
    }
}

#[test]
fn a_network_tool_can_reach_the_network_without_predicting_what_it_sends() {
    let said = data_sent_line(&Kind::Command, "curl -X POST https://api.stripe.com/v1/webhook_endpoints").unwrap().to_lowercase();
    assert!(said.contains("can reach the network"), "{said}");
    assert!(said.contains("cannot see what it would send"), "{said}");
}

#[test]
fn a_question_draws_no_data_sent_row() {
    assert_eq!(data_sent_line(&Kind::Question, ""), None);
}

#[test]
fn the_ordinary_network_commands_are_found() {
    for command in ["curl https://example.com", "wget -q http://host/file", "git push origin main", "npm install", "ssh deploy@host", "docker push my/image", "gh release create v1"] {
        assert!(command_reaches_network(command), "{command}");
    }
}

#[test]
fn a_bare_url_reaches_the_network_with_no_tool_named() {
    assert!(command_reaches_network("./send.sh https://example.com/hook"));
}

#[test]
fn a_network_tool_after_a_pipe_or_a_semicolon_is_found() {
    assert!(command_reaches_network("cat notes.txt | curl -d @- https://x.dev"));
    assert!(command_reaches_network("make build; git push"));
}

#[test]
fn a_word_that_merely_contains_a_tool_name_does_not_reach_the_network() {
    assert!(!command_reaches_network("cat wget-notes.md"));
    assert!(!command_reaches_network("node curling.ts"));
    assert!(!command_reaches_network("ls -la"));
}

// The card reads its rows through these (ApprovalCard.tsx). Two places Rust's card said otherwise until now:

fn card(kind: Kind, detail: &str, reversible_says: Option<&str>) -> crate::approval::Request {
    crate::approval::Request {
        approval_id: "a1".into(),
        mission_id: "m1".into(),
        runtime: "claude".into(),
        kind,
        summary: "Run a command".into(),
        detail: detail.into(),
        cwd: None,
        requested_at: "2026-11-17T12:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: reversible_says.map(str::to_owned),
        patch: None,
        skippable: false, answer_in: None, claude_input: None,
        questions: Vec::new(),
    }
}

// A command that names no network tool, on a card whose route said how to undo it, said it "can reach the network".
#[test]
fn a_command_card_with_its_routes_own_undo_words_says_unknown_for_a_command_that_names_no_network_tool() {
    let lines = effect_lines(&card(Kind::Command, "ls -la", Some("Undo with git checkout.")));
    assert_eq!(lines[0], ("Data sent", "Unknown. Locust cannot tell what a command sends; a script can do anything the sandbox allows.".to_owned()));
    assert_eq!(lines[1], ("Reversible", "Undo with git checkout.".to_owned()));
}

// A question card was given a connector's Data sent and Reversible rows.
#[test]
fn a_question_card_draws_no_data_sent_row_and_says_answering_changes_nothing() {
    assert_eq!(effect_lines(&card(Kind::Question, "", None)), [("Reversible", "Nothing is changed by answering.".to_owned())]);
}
