//! a-bang-in-the-box-is-command-mode.test.tsx through the window: the message box's command mode (`!` in the box), the
//! note it shows (`terminal_panel::commands`), Escape in the box (`composer_input_key`), and the person's command as
//! it runs and once it has ended, with its Stop. Expected words are Electron's (OwnCommand.tsx's test).
use super::*;
use rune::{Env, Runtime};

fn drawn(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| commands(s, &Palette::new(true)), (700.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

fn stop_button(rt: &Runtime<State>) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| e.label.as_deref() == Some("Stop this command") && !e.exiting)
}

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn shows_command_mode_in_the_box_and_escape_leaves_it() {
    // case: shows command mode in the box, and Escape leaves it
    let mut s = State::default();
    s.draft.replace_text("!echo hello");
    let rt = drawn(s);
    let note = rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.contains("Command mode")))
        .expect("the command-mode note under the box");
    let said = note.text.clone().unwrap();
    assert!(said.contains("Escape leaves it"), "{said}");
    assert_eq!(note.role, "status", "said to a screen reader as a status, as Electron's role=\"status\"");

    // Escape in the box leaves command mode: the mark goes, the command stays to edit, nothing runs.
    let mut s = rt.state;
    let escape = KeyEvent { key: Key::Escape, mods: Default::default() };
    assert!(composer_input_key(&mut s, &escape, &mut Tx::default()));
    assert_eq!(s.draft.text, "echo hello");
    assert!(s.terminal.commands.is_empty(), "leaving command mode runs nothing");
    let rt = drawn(s);
    assert!(!words(&rt).iter().any(|w| w.contains("Command mode")), "the note goes with the mark");
}

#[test]
fn shows_the_persons_command_its_output_and_its_exit_code_and_stop_only_while_it_runs() {
    // case: shows the person's command, its output and its exit code, and Stop only while it runs
    let dir = Dir(std::env::temp_dir().join(format!("locust-command-mode-{}-{}", std::process::id(), locust_core::invariants::unique_nanos())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let mut s = State::default();
    let running = own_command::Running::start("echo hello; sleep 30", &dir.0).expect("the command starts");
    s.terminal.commands.push(Entry { key: key(&s), command: "echo hello; sleep 30".into(), running: Some(running), result: None, carried: false });
    let mut rt = drawn(s);
    let said = words(&rt);
    assert!(said.iter().any(|w| w.starts_with("!echo hello")), "{said:?}");
    assert!(said.iter().any(|w| w == "Running"), "{said:?}");
    let stop = stop_button(&rt).expect("Stop, while it runs");
    assert_eq!(stop.role, "button");

    // Stop ends it, and the entry says it was stopped, with no Stop left.
    for _ in 0..6 {
        rt.frame(0.016);
    }
    let (x, y) = rt.find("command-stop-0").expect("Stop on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..6 {
        rt.frame(0.016);
    }
    assert!(rt.state.terminal.commands[0].running.as_ref().is_some_and(|r| r.stopped), "the press reached Stop");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while rt.state.terminal.commands[0].result.is_none() {
        assert!(std::time::Instant::now() < until, "the stopped command never ended");
        rt.update(|s, _| {
            poll(s);
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for _ in 0..30 {
        rt.frame(0.05);
    }
    assert!(words(&rt).iter().any(|w| w.starts_with("Stopped")), "{:?}", words(&rt));
    assert!(stop_button(&rt).is_none(), "no Stop once it has ended");

    // One that finished: its output and its exit code, and no Stop.
    let mut s = State::default();
    s.terminal.commands.push(Entry {
        key: key(&s),
        command: "echo hello".into(),
        running: None,
        result: Some(own_command::ResultRow { output: "hello".into(), omitted: None, exit: Some(0), stopped: false, elapsed: 12 }),
        carried: false,
    });
    let rt = drawn(s);
    let said = words(&rt);
    assert!(said.iter().any(|w| w == "!echo hello"), "{said:?}");
    assert!(said.iter().any(|w| w == "hello"), "{said:?}");
    assert!(said.iter().any(|w| w.starts_with("Exit 0")), "{said:?}");
    assert!(stop_button(&rt).is_none(), "{said:?}");
}
