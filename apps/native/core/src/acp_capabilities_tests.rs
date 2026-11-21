//! What an ACP agent says it can do is kept per runtime and survives a relaunch, and a file that is not the right
//! shape is read as nothing (Electron's main/what-an-acp-agent-can-do-is-kept.test.ts).
use super::*;

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            ".acp-capabilities-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// continuesSessions, images, audio, embeddedContext, mcpOverHttp, mcpOverSse.
const SAID: [bool; 6] = [true, true, false, true, false, false];
fn at() -> String {
    "2026-10-03T12:00:00.000Z".into()
}
fn said_json(told_at: &str) -> Value {
    json!({"continuesSessions": true, "images": true, "audio": false, "embeddedContext": true, "mcpOverHttp": false, "mcpOverSse": false, "toldAt": told_at})
}

// case: keeps what the agent said, and reads it back after a relaunch
#[test]
fn keeps_what_the_agent_said_and_reads_it_back_after_a_relaunch() {
    let root = Scratch::new();
    let mut first = AcpCapabilitiesStore::new(&root.0, at);
    first.load();
    assert_eq!(first.get("copilot"), None);
    first.record("copilot", SAID);
    assert_eq!(first.get("copilot"), Some(said_json("2026-10-03T12:00:00.000Z")));
    let mut again = AcpCapabilitiesStore::new(&root.0, now_iso);
    again.load();
    assert_eq!(again.get("copilot"), Some(said_json("2026-10-03T12:00:00.000Z")));
    assert_eq!(again.get("codex"), None);
    // The file Electron writes: version 1, two-space JSON, the keys in its order, and no temp file left beside it.
    let text = std::fs::read_to_string(root.0.join("acp-capabilities.json")).unwrap();
    assert!(text.starts_with("{\n  \"schemaVersion\": 1,\n  \"runtimes\": {\n    \"copilot\": {\n      \"continuesSessions\": true,\n      \"images\": true,\n      \"audio\": false,\n      \"embeddedContext\": true,\n      \"mcpOverHttp\": false,\n      \"mcpOverSse\": false,\n      \"toldAt\": \"2026-10-03T12:00:00.000Z\"\n"), "{text}");
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1);
    // A second runtime is kept beside the first, and the first is not lost; a load replaces what was known.
    first.record("kilo", [false; 6]);
    let mut third = AcpCapabilitiesStore::new(&root.0, now_iso);
    third.load();
    assert_eq!(third.get("copilot"), Some(said_json("2026-10-03T12:00:00.000Z")));
    assert_eq!(third.get("kilo").unwrap()["images"], false);
    std::fs::write(root.0.join("acp-capabilities.json"), "").unwrap();
    third.load();
    assert_eq!(third.get("copilot"), None);
}

// case: reads a file of the wrong shape as nothing, entry by entry
#[test]
fn reads_a_file_of_the_wrong_shape_as_nothing_entry_by_entry() {
    let root = Scratch::new();
    let file = root.0.join("acp-capabilities.json");
    let mut cursor = said_json("2026-10-03T12:00:00.000Z");
    cursor["images"] = json!("yes");
    let mut no_audio = said_json("2026-10-03T12:00:00.000Z");
    no_audio.as_object_mut().unwrap().remove("audio");
    let mut no_time = said_json("2026-10-03T12:00:00.000Z");
    no_time.as_object_mut().unwrap().remove("toldAt");
    std::fs::write(&file, json!({
        "schemaVersion": 1,
        "runtimes": {
            "copilot": said_json("2026-10-03T12:00:00.000Z"),
            "claude": said_json("2026-10-03"),
            "cursor": cursor,
            "gemini": said_json("whenever"),
            "kilo": no_audio,
            "opencode": no_time,
            "qwen": "not an object",
        }
    }).to_string()).unwrap();
    let mut store = AcpCapabilitiesStore::new(&root.0, now_iso);
    store.load();
    assert_eq!(store.get("copilot").unwrap()["images"], true);
    // A bare date is a time too (`Date.parse("2026-10-03")`).
    assert_eq!(store.get("claude").unwrap()["toldAt"], "2026-10-03");
    for runtime in ["cursor", "gemini", "kilo", "opencode", "qwen"] {
        assert_eq!(store.get(runtime), None, "{runtime}");
    }
    // Only version 1 is read; any other, or none, is nothing.
    for version in [json!(2), json!("1"), Value::Null] {
        std::fs::write(&file, json!({"schemaVersion": version, "runtimes": {"copilot": said_json("2026-10-03T12:00:00.000Z")}}).to_string()).unwrap();
        let mut other = AcpCapabilitiesStore::new(&root.0, now_iso);
        other.load();
        assert_eq!(other.get("copilot"), None, "{version}");
    }
    std::fs::write(&file, json!({"schemaVersion": 1, "runtimes": [said_json("2026-10-03T12:00:00.000Z")]}).to_string()).unwrap();
    let mut listed = AcpCapabilitiesStore::new(&root.0, now_iso);
    listed.load();
    assert_eq!(listed.get("0"), None);
    std::fs::write(&file, "{ not json").unwrap();
    let mut broken = AcpCapabilitiesStore::new(&root.0, now_iso);
    broken.load();
    assert_eq!(broken.get("copilot"), None);
}

