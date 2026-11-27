//! Parity tests for `apps/desktop/src/main/one-path-decides-every-request.test.ts` (15 cases).

use super::*;
use crate::approval_patch::{FileChangeKind, FileChangeRecord};
use crate::approval_rules::{ApprovalRuleDraft, RuleEffect, RuleKind};
use std::sync::Arc;
use std::time::{Duration, Instant};

const FOLDER: &str = "/work/shop";

struct TempRules {
    root: std::path::PathBuf,
    store: crate::store::Store,
    _guard: crate::native::hooks::Guard,
}

impl TempRules {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "one-path-{tag}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let guard = crate::native::hooks::install(store.clone(), None);
        Self { root, store, _guard: guard }
    }

    fn add_rule(&self, effect: RuleEffect, pattern: &str) {
        crate::approval_rule_store::add(
            &self.store,
            ApprovalRuleDraft {
                effect,
                kind: RuleKind::Command,
                pattern: pattern.into(),
                teammate_id: None,
                folder: Some(FOLDER.into()),
            },
        )
        .unwrap();
    }
}

impl Drop for TempRules {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn wait_card(ch: &Channel) -> Request {
    let end = Instant::now() + Duration::from_secs(2);
    while ch.waiting().is_empty() {
        assert!(Instant::now() < end, "no card came");
        std::thread::sleep(Duration::from_millis(5));
    }
    ch.waiting()[0].clone()
}

fn answer_first(ch: &Channel, decision: Decision) -> Request {
    let card = wait_card(ch);
    ch.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision, reason: None }).unwrap();
    card
}

fn ask_claude_bash(ch: &Arc<Channel>, mission: &str, cmd: &str) -> std::thread::JoinHandle<Result<Value, String>> {
    let worker = ch.clone();
    let mission = mission.to_owned();
    let cmd = cmd.to_owned();
    let deadline = Instant::now() + Duration::from_secs(2);
    std::thread::spawn(move || {
        worker.ask_claude("run_claude", &mission, "req_1", "Bash", &json!({"command": cmd}), FOLDER, &|| Instant::now() >= deadline)
    })
}

fn ask_codex_cmd(ch: &Arc<Channel>, mission: &str, cmd: &str) -> std::thread::JoinHandle<Result<Value, String>> {
    let worker = ch.clone();
    let mission = mission.to_owned();
    let cmd = cmd.to_owned();
    let deadline = Instant::now() + Duration::from_secs(2);
    std::thread::spawn(move || {
        worker.ask_codex(
            "run_codex",
            &mission,
            "item/commandExecution/requestApproval",
            &json!({"command": cmd, "cwd": FOLDER}),
            json!(1),
            &|| Instant::now() >= deadline,
        )
    })
}

// case: denies by a saved rule, whatever Always was given earlier in the run
#[test]
fn denies_by_a_saved_rule_whatever_always_was_given_earlier_in_the_run() {
    let rules = TempRules::new("deny-over-always");
    rules.add_rule(RuleEffect::Deny, "git push:*");
    let ch = Channel::new();
    ch.register_teammate("m1", "tm_wren");

    // Give Always on Claude Bash first.
    let seed = ask_claude_bash(&ch, "m1", "echo ok");
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(seed.join().unwrap().unwrap()["behavior"], "allow");
    let _ = ch.take_recorded_for("m1");

    // Simple command matching the deny rule: denied by saved-rule despite earlier Always.
    let direct = ask_claude_bash(&ch, "m1", "git push origin main").join().unwrap().unwrap();
    assert_eq!(direct["behavior"], "deny");
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule");

    // Compound command containing the denied part: also denied by saved-rule.
    let compound = ask_claude_bash(&ch, "m1", "npm run build && git push origin main").join().unwrap().unwrap();
    assert_eq!(compound["behavior"], "deny");
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule");
}

