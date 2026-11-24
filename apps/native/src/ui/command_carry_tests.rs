//! a-command-reaches-the-next-message-once.test.ts through the window: the person's own command, run in the
//! conversation's folder (`own_command::Running`, read back by `terminal_panel::poll`), and what the next message carries
//! of it (`carry`, which `send` hands each message to). Expected words are Electron's.
use super::*;

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir() -> Dir {
    let d = std::env::temp_dir().join(format!("locust-command-carry-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(&d).unwrap();
    Dir(d)
}

/// The person's command, run as the window runs it, until it has ended (stopped first when `stop`).
fn ran(command: &str, stop: bool) -> (State, Dir) {
    let d = dir();
    let mut s = State::default();
    let mut running = own_command::Running::start(command, &d.0).expect("the command starts");
    if stop {
        running.stop();
    }
    s.terminal.commands.push(Entry { key: key(&s), command: command.into(), running: Some(running), result: None, carried: false });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while s.terminal.commands[0].result.is_none() {
        assert!(std::time::Instant::now() < until, "the command never ended");
        poll(&mut s);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    (s, d)
}

#[test]
fn carries_a_finished_command_into_the_next_message_marked_as_the_persons_and_not_again() {
    // case: carries a finished command into the next message, marked as the person's, and not again
    let (mut s, _d) = ran("echo hello", false);
    let first = carry(&mut s, "Look at this.".into());
    assert!(first.contains("The person's command and its output (exit 0):"), "{first}");
    assert!(first.contains("!echo hello"), "{first}");
    assert!(first.contains("hello"), "{first}");
    assert!(first.ends_with("Look at this."), "{first}");
    assert!(s.terminal.commands.iter().all(|e| e.carried), "nothing left to carry");
    assert_eq!(carry(&mut s, "Next.".into()), "Next.");
}

#[test]
fn waits_out_a_command_that_is_still_running_and_says_when_one_was_stopped() {
    // case: waits out a command that is still running, and says when one was stopped
    let d = dir();
    let mut s = State::default();
    let running = own_command::Running::start("sleep 30", &d.0).expect("the command starts");
    s.terminal.commands.push(Entry { key: key(&s), command: "sleep 30".into(), running: Some(running), result: None, carried: false });
    assert_eq!(carry(&mut s, "Wait.".into()), "Wait.");
    assert!(!s.terminal.commands[0].carried, "it is carried once it has ended, not lost");
    drop(s);

    let (mut s, _d) = ran("echo partial; sleep 30", true);
    let stopped = carry(&mut s, "After.".into());
    assert!(stopped.contains("stopped before it finished"), "{stopped}");
    assert!(stopped.ends_with("After."), "{stopped}");

    // What was left out of a long output goes with it, in Electron's words.
    let mut s = State::default();
    s.terminal.commands.push(Entry {
        key: key(&s),
        command: "echo hello".into(),
        running: None,
        result: Some(own_command::ResultRow { output: "partial".into(), omitted: own_command::omitted_note(3, 0), exit: None, stopped: true, elapsed: 5 }),
        carried: false,
    });
    let stopped = carry(&mut s, "After.".into());
    assert!(stopped.contains("stopped before it finished"), "{stopped}");
    assert!(stopped.contains("partial"), "{stopped}");
    assert!(stopped.contains("Left out 3 lines from the start."), "{stopped}");
}

#[test]
fn leaves_a_message_alone_when_there_is_nothing_to_carry() {
    // case: leaves a message alone when there is nothing to carry
    let mut s = State::default();
    assert_eq!(carry(&mut s, "Only this.".into()), "Only this.");
}
