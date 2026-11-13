use super::*;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn queue_edit_and_discard_are_conversation_scoped_and_transactional() {
    let mut s = sample();
    composer_queue_scene(&mut s);
    let mut second = s.queue_rows[0].clone();
    second.id = "two".into();
    second.text = "Second".into();
    second.attachments = vec!["two.md".into()];
    s.queue_rows.push(second);
    let expected = queue_front(&s).unwrap();
    let mut other = s.queue_rows[0].clone();
    other.key = "other".into();
    s.queue_rows.insert(1, other.clone());
    assert!(unqueue(&mut s, true));
    assert_eq!(s.draft.text, expected.text);
    assert_eq!(s.attached, expected.attachments);
    assert_eq!(s.queue_rows, vec![other]);
    assert!(s.off_the_queue);
    composer_queue_scene(&mut s);
    s.draft.replace_text("Keep draft");
    s.demo = false; // no writable store: removal must roll back.
    let before = s.queue_rows.clone();
    assert!(!unqueue(&mut s, false));
    assert_eq!(s.queue_rows, before);
    s.demo = true;
    assert!(unqueue(&mut s, false));
    assert_eq!(s.draft.text, "Keep draft");
}

#[test]
fn mention_scene_uses_real_matching_and_excludes_attached_and_tagged() {
    let mut s = sample();
    composer_mention_scene(&mut s);
    let choices = at_choices_now(&s);
    assert!(!choices.is_empty());
    assert!(choices.iter().all(|c| matches!(c, AtChoice::File { .. })));
    let expected = composer::file_matches(&s.project_files, "re", 8);
    assert_eq!(
        choices
            .iter()
            .map(|c| match c {
                AtChoice::File { path } => path.clone(),
                _ => panic!(),
            })
            .collect::<Vec<_>>(),
        expected
    );
    s.draft.replace_text("@");
    let all = at_choices_now(&s);
    assert!(matches!(all.first(), Some(AtChoice::Teammate { .. })));
    let file_at = all
        .iter()
        .position(|c| matches!(c, AtChoice::File { .. }))
        .unwrap();
    assert!(
        all[file_at..]
            .iter()
            .all(|c| matches!(c, AtChoice::File { .. }))
    );
    s.attached = s.project_files.clone();
    s.tagged = s.teammates.iter().map(|m| m.id.clone()).collect();
    assert!(at_choices_now(&s).is_empty());
    s.tagged.clear();
    s.draft.replace_text("@aR");
    assert!(
        at_choices_now(&s)
            .iter()
            .all(|c| matches!(c, AtChoice::Teammate { name, .. } if name == "Argon"))
    );
}

#[test]
fn queue_strip_keeps_every_word_and_save_failure_keeps_draft() {
    let mut s = sample();
    composer_queue_scene(&mut s);
    let mut second = s.queue_rows[0].clone();
    second.id = "second".into();
    second.text = "Second thought".into();
    s.queue_rows.push(second);
    assert!(
        queue_front(&s)
            .unwrap()
            .text
            .ends_with("\n\nSecond thought")
    );
    s.demo = false;
    s.draft.replace_text("Keep me");
    let before = s.queue_rows.clone();
    assert!(!enqueue_follow_up(&mut s, "unsaved".into(), vec![]));
    assert_eq!(s.queue_rows, before);
    assert_eq!(s.draft.text, "Keep me");
}

#[test]
fn slash_escape_clears_the_command_like_electron() {
    let mut s = sample();
    s.draft.replace_text("/plan");
    let event = KeyEvent {
        key: Key::Escape,
        mods: rune::Mods::default(),
    };
    assert!(composer_input_key(&mut s, &event, &mut Tx::default()));
    assert_eq!(s.draft.text, "");
    assert!(slash_choices_now(&s).is_empty());
}