// case: asks again about a command that reaches other programs, though Always was given, and says why
#[test]
fn asks_again_about_a_command_that_reaches_other_programs_though_always_was_given_and_says_why() {
    let _rules = crate::native::hooks::no_saved_rules();
    let ch = Channel::new();
    let seed = ask_claude_bash(&ch, "m1", "echo ok");
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(seed.join().unwrap().unwrap()["behavior"], "allow");

    // A reaching command must ask again (not auto-allow) and explain why.
    let reaching = ask_claude_bash(&ch, "m1", "taskkill /F /IM python.exe");
    let card = wait_card(&ch);
    let why = asked_again_of(&card.approval_id).expect("asked_again_of reason");
    assert!(
        why.starts_with("Asked again, though you chose Always earlier in this run: this command stops every python.exe"),
        "unexpected why: {why}"
    );
    ch.decide(Answer::Decision { approval_id: card.approval_id, decision: Decision::Deny, reason: None }).unwrap();
    assert_eq!(reaching.join().unwrap().unwrap()["behavior"], "deny");
}

// case: allows by an earlier Always what it covers, and says so
#[test]
fn allows_by_an_earlier_always_what_it_covers_and_says_so() {
    let rules = TempRules::new("earlier-always");
    rules.add_rule(RuleEffect::Deny, "git push:*");
    let ch = Channel::new();
    ch.register_teammate("m1", "tm_wren");

    let seed = ask_claude_bash(&ch, "m1", "echo one");
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(seed.join().unwrap().unwrap()["behavior"], "allow");
    let _ = ch.take_recorded_for("m1");

    // Simple command covered by earlier Always.
    let two = ask_claude_bash(&ch, "m1", "echo two").join().unwrap().unwrap();
    assert_eq!(two["behavior"], "allow");
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "earlier-always");

    // Compound command with no part a rule forbids.
    let compound = ask_claude_bash(&ch, "m1", "npm run build && npm test").join().unwrap().unwrap();
    assert_eq!(compound["behavior"], "allow");
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "earlier-always");

    // File change covered by earlier Always on Edit.
    let worker = ch.clone();
    let edit1 = std::thread::spawn(move || {
        worker.ask_claude("run_claude", "m1", "req_e1", "Edit", &json!({"file_path": "src/a.ts"}), FOLDER, &|| false)
    });
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(edit1.join().unwrap().unwrap()["behavior"], "allow");
    let _ = ch.take_recorded_for("m1");

    let deadline = Instant::now() + Duration::from_secs(2);
    let edit2 = ch.ask_claude("run_claude", "m1", "req_e2", "Edit", &json!({"file_path": "src/app.ts"}), FOLDER, &|| Instant::now() >= deadline).unwrap();
    assert_eq!(edit2["behavior"], "allow");
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "earlier-always");
}

// case: credits a saved rule that allows over an Always that would have
#[test]
fn credits_a_saved_rule_that_allows_over_an_always_that_would_have() {
    let rules = TempRules::new("allow-rule-over-always");
    rules.add_rule(RuleEffect::Allow, "npm test");
    let ch = Channel::new();
    ch.register_teammate("m1", "tm_wren");

    let seed = ask_claude_bash(&ch, "m1", "echo seed");
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(seed.join().unwrap().unwrap()["behavior"], "allow");
    let _ = ch.take_recorded_for("m1");

    let res = ask_claude_bash(&ch, "m1", "npm test").join().unwrap().unwrap();
    assert_eq!(res["behavior"], "allow");
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule");
}

