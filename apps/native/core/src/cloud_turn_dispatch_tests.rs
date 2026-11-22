use std::cell::RefCell;
use std::path::Path;

use crate::cloud_turns::{CloudAgent, CloudPrepared, CloudSetup, CloudSignIn, CloudStartPhase};

use super::{
    CloudDispatchError, CloudDispatchInput, CloudPrepareRequest, CloudSpawnCall, CloudTarget,
    CloudTurnHooks, dispatch_cloud_turn,
};

const CLOUD_DIR: &str = "/workspaces/.locust/t_abc123def456";

struct FakeHooks {
    prepared: CloudPrepared,
    prepare_calls: RefCell<Vec<CloudPrepareRequest>>,
    carry_calls: RefCell<Vec<Vec<String>>>,
    settled_calls: RefCell<Vec<String>>,
    recorded: RefCell<Vec<String>>,
    hints: RefCell<Vec<Option<String>>>,
    cloud_targets: RefCell<Vec<CloudTarget>>,
    claude_spawns: RefCell<Vec<CloudSpawnCall>>,
    codex_spawns: RefCell<Vec<(String, Vec<String>)>>,
    opencode_spawns: RefCell<Vec<Vec<String>>>,
}

impl FakeHooks {
    fn new(prepared: CloudPrepared) -> Self {
        Self {
            prepared,
            prepare_calls: RefCell::new(Vec::new()),
            carry_calls: RefCell::new(Vec::new()),
            settled_calls: RefCell::new(Vec::new()),
            recorded: RefCell::new(Vec::new()),
            hints: RefCell::new(Vec::new()),
            cloud_targets: RefCell::new(Vec::new()),
            claude_spawns: RefCell::new(Vec::new()),
            codex_spawns: RefCell::new(Vec::new()),
            opencode_spawns: RefCell::new(Vec::new()),
        }
    }
}

impl CloudTurnHooks for FakeHooks {
    fn prepare(
        &self,
        req: CloudPrepareRequest,
        say: &mut dyn FnMut(CloudStartPhase),
    ) -> CloudPrepared {
        say(CloudStartPhase::CloudBringing);
        self.prepare_calls.borrow_mut().push(req);
        self.prepared.clone()
    }

    fn carry(&self, _mission_id: &str, _folder: &Path, files: &[String]) {
        self.carry_calls.borrow_mut().push(files.to_vec());
    }

    fn settled(&self, mission_id: &str) {
        self.settled_calls.borrow_mut().push(mission_id.to_string());
    }

    fn record_mission(&self, prompt: &str) {
        self.recorded.borrow_mut().push(prompt.to_string());
    }

    fn brief_hint(&self, own_folder: Option<&str>) {
        self.hints.borrow_mut().push(own_folder.map(str::to_string));
    }

    fn spawn_cloud_claude(&self, target: &CloudTarget, call: CloudSpawnCall) {
        self.cloud_targets.borrow_mut().push(target.clone());
        self.claude_spawns.borrow_mut().push(call);
    }

    fn spawn_cloud_codex(&self, target: &CloudTarget, bin: &str, args: &[String]) {
        self.cloud_targets.borrow_mut().push(target.clone());
        self.codex_spawns
            .borrow_mut()
            .push((bin.to_string(), args.to_vec()));
    }

    fn spawn_cloud_opencode(&self, target: &CloudTarget, args: &[String]) {
        self.cloud_targets.borrow_mut().push(target.clone());
        self.opencode_spawns.borrow_mut().push(args.to_vec());
    }
}

fn default_ok(bin: Option<&str>) -> CloudPrepared {
    CloudPrepared::Ok {
        dir: CLOUD_DIR.into(),
        codespace: "locust-hearth-x7q9".into(),
        bin: bin.map(str::to_string),
    }
}

#[test]
// case: is refused in words on a runtime it does not run there, before anything
fn is_refused_in_words_on_a_runtime_it_does_not_run_there_before_anything() {
    let hooks = FakeHooks::new(default_ok(None));
    let mut phases = Vec::new();
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "cursor",
            cli_version: Some("1.18.27"),
            model: Some("composer-2"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |p| phases.push(p),
    );
    match res {
        Err(CloudDispatchError::RuntimeStartFailed { message }) => {
            assert!(message.contains("runs OpenCode, Claude Code and Codex teammates for now"));
        }
        other => panic!("expected refusal, got {other:?}"),
    }
    assert!(hooks.prepare_calls.borrow().is_empty());
    assert!(hooks.recorded.borrow().is_empty());
}

