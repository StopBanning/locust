//! A question on the side: forks a conversation's session, read-only (Electron `codex-mission.ts`, `side`).

use crate::{approval::Mode, ledger::Tail};
use serde_json::Value;

/// What a question asked ON THE SIDE is told (0.461): it is a copy of the
/// conversation, the conversation carries on without it, and nothing is to
/// change. Short, because the forked session already holds everything else.
pub const SIDE_QUESTION_PREFACE: &str = "This is a side question from the person, asked on a copy of this conversation while the conversation itself carries on. Answer it from what you already know and what you can read. Change nothing and start nothing.";

/// How much of a running turn's ask a side question carries (0.515): enough to know the task.
pub const SIDE_RUNNING_ASK_CHARS: usize = 2_000;

/// Maximum hops back through `continuesFrom` when searching for a session to fork.
const MAX_FORK_HOPS: usize = 8;

/// Parsed `startedBy` payload for a side question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideRequest {
    pub of: String,
    pub question: u64,
}

/// Prepared parameters for starting a forked, read-only side question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedSide {
    pub resume_thread: String,
    pub running_ask: Option<String>,
    pub runtime_prompt: String,
}

/// Parse `startedBy` when `kind == "side"`.
pub fn parse_started_by(started_by: Option<&Value>) -> Option<SideRequest> {
    let v = started_by?;
    if v.get("kind").and_then(Value::as_str) != Some("side") {
        return None;
    }
    let of = v.get("of").and_then(Value::as_str)?.trim();
    if of.is_empty() {
        return None;
    }
    let question = v
        .get("question")
        .and_then(Value::as_u64)
        .filter(|&n| n >= 1)
        .unwrap_or(1);
    Some(SideRequest {
        of: of.to_owned(),
        question,
    })
}

/// Runtimes that can copy a session for a side question: Claude Code (`--fork-session`),
/// Codex (`thread/fork`), and OpenCode (`/session/:id/fork`).
pub fn can_fork_runtime(runtime: &str) -> bool {
    matches!(runtime, "claude" | "codex" | "opencode")
}

/// Build the runtime prompt for a side question, adding the running turn's ask when the
/// forked session predates it.
pub fn build_side_prompt(question: &str, running_ask: Option<&str>) -> String {
    let running = match running_ask.map(str::trim).filter(|s| !s.is_empty()) {
        None => String::new(),
        Some(ask) => {
            let shown = if ask.chars().count() > SIDE_RUNNING_ASK_CHARS {
                let head: String = ask.chars().take(SIDE_RUNNING_ASK_CHARS).collect();
                format!("{head}\u{2026}")
            } else {
                ask.to_owned()
            };
            format!(
                "\n\nThe conversation's latest turn is still running, and your copy of the conversation ends before it. That turn was asked:\n\n{shown}"
            )
        }
    };
    format!("{SIDE_QUESTION_PREFACE}{running}\n\n{}", question.trim())
}

struct MissionSnapshot {
    runtime: String,
    prompt: String,
    own_thread: Option<String>,
    continues_mission: Option<String>,
    continues_thread: Option<String>,
}

