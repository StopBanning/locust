use crate::{
    engine::Teammate,
    native::{self, Origin, Send},
    store::Store,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

const FAKE_CLAUDE: &str = r#"
import json, sys
with open("claude-argv.jsonl", "a") as log:
    print(json.dumps(sys.argv[1:]), file=log)
sys.stdin.readline()
for row in [
    {"type": "system", "subtype": "init", "session_id": "33b21513-a4fb-478d-ad84-21cb7ef30571"},
    {"type": "assistant", "message": {"id": "msg_1", "content": [{"type": "text", "text": "OK"}]}},
    {"type": "result", "subtype": "success", "is_error": False, "result": "OK"},
]:
    print(json.dumps(row), flush=True)
sys.stdin.read()
"#;

struct Host {
    root: PathBuf,
    store: Store,
}

impl Host {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("locust-grid-{}", crate::invariants::unique_nanos()));
        fs::create_dir_all(root.join("work")).unwrap();
        let store = Store::new(root.join("native"), root.join("electron")).unwrap();
        let host = Self { root, store };
        host.settings(false);
        host
    }

    fn settings(&self, ask_connectors: bool) {
        self.store.save_json("teammates.json", &json!({
            "schemaVersion": 1,
            "teammates": [
                {"teammateId": "tm_ticked", "name": "Wren", "connectors": ["claude.ai Gmail", "Robinhood"]},
                {"teammateId": "tm_other", "name": "Ada", "connectors": []}
            ],
            "missionOwners": {},
            "settings": {"askConnectors": ask_connectors, "autoMode": true}
        })).unwrap();
    }

    fn install(&self) -> native::hooks::Guard {
        native::hooks::install(
            self.store.clone(),
            Some((
                PathBuf::from("python3"),
                vec!["-c".into(), FAKE_CLAUDE.into()],
            )),
        )
    }

    fn turn(
        &self,
        teammate: &str,
        mode: &str,
        conversation: Option<&str>,
        origin: Origin,
    ) -> (String, Vec<String>) {
        let folder = self.root.join("work");
        let argv_log = folder.join("claude-argv.jsonl");
        fs::write(&argv_log, "").unwrap();
        let started = native::send_from(
            Send {
                teammate: Teammate {
                    id: teammate.into(),
                    name: teammate.into(),
                    role: String::new(),
                    runtime: Some("claude".into()),
                    model: None,
                    account: None,
                    busy: false,
                    runs_in: mode.into(),
                    folder: Some(folder.to_string_lossy().into_owned()),
                },
                conversation: conversation.map(str::to_owned),
                prompt: "Answer OK.".into(),
                model: None,
                effort: None,
            },
            origin,
        )
        .unwrap();
        let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
        while native::owns(&started.conversation_id) {
            assert!(
                Instant::now() < deadline,
                "the fake Claude turn did not finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let log = fs::read_to_string(argv_log).unwrap();
        (started.conversation_id, serde_json::from_str(&log).unwrap())
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

#[test]
fn native_claude_loads_only_ticked_connectors_and_honors_approval_mode() {
    let host = Host::new();
    let _hook = host.install();
    for (mode, ask_connectors, expected_rules) in [
        (
            "accept-edits",
            false,
            Some("mcp__claude_ai_Gmail__*,mcp__Robinhood__*"),
        ),
        ("ask", false, None),
        ("plan", false, None),
        ("approve-each", false, None),
        ("auto", false, None),
        ("accept-edits", true, None),
    ] {
        host.settings(ask_connectors);
        let (_, args) = host.turn("tm_ticked", mode, None, Origin::default());
        assert!(
            !args.iter().any(|arg| arg == "--strict-mcp-config"),
            "{mode}: {args:?}"
        );
        assert_eq!(
            value_after(&args, "--allowedTools"),
            expected_rules,
            "{mode}: {args:?}"
        );
    }
    for (teammate, origin) in [
        ("tm_other", Origin::default()),
        (
            "tm_ticked",
            Origin {
                slot: Some(native::SlotOrigin {
                    key: "slot:sealed".into(),
                    sealed: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
        ),
    ] {
        let (_, args) = host.turn(teammate, "ask", None, origin);
        assert!(
            args.iter().any(|arg| arg == "--strict-mcp-config"),
            "{args:?}"
        );
        assert_eq!(value_after(&args, "--allowedTools"), None, "{args:?}");
    }
}

#[test]
fn native_claude_forks_a_side_question_and_only_resumes_an_ordinary_reply() {
    let host = Host::new();
    let _hook = host.install();
    let (parent, fresh) = host.turn("tm_other", "ask", None, Origin::default());
    assert!(
        !fresh
            .iter()
            .any(|arg| matches!(arg.as_str(), "--resume" | "--fork-session"))
    );

    let (side, forked) = host.turn(
        "tm_other",
        "ask",
        None,
        Origin {
            started_by: Some(json!({"kind": "side", "of": parent, "question": 1})),
            ..Default::default()
        },
    );
    assert_ne!(side, parent);
    let session = "33b21513-a4fb-478d-ad84-21cb7ef30571";
    assert!(
        forked
            .windows(3)
            .any(|args| args == ["--resume", session, "--fork-session"]),
        "{forked:?}"
    );

    let (reply, resumed) = host.turn("tm_other", "ask", Some(&parent), Origin::default());
    assert_eq!(reply, parent);
    assert_eq!(value_after(&resumed, "--resume"), Some(session));
    assert!(
        !resumed.iter().any(|arg| arg == "--fork-session"),
        "{resumed:?}"
    );
}