#[test]
// case: runs a Claude Code turn there: its own copy, in the copy there, with nothing of this computer's
fn runs_a_claude_code_turn_there_its_own_copy_in_the_copy_there_with_nothing_of_this_computers() {
    let hooks = FakeHooks::new(default_ok(Some("/home/codespace/.locust/npm/bin/claude")));
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "claude",
            cli_version: Some("1.18.27"),
            model: Some("claude-haiku-4-5"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    assert!(res.is_ok());
    assert_eq!(
        hooks.prepare_calls.borrow()[0].agent,
        Some((CloudAgent::Claude, Some("1.18.27".into())))
    );
    assert_eq!(
        *hooks.cloud_targets.borrow(),
        vec![CloudTarget {
            codespace: "locust-hearth-x7q9".into(),
            dir: CLOUD_DIR.into(),
        }]
    );
    let spawns = hooks.claude_spawns.borrow();
    assert_eq!(spawns.len(), 1);
    assert_eq!(spawns[0].path, "/home/codespace/.locust/npm/bin/claude");
    assert_eq!(spawns[0].cwd, CLOUD_DIR);
    assert!(spawns[0].args.contains(&"--print".into()));
    assert!(spawns[0].args.contains(&"--model".into()));
    assert!(spawns[0].args.contains(&"claude-haiku-4-5".into()));
    for local in [
        "--settings",
        "--mcp-config",
        "--permission-prompt-tool",
        "--plugin-dir",
        "--allowedTools",
    ] {
        assert!(!spawns[0].args.iter().any(|a| a == local));
    }
    assert_eq!(*hooks.recorded.borrow(), vec!["Add a greeting file."]);
    assert_eq!(hooks.settled_calls.borrow().len(), 1);
}

#[test]
// case: runs a Codex turn there on its app server, over the connection, never the one here
fn runs_a_codex_turn_there_on_its_app_server_over_the_connection_never_the_one_here() {
    let hooks = FakeHooks::new(default_ok(Some("/home/codespace/.locust/npm/bin/codex")));
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "codex",
            cli_version: Some("1.18.27"),
            model: Some("gpt-6-sol"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    assert!(res.is_ok());
    assert_eq!(
        hooks.prepare_calls.borrow()[0].agent,
        Some((CloudAgent::Codex, Some("1.18.27".into())))
    );
    assert_eq!(
        *hooks.cloud_targets.borrow(),
        vec![CloudTarget {
            codespace: "locust-hearth-x7q9".into(),
            dir: CLOUD_DIR.into(),
        }]
    );
    let codex = hooks.codex_spawns.borrow();
    assert_eq!(codex.len(), 1);
    assert_eq!(codex[0].0, "/home/codespace/.locust/npm/bin/codex");
    assert!(codex[0].1.contains(&"app-server".into()));
    assert_eq!(hooks.settled_calls.borrow().len(), 1);
}

#[test]
// case: hands the words back and offers the sign-in when the agent is not signed in there
fn hands_the_words_back_and_offers_the_sign_in_when_the_agent_is_not_signed_in_there() {
    let sign_in = CloudSignIn {
        computer: "locust-hearth-x7q9".into(),
        repo: "acme/hearth".into(),
        agent: CloudAgent::Claude,
    };
    let hooks = FakeHooks::new(CloudPrepared::Err {
        message: "Claude Code is not signed in on your cloud computer yet.".into(),
        setup: None,
        sign_in: Some(sign_in.clone()),
    });
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "claude",
            cli_version: Some("1.18.27"),
            model: Some("claude-haiku-4-5"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    assert_eq!(
        res,
        Err(CloudDispatchError::CloudSignInNeeded {
            message: "Claude Code is not signed in on your cloud computer yet.".into(),
            sign_in,
        })
    );
    assert!(hooks.recorded.borrow().is_empty());
    assert!(hooks.cloud_targets.borrow().is_empty());
}

#[test]
// case: runs OpenCode's server on the cloud computer, readied first and said on the live line
fn runs_opencodes_server_on_the_cloud_computer_readied_first_and_said_on_the_live_line() {
    let hooks = FakeHooks::new(default_ok(None));
    let mut phases = Vec::new();
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "opencode",
            cli_version: Some("1.18.27"),
            model: Some("opencode/space-bunny-free"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |p| phases.push(p),
    );
    assert!(res.is_ok());
    assert_eq!(
        hooks.prepare_calls.borrow()[0].open_code_version.as_deref(),
        Some("1.18.27")
    );
    assert!(phases.contains(&CloudStartPhase::CloudBringing));
    let oc = hooks.opencode_spawns.borrow();
    assert_eq!(oc.len(), 1);
    assert_eq!(oc[0], vec!["serve", "--port", "0"]);
    assert_eq!(hooks.settled_calls.borrow().len(), 1);
}

#[test]
// case: tells the brief the turn stands in its own copy there, never this folder
fn tells_the_brief_the_turn_stands_in_its_own_copy_there_never_this_folder() {
    let hooks = FakeHooks::new(default_ok(None));
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "opencode",
            cli_version: Some("1.18.27"),
            model: Some("opencode/space-bunny-free"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    assert!(res.is_ok());
    assert_eq!(
        hooks.hints.borrow().last().and_then(|h| h.as_deref()),
        Some(CLOUD_DIR)
    );
}

#[test]
// case: sends the message's own files to the copy there before the agent starts
fn sends_the_messages_own_files_to_the_copy_there_before_the_agent_starts() {
    let hooks = FakeHooks::new(default_ok(None));
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "opencode",
            cli_version: Some("1.18.27"),
            model: Some("opencode/space-bunny-free"),
            prompt: "Read this file in the workspace before you answer:\n- .locust/attachments/shot.png\n\nWhat is wrong in this picture?",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    assert!(res.is_ok());
    assert_eq!(
        *hooks.carry_calls.borrow(),
        vec![vec![".locust/attachments/shot.png".to_string()]]
    );
    assert_eq!(hooks.opencode_spawns.borrow().len(), 1);
}

#[test]
// case: sends nothing over for a message with no files of its own
fn sends_nothing_over_for_a_message_with_no_files_of_its_own() {
    let hooks = FakeHooks::new(default_ok(None));
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "opencode",
            cli_version: Some("1.18.27"),
            model: Some("opencode/space-bunny-free"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    assert!(res.is_ok());
    assert!(hooks.carry_calls.borrow().is_empty());
}

#[test]
// case: hands the words back and asks first when the repository has no cloud computer, recording nothing
fn hands_the_words_back_and_asks_first_when_the_repository_has_no_cloud_computer_recording_nothing() {
    let setup = CloudSetup {
        repo: "acme/hearth".into(),
        branch: Some("main".into()),
        permission: false,
    };
    let hooks = FakeHooks::new(CloudPrepared::Err {
        message: "Your cloud computer for this repository is not set up yet.".into(),
        setup: Some(setup.clone()),
        sign_in: None,
    });
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "opencode",
            cli_version: Some("1.18.27"),
            model: Some("opencode/space-bunny-free"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    assert_eq!(
        res,
        Err(CloudDispatchError::CloudSetupNeeded {
            message: "Your cloud computer for this repository is not set up yet.".into(),
            setup,
        })
    );
    assert!(hooks.recorded.borrow().is_empty());
    assert!(hooks.opencode_spawns.borrow().is_empty());
}

#[test]
// case: is refused for a model of the person's own, which answers from this computer
fn is_refused_for_a_model_of_the_persons_own_which_answers_from_this_computer() {
    let hooks = FakeHooks::new(default_ok(None));
    let res = dispatch_cloud_turn(
        CloudDispatchInput {
            mission_id: "mission_1",
            folder: Path::new("/safe-workspace"),
            runtime: "opencode",
            cli_version: Some("1.18.27"),
            model: Some("own-a1b2c3d4/llama"),
            prompt: "Add a greeting file.",
            follows: None,
            side: false,
            set_up: false,
        },
        &hooks,
        &mut |_| {},
    );
    match res {
        Err(CloudDispatchError::RuntimeStartFailed { message }) => {
            assert!(message.contains("A model of your own answers from this computer"));
        }
        other => panic!("expected refusal, got {other:?}"),
    }
    assert!(hooks.prepare_calls.borrow().is_empty());
}