// case: never governs a question: the person answers it, Always or not
#[test]
fn never_governs_a_question_the_person_answers_it_always_or_not() {
    let rules = TempRules::new("question-always-asks");
    rules.add_rule(RuleEffect::Allow, "npm test");
    rules.add_rule(RuleEffect::Deny, "git push:*");
    let ch = Channel::new();

    // Even if the mission had an Always on a key, a question still asks the person.
    {
        let mut guard = crate::invariants::lock(&ch.inner);
        guard.always.add("m1", "claude:Bash");
    }
    let worker = ch.clone();
    let q = std::thread::spawn(move || {
        worker.ask_codex(
            "run_codex",
            "m1",
            "item/tool/requestUserInput",
            &json!({"questions": [{"id": "q1", "question": "Which branch?", "header": "Branch", "options": [{"label": "main", "description": "main"}]}]}),
            json!(1),
            &|| false,
        )
    });
    let card = wait_card(&ch);
    assert_eq!(card.kind, Kind::Question);
    ch.decide(Answer::Answers {
        approval_id: card.approval_id,
        answers: vec![("q1".into(), vec!["main".into()])],
    })
    .unwrap();
    let reply = q.join().unwrap().unwrap();
    assert_eq!(reply["answers"]["q1"]["answers"], json!(["main"]));
}

// case: asks when nothing decides, with the rules' own reason when they have one
#[test]
fn asks_when_nothing_decides_with_the_rules_own_reason_when_they_have_one() {
    let rules = TempRules::new("asks-with-why");
    rules.add_rule(RuleEffect::Allow, "npm test");
    let ch = Channel::new();

    // Plain command with no rule: asks with no special why.
    let ls = ask_claude_bash(&ch, "m1", "ls");
    let card = wait_card(&ch);
    assert_eq!(asked_again_of(&card.approval_id), None);
    ch.decide(Answer::Decision { approval_id: card.approval_id, decision: Decision::ApproveOnce, reason: None }).unwrap();
    assert_eq!(ls.join().unwrap().unwrap()["behavior"], "allow");

    // Compound command where an allow rule only matched part: asks with the compound refusal reason.
    let compound = ask_claude_bash(&ch, "m1", "npm test; rm -rf build");
    let card = wait_card(&ch);
    let why = asked_again_of(&card.approval_id).expect("compound refusal why");
    assert!(why.contains("more than one simple command"), "unexpected why: {why}");
    ch.decide(Answer::Decision { approval_id: card.approval_id, decision: Decision::Deny, reason: None }).unwrap();
    assert_eq!(compound.join().unwrap().unwrap()["behavior"], "deny");
}

