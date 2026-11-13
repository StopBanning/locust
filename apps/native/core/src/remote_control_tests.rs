//! Electron's a-remote-control-server-lives-only-while-enabled.test.ts, against a launcher that records what it was
//! asked and lets each test say what the process does. Nothing here runs `claude remote-control`, and nothing ever
//! answers Claude Code's trust question.
use super::*;
use std::sync::mpsc;

#[derive(Default)]
struct Fake {
    spawned: Mutex<Vec<(PathBuf, Vec<String>, PathBuf)>>,
    kills: Mutex<u32>,
    setups: Mutex<Vec<(PathBuf, PathBuf)>>,
    events: Mutex<Option<Events>>,
    discovers: Mutex<u32>,
    /// The process closes by itself when it is killed (Electron's autoClose).
    auto_close: bool,
    found: Mutex<Option<Result<Option<PathBuf>, String>>>,
    /// Discovery waits for this before it answers.
    hold: Mutex<Option<mpsc::Receiver<()>>>,
    spawn_fails: Option<String>,
    setup_fails: Option<String>,
}

struct FakeChild(Arc<Fake>);
impl Owned for FakeChild {
    fn pid(&self) -> u32 {
        24680
    }
    fn kill_tree(&self) {
        *self.0.kills.lock().unwrap() += 1;
        if self.0.auto_close {
            let events = self.0.events.lock().unwrap().clone();
            // Later, as a process reports its end: never from inside the kill.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(5));
                if let Some(events) = events {
                    events.closed(None);
                }
            });
        }
    }
}

struct Launch(Arc<Fake>);
impl Launcher for Launch {
    fn discover(&self) -> Result<Option<PathBuf>, String> {
        *self.0.discovers.lock().unwrap() += 1;
        let hold = self.0.hold.lock().unwrap().take();
        if let Some(hold) = hold {
            let _ = hold.recv();
        }
        self.0.found.lock().unwrap().clone().unwrap_or_else(|| Ok(Some(PathBuf::from("/fake/claude"))))
    }
    fn spawn(&self, program: &Path, args: &[&str], cwd: &Path, events: Events) -> Result<Box<dyn Owned>, String> {
        if let Some(why) = &self.0.spawn_fails {
            return Err(why.clone());
        }
        self.0.spawned.lock().unwrap().push((program.to_path_buf(), args.iter().map(|a| a.to_string()).collect(), cwd.to_path_buf()));
        *self.0.events.lock().unwrap() = Some(events);
        Ok(Box::new(FakeChild(self.0.clone())))
    }
    fn open_setup(&self, program: &Path, cwd: &Path) -> Result<(), String> {
        self.0.setups.lock().unwrap().push((program.to_path_buf(), cwd.to_path_buf()));
        self.0.setup_fails.clone().map_or(Ok(()), Err)
    }
}

fn world_with(fake: Fake, deadline: Duration) -> (RemoteControl, Arc<Fake>) {
    let fake = Arc::new(fake);
    let service = RemoteControl::with_deadline(Arc::new(Launch(fake.clone())), Box::new(|| PathBuf::from("/project")), deadline);
    (service, fake)
}

fn world(auto_close: bool) -> (RemoteControl, Arc<Fake>) {
    world_with(Fake { auto_close, ..Default::default() }, Duration::from_millis(4000))
}

fn events(fake: &Fake) -> Events {
    fake.events.lock().unwrap().clone().expect("a process was started")
}

/// Until `done`, or fail: the setup terminal opens on a thread of its own.
fn until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn starts_off_and_runs_no_discovery_or_command_until_enabled() {
    let (service, fake) = world(true);
    assert_eq!(service.snapshot(), RemoteControlState::default());
    assert!(fake.spawned.lock().unwrap().is_empty());
    assert_eq!(*fake.discovers.lock().unwrap(), 0);
}