#[test]
fn activity_status_words_follow_finished_refused_and_background_work() {
    use ledger::{Status, ToolDetail};
    let mut d = ToolDetail::default();
    assert_eq!(tool_result(Status::Running, &d, false), "running");
    assert_eq!(tool_result(Status::Running, &d, true), "did not report");
    d.helper = true;
    assert_eq!(tool_result(Status::Running, &d, false), "working on it");
    assert_eq!(tool_result(Status::Done, &d, true), "reported back");
    d.background = true;
    assert_eq!(
        tool_result(Status::Done, &d, false),
        "working in the background"
    );
    assert_eq!(tool_result(Status::Done, &d, true), "did not report");
    d.background_ended = "stopped-with-run".into();
    assert_eq!(
        tool_result(Status::Done, &d, true),
        "stopped when the run ended"
    );
    d.helper = false;
    assert_eq!(tool_result(Status::Done, &d, true), "stopped");
    d.runtime_status = "refused".into();
    assert_eq!(tool_result(Status::Failed, &d, true), "refused");
    d.runtime_status.clear();
    d.exit_code = Some(2);
    assert_eq!(tool_result(Status::Failed, &d, true), "failed · exit 2");
}

#[test]
fn tool_row_label_omits_zero_calls_background_dupes_and_non_shell_no_output() {
    use ledger::{Status, ToolDetail};
    let mut helper = ToolDetail {
        helper: true,
        background: true,
        background_ended: "stopped-with-run".into(),
        ..ToolDetail::default()
    };
    let label = tool_row_label("Agent", Status::Done, "Review", &helper, true);
    assert!(label.contains("stopped when the run ended"));
    assert!(!label.contains(" · when the run ended"));
    assert!(!label.contains("0 calls"));
    helper.children.push(ledger::Item::Note {
        level: String::new(),
        text: "x".into(),
    });
    assert!(tool_row_label("Agent", Status::Done, "Review", &helper, true).contains("1 call"));

    let mut shell = ToolDetail {
        output_reported: true,
        output: "   ".into(),
        duration_ms: Some(3000),
        ..ToolDetail::default()
    };
    let shell_label = tool_row_label("Bash", Status::Done, "Check", &shell, true);
    assert!(shell_label.contains("no output"));
    assert!(shell_label.contains("3s"));
    let read_label = tool_row_label("Read", Status::Done, "file", &shell, true);
    assert!(!read_label.contains("no output"));
    assert!(!read_label.contains("3s"));
    shell.runtime_status = "declined".into();
    assert!(!tool_row_label("Bash", Status::Done, "Check", &shell, true).contains("3s"));
}

#[test]
fn thought_headline_helpers_match_activity_card() {
    let text = "**Checking the fixture**\nCompare the synthetic input.";
    assert_eq!(thought_headline(text), Some("Checking the fixture"));
    assert_eq!(without_headline(text), "Compare the synthetic input.");
    assert_eq!(thought_headline("no headline"), None);
    assert_eq!(without_headline("no headline"), "no headline");
    let thought = ledger::Item::Thought {
        id: "t".into(),
        text: text.into(),
        duration_ms: Some(3000),
    };
    let label = match &thought {
        ledger::Item::Thought {
            text, duration_ms, ..
        } => {
            let mut label = format!(
                "Thought for {}",
                ledger::duration_text(duration_ms.unwrap())
            );
            if let Some(h) = thought_headline(text) {
                label.push_str(&format!(" · {h}"));
            }
            label
        }
        _ => unreachable!(),
    };
    assert_eq!(label, "Thought for 3s · Checking the fixture");
}

#[test]
fn plan_drawer_keys_do_not_collide_with_tool_item_ids() {
    let plan = ledger::Item::Plan {
        id: "plan".into(),
        steps: vec![],
        finished: false,
    };
    let tool = ledger::Item::Tool {
        name: "Bash".into(),
        status: ledger::Status::Done,
        summary: "x".into(),
        detail: Box::new(ledger::ToolDetail {
            id: "plan".into(),
            ..Default::default()
        }),
    };
    assert_eq!(activity_drawer_key("scope", &plan), "scope/plan/plan");
    assert_eq!(activity_drawer_key("scope", &tool), "scope/plan");
    assert_ne!(
        activity_drawer_key("scope", &plan),
        activity_drawer_key("scope", &tool)
    );
}

