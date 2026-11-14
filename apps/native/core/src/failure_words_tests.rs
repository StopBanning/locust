//! Electron's missionView.test.ts 'what a failure card says', its-own-settings-file.test.ts whole, and the
//! failureMessage cases of a-provider-error-is-readable-and-said-once.test.tsx.
use super::*;

const ESC: char = '\u{1b}';

fn card(message: &str, stderr: &str) -> String {
    failure_message(message, Some(stderr), None)
}

#[test]
fn shows_the_runtimes_own_last_word_because_the_hosts_sentence_names_only_the_shape() {
    assert_eq!(
        card("Codex invocation did not complete successfully", "Not inside a trusted directory and --skip-git-repo-check was not specified.\n"),
        "Codex invocation did not complete successfully The runtime's own last word was: Not inside a trusted directory and --skip-git-repo-check was not specified."
    );
}

#[test]
fn says_capacity_exhaustion_in_english_and_keeps_the_runtimes_words() {
    let said = card("Cursor Agent ended without a terminal result record.", "RetriableError: [resource_exhausted] Error\n");
    assert!(said.contains("out of capacity right now"), "{said}");
    assert!(said.contains("resource_exhausted"), "{said}");
}

#[test]
fn says_a_limit_once_when_the_message_already_names_it_in_plain_words() {
    let quota = "API error 429 [request_id=2370cf39]: Subscription quota exhausted. Your usage window resets at 2026-10-26T00:00:00Z. (rate_limit_error)";
    let muse = "Muse Code's plan is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets.";
    assert_eq!(failure_message(muse, Some(&format!("muse: Agent delegation: auto unavailable: workspace is untrusted.\nrun ended with Failed: {quota}\n")), Some("muse")), muse);
    let agy_line = "AGY_ERROR: {\"short_error\":\"RESOURCE_EXHAUSTED (code 429): Individual quota reached.\",\"status\":\"RESOURCE_EXHAUSTED\",\"error_code\":429}\n";
    let agy = "Antigravity could not run it: Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 33h54m24s.";
    assert_eq!(failure_message(agy, Some(agy_line), Some("antigravity")), agy);
    let sentence = "Antigravity's quota is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets.";
    assert_eq!(failure_message(sentence, Some(agy_line), Some("antigravity")), sentence);
    assert!(card("Cursor Agent ended without a terminal result record.", "RetriableError: [resource_exhausted] Error\n").contains("out of capacity right now"));
}

#[test]
fn adds_nothing_when_the_runtime_said_nothing() {
    assert_eq!(failure_message("Codex CLI is not ready.", None, None), "Codex CLI is not ready.");
    assert_eq!(card("Codex CLI is not ready.", "   \n"), "Codex CLI is not ready.");
}

#[test]
fn strips_the_terminal_colour_codes_a_runtime_writes_around_its_own_words() {
    let message = card(
        "OpenCode ended without a step that reported it had stopped.",
        &format!("{ESC}[93m{ESC}[1m! {ESC}[0mpermission requested: external_directory; auto-rejecting"),
    );
    assert_eq!(message, "OpenCode ended without a step that reported it had stopped. The runtime's own last word was: ! permission requested: external_directory; auto-rejecting");
    assert!(!message.contains(ESC) && !message.contains("[93m"));
}

#[test]
fn still_finds_the_last_speaking_line_when_the_final_one_is_only_colour_codes() {
    assert_eq!(card("Codex stopped.", &format!("the disk is full\n{ESC}[0m{ESC}[2K")), "Codex stopped. The runtime's own last word was: the disk is full");
}

// its-own-settings-file.test.ts: when a runtime cannot save its own settings file.

const EPERM: &str = r"Error: EPERM: operation not permitted, rename 'C:\Users\<home>\.cursor\cli-config.json.19620.f8d831f1-c13a-42f0-9e58-e03665f51f78.tmp' -> 'C:\Users\<home>\.cursor\cli-config.json'";

fn cursor_said(stderr: &str) -> String {
    card("Cursor Agent ended without a terminal result record.", stderr)
}

#[test]
fn a_locked_settings_file_says_whose_file_it_was() {
    let text = cursor_said(EPERM);
    assert!(text.contains("its own settings file") && text.contains("your home folder"), "{text}");
}

#[test]
fn a_locked_settings_file_says_the_workspace_was_not_what_it_was_denied() {
    assert!(cursor_said(EPERM).contains("nothing in this workspace was denied to it"));
}

#[test]
fn a_locked_settings_file_claims_nothing_about_what_the_run_did_before_it_died() {
    let text = cursor_said(EPERM);
    assert!(!text.contains("Nothing in the workspace has changed") && !text.contains("nothing was lost"));
}

#[test]
fn a_locked_settings_file_names_the_way_forward() {
    assert!(cursor_said(EPERM).contains("again"));
}

#[test]
fn a_locked_settings_file_puts_no_errno_or_paths_on_the_card() {
    let text = cursor_said(EPERM);
    assert!(!text.contains(".tmp") && !text.contains("EPERM"), "{text}");
}

#[test]
fn a_locked_settings_file_keeps_the_host_sentence_it_extends() {
    assert!(cursor_said(EPERM).starts_with("Cursor Agent ended without a terminal result record."));
}

