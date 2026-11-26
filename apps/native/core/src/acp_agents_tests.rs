//! Electron `apps/desktop/src/main/an-agent-of-your-own-is-added-by-its-command.test.ts` (10 cases).
use super::*;
use crate::inspector::what_it_may_do;
use crate::opencode_acp_models::{AcpAgentModel, ModelOption, model_option_of};
use crate::scout::host::is_free_route;
use serde_json::json;

const STUB_PY: &str = r#"
import json, sys

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    def reply(result):
        print(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": result}), flush=True)
    method = msg.get("method")
    if method == "initialize":
        reply({
            "protocolVersion": 1,
            "agentInfo": {"name": "Stub Agent", "version": "2.1.0"},
            "authMethods": [{"id": "login", "name": "Login", "description": "Run stub login in a terminal"}],
        })
    elif method == "session/new":
        if "--locked" in sys.argv:
            print(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32000, "message": "auth required"}}), flush=True)
        else:
            reply({
                "sessionId": "s1",
                "configOptions": [
                    {
                        "id": "model",
                        "category": "model",
                        "currentValue": "stub/fast",
                        "options": [{"group": "Stub", "options": [{"value": "stub/fast", "name": "Fast"}, {"value": "stub/deep", "name": "Deep"}]}],
                    },
                    {
                        "id": "mode",
                        "category": "mode",
                        "currentValue": "build",
                        "options": [{"value": "build", "name": "Build"}],
                    },
                ],
            })
"#;

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "locust-acp-own-{tag}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos(),
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_stub(dir: &Path) -> PathBuf {
    let file = dir.join("stub-agent.py");
    std::fs::write(&file, STUB_PY).unwrap();
    file
}

fn python_launch() -> AcpExecutableLaunch {
    AcpExecutableLaunch {
        command_name: "python3".into(),
        executable_path: "python3".into(),
        prefix_args: Vec::new(),
        env: BTreeMap::new(),
    }
}

// case: is a program and its arguments, never a shell line
#[test]
fn is_a_program_and_its_arguments_never_a_shell_line() {
    assert_eq!(command_words("goose acp"), Some(vec!["goose".into(), "acp".into()]));
    assert_eq!(
        command_words("\"C:\\Program Files\\Goose\\goose.exe\" acp --debug"),
        Some(vec![
            "C:\\Program Files\\Goose\\goose.exe".into(),
            "acp".into(),
            "--debug".into(),
        ])
    );
    assert_eq!(
        command_words("npx -y @zed-industries/claude-code-acp"),
        Some(vec![
            "npx".into(),
            "-y".into(),
            "@zed-industries/claude-code-acp".into(),
        ])
    );
    for refused in [
        "",
        "   ",
        "goose acp && rm -rf ~",
        "goose acp | tee x",
        "cmd /c goose",
        "powershell -c goose",
        "\"unclosed acp",
    ] {
        assert_eq!(command_words(refused), None, "{refused}");
    }
}

// case: is named on a route with the model picked on it
#[test]
fn is_named_on_a_route_with_the_model_picked_on_it() {
    assert_eq!(acp_route_model("acp_goose", None), "acp_goose");
    assert_eq!(acp_route_model("acp_goose", Some("gpt-x")), "acp_goose::gpt-x");
    assert_eq!(
        parsed_acp_route_model("acp_goose::opencode/muse-free"),
        Some(ParsedAcpRoute {
            agent_id: "acp_goose".into(),
            model: Some("opencode/muse-free".into()),
        })
    );
    assert_eq!(
        parsed_acp_route_model("acp_goose"),
        Some(ParsedAcpRoute {
            agent_id: "acp_goose".into(),
            model: None,
        })
    );
    assert_eq!(parsed_acp_route_model("opencode/muse-free"), None);
}

// case: reads the models an agent offers from its session, grouped or not
#[test]
fn reads_the_models_an_agent_offers_from_its_session_grouped_or_not() {
    let kilo: Vec<Value> = (0..336)
        .map(|at| json!({"value": format!("kilo/model-{at}"), "name": format!("Model {at}")}))
        .collect();
    let kilo_opt = model_option_of(&json!({
        "configOptions": [{"id": "model", "category": "model", "currentValue": "kilo/model-0", "options": kilo}]
    }))
    .expect("kilo option parsed");
    assert_eq!(kilo_opt.models.len(), 336);

    assert_eq!(
        model_option_of(&json!({
            "configOptions": [{
                "id": "model",
                "category": "model",
                "currentValue": "a",
                "options": [
                    {"options": [{"value": "a", "name": "A"}]},
                    {"value": "b"}
                ]
            }]
        })),
        Some(ModelOption {
            id: "model".into(),
            current: Some("a".into()),
            models: vec![
                AcpAgentModel { value: "a".into(), name: "A".into() },
                AcpAgentModel { value: "b".into(), name: "b".into() },
            ],
        })
    );
    assert_eq!(
        model_option_of(&json!({"configOptions": [{"id": "mode", "category": "mode", "options": []}]})),
        None
    );
    assert_eq!(model_option_of(&json!({})), None);
}