#[test]
fn rich_scenes_start_folded_and_open_variant_has_independent_drawers() {
    let mut s = State::default();
    thread_rich_scene(&mut s);
    assert!(s.activity_open.is_empty());
    let pane = s.ledger.as_ref().unwrap();
    assert!(matches!(pane.items[1], ledger::Item::Plan { .. }));
    assert_eq!(pane.items.iter().filter(|i| is_activity(i)).count(), 3);
    thread_rich_open_scene(&mut s);
    assert_eq!(s.activity_open.get("synthetic-ledger/command"), Some(&true));
    assert!(
        !s.activity_open
            .contains_key("synthetic-ledger/helper/child")
    );
    s.activity_open
        .insert("synthetic-ledger/command".into(), false);
    assert_eq!(s.activity_open.get("synthetic-ledger/helper"), Some(&true));
    let plan_key = s
        .ledger
        .as_ref()
        .unwrap()
        .items
        .iter()
        .find(|i| matches!(i, ledger::Item::Plan { .. }))
        .map(|plan| activity_drawer_key("synthetic-ledger", plan))
        .unwrap();
    assert!(plan_key.starts_with("synthetic-ledger/plan/"));
    thread_rich_light_scene(&mut s);
    assert_eq!(s.appearance, Some(false));
}

#[test]
fn chip_name_uses_known_nonempty_name_and_falls_back_to_id() {
    let mut agent = engine::Models {
        teammate: None,
        runtime: "sample".into(),
        model: None,
        effort: None,
        models: vec![engine::Model {
            id: "raw/id".into(),
            name: "Friendly name".into(),
            efforts: vec![],
        }],
    };
    assert_eq!(model_chip_name("raw/id", Some(&agent)), "Friendly name");
    assert_eq!(model_chip_name("unknown", Some(&agent)), "unknown");
    assert_eq!(model_chip_name("raw/id", None), "raw/id");
    agent.models[0].name = " ".into();
    assert_eq!(model_chip_name("raw/id", Some(&agent)), "raw/id");
}

#[test]
fn synthetic_home_never_requests_models() {
    let mut state = State::default();
    team_scene(&mut state);
    open_models(&mut state, "codec".into());
    assert!(!state.model_job.is_pending());
    assert!(state.model_menu.is_none());
}

#[test]
fn bench_arguments_are_isolated_and_bounded() {
    let parse =
        |args: &[&str]| bench_options(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert_eq!(
        parse(&["--bench", "out.json"]).unwrap().1,
        Duration::from_secs(30)
    );
    assert_eq!(
        parse(&["--bench-seconds", "2", "--bench", "out.json"])
            .unwrap()
            .1,
        Duration::from_secs(2)
    );
    for args in [
        vec!["--bench"],
        vec!["--bench-seconds", "2"],
        vec!["--bench", "x", "--bench-seconds", "0"],
        vec!["--bench", "x", "--scene", "replay"],
        vec!["--bench", "x", "--bench-conversation"],
        vec!["--bench", "x", "--bench-conversation", "../escape"],
    ] {
        assert!(parse(&args).is_err());
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) struct TempDir(pub(super) PathBuf);
impl TempDir {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "locust-main-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn write(&self, id: &str, records: &[serde_json::Value]) {
        let text = records.iter().map(|r| format!("{r}\n")).collect::<String>();
        std::fs::write(self.0.join(format!("{id}.jsonl")), text).unwrap();
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sample() -> State {
    let mut s = State::default();
    team_scene(&mut s);
    s
}

fn wait_job(s: &mut State) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while s.ledger_job.is_pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!s.ledger_job.is_pending(), "ledger_job did not finish");
}

#[test]
fn reload_preserves_history_recent_selection_and_reply_target() {
    let mut s = sample();
    let conversation = s.conversations.get_mut("codec").unwrap();
    conversation.conversation_id = Some("sample-3".into());
    let turns = conversation.turns.clone();
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: Some("sample-3".into()),
        header: None,
        after_tip: None,
        message: None,
    });
    let mut team = s.teammates.clone();
    team.reverse();
    apply_team(&mut s, team);

    assert_eq!(s.teammates.get(s.selected).unwrap().id, "codec");
    let conversation = s.conversations.get("codec").unwrap();
    assert_eq!(conversation.conversation_id.as_deref(), Some("sample-3"));
    assert_eq!(conversation.turns, turns);
    assert!(s.recent.iter().any(|r| r.id == "sample-3"));

    land_reply(
        &mut s,
        "codec",
        Ok(Reply::Answer("landed".into())),
        &mut Tx::default(),
    );
    assert_eq!(
        s.conversations.get("codec").unwrap().turns.last().unwrap(),
        &("Codec".into(), "landed".into()),
    );
    assert!(s.flight.is_none());
}