// case: ${path}: denied by the rule, Always or not; allowed by an allowing rule; by an Always; else asked
#[test]
fn same_action_gets_the_same_verdict_on_every_path() {
    const CMD: &str = "git push origin main";

    fn run_path(ch: &Arc<Channel>, path: &str, mission: &str) -> std::thread::JoinHandle<Result<bool, String>> {
        let ch = ch.clone();
        let path = path.to_owned();
        let mission = mission.to_owned();
        let deadline = Instant::now() + Duration::from_secs(2);
        std::thread::spawn(move || match path.as_str() {
            "claude" => ch
                .ask_claude("run_claude", &mission, "req_1", "Bash", &json!({"command": CMD}), FOLDER, &|| Instant::now() >= deadline)
                .map(|v| v["behavior"] == "allow"),
            "codex" => ch
                .ask_codex(
                    "run_codex",
                    &mission,
                    "item/commandExecution/requestApproval",
                    &json!({"command": CMD, "cwd": FOLDER}),
                    json!(1),
                    &|| Instant::now() >= deadline,
                )
                .map(|v| v["decision"] == "accept" || v["decision"] == "acceptForSession"),
            "opencode" => {
                let req = open_code_permission_request(
                    &json!({"permission": "bash", "patterns": [CMD], "always": ["git push *"], "metadata": {"command": CMD}}),
                    FOLDER,
                );
                ch.ask_codex_with_changes("run_oc", &mission, "opencode", req["method"].as_str().unwrap(), &req["params"], json!(1), None, &|| Instant::now() >= deadline)
                    .map(|v| v["decision"] == "accept" || v["decision"] == "acceptForSession")
            }
            "copilot" => ch.ask_acp_runtime(
                "copilot",
                "run_cp",
                &mission,
                &json!({"toolCall": {"toolCallId": "t1", "title": "Push", "kind": "execute", "rawInput": {"command": CMD}}}),
                FOLDER,
                &|| Instant::now() >= deadline,
            ),
            other => panic!("unknown path {other}"),
        })
    }

    for path in ["claude", "codex", "opencode", "copilot"] {
        // 1. Denied by rule without Always.
        {
            let rules = TempRules::new(&format!("{path}-deny"));
            rules.add_rule(RuleEffect::Deny, "git push:*");
            let ch = Channel::new();
            assert!(!run_path(&ch, path, "m1").join().unwrap().unwrap(), "{path} deny without always");
            assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule", "{path}");
        }
        // 2. Allowed by Always, then denied by rule despite Always.
        {
            let rules = TempRules::new(&format!("{path}-always-then-deny"));
            let ch = Channel::new();
            let first = run_path(&ch, path, "m1");
            answer_first(&ch, Decision::ApproveAlways);
            assert!(first.join().unwrap().unwrap(), "{path} seed always");
            let _ = ch.take_recorded_for("m1");

            // Covered by earlier Always.
            assert!(run_path(&ch, path, "m1").join().unwrap().unwrap(), "{path} earlier always");
            assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "earlier-always", "{path}");

            // Now add deny rule: rule wins over earlier Always.
            rules.add_rule(RuleEffect::Deny, "git push:*");
            assert!(!run_path(&ch, path, "m1").join().unwrap().unwrap(), "{path} deny over always");
            assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule", "{path}");
        }
        // 3. Allowed by allowing rule.
        {
            let rules = TempRules::new(&format!("{path}-allow"));
            rules.add_rule(RuleEffect::Allow, CMD);
            let ch = Channel::new();
            assert!(run_path(&ch, path, "m1").join().unwrap().unwrap(), "{path} allow rule");
            assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule", "{path}");
        }
        // 4. Else asked on a card.
        {
            let _rules = crate::native::hooks::no_saved_rules();
            let ch = Channel::new();
            let asked = run_path(&ch, path, "m1");
            let card = answer_first(&ch, Decision::ApproveOnce);
            assert_eq!(card.kind, Kind::Command, "{path}");
            assert!(asked.join().unwrap().unwrap(), "{path} card approve");
        }
    }
}

// case: is a key the host puts on the request: a tool, an exact command, or OpenCode's own patterns
#[test]
fn always_key_is_put_on_the_request_by_each_host() {
    let opencode = open_code_permission_request(
        &json!({"permission": "bash", "patterns": ["echo SERVED"], "always": ["echo *"], "metadata": {"command": "echo SERVED"}}),
        FOLDER,
    );
    assert_eq!(opencode["params"]["locustAlwaysKey"], "opencode:bash:echo *");

    let (_method, acp) = acp_permission_request(
        &json!({"toolCall": {"toolCallId": "t1", "title": "Test", "kind": "execute", "rawInput": {"command": "npm test"}}}),
        FOLDER,
    );
    assert_eq!(acp["locustAlwaysKey"], "acp:execute:npm test");
    assert_eq!(acp["locustCard"]["alwaysCovers"], "this same command again");

    let bare = open_code_permission_request(&json!({"permission": "bash", "patterns": [], "metadata": {}}), FOLDER);
    assert!(bare["params"].get("locustAlwaysKey").is_none());
}

