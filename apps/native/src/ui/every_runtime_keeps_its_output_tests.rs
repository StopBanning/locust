//! every-runtime-keeps-its-output.test.tsx (Electron 0.571), through the window: every run recorded beside Electron's
//! runtime adapters (`packages/runtime-adapters/test/fixtures/**/*.jsonl`) is replayed through Rust's own reader for that
//! runtime, each event written to a ledger in a scratch store by the engine's writer (`LedgerWriter::append`, as
//! `native.rs` keeps a run), read back by the window's own ledger read (`ledger_tick` and its `Session` job), and drawn by
//! `thread_view` in a Rune runtime. What the runtime said is rebuilt here from the reader's `message.delta` events (and,
//! for the streams `parity/events.json` holds, checked against the same text rebuilt from Electron's reader's events),
//! never from the ledger read or the drawing.
//!
//! One Rust test per Electron case, each looping over every recording and naming it in its messages. Electron's fourth
//! case takes one of two titles by how the run ended; Rust has a test for each.
use super::*;
use locust_core::ledger_write::LedgerWriter;
use locust_core::runtime::Event;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packages/runtime-adapters/test/fixtures");
const KEPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/every_runtime_keeps_its_output.thread-text.txt");
const STARTED: &str = "2026-10-03T12:00:00.000Z";
/// The conversation's folder, as Electron's test draws the thread (`workspacePath`).
const WORKSPACE: &str = "C:/work";

struct Recording {
    /// `folder/file.jsonl`, as Electron names it.
    name: String,
    /// Which Rust reader takes its records.
    reader: &'static str,
    /// The runtime the ledger records it under.
    runtime: &'static str,
    raw: String,
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|x| x == "jsonl") {
            out.push(path);
        }
    }
}

