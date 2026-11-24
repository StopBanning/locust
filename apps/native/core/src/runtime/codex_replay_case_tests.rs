//! Electron's `packages/runtime-adapters/test/a-real-codex-session-replays-through-the-adapter.test.ts`, case by case:
//! Colin's recorded Codex app-server sessions (`test/fixtures/codex/*.jsonl`) replayed through the caller, a Codex turn
//! as the host runs one (`Codex::spawn`, `start`, `next_event`, `steer`) whose `codex app-server` plays the recording
//! back. The player answers each request Locust makes with the recorded answer, under Locust's own id; a request the
//! recording does not hold (Locust reads the settings first in Ask) gets an empty answer. Electron's recorder and its
//! scrubber (`_tools/record-codex-app-server-fixtures.mjs`) are a JavaScript tool with no Rust counterpart, so the
//! scrubbing case carries what the native side owes the same records: hidden reasoning is never kept, the public
//! summary and the ids are.
use super::codex_stand_in::{alive, kinds, of, to_the_end, turn_in, turn_with};
use super::*;
use std::path::PathBuf;

const STEER_TEXT: &str = "After the command finishes, answer only STEER_ACCEPTED instead of BEFORE_STEER.";

const PLAYER: &str = r#"
import json, os, sys
rows = [json.loads(l) for l in open(os.environ['REPLAY_ROWS'], encoding='utf-8') if l.strip()]
log = open(os.environ['REPLAY_LOG'], 'a', encoding='utf-8')
ids = {}
def out(m):
    sys.stdout.write(json.dumps(m) + '\n'); sys.stdout.flush()
i = 0
while i < len(rows):
    row = rows[i]
    if row['dir'] == 'exit':
        open(os.environ['REPLAY_DONE'], 'w').write('done')
        break
    if row['dir'] == 'in':
        m = dict(row['message'])
        if 'method' not in m and 'id' in m: m['id'] = ids.get(m['id'], m['id'])
        out(m); i += 1; continue
    want = row['message']
    while True:
        line = sys.stdin.readline()
        if not line: sys.exit(3)
        log.write(line); log.flush()
        got = json.loads(line)
        if want.get('method') is not None and got.get('method') == want['method']:
            if 'id' in want: ids[want['id']] = got.get('id')
            break
        if want.get('method') is None and got.get('method') is None and got.get('id') == want.get('id'): break
        if got.get('method') is not None and 'id' in got:
            out({'jsonrpc': '2.0', 'id': got['id'], 'result': {'config': {}} if got['method'] == 'config/read' else {}})
    i += 1
for line in sys.stdin:
    log.write(line); log.flush()
"#;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../../packages/runtime-adapters/test/fixtures/codex/{name}.jsonl", env!("CARGO_MANIFEST_DIR")))
}

fn rows(name: &str) -> Vec<Value> {
    std::fs::read_to_string(fixture_path(name)).unwrap().lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect()
}

struct Replayed {
    events: Vec<Event>,
    writes: Vec<Value>,
    steered: Option<bool>,
    cards: Vec<crate::approval::Request>,
}