// case: gives Codex the exact command key and the same rule, Always, and reach verdicts
#[test]
fn gives_codex_the_exact_command_key_and_same_rule_always_and_reach_verdicts() {
    let rules = TempRules::new("codex-verdicts");
    let ch = Channel::new();
    let first = ask_codex_cmd(&ch, "m1", "git push origin main");
    let card = wait_card(&ch);
    assert_eq!(card.always_key.as_deref(), Some("codex:command:git push origin main"));
    assert_eq!(card.always_covers.as_deref(), Some("this same command again"));
    ch.decide(Answer::Decision { approval_id: card.approval_id, decision: Decision::ApproveAlways, reason: None }).unwrap();
    assert_eq!(first.join().unwrap().unwrap(), json!({"decision": "accept"}));
    let _ = ch.take_recorded_for("m1");

    // Earlier Always allows the same command.
    assert_eq!(ask_codex_cmd(&ch, "m1", "git push origin main").join().unwrap().unwrap(), json!({"decision": "accept"}));
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "earlier-always");

    // Saved allow rule takes credit over earlier Always.
    rules.add_rule(RuleEffect::Allow, "git push origin main");
    assert_eq!(ask_codex_cmd(&ch, "m1", "git push origin main").join().unwrap().unwrap(), json!({"decision": "accept"}));
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule");

    // Saved deny rule overrides both.
    rules.add_rule(RuleEffect::Deny, "git push:*");
    assert_eq!(ask_codex_cmd(&ch, "m1", "git push origin main").join().unwrap().unwrap(), json!({"decision": "decline"}));
    assert_eq!(ch.take_recorded_for("m1").last().unwrap().by, "saved-rule");

    // Reaching command asks again even if the key was somehow remembered.
    drop(rules);
    let _no_rules = crate::native::hooks::no_saved_rules();
    let ch2 = Channel::new();
    let reach_cmd = "taskkill /IM locust-no-such-program.exe";
    {
        let mut guard = crate::invariants::lock(&ch2.inner);
        guard.always.add("m2", &format!("codex:command:{reach_cmd}"));
    }
    let reaches = ask_codex_cmd(&ch2, "m2", reach_cmd);
    let rcard = wait_card(&ch2);
    assert_eq!(rcard.always_key.as_deref(), Some("codex:command:taskkill /IM locust-no-such-program.exe"));
    let why = asked_again_of(&rcard.approval_id).expect("reach asked again");
    assert!(why.contains("Asked again"), "got: {why}");
    ch2.release("m2");
    let _ = reaches.join().unwrap();
}

// case: answers Always on an identified Codex command as accept, including the implicit Codex route
#[test]
fn answers_always_on_an_identified_codex_command_as_accept() {
    let _rules = crate::native::hooks::no_saved_rules();
    let ch = Channel::new();
    let waiting = ask_codex_cmd(&ch, "m1", "node --version");
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(waiting.join().unwrap().unwrap(), json!({"decision": "accept"}));
}

// case: keeps acceptForSession for a keyless Codex Always
#[test]
fn keeps_accept_for_session_for_a_keyless_codex_always() {
    let _rules = crate::native::hooks::no_saved_rules();
    for method in ["item/commandExecution/requestApproval", "item/fileChange/requestApproval"] {
        let ch = Channel::new();
        let worker = ch.clone();
        let m = method.to_owned();
        let waiting = std::thread::spawn(move || {
            worker.ask_codex("run_codex", "m1", &m, &json!({}), json!(1), &|| false)
        });
        let card = answer_first(&ch, Decision::ApproveAlways);
        assert_eq!(card.always_key, None, "{method}");
        assert_eq!(card.always_covers, None, "{method}");
        assert_eq!(waiting.join().unwrap().unwrap(), json!({"decision": "acceptForSession"}), "{method}");
    }
}

