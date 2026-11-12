use super::*;

pub(super) fn team_scene(s: &mut State) {
    s.demo = true;
    s.appearance = Some(true);
    s.teammates = [
        (
            "codec",
            "Codec",
            "Code & Migrations",
            "codex",
            "gpt-6.1-sol",
            false,
        ),
        (
            "argon",
            "Argon",
            "Code & Migrations",
            "antigravity",
            "Gemini 3.8 Flash",
            true,
        ),
        (
            "scout",
            "Scout",
            "Custom",
            "opencode",
            "deepseek-v4.1-flash-free",
            false,
        ),
    ]
    .into_iter()
    .map(|(id, name, role, runtime, model, busy)| Teammate {
        id: id.into(),
        name: name.into(),
        role: role.into(),
        runtime: Some(runtime.into()),
        model: Some(model.into()),
        busy,
        runs_in: "ask".into(),
        folder: None,
    })
    .collect();

    s.conversations = s
        .teammates
        .iter()
        .map(|t| (t.id.clone(), Conversation::default()))
        .collect();
    if let Some(conversation) = s.conversations.get_mut("codec") {
        conversation.turns = vec![
            (
                "You".into(),
                "How is the native preview coming along?".into(),
            ),
            (
                "Codec".into(),
                "The window is taking shape. Teammates live on the left, and your conversation stays here.".into(),
            ),
            (
                "You".into(),
                "Let's keep it simple and make it feel like Locust.".into(),
            ),
            (
                "Codec".into(),
                "Agreed. A clear conversation, a quiet status line, and room to write.".into(),
            ),
        ];
    }
    s.recent = [
        (
            "sample-1",
            "Scout",
            "A real Markdown answer",
            "completed",
            "2026-11-08T21:23:37.380Z",
        ),
        (
            "sample-2",
            "Argon",
            "Review the native preview",
            "running",
            "2026-11-08T21:18:00Z",
        ),
        (
            "sample-3",
            "Codec",
            "Check the conversation sidebar",
            "failed",
            "2026-11-08T19:00:00Z",
        ),
        (
            "sample-4",
            "Scout",
            "A longer conversation title that should end in an ellipsis",
            "completed",
            "2026-11-07T12:00:00Z",
        ),
    ]
    .into_iter()
    .map(|(id, teammate, title, state, updated)| ConversationRow {
        id: id.into(),
        teammate: teammate.into(),
        title: title.into(),
        started_by: "the person".into(),
        state: state.into(),
        updated: updated.into(),
    })
    .collect();
    s.selected = 0;
    s.status = "Ready".into();
    s.status_tone = StatusTone::Muted;
}

pub(super) fn empty_scene(s: &mut State) {
    team_scene(s);
    for conversation in s.conversations.values_mut() {
        *conversation = Conversation::default();
    }
}

/*
 * A REAL EXCHANGE, CHECKED WITHOUT A WINDOW. `--ask <teammate> <message>` runs the window's own engine calls (start,
 * then read_reply until the turn ends) against the running Locust and saves what came back; the "replay" scene draws
 * that file headless (`--snap shot.png --scene replay`), so a real answer is seen without opening a window that would
 * take the screen from whoever is working.
 */
pub(super) const REPLAY_FILE: &str = "locust-native-replay.json";

pub(super) fn replay_path() -> std::path::PathBuf {
    std::env::var_os("LOCUST_NATIVE_REPLAY")
        .map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join(REPLAY_FILE))
}