#[test]
fn runs_exactly_server_mode_with_worktree_spawning_and_the_locust_name_in_the_folder() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    assert_eq!(
        fake.spawned.lock().unwrap().clone(),
        vec![(PathBuf::from("/fake/claude"), vec!["remote-control".into(), "--spawn".into(), "worktree".into(), "--name".into(), "Locust".into()], PathBuf::from("/project"))]
    );
    assert!(!REMOTE_CONTROL_ARGS.contains(&"-p"));
    assert_eq!(service.snapshot().phase, Phase::Starting);
    events(&fake).spawned();
    assert_eq!(service.snapshot().phase, Phase::Running);
    service.dispose();
}

#[test]
fn enabling_twice_owns_just_one_server_and_still_receives_its_spawn_event() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    service.set_enabled(true);
    events(&fake).spawned();
    assert_eq!(fake.spawned.lock().unwrap().len(), 1);
    assert_eq!(service.snapshot().phase, Phase::Running);
    service.dispose();
}

#[test]
fn switching_off_kills_only_the_owned_tree_and_waits_until_the_process_reports_close() {
    let (service, fake) = world(false);
    service.set_enabled(true);
    let off = { let service = service.clone(); std::thread::spawn(move || service.set_enabled(false)) };
    until("the kill", || *fake.kills.lock().unwrap() == 1);
    std::thread::sleep(Duration::from_millis(30));
    assert!(!off.is_finished(), "off returned before the process closed");
    assert_eq!(service.snapshot().phase, Phase::Stopping);
    events(&fake).closed(None);
    let state = off.join().unwrap();
    assert!(!state.enabled);
    assert_eq!(state.phase, Phase::Off);
}

#[test]
fn quitting_waits_for_the_owned_process_to_close_and_refuses_any_later_enable() {
    let (service, fake) = world(false);
    service.set_enabled(true);
    let quitting = { let service = service.clone(); std::thread::spawn(move || service.dispose()) };
    until("the kill", || *fake.kills.lock().unwrap() == 1);
    std::thread::sleep(Duration::from_millis(30));
    assert!(!quitting.is_finished());
    events(&fake).closed(None);
    quitting.join().unwrap();
    service.set_enabled(true);
    assert_eq!(fake.spawned.lock().unwrap().len(), 1);
    assert!(!service.snapshot().enabled);
}

#[test]
fn a_pending_discovery_cannot_start_a_server_after_off_or_quit() {
    for quit in [false, true] {
        let (tx, rx) = mpsc::channel();
        let (service, fake) = world_with(Fake { auto_close: true, hold: Mutex::new(Some(rx)), ..Default::default() }, Duration::from_millis(4000));
        let starting = { let service = service.clone(); std::thread::spawn(move || service.set_enabled(true)) };
        until("discovery to begin", || *fake.discovers.lock().unwrap() == 1);
        if quit { service.dispose() } else { service.set_enabled(false); }
        tx.send(()).unwrap();
        starting.join().unwrap();
        assert!(fake.spawned.lock().unwrap().is_empty(), "quit: {quit}");
        assert!(!service.snapshot().enabled);
    }
}

#[test]
fn discovery_errors_are_shown_verbatim_without_starting_a_command() {
    let (service, fake) = world_with(Fake { found: Mutex::new(Some(Err("exact discovery error\r\n  next".into()))), ..Default::default() }, Duration::from_millis(4000));
    let state = service.set_enabled(true);
    assert_eq!((state.error.as_deref(), state.enabled, state.phase), (Some("exact discovery error\r\n  next"), false, Phase::Error));
    assert!(fake.spawned.lock().unwrap().is_empty());
}

#[test]
fn missing_claude_code_leaves_the_switch_off_with_installation_help() {
    let (service, fake) = world_with(Fake { found: Mutex::new(Some(Ok(None))), ..Default::default() }, Duration::from_millis(4000));
    let state = service.set_enabled(true);
    assert!(state.error.as_deref().is_some_and(|e| e.contains("not installed")), "{state:?}");
    assert!(fake.spawned.lock().unwrap().is_empty());
}

