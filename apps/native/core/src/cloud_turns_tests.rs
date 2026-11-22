use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cloud_tasks::Ran;

use super::{
    AgentInstalled, CarriedFile, CloudAgent, CloudChange, CloudChangeFile, CloudComputers,
    CloudPrepared, CloudPullRequestAction, CloudStartPhase, CloudTurnAction, CloudTurnOutcome,
    CloudTurnState, CloudTurns, Codespace, OpenedPullRequest, PrepareInput, PublicCloudTurn,
    PullRequestTarget, PushedBranch, SettleTarget,
};

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SETTLED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Default)]
struct FakeOptions {
    on_github: bool,
    existing: Option<Codespace>,
    still_open: bool,
    refuse_open: bool,
}

impl FakeOptions {
    fn default_github() -> Self {
        Self {
            on_github: true,
            existing: None,
            still_open: true,
            refuse_open: false,
        }
    }

    fn with_existing(state: &str) -> Self {
        Self {
            on_github: true,
            existing: Some(Codespace {
                name: "locust-hearth-x7q9".into(),
                display_name: "locust-hearth".into(),
                state: state.into(),
                repository: "acme/hearth".into(),
                last_used_at: None,
            }),
            still_open: true,
            refuse_open: false,
        }
    }
}

struct FakeComputers {
    space: RefCell<Option<Codespace>>,
    calls: RefCell<Vec<String>>,
    still_open: bool,
    refuse_open: bool,
}

impl FakeComputers {
    fn new(opts: &FakeOptions) -> Self {
        Self {
            space: RefCell::new(opts.existing.clone()),
            calls: RefCell::new(Vec::new()),
            still_open: opts.still_open,
            refuse_open: opts.refuse_open,
        }
    }
}

impl CloudComputers for FakeComputers {
    fn find(&self, _repo: &str) -> Result<Option<Codespace>, String> {
        Ok(self.space.borrow().clone())
    }

    fn make(&self, _repo: &str, _branch: Option<&str>) -> Result<String, String> {
        self.calls.borrow_mut().push("make".into());
        let made = Codespace {
            name: "locust-hearth-x7q9".into(),
            display_name: "locust-hearth".into(),
            state: "Available".into(),
            repository: "acme/hearth".into(),
            last_used_at: None,
        };
        let name = made.name.clone();
        *self.space.borrow_mut() = Some(made);
        Ok(name)
    }

    fn when_ready(&self, _name: &str) -> Result<(), String> {
        self.calls.borrow_mut().push("ready".into());
        Ok(())
    }

    fn ensure_open_code(&self, _name: &str, version: &str) -> Result<String, String> {
        self.calls.borrow_mut().push(format!("opencode@{version}"));
        Ok(version.into())
    }

    fn ensure_agent(
        &self,
        _name: &str,
        agent: CloudAgent,
        version: &str,
    ) -> Result<AgentInstalled, String> {
        self.calls
            .borrow_mut()
            .push(format!("{}@{version}", agent.as_str()));
        Ok(AgentInstalled {
            version: version.into(),
            bin: format!("/home/codespace/.locust/npm/bin/{}", agent.as_str()),
        })
    }

    fn agent_signed_in(&self, _name: &str, _agent: CloudAgent) -> Result<bool, String> {
        Ok(true)
    }

    fn bring(
        &self,
        _name: &str,
        _repo: &str,
        _folder: &Path,
        task_id: &str,
    ) -> Result<String, String> {
        self.calls.borrow_mut().push(format!("bring:{task_id}"));
        Ok(BASE.into())
    }

    fn counts(
        &self,
        _name: &str,
        _task_id: &str,
        base: &str,
    ) -> Result<Vec<CloudChangeFile>, String> {
        self.calls
            .borrow_mut()
            .push(format!("change:{}", &base[..1]));
        Ok(vec![CloudChangeFile {
            path: "greeting.txt".into(),
            added: 1,
            removed: 0,
            binary: false,
        }])
    }