// case: lists one row per model in the picker, or its own default when it offers none
#[test]
fn lists_one_row_per_model_in_the_picker_or_its_own_default_when_it_offers_none() {
    let rows = acp_catalog(&[
        AcpAgent {
            id: "acp_stub".into(),
            name: "Stub".into(),
            command_line: "stub".into(),
            answer: Some(AcpAgentAnswer {
                agent_name: Some("Stub Agent".into()),
                version: None,
                model_option: Some(ModelOption {
                    id: "model".into(),
                    current: None,
                    models: vec![AcpAgentModel {
                        value: "stub/fast".into(),
                        name: "Fast".into(),
                    }],
                }),
                sign_in: Vec::new(),
            }),
            tested_at: None,
        },
        AcpAgent {
            id: "acp_bare".into(),
            name: "Bare".into(),
            command_line: "bare".into(),
            answer: None,
            tested_at: None,
        },
    ]);
    let summary: Vec<(&str, &str, &str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap(),
                row["displayName"].as_str().unwrap(),
                row["agentName"].as_str().unwrap(),
                row["runtime"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("acp_stub::stub/fast", "Fast", "Stub", "acp"),
            ("acp_bare", "Its own default model", "Bare", "acp"),
        ]
    );
}

// case: keeps only well-formed agents from a file someone edited by hand
#[test]
fn keeps_only_well_formed_agents_from_a_file_someone_edited_by_hand() {
    let kept = parsed_acp_agents(&json!({
        "agents": [
            {"id": "acp_ok", "name": "Ok", "commandLine": "ok acp"},
            {"id": "acp_ok", "name": "Twice", "commandLine": "ok acp"},
            {"id": "not-acp", "name": "Bad id", "commandLine": "x"},
            {"id": "acp_shell", "name": "Shell", "commandLine": "x && y"}
        ]
    }));
    let ids: Vec<&str> = kept.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["acp_ok"]);
}

// case: is tested by opening a session and closing it: its name, version and models, no prompt
#[test]
fn is_tested_by_opening_a_session_and_closing_it_its_name_version_and_models_no_prompt() {
    let dir = TempDir::new("stub");
    let stub = write_stub(&dir.0);
    let tested = test_acp_agent(
        &AcpLaunch {
            launch: python_launch(),
            args: vec![stub.to_string_lossy().into_owned()],
        },
        None,
        Duration::from_secs(10),
    )
    .expect("stub answered");
    assert_eq!(
        tested,
        AcpAgentAnswer {
            agent_name: Some("Stub Agent".into()),
            version: Some("2.1.0".into()),
            model_option: Some(ModelOption {
                id: "model".into(),
                current: Some("stub/fast".into()),
                models: vec![
                    AcpAgentModel { value: "stub/fast".into(), name: "Fast".into() },
                    AcpAgentModel { value: "stub/deep".into(), name: "Deep".into() },
                ],
            }),
            sign_in: vec!["Run stub login in a terminal".into()],
        }
    );
}

// case: says how to sign in when the agent will not open a session
#[test]
fn says_how_to_sign_in_when_the_agent_will_not_open_a_session() {
    let dir = TempDir::new("locked");
    let stub = write_stub(&dir.0);
    let err = test_acp_agent(
        &AcpLaunch {
            launch: python_launch(),
            args: vec![stub.to_string_lossy().into_owned(), "--locked".into()],
        },
        None,
        Duration::from_secs(10),
    )
    .unwrap_err();
    assert_eq!(
        err,
        "It answered, but would not open a session. It says to sign in: Run stub login in a terminal."
    );
}

// case: says so when the program is not an agent at all
#[test]
fn says_so_when_the_program_is_not_an_agent_at_all() {
    let dir = TempDir::new("quiet");
    let quiet = dir.0.join("quiet.py");
    std::fs::write(&quiet, "import sys; sys.exit(3)\n").unwrap();
    let err = test_acp_agent(
        &AcpLaunch {
            launch: python_launch(),
            args: vec![quiet.to_string_lossy().into_owned()],
        },
        None,
        Duration::from_secs(10),
    )
    .unwrap_err();
    assert!(
        err.starts_with("It stopped without answering."),
        "unexpected error: {err}"
    );
}