pub(super) fn ask_cli(who: &str, message: &str) -> Result<(), String> {
    // The Locust Quick Ask would ask: Electron's door, the native window's, or the native engine in this process.
    let (engine, _, team) = crate::quick::reach()?;
    let teammate = team
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(who) || t.id == who)
        .ok_or_else(|| format!("No teammate called {who}."))?;
    let started = engine.start(&teammate.id, message, None, None)?;
    eprintln!("started {} in {}", started.conversation_id, started.mode);

    let answer = loop {
        match engine.reply(&started.conversation_id, engine::WAIT_LIMIT_S)? {
            Reply::Working(doing) => eprintln!("working: {doing}"),
            Reply::WaitingForYou(words) => {
                // A card raised in this process has no window to answer it: refuse it rather than wait forever.
                // Joined to the shared engine, the window shows it, so it waits there.
                if engine.is_direct() && !locust_core::engine_api::shared() {
                    for card in locust_core::engine_api::api().waiting() {
                        let _ = locust_core::engine_api::api().decide(&card.approval_id, locust_core::approval::Decision::Deny, Some(crate::quick::QUICK_DENY.into()));
                    }
                }
                eprintln!("waiting for you in Locust: {words}");
                std::thread::sleep(Duration::from_secs(3));
            }
            Reply::Answer(text) => break text,
            Reply::Failed(words) => return Err(words),
        }
    };
    println!("{answer}");
    let saved = serde_json::json!({
        "team": team.iter().map(|t| serde_json::json!({
            "id": t.id,
            "name": t.name,
            "role": t.role,
            "runtime": t.runtime,
            "model": t.model,
            "busy": t.busy,
            "runs_in": t.runs_in,
        })).collect::<Vec<_>>(),
        "teammate": teammate.id,
        "message": message,
        "answer": answer,
    });
    std::fs::write(replay_path(), saved.to_string()).map_err(|e| e.to_string())
}

pub(super) fn replay_scene(s: &mut State) {
    s.demo = true;
    s.appearance = Some(true);
    let Some(saved) = std::fs::read_to_string(replay_path())
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        s.status = "No saved exchange: run --ask first.".into();
        return;
    };

    s.teammates = saved
        .get("team")
        .and_then(|v| engine::parse_teammates(&v.to_string()).ok())
        .unwrap_or_default();
    s.conversations = s
        .teammates
        .iter()
        .map(|t| (t.id.clone(), Conversation::default()))
        .collect();
    let picked = saved
        .get("teammate")
        .and_then(serde_json::Value::as_str)
        .and_then(|id| s.teammates.iter().position(|t| t.id == id))
        .unwrap_or(0);

    if let Some(teammate) = s.teammates.get(picked)
        && let Some(conversation) = s.conversations.get_mut(&teammate.id)
    {
        conversation.turns = vec![
            (
                "You".into(),
                saved
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .into(),
            ),
            (
                teammate.name.clone(),
                saved
                    .get("answer")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .into(),
            ),
        ];
    }
    s.selected = picked;
    s.status = "Ready".into();
    s.status_tone = StatusTone::Muted;
}

pub(super) fn team_light_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(false);
}

pub(super) fn empty_light_scene(s: &mut State) {
    empty_scene(s);
    s.appearance = Some(false);
}

pub(super) fn replay_light_scene(s: &mut State) {
    replay_scene(s);
    s.appearance = Some(false);
}

pub(super) fn composer_base_scene(s: &mut State, light: bool) {
    team_scene(s);
    s.appearance = Some(!light);
    s.engine = Some(Kind::Native);
    s.selected_ready = true;
    s.route_can_run = true;
    s.nothing_installed = false;
    s.project_files = vec![
        "README.md".into(),
        "src/main.rs".into(),
        "docs/resume.md".into(),
        "résumé.md".into(),
        "PLAN.md".into(),
    ];
    s.draft.replace_text("");
    sync_draft_key(s);
    s.status = "Ready".into();
    s.status_tone = StatusTone::Muted;
}

pub(super) fn composer_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.draft
        .replace_text("Shift+Enter makes a new line.\nEnter sends.");
    s.scene_popup = ComposerScenePopup::None;
}

/// The model picker open on the native engine, with Codex's list as the door gives it.
pub(super) fn models_scene(s: &mut State) {
    composer_base_scene(s, false);
    let listed = r#"{"teammate":"Codec","runtime":"codex","model":"gpt-6.1-sol","effort":"high","models":[
        {"id":"account-default","name":"Account default","efforts":["low","medium","high","xhigh"]},
        {"id":"gpt-6.1-sol","name":"GPT-6.1 Sol","efforts":["low","medium","high","xhigh"]},
        {"id":"gpt-6-astra","name":"GPT-6 Astra","efforts":["low","medium","high"]},
        {"id":"gpt-6.1-terra","name":"GPT-6.1 Terra","efforts":["low","medium","high","xhigh"]}]}"#;
    s.models.insert("codec".into(), engine::parse_models(listed).unwrap_or_default());
    s.model_menu = Some("codec".into());
    s.picks.insert("codec".into(), TurnPick { model: "gpt-6.1-sol".into(), effort: Some("high".into()), runtime: None });
}