// What an `initialize` answer says (acp-run.ts acpCapabilitiesOf): each one true only when the agent said true.
#[test]
fn an_agents_initialize_answer_is_read_one_capability_at_a_time() {
    let said = json!({"loadSession": true, "promptCapabilities": {"image": true, "audio": true, "embeddedContext": false}, "mcpCapabilities": {"http": true, "sse": false}});
    assert_eq!(capabilities_of(&said), [true, true, true, false, true, false]);
    let said = json!({"loadSession": "yes", "promptCapabilities": {"image": 1, "embeddedContext": true}, "mcpCapabilities": {"sse": true}});
    assert_eq!(capabilities_of(&said), [false, false, false, true, false, true]);
    // Said false is false, not "said something".
    let said = json!({"loadSession": false, "promptCapabilities": {"image": false, "audio": false, "embeddedContext": false}, "mcpCapabilities": {"http": false, "sse": false}});
    assert_eq!(capabilities_of(&said), [false; 6]);
    assert_eq!(capabilities_of(&Value::Null), [false; 6]);
    assert_eq!(capabilities_of(&json!("everything")), [false; 6]);
}

// Kept only for the runtimes whose runs say it (Kilo, OpenCode, Copilot), in this app's own data folder.
#[test]
fn only_kilo_opencode_and_copilot_are_kept_and_in_the_apps_own_folder() {
    let said = json!({"loadSession": true, "promptCapabilities": {"image": true}});
    record_said("acp_someone_added", &said);
    record_said("", &said);
    assert_eq!(said_by("acp_someone_added"), None);
    assert_eq!(said_by(""), None);
    record_said("copilot", &said);
    assert_eq!(said_by("copilot").unwrap()["images"], true);
    let root = crate::store::Store::local().unwrap().root().to_path_buf();
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(root.join(FILE_NAME)).unwrap()).unwrap();
    assert!(kept["runtimes"]["copilot"]["toldAt"].as_str().is_some_and(|t| t.ends_with('Z') && t.len() == 24), "{kept}");
}

/// One turn through a scripted ACP agent (scripts/fixtures/m4-acp.py) run as `runtime`, to its end.
fn a_turn(runtime: &str, scenario: &str) -> String {
    use crate::runtime::{Runtime, Turn, acp::{Acp, Profile}};
    let root = Scratch::new();
    let mut command = std::process::Command::new("python3");
    command.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m4-acp.py")).arg(scenario).current_dir(&root.0);
    let mut rt = Acp::spawn(&mut command).unwrap();
    rt.set_profile(Profile { runtime: runtime.into(), ..Default::default() });
    let turn = Turn {
        prompt: "Synthetic ACP prompt".into(),
        folder: root.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: crate::approval::Mode::ApproveEach,
        approvals: None,
        mission_id: "acp_capabilities".into(),
        run_id: "run_acp_capabilities".into(),
    };
    rt.start(&turn).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        assert!(std::time::Instant::now() < deadline, "the ACP fake timed out: {scenario}");
        if let Some(e) = rt.next_event().unwrap()
            && matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled")
        {
            return e.kind;
        }
    }
}

// What a Kilo run's agent answers at `initialize` is kept for Kilo, from the run itself (acp.rs `begin`).
#[test]
fn a_kilo_run_keeps_what_its_agent_said_at_initialize() {
    assert_eq!(a_turn("kilo", "capabilities"), "run.completed");
    let kept = said_by("kilo").expect("kept from the run");
    assert_eq!(
        [kept["continuesSessions"].clone(), kept["images"].clone(), kept["audio"].clone(), kept["embeddedContext"].clone(), kept["mcpOverHttp"].clone(), kept["mcpOverSse"].clone()],
        [json!(true), json!(true), json!(false), json!(true), json!(true), json!(false)]
    );
}

// An answer that is not an object is kept as nothing it can do, and the turn goes on to its end.
#[test]
fn an_answer_of_the_wrong_shape_never_stops_the_turn() {
    assert_eq!(a_turn("opencode", "capabilities-garbage"), "run.completed");
    let kept = said_by("opencode").expect("kept from the run");
    assert!(KEYS.iter().all(|key| kept[*key] == false), "{kept}");
}