#[test]
fn every_other_last_word_is_left_exactly_as_it_was() {
    assert!(cursor_said("Error: ENOENT: no such file or directory").contains("The runtime's own last word was:"));
    assert!(cursor_said("RetriableError: [resource_exhausted]").contains("out of capacity right now"));
    assert!(cursor_said("Error: EPERM: operation not permitted, rename 'a.txt' -> 'b.txt'").contains("The runtime's own last word was:"));
    assert!(cursor_said("Error: EPERM: operation not permitted, open cli-config.json").contains("The runtime's own last word was:"));
}

// a-provider-error-is-readable-and-said-once.test.tsx, its failureMessage cases.

#[test]
fn a_provider_error_as_the_message_is_said_as_its_sentence() {
    let model_error = r#"{"type":"error","error":{"message":"model 'gpt-6.1-sol' is not enabled in rustponsesapi","type":"invalid_request_error","param":null,"code":null},"status":400}"#;
    assert_eq!(
        failure_message(model_error, None, Some("codex")),
        "OpenAI refused the model gpt-6.1-sol: it is not enabled here right now. Pick another model and send again."
    );
}

#[test]
fn reads_a_json_stderr_cause_without_appending_the_raw_body_to_a_readable_error() {
    let unavailable = r#"{"message":"Service unavailable","status":503}"#;
    assert_eq!(failure_message("The process could not finish.", Some(unavailable), Some("codex")), "OpenAI is overloaded or unavailable right now. Try again later, or pick another model.");
    let model_error = r#"{"type":"error","error":{"message":"model 'gpt-6.1-sol' is not enabled in rustponsesapi","type":"invalid_request_error","param":null,"code":null},"status":400}"#;
    assert_eq!(failure_message(model_error, Some(model_error), Some("codex")), "OpenAI refused the model gpt-6.1-sol: it is not enabled here right now. Pick another model and send again.");
}

// The corners Electron's tests leave to the code, each pinned here.

#[test]
fn a_structured_log_line_is_said_as_its_error_else_its_message() {
    let line = r#"timestamp=2026-09-29 level=ERROR run=abc message="stream error" providerID=x error.error="AI_APICallError: Rate limit \"hit\" now""#;
    assert_eq!(message_of_log_line(line), r#"AI_APICallError: Rate limit "hit" now"#);
    assert_eq!(message_of_log_line(r#"level=WARN error="plain one" message="m""#), "plain one");
    assert_eq!(message_of_log_line(r#"level=INFO message="  just this  ""#), "just this");
    assert_eq!(message_of_log_line(r#"level=INFO message="never closed"#), "never closed");
    assert_eq!(message_of_log_line(r#"level=INFO other="x""#), r#"level=INFO other="x""#);
    // Not a structured line: kept whole.
    assert_eq!(message_of_log_line(r#"error="x" with no level"#), r#"error="x" with no level"#);
    assert_eq!(message_of_log_line("level=error lowercase"), "level=error lowercase");
    // And it reaches the card.
    assert_eq!(card("OpenCode stopped.", &format!("{line}\n")), "OpenCode stopped. The runtime reported that it is out of capacity right now \u{2014} its own limit, not this machine's: AI_APICallError: Rate limit \"hit\" now");
}

#[test]
fn the_same_sentence_on_both_channels_is_said_once() {
    assert_eq!(card("The disk is full.", "the disk is full\n"), "The disk is full.");
    assert_eq!(card("Codex stopped: the disk is full", "the disk is full"), "Codex stopped: the disk is full");
    assert!(same_sentence("A.", "a"));
    assert!(!same_sentence("", "anything"), "an empty message never matches");
    assert!(!same_sentence("one thing", "another"));
}

#[test]
fn a_crlf_stderr_reads_its_last_line_whole() {
    assert_eq!(last_stderr_line(Some("first\r\nlast word\r\n\r\n")), Some("last word".to_owned()));
    assert_eq!(last_stderr_line(None), None);
}

#[test]
fn each_way_a_runtime_says_it_is_out_of_capacity_is_heard() {
    for line in ["insufficient_quota", "quota exceeded", "quota exhausted", "rate limited", "rate-limit exceeded", "Too Many Requests", "HTTP 429"] {
        assert!(card("Stopped.", line).contains("out of capacity right now"), "{line}");
    }
    for message in ["Its plan is used up.", "The quota reached its end: quota reached", "Its usage limit was reached.", "usage limit has been reached"] {
        assert_eq!(card(message, "rate limited"), message, "{message}");
    }
}

#[test]
fn every_runtime_settings_file_name_counts_and_operation_not_permitted_alone_does() {
    for file in ["config.json", "settings.json", "auth.json"] {
        assert!(cursor_said(&format!("rename failed: operation not permitted on {file}")).contains("its own settings file"), "{file}");
    }
}

#[test]
fn a_locked_settings_file_is_heard_in_any_case_and_from_eperm_alone() {
    assert!(cursor_said("Error: EPERM: operation not permitted, Rename over cli-config.json").contains("its own settings file"));
    assert!(cursor_said("EPERM on rename of cli-config.json").contains("its own settings file"));
}

#[test]
fn a_log_lines_nested_error_wins_over_an_outer_one_written_first() {
    assert_eq!(message_of_log_line(r#"level=ERROR error="outer" error.error="inner""#), "inner");
}

#[test]
fn the_same_sentence_allows_for_a_trailing_stop_and_a_message_inside_the_line() {
    assert_eq!(card("Done.", "done!"), "Done.");
    assert_eq!(card("disk is full", "error: the disk is full now"), "disk is full");
}
