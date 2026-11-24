//! The Claude start in `run_turn` goes through `Claude::launch`: a turn already stopped when its runtime would start is
//! never launched, and is recorded as the person's stop (process-runner.ts abortBeforeLaunchError).
use super::*;

#[test]
fn a_claude_turn_stopped_before_its_launch_is_never_started_and_is_recorded_as_stopped() {
    let t = Temp::new();
    let store = t.store();
    let mark = t.0.join("launched");
    let mut command = Command::new("/usr/bin/touch");
    command.arg(&mark);
    let writer = LedgerWriter::create_runtime(&store, "mission_stopped_early", "Do work.", "account-default", "claude", "claude", &[], None)
        .unwrap();
    let path = writer.path.clone();
    let turn = Turn {
        prompt: "Do work.".into(),
        folder: t.0.join("work"),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "mission_stopped_early".into(),
        run_id: "run_mission_stopped_early".into(),
    };
    run_turn(
        writer,
        command,
        turn,
        Arc::new(AtomicBool::new(true)),
        Registered(Vec::new(), None, None),
        "claude".into(),
        Channel::new(),
        None,
        store,
        false,
        false,
        None,
        None,
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(!mark.exists(), "a stopped turn's Claude was launched");
    assert_eq!(kinds(&path).last().map(String::as_str), Some("run.cancelled"), "{:?}", kinds(&path));
}