/// Every recording, sorted by name (Electron's `RECORDINGS`).
fn recordings() -> Vec<Recording> {
    let root = Path::new(FIXTURES);
    let mut paths = Vec::new();
    walk(root, &mut paths);
    let mut out: Vec<Recording> = paths
        .into_iter()
        .map(|path| {
            let name = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            let (folder, file) = name.split_once('/').unwrap();
            let (reader, runtime) = match folder {
                "acp" if file.starts_with("copilot") => ("acp", "copilot"),
                "acp" => ("acp", "opencode"),
                "codex" => ("codex", "codex"),
                "opencode" if file == "serve-events.jsonl" => ("opencode-serve", "opencode"),
                "opencode" => ("opencode", "opencode"),
                f if f.starts_with("agy") => ("agy", "antigravity"),
                "antigravity" => ("antigravity", "antigravity"),
                "claude" => ("claude", "claude"),
                "copilot" => ("copilot", "copilot"),
                "cursor" => ("cursor", "cursor"),
                "muse" => ("muse", "muse"),
                other => panic!("{name}: no Rust reader is named for recordings in {other}/"),
            };
            Recording { raw: std::fs::read_to_string(&path).unwrap(), name, reader, runtime }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn lines_of(raw: &str) -> Vec<String> {
    raw.split('\n').map(|l| l.trim_end_matches('\r').to_owned()).filter(|l| !l.trim().is_empty()).collect()
}

/// The ACP transcript driven as `acp.rs` drives its reader: the session's id from the answer to `session/new` (or
/// load/resume), each `session/update` after the prompt went out, a call the transcript's own answer rejected declined,
/// and the prompt's answer closing the turn. One cut after each transcript line.
fn acp_events(raw: &str, events: &mut Vec<Event>, cuts: &mut Vec<usize>) {
    let mut normal = locust_core::runtime::acp_events::AcpNormalizer::default();
    normal.started_at = STARTED.into();
    let mut session_calls: HashMap<String, Option<String>> = HashMap::new();
    let mut asked: HashMap<String, (String, Vec<Value>)> = HashMap::new();
    let (mut prompted, mut declined) = (false, false);
    for line in lines_of(raw) {
        let line: Value = serde_json::from_str(&line).unwrap();
        let message = &line["message"];
        if message.is_null() {
            cuts.push(events.len());
            continue;
        }
        let id = (!message["id"].is_null()).then(|| message["id"].to_string());
        if line["dir"] == "out" {
            if matches!(message["method"].as_str(), Some("session/new" | "session/load" | "session/resume")) {
                session_calls.insert(id.clone().unwrap(), message["params"]["sessionId"].as_str().map(str::to_owned));
            }
            if message["method"] == "session/prompt" {
                prompted = true;
            }
            if message["method"].is_null()
                && let Some((tool, options)) = id.as_ref().and_then(|id| asked.get(id))
            {
                let chosen = message["result"]["outcome"]["optionId"].as_str();
                let kind = options.iter().find(|o| o["optionId"].as_str() == chosen).and_then(|o| o["kind"].as_str()).unwrap_or("");
                if kind.starts_with("reject") {
                    normal.decline(tool);
                    declined = true;
                }
            }
        } else if line["dir"] == "in" {
            if let Some(id) = &id
                && !message["result"].is_null()
                && let Some(asked_for) = session_calls.get(id)
            {
                normal.thread = message["result"]["sessionId"].as_str().map(str::to_owned).or(asked_for.clone());
            }
            if message["method"] == "session/update"
                && prompted
                && message["params"]["sessionId"].as_str().is_none_or(|s| Some(s) == normal.thread.as_deref())
            {
                events.extend(normal.update(&message["params"]["update"]));
            }
            if message["method"] == "session/request_permission"
                && let Some(id) = &id
            {
                let tool = message["params"]["toolCall"]["toolCallId"].as_str().unwrap_or("").to_owned();
                asked.insert(id.clone(), (tool, message["params"]["options"].as_array().cloned().unwrap_or_default()));
            }
            if let Some(stop) = message["result"]["stopReason"].as_str() {
                events.extend(normal.prompt_boundary(stop));
                let usage = message["result"]["usage"].clone();
                events.extend(normal.finish(stop, if usage.is_object() { usage } else { Value::Null }, false, None, declined));
            }
        }
        cuts.push(events.len());
    }
}

/// Codex app-server's recorded session as its runtime reads it: each notification it was sent, in order.
fn codex_records(raw: &str) -> Vec<(String, Value)> {
    lines_of(raw)
        .iter()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|row| row["dir"] == "in" && row["message"]["method"].is_string() && row["message"]["id"].is_null())
        .map(|row| (row["message"]["method"].as_str().unwrap().to_owned(), row["message"]["params"].clone()))
        .collect()
}

/// OpenCode's server events turned into `run`'s records, as Electron's test turns them (opencode-serve-run.ts
/// `runRecordFor`, each record once per part): Rust's `opencode_serve::Serve` makes the same records from the same events,
/// but only inside a live server session.
fn serve_records(raw: &str) -> Vec<Value> {
    let mut roles: HashMap<String, String> = HashMap::new();
    let mut emitted: HashSet<String> = HashSet::new();
    let mut records = Vec::new();
    for line in lines_of(raw) {
        let event: Value = serde_json::from_str(&line).unwrap();
        let props = &event["properties"];
        if event["type"] == "message.updated"
            && let (Some(id), Some(role)) = (props["info"]["id"].as_str(), props["info"]["role"].as_str())
        {
            roles.insert(id.into(), role.into());
        }
        if event["type"] != "message.part.updated" {
            continue;
        }
        let part = &props["part"];
        let role = part["messageID"].as_str().and_then(|m| roles.get(m)).map(String::as_str);
        let assistant = role == Some("assistant");
        let kind = match part["type"].as_str() {
            Some("step-start") if assistant => "step_start",
            Some("step-finish") if assistant => "step_finish",
            Some("tool") if assistant && matches!(part["state"]["status"].as_str(), Some("completed" | "error")) => "tool_use",
            Some("text") if !part["time"]["end"].is_number() => {
                if assistant && part["synthetic"] != true { "text_partial" } else { continue }
            }
            Some("text") if assistant || part["synthetic"] == true => "text",
            _ => continue,
        };
        if !emitted.insert(format!("{kind}:{}", part["id"].as_str().unwrap_or(""))) {
            continue;
        }
        records.push(json!({"type": kind, "part": part, "timestamp": 0, "sessionID": props["sessionID"]}));
    }
    records
}

/// The run's events, as Rust's reader for its runtime makes them, and the event count after each record (Electron's
/// `replay`): fed every record, then told the process ended cleanly.
fn replay(recording: &Recording) -> (Vec<Event>, Vec<usize>) {
    use locust_core::runtime::*;
    let mut events = Vec::new();
    let mut cuts = Vec::new();
    let lines = lines_of(&recording.raw);
    fn each(out: Vec<Event>, events: &mut Vec<Event>, cuts: &mut Vec<usize>) {
        events.extend(out);
        cuts.push(events.len());
    }
    match recording.reader {
        "acp" => acp_events(&recording.raw, &mut events, &mut cuts),
        "claude" => {
            let mut n = claude_events::ClaudeNormalizer::default();
            for l in &lines {
                each(serde_json::from_str::<Value>(l).map(|v| n.accept(&v)).unwrap_or_default(), &mut events, &mut cuts);
            }
            if !n.ended {
                events.push(n.finish(false, ""));
            }
        }
        "cursor" => {
            let mut n = cursor_events::CursorNormalizer::default();
            for l in &lines {
                each(n.accept_line(l), &mut events, &mut cuts);
            }
            if !n.ended {
                events.push(n.finish(false, Some(0)));
            }
        }
        "copilot" => {
            let mut n = copilot_events::CopilotNormalizer::default();
            for l in &lines {
                each(n.accept_line(l), &mut events, &mut cuts);
            }
            if !n.ended {
                events.push(n.finish(false, Some(0)));
            }
        }
        "agy" => {
            let mut n = agy_events::AgyNormalizer::default();
            for l in &lines {
                each(n.accept_line(l), &mut events, &mut cuts);
            }
            events.extend(n.finish(false, Some(0)));
        }
        "antigravity" => {
            let mut n = antigravity_events::TranscriptNormalizer::default();
            for l in &lines {
                each(n.accept_line(l), &mut events, &mut cuts);
            }
            events.extend(n.finish(false, Some(0)));
        }
        "muse" => {
            let mut n = muse_events::MuseNormalizer::default();
            for l in &lines {
                each(n.accept_line(l), &mut events, &mut cuts);
            }
            events.extend(n.finish(false, Some(0)));
        }
        "opencode" | "opencode-serve" => {
            let mut n = opencode_events::Normalizer::new(None);
            let records: Vec<Value> = if recording.reader == "opencode" {
                lines.iter().filter_map(|l| serde_json::from_str(l).ok()).collect()
            } else {
                serve_records(&recording.raw)
            };
            for r in &records {
                each(n.accept(r), &mut events, &mut cuts);
            }
            events.extend(n.finish(false, Some(0), "", false));
        }
        "codex" => {
            let mut n = events::Normalizer::default();
            for (method, params) in codex_records(&recording.raw) {
                each(n.accept(&method, &params), &mut events, &mut cuts);
            }
            if !n.ended {
                events.push(n.finish(false, ""));
            }
        }
        other => panic!("{}: no reader {other}", recording.name),
    }
    (events, cuts)
}

/// What the runtime said, message by message (Electron's `saidIn`): each `message.delta` appended or replacing what its
/// item said, trimmed, the empty ones left out.
fn said_in<'a>(events: impl IntoIterator<Item = &'a Value>) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut said: HashMap<String, String> = HashMap::new();
    for event in events {
        if event["type"] != "message.delta" {
            continue;
        }
        let p = &event["payload"];
        let id = match &p["itemId"] {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let text = p["text"].as_str().unwrap_or("");
        let held = said.entry(id.clone()).or_insert_with(|| {
            order.push(id.clone());
            String::new()
        });
        if p["operation"] == "replace" {
            *held = text.to_owned();
        } else {
            held.push_str(text);
        }
    }
    order.iter().map(|id| said[id].trim().to_owned()).filter(|t| !t.is_empty()).collect()
}

fn as_values(events: &[Event]) -> Vec<Value> {
    events.iter().map(|e| json!({"type": e.kind, "payload": e.payload})).collect()
}

/// The calls the runtime made (Electron's `runtimeCalls`): tool events' item ids, a host's own look at the disk left out;
/// the helper's own calls apart from the teammate's.
fn runtime_calls(events: &[Event], settled_only: bool, by_helper: bool) -> BTreeSet<String> {
    events
        .iter()
        .filter(|e| if settled_only { matches!(e.kind.as_str(), "tool.completed" | "tool.failed") } else { e.kind.starts_with("tool.") })
        .filter(|e| matches!(e.kind.as_str(), "tool.started" | "tool.completed" | "tool.failed"))
        .filter(|e| e.payload["toolKind"] != "observed_edit")
        .filter(|e| e.payload["parentItemId"].is_string() == by_helper)
        .map(|e| match &e.payload["itemId"] {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect()
}

/// The words of a message in order (Electron's `wordsOf`): a link's address, a list's numbers and the markdown marks are
/// not drawn as text.
fn words_of(text: &str) -> Vec<String> {
    // A link still being written has no closing bracket yet; its address is not drawn either.
    let links = regex::Regex::new(r"\]\([^)]*(\)|$)").unwrap();
    let numbers = regex::Regex::new(r"(?m)^\s*\d+[.)]\s").unwrap();
    let word = regex::Regex::new(r"[\p{L}\p{N}]+").unwrap();
    let text = links.replace_all(text, "]");
    let text = numbers.replace_all(&text, "");
    word.find_iter(&text).map(|m| m.as_str().to_owned()).collect()
}

fn in_order(needles: &[String], haystack: &[String]) -> bool {
    let mut at = 0;
    for word in haystack {
        if at < needles.len() && *word == needles[at] {
            at += 1;
        }
    }
    at == needles.len()
}

/// Electron's `RUN_FAILED`: the words a thread uses for a run that failed.
const RUN_FAILED: [&str; 6] =
    ["ended without a reply", "no answer was recorded", "the run failed", "could not run it", "ended without a record", "did not finish"];

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A run being kept: its ledger in a scratch store, written by the engine's writer, and the window's pane on it.
struct Kept {
    _scratch: Scratch,
    writer: LedgerWriter,
    s: State,
    now: f64,
}

impl Kept {
    fn new(recording: &Recording) -> Self {
        let root = std::env::temp_dir().join(format!("locust-every-runtime-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
        let scratch = Scratch(root.clone());
        let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let mission = "m_recorded";
        let writer = LedgerWriter::create_runtime(&store, mission, "Replay the recorded run.", "recorded", recording.runtime, recording.runtime, &[], None)
            .unwrap_or_else(|e| panic!("{}: the ledger would not take the run: {e}", recording.name));
        let dir = store.ledger_dir().unwrap();
        let mut s = State::default();
        s.ledger = Some(LedgerPane {
            cold_tip: None,
            header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some(recording.runtime.into()),
                model: None, account: None, busy: false, runs_in: "ask".into(), folder: Some(WORKSPACE.into()) },
            conversation: Conversation { conversation_id: Some(mission.into()), turns: Vec::new() },
            items: Vec::new(),
            dir: dir.clone(),
            worker: Some(ledger::Session::new(dir, mission.into())),
            snapshot: ledger::Snapshot::default(),
            due: 0.0,
            loading: true,
            after_tip: None,
            after_tip_deadline: None,
            backoff: 0.0,
        });
        Kept { _scratch: scratch, writer, s, now: 0.0 }
    }

    fn write(&mut self, events: &[Event], name: &str) {
        for e in events {
            self.writer.append(&e.kind, e.payload.clone(), e.thread.as_deref()).unwrap_or_else(|err| panic!("{name}: {} not kept: {err}", e.kind));
        }
    }

    /// The window reads the ledger again, as its tick does: the read starts, its job lands, the tick takes it.
    fn read(&mut self, name: &str) {
        self.now += 1.0;
        let pane = self.s.ledger.as_mut().unwrap();
        pane.due = self.now;
        let mut tx = Tx::default();
        ledger_tick(&mut self.s, self.now, &mut tx);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.s.ledger_job.is_pending() {
            assert!(std::time::Instant::now() < deadline, "{name}: the window's read of the ledger never finished");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        ledger_tick(&mut self.s, self.now, &mut tx);
        assert!(!self.s.ledger.as_ref().unwrap().loading, "{name}: the window did not take the ledger's read");
    }

    fn pane(&self) -> &LedgerPane {
        self.s.ledger.as_ref().unwrap()
    }
}

/// The thread as `thread_view` draws it: each text element under the thread's scroll, in drawing order.
fn drawn(s: &mut State) -> Vec<Drawn> {
    let taken = std::mem::take(s);
    let mut rt = rune::Runtime::new(taken, |s, _| crate::thread_view::thread_view(s, &Palette::new(true)), (900.0, 60000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    *s = std::mem::take(&mut rt.state);
    let Some(top) = all.iter().position(|e| e.key.as_deref() == Some("messages")) else { return Vec::new() };
    let inside = |mut i: usize| loop {
        if i == top {
            return true;
        }
        match all[i].parent {
            Some(p) => i = p,
            None => return false,
        }
    };
    (0..all.len()).filter(|&i| !all[i].exiting && inside(i)).map(|i| Drawn { key: all[i].key.clone(), text: all[i].text.clone() }).collect()
}

/// One element of the drawn thread.
struct Drawn {
    key: Option<String>,
    text: Option<String>,
}

fn text_of(elements: &[Drawn]) -> String {
    let texts: Vec<String> = elements.iter().filter_map(|e| e.text.clone()).collect();
    texts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The texts of the assistant's messages the window holds (Electron's `shownIn`), a message with nothing to say left
/// out as the thread leaves it out (`steps_fold::says_nothing_on_screen`).
fn shown_in(items: &[ledger::Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| match i {
            ledger::Item::Assistant { text, .. } => Some(text.trim().to_owned()),
            _ => None,
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// The thread's rows (Electron's `rowsIn`): each call in the steps lines the thread draws (`steps_fold::thread_pieces`,
/// which `message_column` draws from), once for each time it is drawn; a thought is the model thinking, not a call, and
/// while the run goes a call still going is the live line's, not a row.
fn rows_in(items: &[ledger::Item], running: bool) -> Vec<&ledger::ToolDetail> {
    locust_core::steps_fold::thread_pieces(items, running)
        .into_iter()
        .flat_map(|piece| match piece {
            locust_core::steps_fold::Piece::Steps { group, .. } => group.items,
            locust_core::steps_fold::Piece::Item(i) => vec![i],
        })
        .filter_map(|i| if let ledger::Item::Tool { detail, .. } = &items[i] { Some(detail.as_ref()) } else { None })
        .collect()
}

/// One recording, finished: what the reader made, what the window holds, and what it drew.
struct Finished {
    name: String,
    events: Vec<Event>,
    items: Vec<ledger::Item>,
    failed: bool,
    drawn: Vec<Drawn>,
    text: String,
}

fn finished() -> &'static [Finished] {
    static ALL: OnceLock<Vec<Finished>> = OnceLock::new();
    ALL.get_or_init(|| {
        recordings()
            .iter()
            .map(|recording| {
                let (events, _) = replay(recording);
                let mut kept = Kept::new(recording);
                kept.write(&events, &recording.name);
                kept.read(&recording.name);
                let elements = drawn(&mut kept.s);
                Finished {
                    name: recording.name.clone(),
                    items: kept.pane().items.clone(),
                    failed: kept.pane().snapshot.failed,
                    text: text_of(&elements),
                    drawn: elements,
                    events,
                }
            })
            .collect()
    })
}

/// Electron's reader's events for the streams `parity/events.json` holds, by recording name.
fn electron_events() -> HashMap<String, Vec<Value>> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/parity/events.json");
    let goldens: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    goldens["streams"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| Some((s["fixture"].as_str()?.to_owned(), s["events"].as_array()?.clone())))
        .collect()
}

#[test]
fn every_recording_has_a_reader_and_is_replayed() {
    // The control: a walk that found nothing would pass every case below.
    let all = finished();
    assert_eq!(all.len(), 44, "{:?}", all.iter().map(|f| &f.name).collect::<Vec<_>>());
    for f in all {
        assert!(!f.events.is_empty(), "{}: the reader made nothing of it", f.name);
        assert!(!f.items.is_empty(), "{}: the window holds nothing of it", f.name);
    }
}

// case: shows every message the runtime finished, word for word, and draws each
#[test]
fn shows_every_message_the_runtime_finished_word_for_word_and_draws_each() {
    let electron = electron_events();
    let mut messages = 0;
    for f in finished() {
        let said = said_in(&as_values(&f.events));
        // What Rust's reader heard is what Electron's reader heard, where Electron's events are kept.
        if let Some(theirs) = electron.get(&f.name) {
            assert_eq!(said, said_in(theirs), "{}: Rust's reader heard other words than Electron's", f.name);
        }
        assert_eq!(shown_in(&f.items), said, "{}: the thread holds other messages than the runtime finished", f.name);
        let drawn_words = words_of(&f.text);
        for message in &said {
            assert!(in_order(&words_of(message), &drawn_words), "{}: not drawn word for word:\n{message}\n--- drawn ---\n{}", f.name, f.text);
            messages += 1;
        }
    }
    eprintln!("{messages} finished messages checked word for word");
    assert!(messages >= 40, "only {messages} messages were checked");
}

/// The remarks a thread makes (Electron: its diagnostics and its activity notices).
fn remarks_in(items: &[ledger::Item]) -> Vec<String> {
    items
        .iter()
        .flat_map(|i| match i {
            ledger::Item::Note { text, .. } | ledger::Item::Limit { text, .. } => vec![text.clone()],
            ledger::Item::Trace(trace) => trace.notices.iter().map(|n| n.message.clone()).collect(),
            _ => vec![],
        })
        .collect()
}

// case: says no remark twice in one turn
#[test]
fn says_no_remark_twice_in_one_turn() {
    let mut remarks = 0;
    for f in finished() {
        let held = remarks_in(&f.items);
        let once: BTreeSet<&String> = held.iter().collect();
        assert_eq!(held.len(), once.len(), "{}: a remark is held twice: {held:?}", f.name);
        // And drawn once: no two text elements of the thread say the same remark.
        for remark in &held {
            let times = f.drawn.iter().filter(|d| d.text.as_deref().is_some_and(|t| t.contains(remark.as_str()))).count();
            assert!(times <= 1, "{}: \"{remark}\" is drawn {times} times\n{}", f.name, f.text);
        }
        remarks += held.len();
    }
    eprintln!("{remarks} remarks checked");
    assert!(remarks > 0, "no recording made a remark, so nothing was checked");
}

// case: draws every call it made as one row
#[test]
fn draws_every_call_it_made_as_one_row() {
    let (mut calls, mut helpers) = (0, 0);
    for f in finished() {
        let rows = rows_in(&f.items, false);
        let own = runtime_calls(&f.events, false, false);
        let ids: Vec<&str> = rows.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(rows.len(), own.len(), "{}: {} rows for the {} calls it made: rows {ids:?}, calls {own:?}", f.name, rows.len(), own.len());
        assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len(), "{}: a call is two rows: {ids:?}", f.name);
        // A helper's own calls are rows under its row, not the teammate's (ledger v22).
        let children: usize = rows.iter().map(|d| d.children.len()).sum();
        let by_helper = runtime_calls(&f.events, false, true);
        assert_eq!(children, by_helper.len(), "{}: {children} rows under helpers for their {} calls", f.name, by_helper.len());
        calls += own.len();
        helpers += by_helper.len();
    }
    eprintln!("{calls} calls and {helpers} helper calls checked");
    assert!(calls >= 40 && helpers >= 1, "{calls} calls and {helpers} helper calls checked");
}

// case: does not read as failed
#[test]
fn does_not_read_as_failed() {
    let mut completed = 0;
    for f in finished().iter().filter(|f| f.events.last().is_some_and(|e| e.kind != "run.failed")) {
        assert_eq!(f.events.last().unwrap().kind, "run.completed", "{}: it ended neither completed nor failed", f.name);
        assert!(!f.failed, "{}: the window holds a completed run as failed", f.name);
        let lower = f.text.to_lowercase();
        for words in RUN_FAILED {
            assert!(!lower.contains(words), "{}: a completed run's thread says \"{words}\":\n{}", f.name, f.text);
        }
        completed += 1;
    }
    eprintln!("{completed} completed runs checked");
    assert!(completed >= 30, "only {completed} completed runs checked");
}

// case: says why it failed
#[test]
fn says_why_it_failed() {
    let mut failed: Vec<&str> = Vec::new();
    for f in finished().iter().filter(|f| f.events.last().is_some_and(|e| e.kind == "run.failed")) {
        let why = f.events.last().unwrap().payload["message"].as_str().unwrap_or("").trim().to_owned();
        assert!(!why.is_empty(), "{}: it failed without saying why", f.name);
        assert!(f.failed, "{}: the window does not hold the run as failed", f.name);
        // And the thread says it, in the runtime's words.
        assert!(in_order(&words_of(&why), &words_of(&f.text)), "{}: the thread does not say why it failed ({why}):\n{}", f.name, f.text);
        failed.push(f.name.as_str());
    }
    // The recordings that end failed, by name: a run that stopped reading as failed would leave this list.
    assert_eq!(failed, ["agy-2026-10-02/conversation-not-found.jsonl", "agy-2026-10-02/unknown-model.jsonl", "copilot/policy-denied.jsonl"]);
}

// case: while it runs: names what it is doing, keeps what it said, and only the newest message types
#[test]
fn while_it_runs_names_what_it_is_doing_keeps_what_it_said_and_only_the_newest_message_types() {
    let mut looks = 0;
    for recording in recordings() {
        let (events, cuts) = replay(&recording);
        let mut kept = Kept::new(&recording);
        let mut written = 0;
        let mut seen = BTreeSet::new();
        for cut in cuts {
            if !seen.insert(cut) {
                continue;
            }
            let sofar = &events[..cut];
            if sofar.iter().any(|e| matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled")) {
                break;
            }
            kept.write(&events[written..cut], &recording.name);
            written = cut;
            kept.read(&recording.name);
            let where_ = format!("{} after {cut} events", recording.name);
            let pane = kept.pane();
            assert!(pane.snapshot.running, "{where_}: the window does not hold the run as running");
            let items = pane.items.clone();
            assert_eq!(shown_in(&items), said_in(&as_values(sofar)), "{where_}: the thread holds other messages than were said");
            let messages: Vec<usize> = (0..items.len()).filter(|&i| matches!(items[i], ledger::Item::Assistant { .. })).collect();
            let typing: Vec<usize> = messages
                .iter()
                .copied()
                .filter(|&i| matches!(&items[i], ledger::Item::Assistant { r#final, .. } if locust_core::thread_view::streaming(*r#final, true)))
                .collect();
            assert!(typing.len() <= 1, "{where_}: {} messages type at once", typing.len());
            if let [one] = typing.as_slice() {
                assert_eq!(Some(one), messages.last(), "{where_}: a message types that is not the newest");
            }
            let elements = drawn(&mut kept.s);
            let line: Vec<&Drawn> = elements.iter().filter(|e| e.key.as_deref() == Some("live-step-label")).collect();
            // Something always says the run is going: the live line, naming what it is doing -- or, while a message is
            // being written, that message's caret.
            assert!(line.len() <= 1, "{where_}: {} live lines", line.len());
            if typing.is_empty() {
                assert_eq!(line.len(), 1, "{where_}: nothing says the run is going\n{}", text_of(&elements));
            }
            if let Some(step) = line.first() {
                assert!(step.text.as_deref().is_some_and(|t| !t.trim().is_empty()), "{where_}: the live line names nothing");
            }
            let drawn_words = words_of(&text_of(&elements));
            for message in said_in(&as_values(sofar)) {
                assert!(in_order(&words_of(&message), &drawn_words), "{where_}: not drawn word for word:\n{message}\n--- drawn ---\n{}", text_of(&elements));
            }
            assert_eq!(rows_in(&items, true).len(), runtime_calls(sofar, true, false).len(), "{where_}: rows for the calls that finished");
            looks += 1;
        }
    }
    eprintln!("{looks} looks while running");
    assert!(looks >= 100, "only {looks} looks while running");
}

/// What each recording's finished thread says, as one text (Electron's `says what each recorded run said, as kept`).
/// What changes from one run of the test to the next is kept as a mark: the clock time the turn started at (the
/// "Started ..." line) and how long the run took, which here is however long the writes took.
fn thread_text() -> String {
    finished()
        .iter()
        .map(|f| {
            let kept: Vec<Drawn> = f
                .drawn
                .iter()
                .filter(|d| d.key.as_deref() != Some("mission-started"))
                .map(|d| Drawn { key: d.key.clone(), text: d.text.clone() })
                .collect();
            let said = text_of(&kept)
                .split(' ')
                .map(|word| if regex::Regex::new(r"^\d+(ms|s|m|h)$").unwrap().is_match(word) { "(time)" } else { word })
                .collect::<Vec<_>>()
                .join(" ");
            format!("## {}\n{}\n", f.name, if said.is_empty() { "(nothing in the thread)" } else { &said })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// case: says what each recorded run said, as kept
#[test]
fn says_what_each_recorded_run_said_as_kept() {
    let now = thread_text();
    if std::env::var_os("LOCUST_THREAD_TEXT").is_some() {
        std::fs::write(KEPT, &now).unwrap();
    }
    let kept = std::fs::read_to_string(KEPT).unwrap_or_default();
    if kept != now {
        let first = kept.lines().zip(now.lines()).position(|(a, b)| a != b).unwrap_or(kept.lines().count().min(now.lines().count()));
        panic!(
            "what a recorded run's thread says changed at line {}: kept {:?}, now {:?}. Read the change; if every line is still what \
             that runtime did, keep it with LOCUST_THREAD_TEXT=1.",
            first + 1,
            kept.lines().nth(first),
            now.lines().nth(first)
        );
    }
}