#[test]
fn removed_teammate_clears_active_and_opening_requests() {
    let mut s = sample();
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: None,
        header: None,
        after_tip: None,
        message: None,
    });
    s.opening_teammate = Some("codec".into());
    let team = s
        .teammates
        .iter()
        .filter(|t| t.id != "codec")
        .cloned()
        .collect();
    apply_team(&mut s, team);

    assert!(s.flight.is_none() && s.opening_teammate.is_none());
    assert!(s.status.contains("no longer in the team"));
}

#[test]
fn missing_conversation_id_ends_the_flight() {
    let mut s = sample();
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: None,
        header: None,
        after_tip: None,
        message: None,
    });
    request_reply(&mut s, "codec", Duration::ZERO);

    assert!(s.flight.is_none());
    assert!(!s.waiting.is_pending());
    assert!(s.status.contains("No active conversation ID"));
}

#[test]
fn escape_stops_watching_then_clears_draft_and_close_clears_state() {
    // on_close asks every native turn in this process to stop; hold the lock the native tests run under.
    let _serial = native::hooks::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut s = sample();
    s.draft.replace_text("new draft");
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: None,
        header: None,
        after_tip: None,
        message: None,
    });
    let escape = KeyEvent {
        key: Key::Escape,
        mods: rune::Mods::default(),
    };
    let mut tx = Tx::default();

    assert!(on_key(&mut s, &escape, &mut tx));
    assert!(s.flight.is_none());
    assert_eq!(s.status, "Stopped watching; the turn goes on in Locust");
    assert_eq!(s.draft.text, "new draft");

    assert!(on_key(&mut s, &escape, &mut tx));
    assert!(s.draft.text.is_empty());

    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: None,
        header: None,
        after_tip: None,
        message: None,
    });
    s.opening_teammate = Some("codec".into());
    on_close(&mut s, &mut tx);

    assert!(s.flight.is_none() && s.opening_teammate.is_none());
    assert!(s.status.is_empty());
}

#[test]
fn failed_send_restores_unsent_text_without_losing_new_typing() {
    let mut s = sample();
    s.demo = false;
    s.draft.replace_text("new typing");
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: None,
        header: None,
        after_tip: None,
        message: Some("unsent text".into()),
    });
    // A local result exercises poll without contacting the engine.
    s.sending.request((), |()| Err("send failed".into()));
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while s.sending.is_pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!s.sending.is_pending(), "fake result did not land");
    assert!(poll(&mut s, 0.0, &mut Tx::default()));
    assert_eq!(s.draft.text, "unsent text\n\nnew typing");
    assert!(s.flight.is_none());
}