    fn change(&self, _name: &str, _task_id: &str, base: &str) -> Result<CloudChange, String> {
        self.calls
            .borrow_mut()
            .push(format!("change:{}", &base[..1]));
        Ok(CloudChange {
            patch: "diff --git a/greeting.txt b/greeting.txt\n".into(),
            files: vec![CloudChangeFile {
                path: "greeting.txt".into(),
                added: 1,
                removed: 0,
                binary: false,
            }],
        })
    }

    fn settle(&self, _name: &str, _task_id: &str, went: SettleTarget) -> Result<String, String> {
        self.calls.borrow_mut().push(match went {
            SettleTarget::PullRequest => "settle:pull-request".into(),
            SettleTarget::Here => "settle".into(),
        });
        Ok(SETTLED.into())
    }

    fn discard(&self, _name: &str, _task_id: &str, _base: &str) -> Result<(), String> {
        self.calls.borrow_mut().push("discard".into());
        Ok(())
    }

    fn carry(&self, _name: &str, _task_id: &str, files: &[CarriedFile]) -> Result<(), String> {
        let joined = files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>()
            .join(",");
        self.calls.borrow_mut().push(format!("carry:{joined}"));
        Ok(())
    }

    fn push_change(
        &self,
        _name: &str,
        _repo: &str,
        _task_id: &str,
        base: &str,
        target: &PullRequestTarget,
        _message: &str,
    ) -> Result<PushedBranch, String> {
        let label = match target {
            PullRequestTarget::New { from, wanted } => {
                format!("new:{wanted}:{}", from.as_deref().unwrap_or("-"))
            }
            PullRequestTarget::Add { branch } => format!("add:{branch}"),
        };
        self.calls
            .borrow_mut()
            .push(format!("push:{}:{label}", &base[..1]));
        Ok(match target {
            PullRequestTarget::New { wanted, .. } => PushedBranch {
                branch: wanted.clone(),
                into: "main".into(),
            },
            PullRequestTarget::Add { branch } => PushedBranch {
                branch: branch.clone(),
                into: branch.clone(),
            },
        })
    }

    fn open_pull_request(
        &self,
        _repo: &str,
        head: &str,
        base: &str,
        title: &str,
        _body: &str,
    ) -> Result<OpenedPullRequest, String> {
        self.calls
            .borrow_mut()
            .push(format!("open:{head}->{base}:{title}"));
        if self.refuse_open {
            return Err("GitHub did not open the pull request: a pull request already exists".into());
        }
        Ok(OpenedPullRequest {
            number: 7,
            url: "https://github.com/acme/hearth/pull/7".into(),
        })
    }

    fn pull_request_open(&self, _repo: &str, number: u64) -> Result<bool, String> {
        self.calls.borrow_mut().push(format!("state:{number}"));
        Ok(self.still_open)
    }

    fn drop_branch(&self, _repo: &str, branch: &str) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("drop:{branch}"));
        Ok(())
    }
}

fn with_world<F>(opts: FakeOptions, body: F)
where
    F: FnOnce(
        &Path,
        &CloudTurns<'_, FakeComputers>,
        &FakeComputers,
        &RefCell<Vec<PublicCloudTurn>>,
        &RefCell<Vec<String>>,
    ),
{
    let dir = TempDir::new("locust-cloud-turns");
    let computers = FakeComputers::new(&opts);
    let told: RefCell<Vec<PublicCloudTurn>> = RefCell::new(Vec::new());
    let applied: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let on_github = opts.on_github;
    let git = |args: &[&str], _cwd: &Path| -> Result<Ran, String> {
        Ok(match args.first().copied() {
            Some("remote") if !on_github => Ran::new(2, "", "no such remote"),
            Some("remote") => Ran::new(0, "https://github.com/acme/hearth.git\n", ""),
            Some("rev-parse") => Ran::new(0, "main\n", ""),
            _ => Ran::new(0, "", ""),
        })
    };
    let git_run = |args: &[&str], _cwd: &Path| -> Result<Ran, String> {
        applied.borrow_mut().push(args.join(" "));
        Ok(Ran::new(0, "", ""))
    };
    let turns = CloudTurns::new(
        dir.path.join("cloud-turns.json"),
        &computers,
        &git,
        &git_run,
    )
    .with_on_change(|t| told.borrow_mut().push(t.clone()))
    .with_now(|| "2026-10-12T19:00:00.000Z".into());
    body(&dir.path, &turns, &computers, &told, &applied);
}