fn replay(name: &str, mode: Mode) -> Replayed {
    let dir = std::env::temp_dir().join(format!("locust-codex-replay-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("player.py"), PLAYER).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(dir.join("player.py"))
        .env("REPLAY_ROWS", fixture_path(name))
        .env("REPLAY_LOG", dir.join("told.jsonl"))
        .env("REPLAY_DONE", dir.join("done"));
    let mut codex = Codex::spawn(&mut command).unwrap();
    let recorded = rows(name);
    let prompt = recorded.iter().find(|r| r["message"]["method"] == "turn/start").unwrap()["message"]["params"]["input"][0]["text"].as_str().unwrap().to_owned();
    let channel = Channel::new();
    let approvals = (mode == Mode::ApproveEach).then(|| channel.clone());
    codex
        .start(&Turn { prompt, model: Some("gpt-6-luna".into()), effort: Some("low".into()), approvals, ..turn_in(mode) })
        .unwrap();
    // A card is declined, as Electron's handler declines it, by the person at the window while the run waits.
    let over = Arc::new(AtomicBool::new(false));
    let person = {
        let (channel, over) = (channel.clone(), over.clone());
        std::thread::spawn(move || {
            let mut cards = vec![];
            while !over.load(Ordering::Relaxed) {
                if let Some(card) = channel.waiting().first() {
                    cards.push(card.clone());
                    let no = crate::approval::Answer::Decision { approval_id: card.approval_id.clone(), decision: crate::approval::Decision::Deny, reason: None };
                    channel.decide(no).unwrap();
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            cards
        })
    };
    let mut events = vec![];
    let mut steered = None;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < deadline, "the replay never ended: {:?}", kinds(&events));
        let Some(e) = codex.next_event().unwrap() else { continue };
        let commanded = e.kind == "tool.started" && e.payload["toolKind"] == "commandExecution";
        let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
        events.push(e);
        if name == "steer" && steered.is_none() && commanded {
            steered = Some(codex.steer(STEER_TEXT));
        }
        if end {
            break;
        }
    }
    over.store(true, Ordering::Relaxed);
    let cards = person.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dir.join("done").exists() {
        assert!(Instant::now() < deadline, "the recording was not played to its end");
        std::thread::sleep(Duration::from_millis(10));
    }
    let pid = codex.child.id();
    drop(codex);
    assert!(!alive(pid), "the server is not left running");
    let writes = std::fs::read_to_string(dir.join("told.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(of(&events, "run.started").len(), 1);
    assert_eq!(of(&events, "run.completed").len(), 1);
    assert!(of(&events, "run.failed").is_empty());
    let usage = &of(&events, "run.completed")[0].payload["usage"];
    assert!(usage["inputTokens"].is_u64() && usage["outputTokens"].is_u64(), "{usage}");
    Replayed { events, writes, steered, cards }
}

fn last_message(events: &[Event]) -> Value {
    of(events, "message.delta").last().unwrap().payload.clone()
}

// case: The recorded approval arrives with its command, is declined, and never becomes a successful tool receipt.
#[test]
fn the_recorded_approval_arrives_with_its_command_and_is_declined() {
    let run = replay("approval-declined", Mode::ApproveEach);
    assert_eq!(run.cards.len(), 1);
    assert!(run.cards[0].detail.contains("FIXTURE_APPROVAL") || run.cards[0].summary.contains("FIXTURE_APPROVAL"), "{:?}", run.cards[0]);
    // The person's no goes back as Codex's own word for it.
    assert!(run.writes.iter().any(|m| m["id"] == 0 && m["result"]["decision"] == "reject"), "{:?}", run.writes);
    assert!(run.events.iter().any(|e| e.kind == "tool.failed" && e.payload["status"] == "declined"));
    assert!(of(&run.events, "tool.completed").is_empty());
    let said = last_message(&run.events);
    assert_eq!((&said["operation"], &said["text"], &said["final"]), (&json!("replace"), &json!("APPROVAL_DECLINED"), &json!(true)));
}

// case: The recorded file change preserves the patch and produces one matched apply_patch receipt.
#[test]
fn the_recorded_file_change_makes_one_matched_receipt() {
    let recorded = rows("file-change");
    let changed = recorded.iter().find(|r| r["dir"] == "in" && r["message"]["method"] == "item/completed" && r["message"]["params"]["item"]["type"] == "fileChange").unwrap();
    let changes = &changed["message"]["params"]["item"]["changes"];
    assert_eq!(changes[0]["path"], "C:\\fixture\\codex\\notes.txt");
    assert!(changes[0]["diff"].as_str().unwrap().contains("+AFTER"));
    let run = replay("file-change", Mode::AcceptEdits);
    let starts: Vec<&Event> = run.events.iter().filter(|e| e.kind == "tool.started" && e.payload["name"] == "apply_patch").collect();
    let ends: Vec<&Event> = run.events.iter().filter(|e| e.kind == "tool.completed" && e.payload["name"] == "apply_patch").collect();
    assert_eq!((starts.len(), ends.len()), (1, 1));
    assert_eq!(ends[0].payload["itemId"], starts[0].payload["itemId"]);
    assert_eq!((&ends[0].payload["status"], &ends[0].payload["command"]), (&json!("completed"), &json!("C:\\fixture\\codex\\notes.txt")));
    let said = last_message(&run.events);
    assert_eq!((&said["text"], &said["final"]), (&json!("FILE_CHANGED"), &json!(true)));
}

// case: The recorded steer targets the live turn, is accepted without interrupting it, and changes the final answer.
#[test]
fn the_recorded_steer_targets_the_live_turn_and_is_accepted() {
    let run = replay("steer", Mode::Ask);
    assert_eq!(run.steered, Some(true));
    let steers: Vec<&Value> = run.writes.iter().filter(|m| m["method"] == "turn/steer").collect();
    assert_eq!(steers.len(), 1);
    assert_eq!(steers[0]["params"], json!({"threadId": "fixture_id_1", "expectedTurnId": "fixture_id_2", "input": [{"type": "text", "text": STEER_TEXT}]}));
    assert!(!run.writes.iter().any(|m| m["method"] == "turn/interrupt"));
    assert_eq!(last_message(&run.events)["text"], "STEER_ACCEPTED");
}

// case: Scrubbing removes personal paths, account data and hidden reasoning while preserving public summaries and correlated ids.
#[test]
fn hidden_reasoning_is_never_kept_and_summaries_and_ids_are() {
    let (mut codex, _told) = turn_with(&[
        ("item/reasoning/textDelta", json!({"threadId": "thread_9", "itemId": "fixture_id_2", "delta": "hidden reasoning"})),
        ("account/rateLimits/updated", json!({"rateLimits": {"planType": "personal-plan"}})),
        ("item/completed", json!({"threadId": "thread_9", "item": {"id": "fixture_id_2", "type": "reasoning", "summary": ["Checking the fixture"], "content": ["hidden reasoning"]}, "email": "person@example.com", "accessToken": "secret-token"})),
        ("turn/completed", json!({"threadId": "thread_9"})),
    ]);
    let events = to_the_end(&mut codex);
    let kept = serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    for gone in ["hidden reasoning", "personal-plan", "person@example.com", "secret-token"] {
        assert!(!kept.contains(gone), "{gone}");
    }
    let step = of(&events, "step.completed")[0];
    assert_eq!((&step.payload["itemId"], &step.payload["message"]), (&json!("fixture_id_2"), &json!("Checking the fixture")));
    assert_eq!(step.thread.as_deref(), Some("thread_9"));
}

// case: Every stored fixture is scrubbed and carries the requested model and low effort, with no reroute or invented limit.
#[test]
fn every_replayed_fixture_is_scrubbed_and_on_the_requested_route() {
    let personal = crate::invariants::regex(
        r#"(?i)C:(?:\\\\|/)Users(?:\\\\|/)|encryptedContent|encrypted_content|accessToken|refreshToken|api[_-]?key|[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}|codex/event/|item/reasoning/textDelta"#,
    );
    for name in ["approval-declined", "file-change", "steer"] {
        let recorded = rows(name);
        let text = serde_json::to_string(&recorded).unwrap();
        assert!(!personal.is_match(&text), "{name}");
        for row in recorded.iter().filter(|r| r["dir"] == "in" || r["dir"] == "out") {
            let message = &row["message"];
            if message["method"] == "turn/start" {
                assert_eq!((&message["params"]["model"], &message["params"]["effort"]), (&json!("gpt-6-luna"), &json!("low")), "{name}");
            }
            assert_ne!(message["method"], "model/rerouted", "{name}");
            if message["params"]["item"]["type"] == "reasoning" {
                assert_eq!(message["params"]["item"]["content"], json!([]), "{name}");
            }
        }
    }
}