#[test]
fn a_spawn_failure_is_kept_verbatim() {
    let (service, _) = world_with(Fake { spawn_fails: Some("spawn EPERM: exact".into()), ..Default::default() }, Duration::from_millis(4000));
    let state = service.set_enabled(true);
    assert_eq!((state.error.as_deref(), state.enabled), (Some("spawn EPERM: exact"), false));
}

#[test]
fn an_asynchronous_spawn_error_and_close_remain_an_error() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    events(&fake).error("spawn ENOENT: exact");
    events(&fake).closed(Some(-2));
    let state = service.snapshot();
    assert_eq!((state.error.as_deref(), state.phase, state.enabled), (Some("spawn ENOENT: exact"), Phase::Error, false));
}

#[test]
fn first_lines_whitespace_ansi_bytes_and_a_split_character_survive_unchanged() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    let line = "  first\r\nhttps://claude.ai/code/session_fake\r\n\u{1b}[31m\u{1F41B}\u{1b}[0m\n";
    let bytes = line.as_bytes();
    let split = line.find('\u{1F41B}').unwrap() + 1;
    events(&fake).output(Stream::Stdout, &bytes[..split]);
    events(&fake).output(Stream::Stdout, &bytes[split..]);
    events(&fake).output(Stream::Stderr, b"  exact error\r\n");
    let state = service.snapshot();
    assert_eq!(state.stdout, line);
    assert_eq!(state.stderr, "  exact error\r\n");
    service.dispose();
}

#[test]
fn a_failed_server_keeps_stderr_and_exit_code_without_translating_or_retrying() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    events(&fake).output(Stream::Stderr, b"Remote Control requires a claude.ai subscription\r\n");
    events(&fake).closed(Some(1));
    let state = service.snapshot();
    assert_eq!(state.stderr, "Remote Control requires a claude.ai subscription\r\n");
    assert_eq!((state.exit_code, state.phase, state.enabled), (Some(Some(1)), Phase::Error, false));
    assert_eq!(fake.spawned.lock().unwrap().len(), 1);
}

#[test]
fn a_server_that_exits_successfully_turns_the_switch_off_without_a_retry() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    events(&fake).closed(Some(0));
    let state = service.snapshot();
    assert_eq!((state.phase, state.enabled, state.exit_code), (Phase::Ended, false, Some(Some(0))));
    assert_eq!(fake.spawned.lock().unwrap().len(), 1);
}

#[test]
fn stops_for_a_trust_question_without_answering_and_opens_setup_only_after_close() {
    for line in ["Trust /project? [y/N]", "Trust /project? \u{1b}[31m[y/N]\u{1b}[0m", "Workspace not trusted", "Enable Remote Control? (y/n)"] {
        let (service, fake) = world(false);
        service.set_enabled(true);
        events(&fake).output(Stream::Stdout, &line.as_bytes()[..9]);
        events(&fake).output(Stream::Stdout, &line.as_bytes()[9..]);
        until("the kill", || *fake.kills.lock().unwrap() == 1);
        std::thread::sleep(Duration::from_millis(30));
        assert!(fake.setups.lock().unwrap().is_empty(), "setup opened before the server closed: {line}");
        events(&fake).closed(None);
        until("the setup terminal", || !fake.setups.lock().unwrap().is_empty());
        let state = service.snapshot();
        assert_eq!((state.phase, state.enabled, state.stdout.as_str()), (Phase::NeedsPerson, false, line));
        assert_eq!(fake.setups.lock().unwrap().clone(), vec![(PathBuf::from("/fake/claude"), PathBuf::from("/project"))]);
    }
}

#[test]
fn no_setup_terminal_opens_when_off_or_quit_happens_while_ending_for_trust() {
    for quit in [false, true] {
        let (service, fake) = world(false);
        service.set_enabled(true);
        events(&fake).output(Stream::Stderr, b"Workspace not trusted");
        until("the kill", || *fake.kills.lock().unwrap() == 1);
        let ending = { let service = service.clone(); std::thread::spawn(move || if quit { service.dispose() } else { service.set_enabled(false); }) };
        std::thread::sleep(Duration::from_millis(20));
        events(&fake).closed(None);
        ending.join().unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert!(fake.setups.lock().unwrap().is_empty(), "quit: {quit}");
    }
}