#[test]
// case: is refused in words for a folder that is not on GitHub
fn is_refused_in_words_for_a_folder_that_is_not_on_github() {
    let mut opts = FakeOptions::default_github();
    opts.on_github = false;
    with_world(opts, |folder, turns, _computers, _told, _applied| {
        let ready = turns.prepare(
            PrepareInput {
                mission_id: "mission_1",
                folder,
                follows: None,
                open_code_version: Some("1.18.27"),
                agent: None,
                set_up: false,
            },
            &mut |_| {},
        );
        match ready {
            CloudPrepared::Err { message, .. } => assert!(message.contains("is not on GitHub")),
            CloudPrepared::Ok { .. } => panic!("expected refusal"),
        }
    });
}

#[test]
// case: asks first when the repository has no cloud computer, and makes nothing until it is told to
fn asks_first_when_the_repository_has_no_cloud_computer_and_makes_nothing_until_it_is_told_to() {
    with_world(FakeOptions::default_github(), |folder, turns, computers, _told, _applied| {
        let ready = turns.prepare(
            PrepareInput {
                mission_id: "mission_1",
                folder,
                follows: None,
                open_code_version: Some("1.18.27"),
                agent: None,
                set_up: false,
            },
            &mut |_| {},
        );
        match ready {
            CloudPrepared::Err { setup: Some(s), .. } => {
                assert_eq!(s.repo, "acme/hearth");
                assert_eq!(s.branch.as_deref(), Some("main"));
            }
            other => panic!("expected setup ask, got {other:?}"),
        }
        assert!(computers.calls.borrow().is_empty());
    });
}

#[test]
// case: makes one when told to, says each step as it begins, and starts the turn in a copy of its own
fn makes_one_when_told_to_says_each_step_as_it_begins_and_starts_the_turn_in_a_copy_of_its_own() {
    with_world(FakeOptions::default_github(), |folder, turns, computers, told, _applied| {
        let mut phases = Vec::new();
        let ready = turns.prepare(
            PrepareInput {
                mission_id: "mission_1",
                folder,
                follows: None,
                open_code_version: Some("1.18.27"),
                agent: None,
                set_up: true,
            },
            &mut |p| phases.push(p),
        );
        assert!(matches!(ready, CloudPrepared::Ok { .. }));
        assert_eq!(
            phases,
            vec![
                CloudStartPhase::CloudMaking,
                CloudStartPhase::CloudInstalling,
                CloudStartPhase::CloudBringing,
            ]
        );
        let calls = computers.calls.borrow();
        assert_eq!(&calls[0..3], &["make", "ready", "opencode@1.18.27"]);
        assert!(calls[3].starts_with("bring:t_"));
        let last = told.borrow().last().cloned().expect("told");
        assert_eq!(last.mission_id, "mission_1");
        assert_eq!(last.state, CloudTurnState::Working);
        assert_eq!(last.repo, "acme/hearth");
        assert_eq!(last.branch.as_deref(), Some("main"));
        assert_eq!(last.codespace, "locust-hearth-x7q9");
    });
}