// case: is kept only when it answered, under a name of its own, and found again for a run
#[test]
fn is_kept_only_when_it_answered_under_a_name_of_its_own_and_found_again_for_a_run() {
    let dir = TempDir::new("store");
    let stub = write_stub(&dir.0);
    let stub_str = stub.to_string_lossy().into_owned();
    let agents = AcpAgentsStore::new(dir.0.join("profile"), |cmd| {
        (cmd == "python3").then(python_launch)
    });

    assert_eq!(
        agents.save("Stub", "nowhere-agent acp").unwrap_err(),
        "Locust could not find nowhere-agent. Install it, or check it opens in a new terminal, then test again."
    );

    let saved = agents
        .save("Stub", &format!("python3 \"{stub_str}\""))
        .expect("saved stub");
    let pairs: Vec<(&str, Option<&str>)> = saved
        .iter()
        .map(|a| (a.id.as_str(), a.answer.as_ref().and_then(|ans| ans.agent_name.as_deref())))
        .collect();
    assert_eq!(pairs, vec![("acp_stub", Some("Stub Agent"))]);

    assert_eq!(
        agents.save("stub", &format!("python3 \"{stub_str}\"")).unwrap_err(),
        "You already have an agent called stub."
    );

    let found = agents.resolve("acp_stub").expect("resolved saved agent");
    assert_eq!(found.found.args, vec![stub_str]);

    assert_eq!(
        agents.resolve("acp_gone").unwrap_err(),
        "That agent is not in Settings > AI agents > Your own agents any more."
    );

    let after_remove = agents.remove("acp_stub").expect("removed");
    assert!(after_remove.is_empty());
    assert!(agents.list().unwrap().is_empty());
}

// case: may do only what it asks and the person approves, and costs something unless it is OpenCode’s own free model
#[test]
fn may_do_only_what_it_asks_and_the_person_approves_and_costs_something_unless_it_is_opencodes_own_free_model() {
    let rows: Vec<String> = what_it_may_do("acp", "workspace-write", Some("approve-each"))
        .rows
        .into_iter()
        .map(|r| r.text)
        .collect();
    assert_eq!(
        rows,
        vec![
            "read the files in this folder",
            "change files and run commands when it asks and you approve; which actions it asks about is the agent's own choice",
        ]
    );
    assert!(is_free_route(
        "acp",
        Some("acp_opencode::opencode/muse-spark-1.3-contributor-free")
    ));
    assert!(!is_free_route("acp", Some("acp_goose::gpt-5")));
    assert!(!is_free_route("acp", Some("acp_goose")));
}

// Not an Electron case: Electron keeps one profile. Native reads Electron's list while it has none of its own
// (`agents_for_store`), so the first change it makes starts from that list, and Electron's file is never written.
#[test]
fn reads_electrons_list_until_it_has_its_own_and_the_first_change_keeps_what_electron_had() {
    let dir = TempDir::new("fallback");
    let (native, electron) = (dir.0.join("native"), dir.0.join("electron"));
    std::fs::create_dir_all(&electron).unwrap();
    let saved = json!({"agents": [
        {"id": "acp_goose", "name": "Goose", "commandLine": "goose acp"},
        {"id": "acp_qwen", "name": "Qwen", "commandLine": "qwen --acp"}
    ]});
    let electron_file = electron.join(ACP_AGENTS_FILE);
    std::fs::write(&electron_file, saved.to_string()).unwrap();
    let agents = AcpAgentsStore::new(native.clone(), |_| None).with_fallback(electron.clone());
    let names = |list: Vec<AcpAgent>| list.into_iter().map(|a| a.name).collect::<Vec<_>>();
    assert_eq!(names(agents.list().unwrap()), vec!["Goose", "Qwen"]);
    assert_eq!(agents.resolve("acp_goose").unwrap_err(), "Locust could not find goose. Install it, or check it opens in a new terminal, then test again.");
    assert_eq!(names(agents.remove("acp_qwen").unwrap()), vec!["Goose"]);
    assert_eq!(names(read_saved_agents(&native)), vec!["Goose"], "the native file starts from Electron's list");
    assert_eq!(std::fs::read_to_string(&electron_file).unwrap(), saved.to_string(), "Electron's file is never written");
    assert_eq!(names(agents.list().unwrap()), vec!["Goose"], "native's own file is read once it has one");
}