// case: keys Codex file changes by sorted paths, names the scope, and answers Always once
#[test]
fn keys_codex_file_changes_by_sorted_paths_names_scope_and_answers_always_once() {
    let _rules = crate::native::hooks::no_saved_rules();
    let change = |path: &str| FileChangeRecord {
        path: path.into(),
        kind: FileChangeKind::Update,
        move_path: None,
        diff: "@@ -1 +1 @@\n-old\n+new".into(),
    };
    for paths in [["z.ts", "a.ts"], ["a.ts", "z.ts"]] {
        let ch = Channel::new();
        let worker = ch.clone();
        let list: Vec<FileChangeRecord> = paths.iter().map(|p| change(p)).collect();
        let by_item = std::sync::Mutex::new(std::collections::HashMap::from([("item_files".to_owned(), list)]));
        let waiting = std::thread::spawn(move || {
            worker.ask_codex_with_changes(
                "run_codex",
                "m1",
                "codex",
                "item/fileChange/requestApproval",
                &json!({"itemId": "item_files"}),
                json!(1),
                Some(&by_item),
                &|| false,
            )
        });
        let card = answer_first(&ch, Decision::ApproveAlways);
        assert_eq!(card.always_key.as_deref(), Some(r#"codex:files:["a.ts","z.ts"]"#));
        assert_eq!(card.always_covers.as_deref(), Some("the same change to these files again"));
        assert_eq!(waiting.join().unwrap().unwrap(), json!({"decision": "accept"}));
    }
}

// case: keeps long Codex command keys exact instead of sharing a truncated prefix
#[test]
fn keeps_long_codex_command_keys_exact_instead_of_truncating() {
    let _rules = crate::native::hooks::no_saved_rules();
    let cmd = format!("{} last-argument", "x".repeat(2_100));
    let ch = Channel::new();
    let waiting = ask_codex_cmd(&ch, "m1", &cmd);
    let card = wait_card(&ch);
    assert_eq!(card.always_key.as_deref(), Some(format!("codex:command:{cmd}").as_str()));
    ch.release("m1");
    let _ = waiting.join().unwrap();
}

// case: is kept per run, by the main process, bounded
#[test]
fn run_always_is_kept_per_run_bounded() {
    let mut always = RunAlways::new(2, 2);
    always.add("run_a", "claude:Bash");
    assert!(always.has("run_a", "claude:Bash"));
    assert!(!always.has("run_b", "claude:Bash"));
    always.add("run_a", "claude:Edit");
    always.add("run_a", "claude:Write");
    assert!(!always.has("run_a", "claude:Write"));
    always.remove("run_a", "claude:Edit");
    assert!(!always.has("run_a", "claude:Edit"));
    always.add("run_b", "x");
    always.add("run_c", "y");
    assert!(!always.has("run_a", "claude:Bash"));
    assert!(always.has("run_c", "y"));
}

// case: is decided in main before any card, and remembered there before the answer is released
#[test]
fn decided_before_any_card_and_remembered_before_answer_is_released() {
    let rules = TempRules::new("before-card-and-release");
    rules.add_rule(RuleEffect::Allow, "npm test");
    let ch = Channel::new();

    // Decided before any card is queued: pending stays empty and ask returns immediately.
    let deadline = Instant::now() + Duration::from_secs(2);
    let res = ch.ask_claude("run_claude", "m1", "req_1", "Bash", &json!({"command": "npm test"}), FOLDER, &|| Instant::now() >= deadline).unwrap();
    assert_eq!(res["behavior"], "allow");
    assert!(ch.waiting().is_empty());

    // Remembered in Channel before the answer is released to the waiting caller:
    // as soon as ask_codex returns, the next identical request is answered immediately by earlier-always.
    let first = ask_codex_cmd(&ch, "m2", "echo hi");
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(first.join().unwrap().unwrap(), json!({"decision": "accept"}));
    let deadline2 = Instant::now() + Duration::from_secs(2);
    let second = ch
        .ask_codex("run_codex", "m2", "item/commandExecution/requestApproval", &json!({"command": "echo hi", "cwd": FOLDER}), json!(2), &|| Instant::now() >= deadline2)
        .unwrap();
    assert_eq!(second, json!({"decision": "accept"}));
    assert!(ch.waiting().is_empty());
    assert_eq!(ch.take_recorded_for("m2").last().unwrap().by, "earlier-always");
}