#[test]
// case: carries on in the conversation's own copy on the next turn, without bringing the folder again
fn carries_on_in_the_conversations_own_copy_on_the_next_turn_without_bringing_the_folder_again() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, _told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            let mut phases = Vec::new();
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_2",
                    folder,
                    follows: Some("mission_1"),
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |p| phases.push(p),
            );
            let brings = computers
                .calls
                .borrow()
                .iter()
                .filter(|c| c.starts_with("bring:"))
                .count();
            assert_eq!(brings, 1);
            assert!(phases.is_empty());
            assert_eq!(
                turns.of("mission_2").map(|t| t.0),
                turns.of("mission_1").map(|t| t.0)
            );
        },
    );
}

#[test]
// case: wakes a stopped one first, and says so
fn wakes_a_stopped_one_first_and_says_so() {
    with_world(
        FakeOptions::with_existing("Shutdown"),
        |folder, turns, _computers, _told, _applied| {
            let mut phases = Vec::new();
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |p| phases.push(p),
            );
            assert_eq!(phases.first().copied(), Some(CloudStartPhase::CloudWaking));
        },
    );
}

#[test]
// case: keeps its change there once it ends, and only the newest turn offers it
fn keeps_its_change_there_once_it_ends_and_only_the_newest_turn_offers_it() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, _computers, told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_1");
            let last = told.borrow().last().cloned().expect("told");
            assert_eq!(last.mission_id, "mission_1");
            assert_eq!(last.state, CloudTurnState::There);
            assert_eq!(
                last.files,
                Some(vec![CloudChangeFile {
                    path: "greeting.txt".into(),
                    added: 1,
                    removed: 0,
                    binary: false,
                }])
            );

            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_2",
                    folder,
                    follows: Some("mission_1"),
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_2");
            let list = turns.turns(&["mission_1", "mission_2"]);
            assert_eq!(list[0].state, CloudTurnState::Carried);
            assert_eq!(list[1].state, CloudTurnState::There);
        },
    );
}

#[test]
// case: brings it home on Apply, and the copy starts again from what was brought
fn brings_it_home_on_apply_and_the_copy_starts_again_from_what_was_brought() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, _told, applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_1");
            match turns.apply("mission_1") {
                CloudTurnAction::Ok { turn } => {
                    assert_eq!(turn.state, CloudTurnState::Home);
                    assert_eq!(
                        turn.applied_at.as_deref(),
                        Some("2026-10-12T19:00:00.000Z")
                    );
                }
                CloudTurnAction::Err { message } => panic!("expected ok, got {message}"),
            }
            assert!(
                applied
                    .borrow()
                    .iter()
                    .any(|c| c.starts_with("apply --check --binary"))
            );
            assert!(computers.calls.borrow().iter().any(|c| c == "settle"));

            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_2",
                    folder,
                    follows: Some("mission_1"),
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_2");
            let last_change = computers
                .calls
                .borrow()
                .iter()
                .filter(|c| c.starts_with("change:"))
                .last()
                .cloned();
            assert_eq!(last_change.as_deref(), Some("change:b"));
            assert!(matches!(
                turns.apply("mission_1"),
                CloudTurnAction::Err { .. }
            ));
        },
    );
}

#[test]
// case: puts the copy back on Discard
fn puts_the_copy_back_on_discard() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, _told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_1");
            match turns.discard("mission_1") {
                CloudTurnAction::Ok { turn } => {
                    assert_eq!(turn.state, CloudTurnState::Discarded);
                }
                CloudTurnAction::Err { message } => panic!("expected ok, got {message}"),
            }
            assert!(computers.calls.borrow().iter().any(|c| c == "discard"));
        },
    );
}

