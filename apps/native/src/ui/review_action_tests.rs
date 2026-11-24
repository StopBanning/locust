//! Headless window tests for asking a reviewer (or a pair) for a review from the conversation's More actions menu:
//! assembles `ReviewMaterial` from the conversation, builds the brief with `review_brief::review_brief`, forces read-only
//! Ask mode via `as_reviewer`, and starts the review turn(s) as the reviewer(s).
use super::*;
use rune::{Env, Runtime};
use std::sync::{Arc, Mutex};

struct RecordedSend {
    teammate_id: String,
    conversation: Option<String>,
    prompt: String,
    model: Option<String>,
    mode: Option<Mode>,
    runtime: Option<String>,
}

static TEST_LOCK: Mutex<()> = Mutex::new(());

struct HookGuard {
    sent: Arc<Mutex<Vec<RecordedSend>>>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        *SEND_HOOK.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

fn capture_sends() -> HookGuard {
    let lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sent = Arc::new(Mutex::new(Vec::new()));
    let sink = sent.clone();
    let seq = Arc::new(std::sync::atomic::AtomicUsize::new(1));
    *SEND_HOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |job, origin| {
        let n = seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        sink.lock().unwrap_or_else(|e| e.into_inner()).push(RecordedSend {
            teammate_id: job.teammate.id.clone(),
            conversation: job.conversation.clone(),
            prompt: job.prompt.clone(),
            model: job.model.clone(),
            mode: origin.mode,
            runtime: origin.runtime.clone(),
        });
        Ok(engine::Started {
            conversation_id: format!("review_conv_{n}"),
            mode: "ask".into(),
        })
    }));
    HookGuard { sent, _lock: lock }
}

fn teammate(id: &str, name: &str, runtime: &str, model: Option<&str>, runs_in: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Custom".into(),
        model: model.map(str::to_owned),
        runtime: Some(runtime.into()),
        folder: Some("/tmp/shop".into()),
        runs_in: runs_in.into(),
        account: None,
        busy: false,
    }
}

fn tool_edit(id: &str, path: &str, failed: bool) -> ledger::Item {
    ledger::Item::Tool {
        name: "file_change".into(),
        status: if failed { ledger::Status::Failed } else { ledger::Status::Done },
        summary: path.into(),
        detail: Box::new(ledger::ToolDetail {
            id: id.into(),
            arguments: path.into(),
            ..Default::default()
        }),
    }
}

fn tool_shell(id: &str, command: &str, exit_code: Option<i64>) -> ledger::Item {
    ledger::Item::Tool {
        name: "shell".into(),
        status: if exit_code == Some(0) { ledger::Status::Done } else { ledger::Status::Failed },
        summary: command.into(),
        detail: Box::new(ledger::ToolDetail {
            id: id.into(),
            arguments: command.into(),
            exit_code,
            ..Default::default()
        }),
    }
}

fn review_window(others: Vec<Teammate>, items: Vec<ledger::Item>, turn_metas: Vec<ledger::Metadata>) -> Runtime<State> {
    let mut s = State::default();
    let author = teammate("wren", "Wren", "codex", Some("gpt-5.4"), "accept-edits");
    s.teammates.push(author.clone());
    s.conversations.insert("wren".into(), Default::default());
    for mate in others {
        s.conversations.insert(mate.id.clone(), Default::default());
        s.teammates.push(mate);
    }
    let tip = turn_metas.last().map(|m| m.mission_id.clone()).unwrap_or_default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: author,
        conversation: Conversation { conversation_id: Some(tip.clone()), turns: Vec::new() },
        items,
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { tip, turn_metas, ..Default::default() },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    for _ in 0..8 {
        rt.frame(0.05);
    }
    rt
}

fn press(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..5 {
        rt.frame(0.05);
    }
    let (x, y) = rt.find(key).unwrap_or_else(|| {
        panic!(
            "no element {key} on screen: {:?}",
            rt.elements().iter().filter_map(|e| e.key.clone()).collect::<Vec<_>>()
        )
    });
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..5 {
        rt.frame(0.05);
    }
}