pub(super) fn models_light_scene(s: &mut State) {
    models_scene(s);
    s.appearance = Some(false);
}

/// Two reference folders on the conversation and the picker open.
pub(super) fn references_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.reference_items = vec![
        locust_core::references::Item { id: "r1".into(), name: "locust-site".into(), path: "/Users/locust/work/locust-site".into(), repository: None },
        locust_core::references::Item { id: "r2".into(), name: "now-coding".into(), path: "/Users/locust/work/now-coding".into(), repository: None },
    ];
    s.reference_picker = true;
    s.references_key = "scene".into();
}

pub(super) fn references_light_scene(s: &mut State) {
    references_scene(s);
    s.appearance = Some(false);
}

pub(super) fn composer_light_scene(s: &mut State) {
    composer_base_scene(s, true);
    s.draft
        .replace_text("Shift+Enter makes a new line.\nEnter sends.");
    s.scene_popup = ComposerScenePopup::None;
}

pub(super) fn composer_slash_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.draft.replace_text("/");
    s.slash_at = 0;
    s.scene_popup = ComposerScenePopup::Slash;
}

pub(super) fn composer_slash_light_scene(s: &mut State) {
    composer_base_scene(s, true);
    s.draft.replace_text("/");
    s.slash_at = 0;
    s.scene_popup = ComposerScenePopup::Slash;
}

pub(super) fn composer_mention_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.draft.replace_text("Please read @re");
    s.at_at = 0;
    s.scene_popup = ComposerScenePopup::Mention;
}

pub(super) fn composer_mention_light_scene(s: &mut State) {
    composer_base_scene(s, true);
    s.draft.replace_text("Please read @re");
    s.at_at = 0;
    s.scene_popup = ComposerScenePopup::Mention;
}

pub(super) fn composer_queue_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.scene_popup = ComposerScenePopup::Queue;
    if let Some(conversation) = s.conversations.get_mut("codec") {
        conversation.conversation_id = Some("composer-queue".into());
    }
    s.flight = Some(Flight {
        teammate_id: "codec".into(),
        conversation_id: Some("composer-queue".into()),
        header: s.teammates.first().cloned(),
        after_tip: None,
        message: None,
    });
    s.own_turn = Some(OwnTurn {
        conversation: "composer-queue".into(),
        tip: Some("composer-queue".into()),
        before: None,
    });
    s.queue_rows = vec![composer::QueuedRow {
        id: "q1".into(),
        key: "composer-queue".into(),
        text: "Also check the queue strip while this turn runs.".into(),
        origin: composer::Origin::Person,
        attachments: Vec::new(),
        issue: None,
        restored: false,
        held: None,
    }];
    s.draft.replace_text("");
    s.status = "Codec: Working…".into();
}

pub(super) fn composer_queue_light_scene(s: &mut State) {
    composer_queue_scene(s);
    s.appearance = Some(false);
}

pub(super) fn composer_blocked_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.nothing_installed = true;
    s.selected_ready = false;
    s.draft.replace_text("hello");
    s.scene_popup = ComposerScenePopup::Blocked;
}

pub(super) fn composer_blocked_light_scene(s: &mut State) {
    composer_blocked_scene(s);
    s.appearance = Some(false);
}