#[test]
// case: sends the change to a new pull request, named from the message, and the copy moves on
fn sends_the_change_to_a_new_pull_request_named_from_the_message_and_the_copy_moves_on() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_1");
            match turns.pull_request("mission_1", "Add a greeting file\n\nLocust-Teammate: Wren") {
                CloudPullRequestAction::Ok { turn, added } => {
                    assert!(!added);
                    assert_eq!(turn.state, CloudTurnState::Pr);
                    let pr = turn.pull_request.expect("pr");
                    assert_eq!(pr.number, 7);
                    assert_eq!(pr.url, "https://github.com/acme/hearth/pull/7");
                    assert_eq!(pr.into.as_deref(), Some("main"));
                }
                CloudPullRequestAction::Err { message } => panic!("expected ok, got {message}"),
            }
            let calls = computers.calls.borrow().clone();
            assert!(calls.contains(&"push:a:new:locust/add-a-greeting-file:main".into()));
            assert!(calls.contains(&"open:locust/add-a-greeting-file->main:Add a greeting file".into()));
            assert!(calls.contains(&"settle:pull-request".into()));
            assert_eq!(told.borrow().last().unwrap().state, CloudTurnState::Pr);
            assert!(matches!(
                turns.pull_request("mission_1", "Again"),
                CloudPullRequestAction::Err { .. }
            ));

            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_2",
                    folder,
                    follows: Some("mission_1"),
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_2");
            let last_change = computers
                .calls
                .borrow()
                .iter()
                .filter(|c| c.starts_with("change:"))
                .last()
                .cloned();
            assert_eq!(last_change.as_deref(), Some("change:b"));
        },
    );
}

#[test]
// case: adds the next change to the conversation's pull request while it is open
fn adds_the_next_change_to_the_conversations_pull_request_while_it_is_open() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, _told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_1");
            let _ = turns.pull_request("mission_1", "Add a greeting file");
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_2",
                    folder,
                    follows: Some("mission_1"),
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_2");
            let waiting = &turns.turns(&["mission_2"])[0];
            assert_eq!(waiting.state, CloudTurnState::There);
            assert_eq!(waiting.open_pull_request.as_ref().map(|p| p.number), Some(7));

            match turns.pull_request("mission_2", "Say goodbye too") {
                CloudPullRequestAction::Ok { turn, added } => {
                    assert!(added);
                    assert_eq!(turn.state, CloudTurnState::Pr);
                    let pr = turn.pull_request.expect("pr");
                    assert_eq!(pr.number, 7);
                    assert_eq!(pr.into.as_deref(), Some("main"));
                }
                CloudPullRequestAction::Err { message } => panic!("expected ok, got {message}"),
            }
            let calls = computers.calls.borrow();
            assert!(calls.contains(&"state:7".into()));
            assert!(calls.contains(&"push:b:add:locust/add-a-greeting-file".into()));
            assert_eq!(
                calls.iter().filter(|c| c.starts_with("open:")).count(),
                1
            );
        },
    );
}

#[test]
// case: opens a new one when the last was merged or closed
fn opens_a_new_one_when_the_last_was_merged_or_closed() {
    let mut opts = FakeOptions::with_existing("Available");
    opts.still_open = false;
    with_world(opts, |folder, turns, computers, _told, _applied| {
        let _ = turns.prepare(
            PrepareInput {
                mission_id: "mission_1",
                folder,
                follows: None,
                open_code_version: Some("1.18.27"),
                agent: None,
                set_up: false,
            },
            &mut |_| {},
        );
        turns.settled("mission_1");
        let _ = turns.pull_request("mission_1", "Add a greeting file");
        let _ = turns.prepare(
            PrepareInput {
                mission_id: "mission_2",
                folder,
                follows: Some("mission_1"),
                open_code_version: Some("1.18.27"),
                agent: None,
                set_up: false,
            },
            &mut |_| {},
        );
        turns.settled("mission_2");
        match turns.pull_request("mission_2", "Say goodbye too") {
            CloudPullRequestAction::Ok { added, .. } => assert!(!added),
            CloudPullRequestAction::Err { message } => panic!("expected ok, got {message}"),
        }
        assert!(
            computers
                .calls
                .borrow()
                .contains(&"push:b:new:locust/say-goodbye-too:main".into())
        );
    });
}