#[test]
fn switching_conversations_restores_cancelled_ledger_send_and_drops_stale_results() {
    let mut s = sample();
    s.demo = false;
    ledger_transcript_scene(&mut s);
    s.demo = false;
    let codec_header = s.teammates[0].clone();
    s.flight = Some(Flight {
        teammate_id: codec_header.id.clone(),
        conversation_id: Some("synthetic-ledger".into()),
        header: Some(codec_header),
        after_tip: Some("synthetic-ledger".into()),
        message: Some("follow-up question".into()),
    });

    // Switching teammates cancels the ledger send, keeps the unsent words under that
    // conversation's draft key (Electron Map), and shows Argon's own draft in the box.
    let left_key = conversation_draft_key(&s);
    select_teammate(&mut s, "argon", &mut Tx::default());
    assert!(s.ledger.is_none());
    assert!(s.flight.is_none());
    assert!(s.draft.text.is_empty(), "Argon's box starts empty");
    assert_eq!(
        left_key.as_deref().and_then(|k| s.drafts.get(k)),
        Some("follow-up question")
    );
    assert_eq!(s.status, "Argon: Ready");

    // A stale send result tagged for Codec must not swap in Argon's state or hijack Argon's view.
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: Some("old-conv".into()),
        header: Some(s.teammates[0].clone()),
        after_tip: Some("old-tip".into()),
        message: None,
    });
    s.sending.request((), |()| {
        Ok(Started {
            conversation_id: "codec-new".into(),
            mode: "ask".into(),
        })
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while s.sending.is_pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    poll(&mut s, 1.0, &mut Tx::default());
    assert!(s.ledger.is_none());
    assert_eq!(s.teammates[s.selected].id, "argon");
}

#[test]
fn idle_ledger_stops_polling_errors_back_off_without_rebuilding_and_after_tip_times_out() {
    let temp = TempDir::new();
    temp.write(
        "done",
        &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"done",
                    "prompt":"Synthetic prompt",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:00Z"
                }
            }),
            json!({
                "schemaVersion":27,
                "recordType":"mission.event",
                "event":{
                    "sequence":1,
                    "occurredAt":"2026-01-01T00:00:01Z",
                    "type":"run.completed",
                    "payload":{}
                }
            }),
        ],
    );
    temp.write(
        "other",
        &[json!({
            "schemaVersion":27,
            "recordType":"mission.created",
            "metadata":{
                "missionId":"other",
                "prompt":"Other prompt",
                "runtime":"codex",
                "model":"sample",
                "createdAt":"2026-01-01T00:00:00Z"
            }
        })],
    );

    let mut s = sample();
    s.demo = false;
    let header = s.teammates[0].clone();
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some("done".into()),
            turns: Vec::new(),
        },
        items: Vec::new(),
        dir: temp.0.clone(),
        worker: Some(ledger::Session::new(temp.0.clone(), "done".into())),
        snapshot: ledger::Snapshot::default(),
        due: 0.0,
        loading: true,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });

    // Finished conversation stops scheduling wakeups after its initial load.
    let mut tx = Tx::default();
    ledger_tick(&mut s, 0.0, &mut tx);
    wait_job(&mut s);
    ledger_tick(&mut s, 0.1, &mut tx);
    let pane = s.ledger.as_ref().unwrap();
    assert!(!pane.loading && !pane.snapshot.running);
    assert!(pane.due.is_infinite());
    assert!(!s.ledger_job.is_pending());
    assert_eq!(pane.worker.as_ref().unwrap().cached_headers(), 2);

    // Bound after_tip waiting to 30 s, then show "No new reply yet" and unlock Send.
    s.ledger.as_mut().unwrap().after_tip = Some("done".into());
    s.ledger.as_mut().unwrap().due = 10.0;
    ledger_tick(&mut s, 10.0, &mut tx);
    wait_job(&mut s);
    ledger_tick(&mut s, 10.1, &mut tx);
    assert_eq!(s.status, "Working…");
    assert!(s.flight.is_some());

    s.ledger.as_mut().unwrap().due = 40.0;
    ledger_tick(&mut s, 40.0, &mut tx);
    wait_job(&mut s);
    ledger_tick(&mut s, 40.0, &mut tx);
    assert_eq!(s.status, "No new reply yet");
    assert!(s.flight.is_none());
    assert!(s.ledger.as_ref().unwrap().after_tip.is_none());

    // Transient errors back off 2 s, 4 s, 8 s, 16 s, 30 s without rebuilding the Session.
    std::fs::remove_file(temp.0.join("done.jsonl")).unwrap();
    std::fs::remove_file(temp.0.join("other.jsonl")).unwrap();
    let mut now = 100.0;
    s.ledger.as_mut().unwrap().loading = true;
    s.ledger.as_mut().unwrap().due = now;
    for expected in [2.0, 4.0, 8.0, 16.0, 30.0, 30.0] {
        ledger_tick(&mut s, now, &mut tx);
        wait_job(&mut s);
        ledger_tick(&mut s, now, &mut tx);
        let pane = s.ledger.as_ref().unwrap();
        assert_eq!(pane.backoff, expected);
        assert_eq!(pane.due, now + expected);
        // If Session had been rebuilt after `other.jsonl` was removed, cached_headers() would be 0.
        assert_eq!(pane.worker.as_ref().unwrap().cached_headers(), 2);
        now += expected;
    }
}

