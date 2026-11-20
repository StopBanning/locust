// Carries apps/desktop/src/main/a-dying-server-says-why.test.ts (N10): a Codex app server's stderr is drained, always,
// and only its end is kept; a server that dies says why there, after Locust's own words (codex-app-server-run.ts `lose`).
use super::*;

const WHY: &str = "Codex stdout closed before the turn completed";

fn server(script: &str) -> Codex {
    let mut command = Command::new("python3");
    command.args(["-c", script]);
    Codex::spawn(&mut command).unwrap()
}

/// The run's terminal event, once the server has gone.
fn failed(runtime: &mut Codex) -> Event {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(event) = runtime.next_event().unwrap()
            && event.kind.starts_with("run.")
        {
            return event;
        }
    }
    panic!("the run never ended");
}

fn stderr_of(event: &Event) -> &str {
    assert_eq!(event.kind, "run.failed");
    event.payload["process"]["stderr"].as_str().unwrap()
}

// case: is drained, and its end kept
#[test]
fn a_chatty_servers_stderr_is_drained_and_its_end_kept() {
    // Far more than a pipe holds: a server nobody drains would stall here and never reach its last line.
    let mut runtime = server(
        "import sys\nfor n in range(20000): sys.stderr.write('noise line %d\\n' % n)\nsys.stderr.write('Error: sandbox could not start\\n')\nsys.stderr.flush()",
    );
    let event = failed(&mut runtime);
    let tail = runtime.stderr_tail();
    assert!(tail.encode_utf16().count() <= 4_096, "{}", tail.len());
    assert!(tail.encode_utf16().count() > 4_000, "the end is kept, not a scrap of it: {}", tail.len());
    assert_eq!(tail.trim().split('\n').next_back(), Some("Error: sandbox could not start"));
    // And the run that died says it, after Locust's own words.
    let said = stderr_of(&event);
    // Cut where 4,096 characters end, mid-line as Electron's `slice` cuts it.
    assert_eq!(said, format!("{WHY}\n{}", tail.trim()));
    assert!(said.contains("\nnoise line 19999\n"), "{said:.200}");
    assert!(said.ends_with("\nError: sandbox could not start"), "{said:.200}");
    assert_eq!(event.payload["message"], WHY);
}

// case: is empty, not missing, for a child that has none
#[test]
fn a_server_that_says_nothing_has_an_empty_tail_and_dies_in_our_words_alone() {
    let mut runtime = server("pass");
    let event = failed(&mut runtime);
    assert_eq!(runtime.stderr_tail(), "");
    assert_eq!(stderr_of(&event), WHY);
}

// A config warning in terminal colours was read as the cause on Colin's 10/02 failure: the codes are taken out.
#[test]
fn a_dying_servers_terminal_colour_codes_are_taken_out_of_its_last_words() {
    let mut runtime = server("import sys\nsys.stderr.write('\\x1b[31mError: \\x1b[1;4mboom\\x1b[0m\\n  \\n')");
    let event = failed(&mut runtime);
    assert_eq!(stderr_of(&event), format!("{WHY}\nError: boom"));
}

// `knownCause`: Locust saw why (a line past the size limit), so the server's stderr is not added.
#[test]
fn a_cause_locust_saw_itself_is_said_without_the_servers_stderr() {
    let mut runtime = server(
        "import sys\nsys.stderr.write('warning: an unrelated config note\\n')\nsys.stderr.flush()\nsys.stdout.write('x' * (9 * 1024 * 1024))\nsys.stdout.flush()",
    );
    let event = failed(&mut runtime);
    assert_eq!(event.payload["message"], LINE_TOO_LONG);
    assert_eq!(stderr_of(&event), LINE_TOO_LONG);
}

// A server's last words can come after its stdout has closed: they are waited for, a moment, not lost to the race.
#[test]
fn a_dying_servers_last_words_after_its_stdout_closed_are_waited_for() {
    let mut runtime = server("import os, sys, time\nos.close(1)\ntime.sleep(0.2)\nsys.stderr.write('Error: sandbox could not start\\n')");
    let event = failed(&mut runtime);
    assert_eq!(stderr_of(&event), format!("{WHY}\nError: sandbox could not start"));
}
