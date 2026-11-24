//! Electron's `main/a-denial-can-say-why.test.ts` (6 cases, 0.374): a bare "denied" left the teammate to guess, and the
//! guess was usually the same action by another route. The person may say why, and it reaches the teammate by whatever
//! that runtime can carry: in OpenCode's reject message, and as the next input to a Codex run, whose reply has no room
//! for it (`take_denied_saying`, which codex.rs sends as `turn/steer`; see `codex_denial_reason_tests.rs`).
use super::*;

fn deny(id: &str, reason: Option<&str>) -> Answer {
    Answer::Decision { approval_id: id.into(), decision: Decision::Deny, reason: reason.map(str::to_owned) }
}

/// The answer as the channel took it, for comparing: (decision word, reason).
fn taken(raw: Value) -> (String, Option<String>) {
    match approval_answer_from(&raw).expect("an answer") {
        Answer::Decision { approval_id, decision, reason } => {
            assert_eq!(approval_id, "a");
            (protocol_decision_for(&decision).to_owned(), reason)
        }
        Answer::Answers { .. } => panic!("a decision was read as answers"),
    }
}

/// One `rm -rf build` card raised by `runtime` on run_1 / mission_1, answered `answer`; the reply the runtime gets.
fn denied_on(runtime: &'static str, reason: Option<&'static str>) -> (Arc<Channel>, Value) {
    let channel = Channel::new();
    let ch = channel.clone();
    let asking = std::thread::spawn(move || {
        ch.ask_codex_with_changes(
            "run_1",
            "mission_1",
            runtime,
            "item/commandExecution/requestApproval",
            &json!({"command": "rm -rf build", "cwd": "/work"}),
            json!(1),
            None,
            &|| false,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let card = loop {
        if let Some(card) = channel.waiting().first().cloned() {
            break card;
        }
        assert!(Instant::now() < deadline, "no card was raised");
        std::thread::sleep(Duration::from_millis(5));
    };
    channel.decide(deny(&card.approval_id, reason)).unwrap();
    let reply = asking.join().unwrap().unwrap();
    (channel, reply)
}

// case: keeps a denial’s reason, in one line and bounded, and nothing when there is none
#[test]
fn keeps_a_denials_reason_in_one_line_and_bounded_and_nothing_when_there_is_none() {
    let _rules = crate::native::hooks::no_saved_rules();
    assert_eq!(
        taken(json!({"approvalId": "a", "decision": "deny", "reason": "  use the\nbuild script instead  "})),
        ("reject".into(), Some("use the build script instead".into()))
    );
    assert_eq!(taken(json!({"approvalId": "a", "decision": "deny", "reason": "   "})), ("reject".into(), None));
    assert_eq!(taken(json!({"approvalId": "a", "decision": "deny"})), ("reject".into(), None));
    let long = taken(json!({"approvalId": "a", "decision": "deny", "reason": "x".repeat(2_000)})).1.unwrap();
    assert_eq!(long.encode_utf16().count(), MAX_DENY_REASON);
    assert_eq!(MAX_DENY_REASON, 500);
    // The channel keeps the same rule for every answer it is given, whoever read it from the window.
    let (channel, _) = denied_on("codex", Some("  use the\nbuild script instead  "));
    assert_eq!(channel.take_denied_saying("run_1")[0].reason, "use the build script instead");
}

// case: never lets a reason ride an approval, or turn a malformed answer into anything but a denial
#[test]
fn never_lets_a_reason_ride_an_approval_or_turn_a_malformed_answer_into_anything_but_a_denial() {
    let _rules = crate::native::hooks::no_saved_rules();
    assert_eq!(taken(json!({"approvalId": "a", "decision": "approve-once", "reason": "sure"})), ("accept".into(), None));
    assert_eq!(taken(json!({"approvalId": "a", "decision": "yes please", "reason": "no"})), ("reject".into(), Some("no".into())));
    // An approval given a reason anyway reaches the runtime's adapter as a plain approval, and nothing is steered.
    let channel = Channel::new();
    let ch = channel.clone();
    let asking = std::thread::spawn(move || {
        let request = Request { approval_id: new_id(), mission_id: "mission_1".into(), runtime: "codex".into(), kind: Kind::Command, summary: "Run ls".into(), detail: "ls".into(), cwd: Some("/work".into()), requested_at: now_iso(), always_covers: None, always_key: None, data_sent_says: None, reversible_says: None, patch: None, skippable: false, answer_in: None, claude_input: None, questions: Vec::new() };
        ch.ask_by_run(request, "run_1", None, true, &|| false).map(|(answer, _)| answer)
    });
    let card = loop {
        if let Some(card) = channel.waiting().first().cloned() {
            break card;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let recorded = channel
        .decide(Answer::Decision { approval_id: card.approval_id, decision: Decision::ApproveOnce, reason: Some("sure".into()) })
        .unwrap();
    match asking.join().unwrap().unwrap() {
        Answer::Decision { decision: Decision::ApproveOnce, reason, .. } => assert_eq!(reason, None),
        other => panic!("not a plain approval: {other:?}"),
    }
    assert_eq!(recorded.words, None);
    assert_eq!(channel.take_denied_saying("run_1"), vec![]);
}

// case: on Codex: the reply is the plain protocol reject, and the reason goes to the run as its next input
#[test]
fn on_codex_the_reply_is_the_plain_protocol_reject_and_the_reason_goes_to_the_run_as_its_next_input() {
    let _rules = crate::native::hooks::no_saved_rules();
    let (channel, reply) = denied_on("codex", Some("use the build script instead"));
    // Exactly what Codex's app-server expects: no field it does not know.
    assert_eq!(reply, json!({"decision": "reject"}));
    // Only to the run that asked.
    assert_eq!(channel.take_denied_saying("run_other"), vec![]);
    assert_eq!(
        channel.take_denied_saying("run_1"),
        vec![DeniedSaying { run_id: "run_1".into(), mission_id: "mission_1".into(), reason: "use the build script instead".into() }]
    );
    // Shown once: taken, it is gone.
    assert_eq!(channel.take_denied_saying("run_1"), vec![]);
}

// case: on OpenCode: the reason rides the reject itself, and nothing is steered
#[test]
fn on_opencode_the_reason_rides_the_reject_itself_and_nothing_is_steered() {
    let _rules = crate::native::hooks::no_saved_rules();
    let (channel, reply) = denied_on("opencode", Some("use the build script instead"));
    assert_eq!(
        open_code_reply_for(&reply),
        json!({"reply": "reject", "message": denied_saying("use the build script instead")})
    );
    assert_eq!(channel.take_denied_saying("run_1"), vec![]);
}

// case: without a reason: a plain reject everywhere, and nothing steered
#[test]
fn without_a_reason_a_plain_reject_everywhere_and_nothing_steered() {
    let _rules = crate::native::hooks::no_saved_rules();
    let (codex, on_codex) = denied_on("codex", None);
    assert_eq!(on_codex, json!({"decision": "reject"}));
    let (open, on_open) = denied_on("opencode", None);
    assert_eq!(open_code_reply_for(&on_open), json!("reject"));
    // Only spaces is no reason at all.
    let (blank, on_blank) = denied_on("codex", Some("   "));
    assert_eq!(on_blank, json!({"decision": "reject"}));
    assert_eq!(codex.take_denied_saying("run_1"), vec![]);
    assert_eq!(open.take_denied_saying("run_1"), vec![]);
    assert_eq!(blank.take_denied_saying("run_1"), vec![]);
}

// case: reads, to the runtime, as the person’s words
#[test]
fn reads_to_the_runtime_as_the_persons_words() {
    let _rules = crate::native::hooks::no_saved_rules();
    assert_eq!(denied_saying("use the build script instead"), "The person declined this, and said: use the build script instead");
}

/// A reason waits only while its run lasts.
#[test]
fn a_reason_not_yet_shown_goes_with_its_run() {
    let _rules = crate::native::hooks::no_saved_rules();
    let (channel, _) = denied_on("codex", Some("use the build script instead"));
    channel.release("mission_1");
    assert_eq!(channel.take_denied_saying("run_1"), vec![]);
}