#[test]
fn a_setup_terminal_failure_is_kept_verbatim_and_leaves_the_server_off() {
    let (service, fake) = world_with(Fake { auto_close: true, setup_fails: Some("terminal failed: exact".into()), ..Default::default() }, Duration::from_millis(4000));
    service.set_enabled(true);
    events(&fake).output(Stream::Stderr, b"Workspace not trusted");
    until("the refusal", || service.snapshot().error.is_some());
    let state = service.snapshot();
    assert_eq!((state.enabled, state.error.as_deref(), state.phase), (false, Some("terminal failed: exact"), Phase::Error));
}

#[test]
fn keeps_bounded_first_output_for_each_stream_and_still_sees_a_later_trust_question() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    events(&fake).output(Stream::Stdout, "x".repeat(70_000).as_bytes());
    events(&fake).output(Stream::Stderr, "y".repeat(70_000).as_bytes());
    let state = service.snapshot();
    assert_eq!((state.stdout, state.stderr, state.truncated), ("x".repeat(65536), "y".repeat(65536), true));
    events(&fake).output(Stream::Stderr, b"Workspace not trusted");
    until("the setup terminal", || fake.setups.lock().unwrap().len() == 1);
}

#[test]
fn a_termination_timeout_is_an_error_and_starts_no_other_server() {
    let (service, fake) = world_with(Fake::default(), Duration::from_millis(5));
    service.set_enabled(true);
    let state = service.set_enabled(false);
    assert_eq!((state.phase, state.enabled, state.error.as_deref()), (Phase::Error, false, Some(DID_NOT_END)));
    service.set_enabled(true);
    assert_eq!(fake.spawned.lock().unwrap().len(), 1);
    events(&fake).closed(None);
}

#[test]
fn an_earlier_off_cannot_overwrite_a_later_start() {
    let (service, fake) = world(true);
    service.set_enabled(false);
    service.set_enabled(true);
    events(&fake).spawned();
    let state = service.snapshot();
    assert_eq!((state.enabled, state.phase), (true, Phase::Running));
    service.dispose();
}

#[test]
fn repeated_trust_output_opens_setup_only_once() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    events(&fake).output(Stream::Stdout, b"Workspace not trusted");
    events(&fake).output(Stream::Stderr, b"Workspace not trusted");
    until("the setup terminal", || !fake.setups.lock().unwrap().is_empty());
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(fake.setups.lock().unwrap().len(), 1);
}

#[test]
fn an_ended_process_reports_nothing_into_a_later_one() {
    let (service, fake) = world(true);
    service.set_enabled(true);
    let first = events(&fake);
    first.closed(Some(0));
    service.set_enabled(true);
    first.output(Stream::Stdout, b"late words");
    first.closed(Some(1));
    let state = service.snapshot();
    assert_eq!((state.stdout.as_str(), state.phase), ("", Phase::Starting));
    service.dispose();
}

// Electron's a-remote-control-switch-shows-claudes-own-words.test.tsx: the line under the switch.
#[test]
fn the_switch_says_where_it_stands_first() {
    let state = |phase, exit_code| RemoteControlState { phase, exit_code, ..Default::default() };
    assert!(says(&state(Phase::Off, None)).starts_with("Off. When on, you can start Claude Code sessions in this folder from claude.ai"));
    assert!(says(&state(Phase::Running, None)).starts_with("On. Start a session in this folder"));
    assert_eq!(says(&state(Phase::NeedsPerson, None)), "Complete setup in the Claude Code terminal, then enable this switch again.");
    assert_eq!(says(&state(Phase::Error, Some(Some(1)))), "Claude Code reported a problem (exit 1).");
    assert_eq!(says(&state(Phase::Error, Some(None))), "Claude Code reported a problem (exit unknown).");
    assert_eq!(says(&state(Phase::Error, None)), "Claude Code reported a problem.");
    assert_eq!(says(&state(Phase::Stopping, None)), "Waiting for Claude Code to end\u{2026}");
}
