//! Electron one-path-decides-every-request.test.ts, "gives Codex the exact command key and the same rule, Always, and
//! reach verdicts": Codex's Always is kept by Locust under the exact action, answered to Codex as once, so a saved rule
//! still decides the next request.
use super::*;
use std::time::{Duration, Instant};

fn answer_first(ch: &Channel, decision: Decision) -> Request {
    let end = Instant::now() + Duration::from_secs(5);
    while ch.waiting().is_empty() {
        assert!(Instant::now() < end, "no card came");
        std::thread::sleep(Duration::from_millis(5));
    }
    let card = ch.waiting()[0].clone();
    ch.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision, reason: None }).unwrap();
    card
}

fn ask(ch: &std::sync::Arc<Channel>, command: &str) -> std::thread::JoinHandle<Result<Value, String>> {
    let worker = ch.clone();
    let command = command.to_owned();
    let deadline = Instant::now() + Duration::from_secs(2);
    std::thread::spawn(move || {
        worker.ask_codex("run_1", "mission_1", "item/commandExecution/requestApproval", &json!({"command": command}), json!(1), &|| Instant::now() >= deadline)
    })
}

#[test]
fn a_codex_command_is_kept_by_its_exact_text_and_a_change_by_its_paths() {
    assert_eq!(
        codex_always_for(&Kind::Command, &json!({"command": "git push origin main"}), None),
        Some(("codex:command:git push origin main".into(), "this same command again".into()))
    );
    assert_eq!(codex_always_for(&Kind::Command, &json!({"command": ""}), None), None);
    let changes = json!({"changes": [{"path": "src/b.ts"}, {"path": "src/a.ts", "movePath": "src/c.ts"}]});
    assert_eq!(
        codex_always_for(&Kind::FileChange, &changes, None),
        Some(("codex:files:[\"src/a.ts\",\"src/b.ts\",\"src/c.ts\"]".into(), "the same change to these files again".into()))
    );
    assert_eq!(codex_always_for(&Kind::FileChange, &json!({"changes": [{"path": ""}]}), None), None);
    assert_eq!(codex_always_for(&Kind::FileChange, &json!({"changes": []}), None), None);
    assert_eq!(codex_always_for(&Kind::Question, &json!({"command": "x"}), None), None);
}

#[test]
fn an_always_on_a_codex_command_is_answered_once_and_kept_by_locust_for_that_command_alone() {
    let ch = Channel::new();
    let first = ask(&ch, "git push origin main");
    let card = answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(card.always_key.as_deref(), Some("codex:command:git push origin main"));
    assert_eq!(card.always_covers.as_deref(), Some("this same command again"));
    // Codex is told once, never "acceptForSession": it keeps asking, and Locust answers.
    assert_eq!(first.join().unwrap().unwrap(), json!({"decision": "accept"}));
    // The same command again is allowed by the earlier Always, with no card.
    assert_eq!(ask(&ch, "git push origin main").join().unwrap().unwrap(), json!({"decision": "accept"}));
    assert_eq!(ch.take_recorded_for("mission_1").last().unwrap().by, "earlier-always");
    // Another command still asks.
    let other = ask(&ch, "git push origin release");
    let asked = answer_first(&ch, Decision::Deny);
    assert_eq!(asked.always_key.as_deref(), Some("codex:command:git push origin release"));
    assert_eq!(other.join().unwrap().unwrap(), json!({"decision": "reject"}));
}

#[test]
fn a_saved_rule_still_denies_a_codex_command_after_always() {
    let root = std::env::temp_dir().join(format!("codex-always-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let _hook = crate::native::hooks::install(store.clone(), None);
    let ch = Channel::new();
    ch.register_teammate("mission_1", "tm_one");
    let first = ask(&ch, "npm publish");
    answer_first(&ch, Decision::ApproveAlways);
    assert_eq!(first.join().unwrap().unwrap(), json!({"decision": "accept"}));
    crate::approval_rule_store::add(
        &store,
        crate::approval_rules::ApprovalRuleDraft {
            effect: crate::approval_rules::RuleEffect::Deny,
            kind: crate::approval_rules::RuleKind::Command,
            pattern: "npm publish".into(),
            teammate_id: Some("tm_one".into()),
            folder: None,
        },
    )
    .unwrap();
    let again = ask(&ch, "npm publish").join().unwrap().unwrap();
    assert_eq!(again["decision"], "reject", "{again}");
    assert_eq!(ch.take_recorded_for("mission_1").last().unwrap().by, "saved-rule");
    let _ = std::fs::remove_dir_all(&root);
}