pub(super) fn approval_scene(s: &mut State, light: bool) {
    team_scene(s);
    s.appearance = Some(!light);
    s.engine = Some(Kind::Native);
    let Some(header) = s.teammates.first().cloned() else {
        return;
    };
    s.ledger = Some(LedgerPane {
        header: header.clone(),
        conversation: Conversation {
            conversation_id: Some("synthetic-approval".into()),
            turns: Vec::new(),
        },
        items: vec![
            ledger::Item::User("Create a file named hello.txt".into()),
            ledger::Item::Assistant {
                item_id: "ask".into(),
                text: "I need approval to write the file.".into(),
                r#final: true,
            },
        ],
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot {
            recovery: None,
            context: None,
            items: vec![],
            tip: "synthetic-approval".into(),
            running: true,
            failed: false,
        },
        due: f64::INFINITY,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    // Rebuild items into snapshot for the pane.
    if let Some(pane) = &mut s.ledger {
        pane.snapshot.items = pane.items.clone();
    }
    s.pending_approval = Some(approval::Request {
        approval_id: "ap_snap".into(),

        mission_id: "synthetic-approval".into(),
        runtime: "codex".into(),
        kind: approval::Kind::Command,
        summary: "Run a command".into(),
        detail: "touch hello.txt".into(),
        cwd: Some("/tmp/project".into()),
        requested_at: "2026-11-09T00:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,

        claude_input: None,
    });
    s.status = "Needs you".into();
    s.status_tone = StatusTone::Amber;
}

pub(super) fn approval_scene_dark(s: &mut State) {
    approval_scene(s, false);
}
pub(super) fn approval_scene_light(s: &mut State) {
    approval_scene(s, true);
}

pub(super) fn approval_reach_scene(s: &mut State, light: bool) {
    approval_scene(s, light);
    if let Some(card) = &mut s.pending_approval {
        card.detail = "taskkill //F //IM python.exe".into();
        card.summary = "Run a command".into();
        card.always_covers = None;
        card.always_key = None;
    }
}
pub(super) fn approval_reach_scene_dark(s: &mut State) {
    approval_reach_scene(s, false);
}
pub(super) fn approval_reach_scene_light(s: &mut State) {
    approval_reach_scene(s, true);
}

pub(super) fn approval_rules_scene(s: &mut State, light: bool) {
    team_scene(s);
    s.appearance = Some(!light);
    s.people_panel.open = true;
    s.people_panel.tab = 2;
    s.people_panel.memory_tab = false;
    s.people_panel.rules = vec![
        crate::approval_rules::ApprovalRule {
            rule_id: "rule_demo_allow".into(),
            effect: crate::approval_rules::RuleEffect::Allow,
            kind: crate::approval_rules::RuleKind::Command,
            pattern: "npm test".into(),
            teammate_id: Some("codec".into()),
            folder: Some("/Users/locust/Documents/Locust".into()),
            created_at: "2026-11-10T00:00:00.000Z".into(),
            uses: Some(3),
            last_used_at: Some("2026-11-10T12:00:00.000Z".into()),
        },
        crate::approval_rules::ApprovalRule {
            rule_id: "rule_demo_deny".into(),
            effect: crate::approval_rules::RuleEffect::Deny,
            kind: crate::approval_rules::RuleKind::Command,
            pattern: "git push".into(),
            teammate_id: None,
            folder: None,
            created_at: "2026-11-10T00:00:00.000Z".into(),
            uses: Some(1),
            last_used_at: None,
        },
    ];
}
pub(super) fn approval_rules_scene_dark(s: &mut State) {
    approval_rules_scene(s, false);
}
pub(super) fn approval_rules_scene_light(s: &mut State) {
    approval_rules_scene(s, true);
}

pub(super) fn ledger_scene(s: &mut State, live: bool, light: bool) {
    team_scene(s);
    s.appearance = Some(!light);
    let Some(header) = s.teammates.first().cloned() else {
        return;
    };
    let items = vec![
        ledger::Item::User("Explain this synthetic preview.".into()),
        ledger::Item::Plan {
            id: "preview-plan".into(),
            steps: ledger::read_plan(&serde_json::json!([
                {"step":"Read the input","status":"completed"}, {"step":"Summarize the result","status":"in_progress"}
            ])),
            finished: !live,
        },
        ledger::Item::Tool {
            name: "inspect".into(),
            status: ledger::Status::Done,
            summary: "Read a synthetic fixture".into(),
            detail: Box::new(ledger::ToolDetail {
                id: "preview-tool".into(),
                ..Default::default()
            }),
        },
        ledger::Item::Assistant {
            item_id: "synthetic-answer".into(),
            text: if live {
                "### Live answer\n\nThis text is still arriving".into()
            } else {
                "### Complete transcript\n\nThe preview shows **messages**, tool steps, plans, and notes.".into()
            },
            r#final: !live,
        },
        ledger::Item::Note {
            level: "warning".into(),
            text: "Synthetic demonstration note.".into(),
        },
    ];
    s.ledger = Some(LedgerPane {
        header: header.clone(),
        conversation: Conversation {
            conversation_id: Some("synthetic-ledger".into()),
            turns: Vec::new(),
        },
        items: items.clone(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot {
            recovery: None,
            context: None,
            items,
            tip: "synthetic-ledger".into(),
            running: live,
            failed: false,
        },
        due: if live { 0.0 } else { f64::INFINITY },
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    if live {
        s.flight = Some(Flight {
            teammate_id: header.id.clone(),
            conversation_id: Some("synthetic-ledger".into()),
            header: Some(header),
            after_tip: None,
            message: None,
        });
    }
    s.status = if live { "Working…" } else { "Ready" }.into();
    s.status_tone = if live {
        StatusTone::Green
    } else {
        StatusTone::Muted
    };
}

pub(super) fn thread_rich_scene(s: &mut State) {
    ledger_scene(s, true, false);
    let mut transcript = ledger::Transcript::default();
    let rows = [
        ("run.started", serde_json::json!({})),
        (
            "plan.updated",
            serde_json::json!({"itemId":"plan","plan":[{"step":"Read the fixture","status":"completed"},{"step":"Check the result","status":"in_progress"},{"step":"Write the report","status":"pending"}]}),
        ),
        (
            "step.started",
            serde_json::json!({"itemId":"thought","stepKind":"reasoning"}),
        ),
        (
            "step.completed",
            serde_json::json!({"itemId":"thought","stepKind":"reasoning","message":"**Checking the fixture**\nCompare the synthetic input with the expected result.","durationMs":12000}),
        ),
        (
            "tool.started",
            serde_json::json!({"itemId":"command","name":"Bash","title":"Check the fixture","command":"printf 'synthetic result\\n'"}),
        ),
        (
            "tool.completed",
            serde_json::json!({"itemId":"command","name":"Bash","output":"synthetic result\n1 check passed","exitCode":0,"durationMs":1500}),
        ),
        (
            "tool.started",
            serde_json::json!({"itemId":"helper","name":"Agent","title":"Review the fixture","status":"Explore","command":"Review the synthetic fixture and report what it contains."}),
        ),
        (
            "tool.started",
            serde_json::json!({"itemId":"child","parentItemId":"helper","name":"Read","command":"fixture.txt"}),
        ),
        (
            "tool.completed",
            serde_json::json!({"itemId":"child","parentItemId":"helper","name":"Read","output":"Synthetic fixture."}),
        ),
        (
            "tool.completed",
            serde_json::json!({"itemId":"helper","name":"Agent","output":"The synthetic fixture contains one passing check."}),
        ),
    ];
    let records = rows.into_iter().enumerate().map(|(n,(kind,payload))| serde_json::json!({"event":{"sequence":n,"type":kind,"occurredAt":"2026-01-01T00:00:00Z","payload":payload}})).collect();
    transcript.apply(records);
    if let Some(pane) = s.ledger.as_mut() {
        pane.items = vec![ledger::Item::User("Inspect this synthetic fixture.".into())];
        pane.items.extend(transcript.items);
        pane.snapshot.items = pane.items.clone();
    }
}
pub(super) fn thread_rich_open_scene(s: &mut State) {
    thread_rich_scene(s);
    for key in [
        "synthetic-ledger/group/thought:3",
        "synthetic-ledger/thought:3",
        "synthetic-ledger/command",
        "synthetic-ledger/helper",
    ] {
        s.activity_open.insert(key.into(), true);
    }
}
pub(super) fn thread_rich_open_light_scene(s: &mut State) {
    thread_rich_open_scene(s);
    s.appearance = Some(false);
}
pub(super) fn thread_rich_light_scene(s: &mut State) {
    thread_rich_scene(s);
    s.appearance = Some(false);
}

// The native engine, synthetic: a native-run Codex conversation streaming, a teammate that runs in Locust for now, and
// a native-marked row in Recent. No data folder is read.
pub(super) fn native_scene(s: &mut State, live: bool) {
    ledger_scene(s, live, false);
    s.engine = Some(Kind::Native);
    if let Some(t) = s.teammates.get_mut(1) {
        t.runs_in = native::IN_LOCUST.into();
    }
    if let Some(row) = s.recent.get_mut(0) {
        row.started_by = "native".into();
        row.teammate = "Codex".into();
        row.title = "Continue the native preview".into();
    }
    if live {
        // This window started the turn, so Stop shows.
        s.own_turn = Some(OwnTurn {
            conversation: "synthetic-ledger".into(),
            tip: Some("synthetic-ledger".into()),
            before: None,
        });
    }
}

pub(super) fn native_scene_idle(s: &mut State) {
    native_scene(s, false);
}
pub(super) fn native_scene_live(s: &mut State) {
    native_scene(s, true);
}

pub(super) fn ledger_transcript_scene(s: &mut State) {
    ledger_scene(s, false, false);
}

pub(super) fn ledger_live_scene(s: &mut State) {
    ledger_scene(s, true, false);
}

pub(super) fn ledger_transcript_light_scene(s: &mut State) {
    ledger_scene(s, false, true);
}

pub(super) fn ledger_live_light_scene(s: &mut State) {
    ledger_scene(s, true, true);
}

// --- P4 conversation organization scenes ---

fn org_base(s: &mut State, light: bool) {
    team_scene(s);
    s.appearance = Some(!light);
    s.org.pins = vec!["sample-2".into(), "sample-1".into()];
    s.org
        .titles
        .insert("sample-1".into(), "Pinned deploy notes".into());
    s.org.settled.insert(
        "sample-4".into(),
        settled::SettledEntry {
            at: "2026-11-09T12:00:00.000Z".into(),
            until: None,
        },
    );
    s.org.groups = vec![org::Group {
        group_id: "grp_locust".into(),
        name: "Locust".into(),
        created_at: "2026-11-01T00:00:00.000Z".into(),
        instructions: String::new(),
    }];
    s.org.members.insert("sample-3".into(), "grp_locust".into());
}

pub(super) fn org_sidebar_scene(s: &mut State) {
    org_base(s, false);
}

pub(super) fn org_sidebar_light_scene(s: &mut State) {
    org_base(s, true);
}

pub(super) fn org_search_scene(s: &mut State) {
    org_base(s, false);
    s.sidebar_query.replace_text("deploy");
}

pub(super) fn org_search_light_scene(s: &mut State) {
    org_search_scene(s);
    s.appearance = Some(false);
}

pub(super) fn org_find_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.find_open = true;
    s.find_pieces = vec!["The deploy failed. ".into(), "Deploy again?".into()];
    s.find_query.replace_text("deploy");
    refresh_find_hits(s);
}

pub(super) fn org_find_light_scene(s: &mut State) {
    org_find_scene(s);
    s.appearance = Some(false);
}

pub(super) fn org_palette_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.palette_open = true;
    let missions = sidebar_missions_now(s);
    s.palette_actions = default_palette_actions(&missions, false);
    s.palette_query.replace_text("");
}

pub(super) fn org_palette_light_scene(s: &mut State) {
    org_palette_scene(s);
    s.appearance = Some(false);
}

pub(super) fn org_beside_scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.beside = Some(beside::BesideState {
        mission_id: "sample-2".into(),
        title: "Count from 1 to 150.".into(),
        owner_name: Some("Sable".into()),
        prompt: "Count from 1 to 150.".into(),
        reply_lines: vec!["1".into(), "2".into(), "3".into()],
    });
}

pub(super) fn org_beside_light_scene(s: &mut State) {
    org_beside_scene(s);
    s.appearance = Some(false);
}

pub(super) fn diagram_scene(s: &mut State) {
    ledger_scene(s, false, false);
    if let Some(pane) = &mut s.ledger {
        pane.items = vec![
            ledger::Item::User("Draw how a deploy runs.".into()),
            ledger::Item::Assistant {
                item_id: "diagram-example".into(),
                r#final: true,
                text: "Here is how a deploy runs:\n\n```mermaid\ngraph LR\n  A[Push] --> B[Tests] --> C[Staging]\n```\n\nStaging goes first.\n\n```mermaid\nkanban\n  Todo\n```\n".into(),
            },
        ];
    }
}

pub(super) fn diagram_light_scene(s: &mut State) {
    diagram_scene(s);
    s.appearance = Some(false);
}

pub(super) fn math_scene(s: &mut State) {
    ledger_scene(s, false, false);
    if let Some(pane) = &mut s.ledger {
        pane.items = vec![
            ledger::Item::User("Show the linear algebra.".into()),
            ledger::Item::Assistant {
                item_id: "math-example".into(),
                r#final: true,
                text: "The first coefficient is $c_1 = y$, and the whole system is\n\n$$\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}$$\n\nMoney like $5 and $10 stays prose.".into(),
            },
        ];
    }
}

pub(super) fn math_light_scene(s: &mut State) {
    math_scene(s);
    s.appearance = Some(false);
}

pub(super) fn documents_scene(s: &mut State) {
    file_viewer_scene(s);
    if let Some(pane) = &mut s.ledger {
        pane.items.push(ledger::Item::Assistant {
            item_id: "documents-example".into(),
            r#final: true,
            text: "Attached: notes.pdf, budget.xlsx, Brief.docx. Diagram and math above.".into(),
        });
    }
}

pub(super) fn documents_light_scene(s: &mut State) {
    documents_scene(s);
    s.appearance = Some(false);
}

pub(super) fn reply_code_scene(s: &mut State) {
    ledger_scene(s, false, false);
    if let Some(pane) = &mut s.ledger {
        pane.items = vec![ledger::Item::User("Show the code and the file reference.".into()), ledger::Item::Assistant {
            item_id: "reply-code".into(), r#final: true,
            text: "The total now keeps the original price.\n\n```ts\nconst total = items.reduce((sum, item) => sum + item.price, 0);\nexport default \"done\"; // keep every character\n```\n\nRead [src/cart.ts:12-18](src/cart.ts:12-18) for the calculation.".into() }];
        pane.snapshot.context = Some(context_usage::Reading {
            used: 164000,
            window: 200000,
            percent: 82,
        });
    }
}
pub(super) fn reply_code_light_scene(s: &mut State) {
    reply_code_scene(s);
    s.appearance = Some(false);
}
pub(super) fn file_viewer_scene(s: &mut State) {
    reply_code_scene(s);
    s.file_view = Some((
        reply_actions::FileTarget {
            path: "src/cart.ts".into(),
            start: 12,
            end: 18,
        },
        Ok((1..=22)
            .map(|n| format!("// line {n}: synthetic file reader fixture"))
            .collect::<Vec<_>>()
            .join("\n")),
    ));
}
pub(super) fn file_viewer_light_scene(s: &mut State) {
    file_viewer_scene(s);
    s.appearance = Some(false);
}
pub(super) fn recovery_scene(s: &mut State) {
    reply_code_scene(s);
    if let Some(pane) = &mut s.ledger {
        pane.items.truncate(1);
        pane.snapshot.recovery = Some(recovery::Offer::Resume {epoch:3,note:"One action started and never reported an outcome, so whether it took effect is unknown.".into(),unverified:vec!["command: npm publish".into()]});
    }
}
pub(super) fn recovery_light_scene(s: &mut State) {
    recovery_scene(s);
    s.appearance = Some(false);
}

/// A turn whose plan ran out: the warning, Locust's note, and the card that carries it on.
pub(super) fn carry_on_scene(s: &mut State) {
    reply_code_scene(s);
    if let Some(pane) = &mut s.ledger {
        pane.items.truncate(2);
        pane.items.push(ledger::Item::Note { level: "warning".into(), text: "You've hit your usage limit. Upgrade to Pro or try again in 3 hours.".into() });
        pane.items.push(ledger::Item::CarryOn { conversation_id: "mission_scene".into(), from: "codex".into(), why: "plan".into() });
        pane.snapshot.running = false;
        pane.after_tip = None;
    }
    s.flight = None;
    s.carry_offer = Some(("claude".into(), None));
}
pub(super) fn carry_on_light_scene(s: &mut State) {
    carry_on_scene(s);
    s.appearance = Some(false);
}

pub(super) fn people_scene(s: &mut State) {
    team_scene(s);
    s.people_panel.open = true;
    s.people_panel.entry = serde_json::json!({"teammateId":"codec"});
    s.people_panel.team = serde_json::json!({"schemaVersion":1,"teammates":[
        {"teammateId":"codec","name":"Codec","role":"Code & Migrations","route":{"runtime":"codex","model":"gpt-6.1-sol","mode":"accept-edits","effort":"high"}},
        {"teammateId":"argon","name":"Argon","role":"Code & Migrations","route":{"runtime":"antigravity","model":"gemini-3.8-flash","mode":"auto"}},
        {"teammateId":"scout","name":"Scout","role":"Custom","route":{"runtime":"opencode","model":"opencode/deepseek-v4.1-flash-free","mode":"ask"}}
    ],"settings":{}});
    s.people_panel.fields = [
        "Codec",
        "Code & Migrations",
        "Read before you change. Run what proves it.",
        "codex",
        "gpt-6.1-sol",
        "high",
        "ask",
        "/Users/locust/Documents/Locust",
    ]
    .iter()
    .map(|t| {
        let mut e = TextEdit::default();
        e.replace_text(*t);
        e
    })
    .collect();
}
pub(super) fn people_light_scene(s: &mut State) {
    people_scene(s);
    s.appearance = Some(false);
}
pub(super) fn memory_scene(s: &mut State) {
    team_scene(s);
    s.people_panel.open = true;
    s.people_panel.memory_tab = true;
    s.people_panel
        .note
        .replace_text("Lead with the result. Read before changing code.");
    s.people_panel.team = serde_json::json!({"settings":{"aboutYouSuggestions":[{"id":"ays_demo","text":"Keep verification explicit.","by":"Codec","at":"2026-11-10T12:00:00Z"}]}});
    s.people_panel.memories = serde_json::json!({"schemaVersion":1,"memories":[{"memoryId":"mem_example","text":"The native suite passes 338 tests.","name":"native-suite","previousText":"The native suite passes 324 tests.","by":{"name":"Codec"},"scope":"workspace","status":"kept","enabled":true}],"forgotten":[{"memory":{"memoryId":"mem_old","text":"Use the previous build folder."}}]});
}
pub(super) fn memory_light_scene(s: &mut State) {
    memory_scene(s);
    s.appearance = Some(false);
}

pub(super) fn keychain_note_scene_dark(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    // Scene forces the note so snaps show copy. Live gating: may_ask_on_this_os + keeps_key.
    let _ = crate::keychain_note::may_ask_on_this_os();
    s.keychain_asking = true;
}

pub(super) fn keychain_note_scene_light(s: &mut State) {
    keychain_note_scene_dark(s);
    s.appearance = Some(false);
}

/// Every control of Locust's kit (look::gallery), the exit test of round C1.
pub(super) fn kit_scene(s: &mut State) {
    s.demo = true;
    s.gallery = true;
    s.appearance = Some(true);
}

pub(super) fn kit_light_scene(s: &mut State) {
    s.demo = true;
    s.gallery = true;
    s.appearance = Some(false);
}

/// Every icon and agent mark, and the Locust lockup (look::gallery::icons_view), the exit test of round C2.
pub(super) fn icons_scene(s: &mut State) {
    s.demo = true;
    s.icon_gallery = true;
    s.appearance = Some(true);
}

pub(super) fn icons_light_scene(s: &mut State) {
    s.demo = true;
    s.icon_gallery = true;
    s.appearance = Some(false);
}

/// The message box's mode menu, open over the composer.
pub(super) fn mode_menu_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.mode_menu = true;
}

pub(super) fn mode_menu_light_scene(s: &mut State) {
    mode_menu_scene(s);
    s.appearance = Some(false);
}

/// The message box's + menu, open.
pub(super) fn plus_menu_scene(s: &mut State) {
    composer_base_scene(s, false);
    s.plus_menu = true;
}

pub(super) fn plus_menu_light_scene(s: &mut State) {
    plus_menu_scene(s);
    s.appearance = Some(false);
}