fn wait_for_sends(rt: &mut Runtime<State>, sent: &Arc<Mutex<Vec<RecordedSend>>>, expected: usize) {
    for _ in 0..80 {
        rt.frame(0.05);
        if sent.lock().unwrap().len() >= expected && !rt.state.sending.is_pending() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("timed out waiting for {expected} review sends (got {})", sent.lock().unwrap().len());
}

#[test]
fn choosing_a_lone_reviewer_builds_the_brief_from_the_conversation_and_starts_a_read_only_turn() {
    // case: names this turn as this turn, and earlier work as earlier
    // case: does not repeat a path this turn touched again
    // case: names the request, what changed, and every command with its exit code
    let _hook = capture_sends();
    let items = vec![
        ledger::Item::User("Build the export module.".into()),
        tool_edit("e1", "/tmp/shop/src/export.ts\n/tmp/shop/src/export.test.ts", false),
        ledger::Item::Assistant { item_id: "a1".into(), text: "Built export.".into(), r#final: true },
        ledger::Item::User("Add the export button.".into()),
        tool_edit("e2", "/tmp/shop/src/Toolbar.tsx\n/tmp/shop/src/export.ts", false),
        tool_edit("e3", "/tmp/shop/src/broken.ts", true),
        tool_shell("s1", "powershell.exe -Command \"pnpm test\"", Some(0)),
        ledger::Item::Assistant {
            item_id: "a2".into(),
            text: "Done — the button is wired.\n<locust-memory>\nremember :: x\n</locust-memory>".into(),
            r#final: true,
        },
    ];
    let metas = vec![
        ledger::Metadata { mission_id: "m1".into(), prompt: "Build the export module.".into(), runtime: "codex".into(), cwd: Some("/tmp/shop".into()), ..Default::default() },
        ledger::Metadata { mission_id: "m2".into(), prompt: "Add the export button.".into(), runtime: "codex".into(), cwd: Some("/tmp/shop".into()), ..Default::default() },
    ];
    let mut rt = review_window(vec![teammate("casper", "Casper", "claude", Some("claude-sonnet-4-6"), "accept-edits")], items, metas);

    press(&mut rt, "thread-header-more");
    press(&mut rt, "row-menu-item-ask-casper-for-a-review");
    wait_for_sends(&mut rt, &_hook.sent, 1);

    assert_eq!(rt.state.teammates[rt.state.selected].id, "casper", "switched to the reviewer");
    let calls = _hook.sent.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert_eq!(call.teammate_id, "casper");
    assert_eq!(call.conversation, None, "a review is a new conversation of the reviewer's");
    assert_eq!(call.runtime.as_deref(), Some("claude"));
    assert_eq!(call.model.as_deref(), Some("claude-sonnet-4-6"));
    assert_eq!(call.mode, Some(Mode::Ask), "as_reviewer holds accept-edits reviewers to read-only Ask");

    let brief = &call.prompt;
    assert!(brief.starts_with("Wren finished a piece of work"), "{brief}");
    assert!(brief.contains("WHAT WAS ASKED FOR\nAdd the export button."), "{brief}");
    assert!(brief.contains("EARLIER IN THE SAME CONVERSATION (context, not the request)\nBuild the export module."), "{brief}");
    assert!(brief.contains("WHAT THEY SAID\nDone — the button is wired."), "{brief}");
    assert!(!brief.contains("<locust-memory>"), "protocol tags defanged: {brief}");
    assert!(brief.contains("WHAT CHANGED\n- src/Toolbar.tsx\n- src/export.ts"), "{brief}");
    assert!(!brief.contains("src/broken.ts"), "failed edits are excluded: {brief}");
    assert!(brief.contains("WHAT RAN\n- pnpm test — exit 0"), "{brief}");
    assert!(brief.contains("CHANGED EARLIER IN THIS CONVERSATION (already done, not this turn)\n- src/export.test.ts"), "{brief}");
    assert_eq!(brief.matches("- src/export.ts").count(), 1, "touched again this turn: listed once under WHAT CHANGED");
    assert!(brief.contains("WHERE IT RAN\n") && brief.contains("shop"), "{brief}");
    assert!(brief.contains("Start your answer with a one-line verdict"), "{brief}");
}

#[test]
fn choosing_the_pair_from_the_submenu_starts_two_reviews_on_two_agents_and_selects_the_first() {
    let _hook = capture_sends();
    let items = vec![
        ledger::Item::User("Answer the logistics question.".into()),
        ledger::Item::Assistant { item_id: "a1".into(), text: "Keel is a private logistics company.".into(), r#final: true },
    ];
    let metas = vec![ledger::Metadata {
        mission_id: "m1".into(),
        prompt: "Answer the logistics question.".into(),
        runtime: "codex".into(),
        cwd: Some("/tmp/shop".into()),
        ..Default::default()
    }];
    let mut rt = review_window(
        vec![
            teammate("booty", "Booty", "claude", Some("claude-opus-4-6"), "full-access"),
            teammate("vale", "Vale", "opencode", Some("opencode/free"), "accept-edits"),
        ],
        items,
        metas,
    );

    press(&mut rt, "thread-header-more");
    press(&mut rt, "row-menu-item-ask-for-a-review");
    press(&mut rt, "row-menu-sub-item-booty-claude-code-and-vale-opencode");
    wait_for_sends(&mut rt, &_hook.sent, 2);

    assert_eq!(rt.state.teammates[rt.state.selected].id, "booty", "selects the first reviewer of the pair");
    let calls = _hook.sent.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].teammate_id, "booty");
    assert_eq!(calls[0].runtime.as_deref(), Some("claude"));
    assert_eq!(calls[0].mode, Some(Mode::Ask));
    assert_eq!(calls[1].teammate_id, "vale");
    assert_eq!(calls[1].runtime.as_deref(), Some("opencode"));
    assert_eq!(calls[1].mode, Some(Mode::Ask));
    assert_eq!(calls[0].prompt, calls[1].prompt, "both reviewers receive the same brief");
    assert!(calls[0].prompt.contains("This turn answered in the conversation; it changed no files and ran no commands."));
}

#[test]
fn host_started_turn_uses_root_ask_as_request_and_answer_only_turn_carries_earlier_edits() {
    // case: carries earlier work into a turn that only answered -- where it matters most
    let _hook = capture_sends();
    let items = vec![
        ledger::Item::User("Build the export module.".into()),
        tool_edit("e1", "/tmp/shop/src/export.ts", false),
        ledger::Item::Assistant { item_id: "a1".into(), text: "Done.".into(), r#final: true },
        ledger::Item::User("Host assembled briefing".into()),
        ledger::Item::Assistant { item_id: "a2".into(), text: "Explained the design.".into(), r#final: true },
    ];
    let metas = vec![
        ledger::Metadata { mission_id: "m1".into(), prompt: "Build the export module.".into(), runtime: "codex".into(), cwd: Some("/tmp/shop".into()), ..Default::default() },
        ledger::Metadata { mission_id: "m2".into(), prompt: "Host assembled briefing".into(), runtime: "codex".into(), cwd: Some("/tmp/shop".into()), started_by_kind: Some("relay".into()), ..Default::default() },
    ];
    let mut rt = review_window(vec![teammate("casper", "Casper", "claude", None, "ask")], items, metas);

    press(&mut rt, "thread-header-more");
    press(&mut rt, "row-menu-item-ask-casper-for-a-review");
    wait_for_sends(&mut rt, &_hook.sent, 1);

    let calls = _hook.sent.lock().unwrap();
    let brief = &calls[0].prompt;
    assert!(brief.contains("WHAT WAS ASKED FOR\nBuild the export module."), "{brief}");
    assert!(!brief.contains("EARLIER IN THE SAME CONVERSATION"), "root ask is already WHAT WAS ASKED FOR: {brief}");
    assert!(brief.contains("This turn answered in the conversation; it changed no files and ran no commands."), "{brief}");
    assert!(brief.contains("CHANGED EARLIER IN THIS CONVERSATION (already done, not this turn)\n- src/export.ts"), "{brief}");
}

// Lane check 23 Nov: the author is who started the conversation's newest turn (`missionOwners`), read before the pane's
// header; a conversation shown under one teammate but started by another asks the header's teammate, not the author.
#[test]
fn the_author_is_who_started_the_newest_turn_not_the_teammate_the_pane_is_under() {
    let _hook = capture_sends();
    let items = vec![
        ledger::Item::User("Tidy the README.".into()),
        ledger::Item::Assistant { item_id: "a1".into(), text: "Tidied.".into(), r#final: true },
    ];
    let metas = vec![ledger::Metadata { mission_id: "m1".into(), prompt: "Tidy the README.".into(), runtime: "opencode".into(), cwd: Some("/tmp/shop".into()), ..Default::default() }];
    let mut rt = review_window(vec![teammate("vale", "Vale", "opencode", Some("opencode/free"), "accept-edits")], items, metas);
    rt.update(|s, _| {
        s.mission_owners.insert("m1".into(), "vale".into());
        if let Some(pane) = s.ledger.as_mut() {
            pane.conversation.conversation_id = Some("c_root".into());
        }
    });
    for _ in 0..5 {
        rt.frame(0.05);
    }
    press(&mut rt, "thread-header-more");
    press(&mut rt, "row-menu-item-ask-wren-for-a-review");
    wait_for_sends(&mut rt, &_hook.sent, 1);
    let calls = _hook.sent.lock().unwrap();
    assert_eq!(calls[0].teammate_id, "wren", "Vale started the turn, so Wren is the one asked");
    assert!(calls[0].prompt.starts_with("Vale finished a piece of work"), "{}", calls[0].prompt);
}