#[test]
fn an_added_account_is_named_in_the_picker_and_respects_policy_and_limits() {
    use locust_core::agent_accounts::{Accounts, AccountState};
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            locust_core::org_policy::use_policy(None);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _guard = crate::agent_accounts_card::test_guard();
    locust_core::org_policy::use_policy(None);
    let root = std::env::temp_dir().join(format!(
        "locust-picker-accounts-{}-{}",
        std::process::id(),
        locust_core::invariants::unique_nanos()
    ));
    let _cleanup = Cleanup(root.clone());
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let accounts = Accounts::new(
        store.root(),
        std::sync::Arc::new(|_, _, _, _| Ok(Box::new(|| {}))),
        Box::new(|| locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "accounts")),
        Box::new(|_, _| false),
    );
    let work = accounts.add("codex", "Work").unwrap();
    let personal = accounts.add("codex", "Personal").unwrap();
    // Mark Personal ready; leave Work in New state so it renders "SIGN IN".
    std::fs::write(
        accounts.root().join("accounts.json"),
        serde_json::to_vec_pretty(&json!([
            {"id": work.id, "runtime": "codex", "name": "Work", "state": "new"},
            {"id": personal.id, "runtime": "codex", "name": "Personal", "state": "ready"}
        ]))
        .unwrap(),
    )
    .unwrap();
    crate::agent_accounts_card::use_accounts(accounts);

    let mut s = sample();
    s.data = Some(store.clone());
    s.models.insert(
        "codec".into(),
        vec![engine::Models {
            teammate: Some("codec".into()),
            runtime: "codex".into(),
            model: Some("gpt-6.1-sol".into()),
            effort: None,
            models: vec![engine::Model {
                id: "gpt-6.1-sol".into(),
                name: "GPT-6.1 Sol".into(),
                efforts: vec!["high".into()],
            }],
        }],
    );
    s.model_menu = Some("codec".into());
    crate::agent_accounts_card::reload(&mut s);
    s.limited_runtimes.insert(
        locust_core::agent_accounts::usage_key("codex", Some(&personal.id)),
        "Quota exhausted".into(),
    );

    let text_in = |rt: &rune::Runtime<State>, key: &str| -> Vec<String> {
        let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
        rt.elements()
            .into_iter()
            .filter_map(|e| {
                let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
                (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
            })
            .collect()
    };

    let mut rt = rune::Runtime::new(s, view, (1100.0, 780.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let menu_text = text_in(&rt, "model-menu").join(" | ");
    assert!(menu_text.contains("GPT-6.1 Sol · Work") && menu_text.contains("SIGN IN"), "{menu_text}");
    assert!(menu_text.contains("GPT-6.1 Sol · Personal") && menu_text.contains("AT LIMIT"), "{menu_text}");
    assert!(!rt.find("model-codex-gpt-6.1-sol").is_none());

    // Clicking an added account sets explicit account override; clicking machine sign-in sets Some(None).
    let work_key = format!("model-codex-gpt-6.1-sol:{}", work.id);
    let (x, y) = rt.find(&work_key).expect("Work model row");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(rt.state.picks.get("codec").and_then(|p| p.account.clone()), Some(Some(work.id.clone())));

    rt.update(|s, _| s.model_menu = Some("codec".into()));
    rt.frame(0.05);
    let (x, y) = rt.find("model-codex-gpt-6.1-sol").expect("machine sign-in model row");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(rt.state.picks.get("codec").and_then(|p| p.account.clone()), Some(None));

    // Spec 2 & 3: org policy hides added account rows in picker and hides Add in Settings without auto-login.
    let mut policy = locust_core::org_policy::PolicyState {
        path: String::new(),
        present: true,
        policy: Default::default(),
        error: None,
    };
    policy.policy.insert("accounts".into(), json!(false));
    locust_core::org_policy::use_policy(Some(policy));
    rt.update(|s, _| {
        s.model_menu = Some("codec".into());
    });
    for _ in 0..40 {
        rt.frame(0.05);
    }
    let menu_text = text_in(&rt, "model-menu").join(" | ");
    assert!(
        !menu_text.contains("Work") && !menu_text.contains("Personal"),
        "policy should hide added accounts (refusal={:?}): {menu_text}",
        locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "accounts")
    );
    rt.update(|s, _| {
        crate::settings_panel::open(s);
        s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("runtimes");
    });
    for _ in 0..40 {
        rt.frame(0.05);
    }
    assert!(rt.find("account-add-another-codex").is_none(), "policy must hide Add account");
    let accounts_text = text_in(&rt, "accounts-codex").join(" | ");
    assert!(accounts_text.contains("This computer's sign-in") && accounts_text.contains("Work"), "{accounts_text}");
    let short_work = &work.id[8..16];
    let (sx, sy) = rt.find(&format!("account-sign-in-{short_work}")).expect("Sign in button");
    rt.pointer_down(sx, sy);
    rt.pointer_up(sx, sy);
    for _ in 0..10 {
        rt.frame(0.05);
    }
    assert_eq!(rt.state.agent_accounts.accounts.iter().find(|a| a.id == work.id).unwrap().state, AccountState::New);
}