fn read_mission_snapshot(mission_id: &str) -> Option<MissionSnapshot> {
    let path = crate::native::find_ledger(mission_id)?;
    let records = Tail::new(&path, 0).poll();
    let meta = records
        .iter()
        .find(|r| r["recordType"] == "mission.created")
        .map(|r| &r["metadata"])?;
    let runtime = meta["runtime"].as_str()?.to_owned();
    let prompt = meta["prompt"].as_str().unwrap_or_default().to_owned();
    let mut held: Option<String> = None;
    let mut ended = false;
    let mut moved = false;
    for event in records.iter().filter_map(|r| r.get("event")) {
        let payload = &event["payload"];
        if event["type"] == "run.failed" && payload["sessionEnded"] == true {
            held = None;
            ended = true;
            continue;
        }
        if event["type"] == "adapter.diagnostic" && payload["code"] == "host.folder_moved" {
            moved = true;
        }
        if let Some(t) = payload["runtimeThreadId"]
            .as_str()
            .filter(|t| !t.is_empty())
        {
            held = Some(t.to_owned());
        }
    }
    let own_thread = if moved || ended { None } else { held };
    let continues = meta.get("continuesFrom");
    let continues_mission = continues
        .and_then(|c| c.get("missionId"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let continues_thread = if moved || ended {
        None
    } else {
        continues
            .and_then(|c| c.get("runtimeThreadId"))
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty() && t.len() <= 2048)
            .map(str::to_owned)
    };
    Some(MissionSnapshot {
        runtime,
        prompt,
        own_thread,
        continues_mission,
        continues_thread,
    })
}

/// Validate and prepare a side question against the conversation's ledger.
pub fn prepare(of: &str, runtime: &str, mode: Mode, prompt: &str) -> Result<PreparedSide, String> {
    if !can_fork_runtime(runtime) {
        let display = crate::carry_on::display_name(runtime);
        return Err(format!(
            "{display} cannot answer on the side of a conversation: it has no way to copy one."
        ));
    }
    if mode != Mode::Ask {
        return Err("A question on the side is only ever asked read-only.".into());
    }
    let target = read_mission_snapshot(of)
        .filter(|m| m.runtime == runtime)
        .ok_or_else(|| "That conversation cannot be asked about on the side.".to_owned())?;

    let mut running_ask: Option<String> = None;
    let mut forked_thread: Option<String> = target.own_thread.clone();
    if forked_thread.is_none() {
        running_ask = Some(target.prompt.clone());
        forked_thread = target.continues_thread.clone();
        let mut cursor = target.continues_mission;
        for _ in 1..MAX_FORK_HOPS {
            if forked_thread.is_some() {
                break;
            }
            let Some(earlier_id) = cursor else {
                break;
            };
            let Some(earlier) = read_mission_snapshot(&earlier_id) else {
                break;
            };
            if earlier.runtime != runtime {
                break;
            }
            forked_thread = earlier.own_thread.or(earlier.continues_thread);
            cursor = earlier.continues_mission;
        }
    }

    let resume_thread = forked_thread.ok_or_else(|| {
        "This conversation has no session to ask about yet. Ask again once its reply has started."
            .to_owned()
    })?;
    let runtime_prompt = build_side_prompt(prompt, running_ask.as_deref());
    Ok(PreparedSide {
        resume_thread,
        running_ask,
        runtime_prompt,
    })
}

/// OpenCode 2 runs over ACP, which cannot copy a session (Electron `codex-mission.ts`, `opencodeAcp`): its side
/// question starts a fresh session told what the conversation said, so it can never add to the conversation's own.
pub fn fresh_session_prompt(of: &str, side_prompt: &str) -> String {
    crate::carry_on::compose_cold_follow_up(&crate::native::turns_through(of), side_prompt, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::Teammate,
        native::{self, Origin, Send},
        store::Store,
    };
    use serde_json::json;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn setup_store(name: &str) -> (TempDir, Store) {
        let root = std::env::temp_dir().join(format!(
            "locust-side-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("native/mission-ledger")).unwrap();
        std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
        std::fs::create_dir_all(root.join("work")).unwrap();
        let store = Store::new(root.join("native"), root.join("electron")).unwrap();
        store
            .save_json(
                "teammates.json",
                &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_1", "name": "Codec"}], "missionOwners": {}, "settings": {}}),
            )
            .unwrap();
        (TempDir(root), store)
    }

    fn write_mission(
        store: &Store,
        id: &str,
        runtime: &str,
        session: Option<&str>,
        resumed: Option<&str>,
    ) {
        let mut meta = json!({
            "missionId": id,
            "runId": format!("run_{id}"),
            "runtime": runtime,
            "model": "account-default",
            "mode": "accept-edits",
            "createdAt": "2026-09-29T15:00:00.000Z",
            "prompt": "Add a rename command, with tests."
        });
        if let Some(prev) = resumed {
            meta["continuesFrom"] = json!({
                "missionId": "mission_before",
                "checkpointEpoch": 1,
                "reason": "follow-up",
                "runtimeThreadId": prev
            });
        }
        let mut lines = vec![
            json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": "2026-09-29T15:00:00.000Z", "metadata": meta}).to_string(),
        ];
        if let Some(sess) = session {
            lines.push(
                json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": "2026-09-29T15:00:01.000Z", "event": {"sequence": 1, "type": "run.started", "payload": {"runtimeThreadId": sess}}}).to_string(),
            );
        }
        let path = store.write_path(id).unwrap();
        std::fs::write(path, format!("{}\n", lines.join("\n"))).unwrap();
    }

    #[test]
    fn on_opencode_2_it_is_a_fresh_session_told_the_conversation_never_the_conversations_own() {
        let (_dir, store) = setup_store("acp");
        write_mission(&store, "mission_convo", "opencode", Some("ses_main"), None);
        let path = store.write_path("mission_convo").unwrap();
        let mut lines = std::fs::read_to_string(&path).unwrap();
        lines.push_str(&format!("{}\n", json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 3, "occurredAt": "2026-09-29T15:00:02.000Z", "event": {"sequence": 2, "type": "message.delta", "payload": {"itemId": "m1", "text": "Added rename, with two tests.", "final": true}}})));
        std::fs::write(&path, lines).unwrap();
        let _hook = native::hooks::install(store, None);
        let side = build_side_prompt("Why two tests?", None);
        let prompt = fresh_session_prompt("mission_convo", &side);
        assert!(prompt.starts_with("This conversation has earlier turns, but not in your session: it could not be resumed. What was said, oldest first:\n\n- Asked: \"Add a rename command, with tests.\" -- answered: \"Added rename, with two tests.\"\n\nThe person now asks:"), "{prompt}");
        assert!(prompt.ends_with(&format!("\n\nThe person now asks:\n\n{side}")), "{prompt}");
        let mut acp = std::process::Command::new("opencode");
        acp.arg("acp");
        assert!(crate::runtime::opencode::uses_acp(&acp));
        let mut serve = std::process::Command::new("opencode");
        serve.args(["serve", "--port", "0"]);
        assert!(!crate::runtime::opencode::uses_acp(&serve));
    }

    #[test]
    fn a_side_question_on_opencode_2_opens_a_new_session_and_never_loads_the_conversations() {
        let (dir, store) = setup_store("acp-run");
        write_mission(&store, "mission_convo", "opencode", Some("ses_main"), None);
        let work = dir.0.join("work");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("scripts/fixtures/m4-opencode-acp.py");
        let _hook = native::hooks::install(
            store.clone(),
            Some((PathBuf::from("python3"), vec![fixture.to_string_lossy().into(), "acp".into(), "logged".into()])),
        );
        let teammate = Teammate {
            id: "tm_1".into(),
            name: "Codec".into(),
            role: "Code".into(),
            runtime: Some("opencode".into()),
            model: Some("opencode/beam-free".into()),
            busy: false,
            runs_in: "accept-edits".into(),
            folder: Some(work.to_string_lossy().into()),
        };
        let started = native::send_from(
            Send { teammate, conversation: None, prompt: "Why two tests?".into(), model: None, effort: None },
            Origin {
                mode: Some(Mode::Ask),
                folder: None,
                started_by: Some(json!({"kind": "side", "of": "mission_convo", "question": 1})),
                runtime: Some("opencode".into()),
                reference_ids: None,
            },
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while native::owns(&started.conversation_id) {
            for card in native::waiting_approvals_for(&started.conversation_id) {
                let _ = native::decide_approval(&card.approval_id, crate::approval::Decision::Deny, None);
            }
            assert!(std::time::Instant::now() < deadline, "the side question never finished");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let log = std::fs::read_to_string(work.join("acp-methods.log")).unwrap();
        let methods: Vec<Value> = log.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert!(methods.iter().any(|m| m["method"] == "session/new"), "{log}");
        assert!(!methods.iter().any(|m| m["method"] == "session/load"), "{log}");
        assert!(!log.contains("ses_main"), "{log}");
        let asked = methods.iter().find(|m| m["method"] == "session/prompt").expect("prompted").to_string();
        assert!(asked.contains("This conversation has earlier turns, but not in your session"), "{asked}");
        assert!(asked.contains("Add a rename command, with tests."), "{asked}");
        assert!(asked.contains("Why two tests?"), "{asked}");
    }

    #[test]
    fn preface_tells_it_only_what_it_is() {
        assert!(SIDE_QUESTION_PREFACE.contains("copy of this conversation"));
        assert!(SIDE_QUESTION_PREFACE.contains("Change nothing"));
    }

    #[test]
    fn build_side_prompt_includes_running_ask_only_when_present_and_caps_it() {
        let settled = build_side_prompt("Why?", None);
        assert!(settled.starts_with(SIDE_QUESTION_PREFACE));
        assert!(!settled.contains("still running"));
        assert!(settled.ends_with("\n\nWhy?"));

        let running = build_side_prompt("Why?", Some("Add a rename command, with tests."));
        assert!(running.contains(
            "The conversation's latest turn is still running, and your copy of the conversation ends before it. That turn was asked:\n\nAdd a rename command, with tests."
        ));

        let long_ask = "x".repeat(SIDE_RUNNING_ASK_CHARS + 50);
        let capped = build_side_prompt("Why?", Some(&long_ask));
        assert!(capped.contains(&format!("{}\u{2026}", "x".repeat(SIDE_RUNNING_ASK_CHARS))));
    }

    #[test]
    fn copies_the_session_a_turn_still_running_resumed_before_it_has_said_its_own() {
        let (_dir, store) = setup_store("resumed");
        write_mission(&store, "mission_convo", "claude", None, Some("sess-0"));
        let _hook = native::hooks::install(store, None);
        let prepared = prepare("mission_convo", "claude", Mode::Ask, "Why did you pick that?").unwrap();
        assert_eq!(prepared.resume_thread, "sess-0");
        assert_eq!(
            prepared.running_ask.as_deref(),
            Some("Add a rename command, with tests.")
        );
        assert!(prepared.runtime_prompt.contains(
            "The conversation's latest turn is still running, and your copy of the conversation ends before it. That turn was asked:\n\nAdd a rename command, with tests."
        ));
    }

    #[test]
    fn is_told_what_the_running_turn_was_asked_only_when_copy_ends_before_it() {
        let (_dir, store) = setup_store("settled");
        write_mission(&store, "mission_convo", "claude", Some("sess-1"), None);
        let _hook = native::hooks::install(store, None);
        let prepared = prepare("mission_convo", "claude", Mode::Ask, "Why?").unwrap();
        assert_eq!(prepared.resume_thread, "sess-1");
        assert_eq!(prepared.running_ask, None);
        assert!(!prepared.runtime_prompt.contains("still running"));
    }

    #[test]
    fn is_refused_when_there_is_no_session_to_copy() {
        let (_dir, store) = setup_store("nosess");
        write_mission(&store, "mission_convo", "claude", None, None);
        let _hook = native::hooks::install(store, None);
        let err = prepare("mission_convo", "claude", Mode::Ask, "Why?").unwrap_err();
        assert!(err.contains("no session to ask about yet"), "{err}");
    }

    #[test]
    fn is_only_ever_read_only() {
        let (_dir, store) = setup_store("readonly");
        write_mission(&store, "mission_convo", "claude", Some("sess-1"), None);
        let _hook = native::hooks::install(store, None);
        let err = prepare("mission_convo", "claude", Mode::AcceptEdits, "Why?").unwrap_err();
        assert!(err.contains("read-only"), "{err}");
    }

    #[test]
    fn is_refused_on_a_runtime_that_cannot_copy_a_session() {
        let (_dir, store) = setup_store("cursor");
        write_mission(&store, "mission_convo", "cursor", Some("sess-1"), None);
        let _hook = native::hooks::install(store, None);
        let err = prepare("mission_convo", "cursor", Mode::Ask, "Why?").unwrap_err();
        assert!(err.contains("cannot answer on the side"), "{err}");
    }

    #[test]
    fn forks_the_conversations_session_read_only_and_records_as_side_not_next_turn() {
        let (dir, store) = setup_store("fork-run");
        write_mission(&store, "mission_convo", "codex", Some("thread_1"), None);
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("scripts/fixtures/m1-app-server.py");
        let _hook = native::hooks::install(
            store.clone(),
            Some((PathBuf::from("python3"), vec![fixture.to_string_lossy().into(), "success".into()])),
        );
        let teammate = Teammate {
            id: "tm_1".into(),
            name: "Codec".into(),
            role: "Code".into(),
            runtime: Some("codex".into()),
            model: Some("synthetic-model".into()),
            busy: false,
            runs_in: "accept-edits".into(),
            folder: Some(dir.0.join("work").to_string_lossy().into()),
        };
        let started = native::send_from(
            Send {
                teammate,
                conversation: None,
                prompt: "Why did you pick that library?".into(),
                model: Some("synthetic-model".into()),
                effort: None,
            },
            Origin {
                mode: Some(Mode::Ask),
                folder: None,
                started_by: Some(json!({"kind": "side", "of": "mission_convo", "question": 1})),
                runtime: Some("codex".into()),
                reference_ids: None,
            },
        )
        .unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while native::owns(&started.conversation_id) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let side_path = store.write_path(&started.conversation_id).unwrap();
        let records = Tail::new(&side_path, 0).poll();
        let meta = &records
            .iter()
            .find(|r| r["recordType"] == "mission.created")
            .unwrap()["metadata"];
        assert_eq!(
            meta["startedBy"],
            json!({"kind": "side", "of": "mission_convo", "question": 1})
        );
        assert!(meta.get("continuesFrom").is_none());
        assert_eq!(meta["sandbox"], "read-only");
        assert_eq!(meta["mode"], "ask");

        let transcript = crate::ledger::read_transcript(&side_path);
        assert!(
            transcript
                .items
                .iter()
                .any(|i| matches!(i, crate::ledger::Item::Assistant { text, .. } if text == "Side answer from copy.")),
            "items={:?}\nrecords={records:?}",
            transcript.items
        );

        // Side questions are never listed as conversations in the sidebar.
        let recent = crate::ledger::list_recent(store.ledger_dir().unwrap(), 20);
        assert!(recent.iter().all(|s| s.mission_id != started.conversation_id));
        assert!(recent.iter().any(|s| s.mission_id == "mission_convo"));
    }
}