#[test]
// case: takes its branch away again when GitHub will not open the pull request, and keeps the change waiting
fn takes_its_branch_away_again_when_github_will_not_open_the_pull_request_and_keeps_the_change_waiting() {
    let mut opts = FakeOptions::with_existing("Available");
    opts.refuse_open = true;
    with_world(opts, |folder, turns, computers, _told, _applied| {
        let _ = turns.prepare(
            PrepareInput {
                mission_id: "mission_1",
                folder,
                follows: None,
                open_code_version: Some("1.18.27"),
                agent: None,
                set_up: false,
            },
            &mut |_| {},
        );
        turns.settled("mission_1");
        match turns.pull_request("mission_1", "Add a greeting file") {
            CloudPullRequestAction::Err { message } => {
                assert!(message.contains("already exists"));
            }
            CloudPullRequestAction::Ok { .. } => panic!("expected refusal"),
        }
        let calls = computers.calls.borrow();
        assert!(calls.contains(&"drop:locust/add-a-greeting-file".into()));
        assert!(!calls.contains(&"settle:pull-request".into()));
        assert_eq!(turns.turns(&["mission_1"])[0].state, CloudTurnState::There);
    });
}

#[test]
// case: needs a title, the first line of its message
fn needs_a_title_the_first_line_of_its_message() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, _told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_1");
            assert!(matches!(
                turns.pull_request("mission_1", "   \n\n"),
                CloudPullRequestAction::Err { .. }
            ));
            assert!(
                !computers
                    .calls
                    .borrow()
                    .iter()
                    .any(|c| c.starts_with("push:"))
            );
        },
    );
}

#[test]
// case: says where the earlier turns a change held went, once it goes somewhere
fn says_where_the_earlier_turns_a_change_held_went_once_it_goes_somewhere() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, _computers, _told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_1");
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_2",
                    folder,
                    follows: Some("mission_1"),
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            turns.settled("mission_2");
            let before = &turns.turns(&["mission_1"])[0];
            assert_eq!(before.state, CloudTurnState::Carried);
            assert_eq!(before.outcome, None);
            let _ = turns.apply("mission_2");
            let after = &turns.turns(&["mission_1"])[0];
            assert_eq!(after.state, CloudTurnState::Carried);
            assert_eq!(after.outcome, Some(CloudTurnOutcome::Home));
        },
    );
}

#[test]
// case: sends a message's own files to the copy there, and nothing outside .locust/attachments
fn sends_a_messages_own_files_to_the_copy_there_and_nothing_outside_locust_attachments() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, _told, _applied| {
            let _ = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: Some("1.18.27"),
                    agent: None,
                    set_up: false,
                },
                &mut |_| {},
            );
            fs::create_dir_all(folder.join(".locust/attachments")).expect("mkdir");
            fs::write(
                folder.join(".locust/attachments/shot.png"),
                [137u8, 80, 78, 71],
            )
            .expect("write png");
            turns.carry(
                "mission_1",
                folder,
                &[
                    ".locust/attachments/shot.png",
                    ".locust/attachments/missing.txt",
                    "src/app.ts",
                    ".locust/attachments/../../secret.txt",
                ],
            );
            let carries: Vec<String> = computers
                .calls
                .borrow()
                .iter()
                .filter(|c| c.starts_with("carry:"))
                .cloned()
                .collect();
            assert_eq!(carries, vec!["carry:.locust/attachments/shot.png"]);
        },
    );
}

#[test]
// case: cannot put a different OpenCode there than the one it reads here
fn cannot_put_a_different_opencode_there_than_the_one_it_reads_here() {
    with_world(
        FakeOptions::default_github(),
        |folder, turns, _computers, _told, _applied| {
            let ready = turns.prepare(
                PrepareInput {
                    mission_id: "mission_1",
                    folder,
                    follows: None,
                    open_code_version: None,
                    agent: None,
                    set_up: true,
                },
                &mut |_| {},
            );
            match ready {
                CloudPrepared::Err { message, .. } => {
                    assert!(message.contains("which OpenCode"));
                }
                CloudPrepared::Ok { .. } => panic!("expected refusal"),
            }
        },
    );
}
