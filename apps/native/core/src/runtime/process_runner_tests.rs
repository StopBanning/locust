//! process-runner.test.ts, case for case. The transport's rules are held on `Lines`, `child_environment`,
//! `BoundedStderr` and `Stopping`; what a run does with them is held on the Claude driver, with a Python stand-in for
//! Claude Code (no CLI, no account, no network).
use super::*;
use crate::approval::Mode;
use crate::runtime::claude::Claude;
use crate::runtime::{Event, Runtime, Turn};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;

const PROMPT: &str = "private mission prompt";

fn small(max_record: usize) -> Limits {
    Limits { max_record_bytes: max_record, ..Limits::default() }
}

fn records(pieces: &[Piece]) -> Vec<&str> {
    pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Record(raw) => Some(raw.as_str()),
            Piece::Dropped { .. } => None,
        })
        .collect()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locust-runner-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn turn(prompt: &str, folder: PathBuf) -> Turn {
    Turn {
        prompt: prompt.into(),
        folder,
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "runner".into(),
        run_id: "run_runner".into(),
    }
}

fn python(script: &str) -> Command {
    let mut command = Command::new("python3");
    command.args(["-c", script]);
    command
}

fn started(script: &str, limits: Limits) -> Claude {
    let mut runtime = Claude::spawn_with(&mut python(script), limits).unwrap();
    runtime.start(&turn(PROMPT, std::env::temp_dir())).unwrap();
    runtime
}

/// Every event up to and including the terminal one.
fn run_out(runtime: &mut Claude) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut events = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "no terminal event: {:?}", kinds(&events));
        if let Some(e) = runtime.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        }
    }
}

fn kinds(events: &[Event]) -> Vec<String> {
    events.iter().map(|e| e.kind.clone()).collect()
}

fn last(events: &[Event]) -> &Event {
    events.last().unwrap()
}

fn process(events: &[Event]) -> &Value {
    &last(events).payload["process"]
}

const RESULT: &str = r#"{"type":"result","subtype":"success","result":"done","session_id":"s1"}"#;

#[test]
fn uses_the_validated_argv_and_cwd_sends_the_prompt_only_to_stdin_and_streams_raw_records() {
    // case: uses the validated argv and cwd, sends the prompt only to stdin, and streams raw records
    // The records: a character split across two reads is whole, `\r\n` and a blank line are no records.
    let emoji = "🙂".as_bytes();
    let mut lines = Lines::new(&Limits::default());
    let mut pieces = lines.feed(&[br#"{"one":""#.as_slice(), &emoji[..2]].concat());
    pieces.extend(lines.feed(&[&emoji[2..], b"\"}\r\n\n{\"two\":2}".as_slice()].concat()));
    pieces.extend(lines.end());
    assert_eq!(records(&pieces), [r#"{"one":"🙂"}"#, r#"{"two":2}"#]);
    // The machine's provider keys and anything else not on the allowlist never reach the child.
    let machine = [
        ("PATH", "C:\\tools"),
        ("USERPROFILE", "C:\\Users\\tester"),
        ("CODEX_HOME", "C:\\runtime-state\\codex"),
        ("ANTHROPIC_API_KEY", "must-not-leak"),
        ("CUSTOM_SECRET", "must-not-leak"),
    ]
    .map(|(k, v)| (OsString::from(k), OsString::from(v)));
    let kept = child_environment(machine, &[]);
    let names: Vec<_> = kept.iter().map(|(k, _)| k.to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["PATH", "USERPROFILE", "CODEX_HOME"]);
    // A real launch: its argv, its folder and its environment, and the prompt only on stdin.
    let dir = scratch("argv");
    let seen = dir.join("seen.json");
    let script = format!(
        r#"import json,os,sys
line=sys.stdin.readline()
json.dump({{"argv":sys.argv,"cwd":os.getcwd(),"env":sorted(os.environ),"stdin":line,"spec":os.environ.get("LOCUST_SPEC_VAR")}},open({seen:?},"w"))
print({RESULT:?},flush=True)
"#,
        seen = seen.to_string_lossy()
    );
    let mut command = python(&script);
    command.arg("--print").current_dir(&dir).env("LOCUST_SPEC_VAR", "from-the-spec");
    let mut runtime = Claude::spawn_with(&mut command, Limits::default()).unwrap();
    runtime.start(&turn(PROMPT, dir.clone())).unwrap();
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.completed", "{:?}", kinds(&events));
    let seen: Value = serde_json::from_str(&std::fs::read_to_string(&seen).unwrap()).unwrap();
    assert!(!seen["argv"].to_string().contains(PROMPT));
    assert_eq!(seen["cwd"].as_str().map(std::fs::canonicalize).unwrap().unwrap(), std::fs::canonicalize(&dir).unwrap());
    let stdin: Value = serde_json::from_str(seen["stdin"].as_str().unwrap()).unwrap();
    assert_eq!(stdin, json!({"type":"user","message":{"role":"user","content":PROMPT}}));
    assert_eq!(seen["spec"], "from-the-spec");
    let env: Vec<&str> = seen["env"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(env.contains(&"PATH"), "{env:?}");
    // cargo gives this test process its own variables; none of them is on the allowlist.
    assert!(!env.iter().any(|k| k.starts_with("CARGO") || k.starts_with("RUST_")), "{env:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bounds_captured_stderr_and_reports_truncation() {
    // case: bounds captured stderr and reports truncation
    let mut held = BoundedStderr::new(5);
    held.push(b"abc");
    held.push(b"defgh");
    assert_eq!(held.text(PROMPT), ("abcde".to_owned(), true));
    let mut runtime = started(
        "import sys\nsys.stderr.write('abcdefgh'); sys.stderr.flush(); sys.exit(2)",
        Limits { max_stderr_bytes: 5, ..Limits::default() },
    );
    let events = run_out(&mut runtime);
    assert_eq!(process(&events)["exitCode"], 2);
    assert_eq!(process(&events)["stderr"], "abcde");
    assert_eq!(process(&events)["stderrTruncated"], true);
}

#[test]
fn redacts_the_prompt_from_captured_stderr() {
    // case: redacts the prompt from captured stderr
    let mut runtime = started(
        "import json,sys\nline=json.loads(sys.stdin.readline())\nsys.stderr.write('provider echoed: '+line['message']['content']); sys.stderr.flush(); sys.exit(1)",
        Limits::default(),
    );
    let events = run_out(&mut runtime);
    assert_eq!(process(&events)["stderr"], "provider echoed: [prompt redacted]");
    assert!(!serde_json::to_string(&last(&events).payload).unwrap().contains(PROMPT));
}

#[test]
fn cancels_gracefully_with_sigint_when_the_child_acknowledges_it() {
    // case: cancels gracefully with SIGINT when the child acknowledges it
    let mut runtime = started(
        "import signal,sys,time\nsignal.signal(signal.SIGINT, lambda *a: sys.exit(0))\nprint('{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s\"}',flush=True)\ntime.sleep(30)",
        Limits { cancellation_grace: Duration::from_secs(10), ..Limits::default() },
    );
    std::thread::sleep(Duration::from_millis(300));
    let asked = Instant::now();
    runtime.stop().unwrap();
    let events = run_out(&mut runtime);
    assert!(asked.elapsed() < Duration::from_secs(5), "the SIGINT was not enough");
    assert_eq!(last(&events).kind, "run.cancelled");
    assert_eq!(process(&events)["forcedTerminationAttempted"], false);
}

#[test]
fn forces_termination_after_the_cancellation_grace_period_and_never_retries() {
    // case: forces termination after the cancellation grace period and never retries
    let limits = Limits {
        cancellation_grace: Duration::from_millis(5),
        termination_confirmation: Duration::from_millis(5),
        ..Limits::default()
    };
    let t0 = Instant::now();
    let mut stopping = Stopping::new(t0);
    assert_eq!(stopping.due(t0, &limits), Termination::Wait);
    assert_eq!(stopping.due(t0 + Duration::from_millis(5), &limits), Termination::Force);
    // Forced once: the clock moves on to confirmation, never to another start.
    assert_eq!(stopping.due(t0 + Duration::from_millis(6), &limits), Termination::Wait);
    assert_eq!(stopping.due(t0 + Duration::from_millis(10), &limits), Termination::Unconfirmed);
    // A child that ignores SIGINT is ended by force, and the run is the person's cancellation.
    let dir = scratch("retry");
    let starts = dir.join("starts");
    let script = format!(
        "import signal,time\nopen({:?},'a').write('x')\nsignal.signal(signal.SIGINT, signal.SIG_IGN)\nprint('{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s\"}}',flush=True)\ntime.sleep(30)",
        starts.to_string_lossy()
    );
    let mut runtime = started(&script, Limits { cancellation_grace: Duration::from_millis(200), ..Limits::default() });
    std::thread::sleep(Duration::from_millis(300));
    runtime.stop().unwrap();
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.cancelled");
    assert_eq!(process(&events)["forcedTerminationAttempted"], true);
    assert_eq!(process(&events)["terminationUnconfirmed"], false);
    assert_eq!(std::fs::read_to_string(&starts).unwrap(), "x");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn waits_for_close_after_a_forced_signal_even_when_kill_returns_false() {
    // case: waits for close after a forced signal even when kill returns false
    let limits = Limits {
        cancellation_grace: Duration::from_millis(5),
        termination_confirmation: Duration::from_millis(100),
        ..Limits::default()
    };
    let t0 = Instant::now();
    let mut stopping = Stopping::new(t0);
    assert_eq!(stopping.due(t0 + Duration::from_millis(5), &limits), Termination::Force);
    // Fifteen milliseconds on, the signal is sent and the run is not settled: it waits for the close.
    assert_eq!(stopping.due(t0 + Duration::from_millis(15), &limits), Termination::Wait);
    assert_eq!(stopping.due(t0 + Duration::from_millis(104), &limits), Termination::Wait);
    assert_eq!(stopping.forced, Some(t0 + Duration::from_millis(5)));
}

#[test]
fn skips_an_oversized_jsonl_record_and_lets_the_run_carry_on() {
    // case: skips an oversized JSONL record and lets the run carry on
    let mut lines = Lines::new(&small(8));
    let mut pieces = lines.feed(format!("{PROMPT}\n{{\"ok\":1}}\n").as_bytes());
    pieces.extend(lines.end());
    assert_eq!(records(&pieces), [r#"{"ok":1}"#]);
    assert_eq!(lines.dropped, 1);
    // On a run: the record after it still arrives, the turn completes, and nothing was killed for it.
    let script = format!(
        "print('{{\"type\":\"user\",\"x\":\"' + 'y'*400 + '\"}}',flush=True)\nprint({RESULT:?},flush=True)"
    );
    let mut runtime = started(&script, small(256));
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.completed", "{:?}", kinds(&events));
    assert_eq!(process(&events)["oversizedRecordsDropped"], 1);
    assert_eq!(process(&events)["outputLimitExceeded"], false);
    assert_eq!(process(&events)["forcedTerminationAttempted"], false);
}

#[test]
fn never_exposes_the_content_of_a_record_it_skipped() {
    // case: never exposes the content of a record it skipped
    let mut lines = Lines::new(&small(8));
    let pieces = lines.feed(format!("{PROMPT}\n").as_bytes());
    let Piece::Dropped { head, bytes, .. } = &pieces[0] else { panic!("{pieces:?}") };
    assert!(!stand_in(head, *bytes).to_string().contains(PROMPT));
    let script = format!("print('{{\"type\":\"user\",\"x\":\"{PROMPT} ' + 'y'*400 + '\"}}',flush=True)\nprint({RESULT:?},flush=True)");
    let mut runtime = Claude::spawn_with(&mut python(&script), small(256)).unwrap();
    runtime.start(&turn("another prompt", std::env::temp_dir())).unwrap();
    let events = run_out(&mut runtime);
    assert!(!events.iter().any(|e| e.payload.to_string().contains(PROMPT)));
}

#[test]
fn resynchronises_on_the_next_newline_after_a_line_it_gave_up_on() {
    // case: resynchronises on the next newline after a line it gave up on
    let mut lines = Lines::new(&small(24));
    let mut pieces = lines.feed(br#"{"big":"aaaaaaaaaaaaaaa"#);
    pieces.extend(lines.feed(b"aaaaaaaaaaaaaaaaaaaaaaa"));
    pieces.extend(lines.feed(b"tail-of-the-same-line\"}\n{\"next\":2}\n"));
    pieces.extend(lines.end());
    assert_eq!(records(&pieces), [r#"{"next":2}"#]);
    assert_eq!(lines.dropped, 1);
}

#[test]
fn carries_a_long_productive_burst_without_stopping_the_run() {
    // case: carries a long, productive burst without stopping the run
    let script = format!(
        "import sys\nfor n in range(500): sys.stdout.write('{{\"type\":\"system\",\"subtype\":\"status\",\"n\":%d}}\\n' % n)\nsys.stdout.write({RESULT:?}+'\\n'); sys.stdout.flush()"
    );
    let mut runtime = started(&script, Limits::default());
    // Nobody reads while the burst arrives: far more than the old 256.
    std::thread::sleep(Duration::from_millis(500));
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.completed", "{:?}", kinds(&events));
    assert_eq!(process(&events)["outputLimitExceeded"], false);
    assert_eq!(process(&events)["recordCount"], 501);
    assert_eq!(MAX_QUEUED_RECORDS, 2_048);
}

#[test]
fn bounds_queued_records_when_a_consumer_falls_behind() {
    // case: bounds queued records when a consumer falls behind
    let script = "print('{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"one\"}',flush=True)\nprint('{\"type\":\"system\",\"subtype\":\"status\"}',flush=True)\nprint('{\"type\":\"system\",\"subtype\":\"status\"}',flush=True)\nimport time; time.sleep(30)";
    let mut runtime = started(script, Limits { max_queued_records: 1, ..Limits::default() });
    std::thread::sleep(Duration::from_millis(500));
    let events = run_out(&mut runtime);
    // The record that was queued is still read; then the run stops and says why.
    assert_eq!(events[0].kind, "run.started", "{:?}", kinds(&events));
    assert_eq!(last(&events).kind, "run.failed", "{:?}", kinds(&events));
    assert_eq!(last(&events).payload["message"], OUTPUT_LIMIT_MESSAGE);
    assert_eq!(process(&events)["outputLimitExceeded"], true);
}

#[test]
fn does_not_launch_when_the_abortsignal_is_already_aborted() {
    // case: does not launch when the AbortSignal is already aborted
    let dir = scratch("abort");
    let mark = dir.join("launched");
    let mut command = python(&format!("open({:?},'w').write('x')", mark.to_string_lossy()));
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let Err(error) = Claude::launch(&mut command, &stopped) else { panic!("launched") };
    assert_eq!(error.to_string(), "Runtime process was cancelled before launch");
    std::thread::sleep(Duration::from_millis(200));
    assert!(!mark.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sanitizes_command_validation_errors_even_when_the_prompt_matches_an_unsafe_argument() {
    // case: sanitizes command-validation errors even when the prompt matches an unsafe argument
    use crate::runtime::claude::{PrintOptions, print_args};
    let unsafe_prompt = "--full-auto";
    let effort = print_args(&PrintOptions { effort: Some(unsafe_prompt), ..Default::default() }).unwrap_err();
    assert_eq!(effort, "Effort is invalid");
    let model = print_args(&PrintOptions { model: Some("--full-auto\0"), ..Default::default() }).unwrap_err();
    assert!(!model.contains(unsafe_prompt), "{model}");
}

#[test]
fn a_failed_start_is_never_an_unhandled_rejection_and_names_the_systems_code() {
    // case: a failed start is never an unhandled rejection, and names the system's code
    let Err(error) = Claude::spawn_with(&mut Command::new("/nonexistent/claude"), Limits::default()) else {
        panic!("started")
    };
    assert_eq!(error.to_string(), "Runtime process failed to start (ENOENT)");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn sanitizes_asynchronous_launch_errors_and_does_not_retry() {
    // case: sanitizes asynchronous launch errors and does not retry
    let dir = scratch("launch");
    let program = dir.join(format!("launch failed while handling {PROMPT}"));
    std::fs::write(&program, "not a program").unwrap();
    let Err(error) = Claude::spawn_with(&mut Command::new(&program), Limits::default()) else { panic!("started") };
    assert_eq!(error.to_string(), "Runtime process failed to start (EACCES)");
    assert!(!format!("{error:?}").contains(PROMPT));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sanitizes_synchronous_spawn_errors() {
    // case: sanitizes synchronous spawn errors
    let raw = std::io::Error::other(format!("cannot launch {PROMPT}"));
    let error = launch_error(&raw);
    assert_eq!(error.to_string(), "Runtime process failed to start");
    assert!(!format!("{error:?}").contains(PROMPT));
}

#[test]
fn marks_stdin_delivery_failure_without_treating_it_as_user_cancellation() {
    // case: marks stdin delivery failure without treating it as user cancellation
    let mut command = python("import os,signal,time\nos.close(0)\nsignal.signal(signal.SIGINT, signal.SIG_IGN)\ntime.sleep(30)");
    let mut runtime = Claude::spawn_with(
        &mut command,
        Limits { cancellation_grace: Duration::from_millis(100), ..Limits::default() },
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(400));
    // Large enough that no pipe buffer takes it for a reader that is gone.
    let _ = runtime.start(&turn(&"p".repeat(1024 * 1024), std::env::temp_dir()));
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.failed", "{:?}", kinds(&events));
    assert_eq!(process(&events)["inputDeliveryFailed"], true);
    assert_eq!(process(&events)["forcedTerminationAttempted"], true);
}

#[test]
fn writes_nothing_to_stdin_for_a_runtime_whose_prompt_is_already_in_its_argv() {
    // case: writes nothing to stdin for a runtime whose prompt is already in its argv
    let dir = scratch("argv-prompt");
    let seen = dir.join("stdin");
    let mut command = python(&format!(
        "import sys\nopen({:?},'w').write(repr(sys.stdin.read()))",
        seen.to_string_lossy()
    ));
    command.arg(PROMPT);
    let mut copilot = crate::runtime::copilot::Copilot::spawn(&mut command).unwrap();
    let _ = copilot.start(&turn(PROMPT, dir.clone()));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !seen.exists() || std::fs::read_to_string(&seen).unwrap().is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read_to_string(&seen).unwrap(), "''");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn puts_a_specs_own_variables_on_top_of_the_allowlist_where_nothing_on_the_machine_can_undo_them() {
    // case: puts a spec's own variables on top of the allowlist, where nothing on the machine can undo them
    let machine = [("PATH", "C:\\tools"), ("NO_COLOR", "0"), ("OPENCODE_CONFIG_CONTENT", "{}")]
        .map(|(k, v)| (OsString::from(k), OsString::from(v)));
    let spec = [("OPENCODE_CONFIG_CONTENT", r#"{"permission":{"write":"deny"}}"#), ("NO_COLOR", "1")]
        .map(|(k, v)| (OsString::from(k), OsString::from(v)));
    let mut kept = child_environment(machine, &spec);
    kept.sort();
    let kept: Vec<(String, String)> =
        kept.into_iter().map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned())).collect();
    assert_eq!(
        kept,
        [
            ("NO_COLOR".to_owned(), "1".to_owned()),
            ("OPENCODE_CONFIG_CONTENT".to_owned(), r#"{"permission":{"write":"deny"}}"#.to_owned()),
            ("PATH".to_owned(), "C:\\tools".to_owned()),
        ]
    );
    // On a command: what it sets itself survives the clearing, and what it removed stays removed.
    let mut command = Command::new("true");
    command.env("NO_COLOR", "1").env_remove("PATH");
    keep_allowlisted_environment(&mut command);
    let envs: Vec<_> = command.get_envs().map(|(k, v)| (k.to_owned(), v.map(|v| v.to_owned()))).collect();
    assert!(envs.contains(&(OsString::from("NO_COLOR"), Some(OsString::from("1")))));
    assert!(!envs.iter().any(|(k, v)| k == "PATH" && v.is_some()));
}

// a stream-json run

#[test]
fn sends_the_prompt_as_a_user_turn_and_keeps_input_open_for_the_running_turn() {
    // case: sends the prompt as a user turn and keeps input open for the running turn
    let dir = scratch("stream");
    let seen = dir.join("lines");
    let script = format!(
        "import sys\nfirst=sys.stdin.readline()\nprint('{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s\"}}',flush=True)\nprint('{{\"type\":\"assistant\",\"message\":{{\"content\":[]}}}}',flush=True)\nsecond=sys.stdin.readline()\nopen({:?},'w').write(first+second)\nprint({RESULT:?},flush=True)",
        seen.to_string_lossy()
    );
    let mut runtime = started(&script, Limits::default());
    assert!(runtime.can_steer());
    let deadline = Instant::now() + Duration::from_secs(10);
    // Once the turn is running, another message goes into it.
    while runtime.next_event().unwrap().is_none_or(|e| e.kind != "run.started") {
        assert!(Instant::now() < deadline);
    }
    assert!(runtime.steer("take this too"));
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.completed");
    let lines = std::fs::read_to_string(&seen).unwrap();
    assert_eq!(lines, [user_line_text(PROMPT), user_line_text("take this too")].concat());
    let _ = std::fs::remove_dir_all(dir);
}

fn user_line_text(text: &str) -> String {
    String::from_utf8(crate::runtime::claude::user_line(text)).unwrap()
}

#[test]
fn closes_input_on_the_turns_result_after_which_nothing_more_is_sent() {
    // case: closes input on the turn's result, after which nothing more is sent
    let dir = scratch("closed");
    let seen = dir.join("rest");
    let script = format!(
        "import sys\nsys.stdin.readline()\nprint({RESULT:?},flush=True)\nrest=sys.stdin.read()\nopen({:?},'w').write('eof:'+rest)",
        seen.to_string_lossy()
    );
    let mut runtime = started(&script, Limits::default());
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.completed");
    assert!(!runtime.steer("too late"));
    assert_eq!(std::fs::read_to_string(&seen).unwrap(), "eof:");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn ends_a_run_that_is_still_alive_after_its_result_and_says_it_ended_after_its_result() {
    // case: ends a run that is still alive after its result, and says it ended after its result
    let script = format!("import time\nprint({RESULT:?},flush=True)\ntime.sleep(30)");
    let limits = Limits { linger_after_result: Duration::from_millis(300), ..Limits::default() };
    let t0 = Instant::now();
    let mut runtime = started(&script, limits);
    let events = run_out(&mut runtime);
    assert!(t0.elapsed() >= Duration::from_millis(300));
    assert!(t0.elapsed() < Duration::from_secs(10));
    assert_eq!(last(&events).kind, "run.completed", "{:?}", kinds(&events));
    assert_eq!(process(&events)["endedAfterResult"], true);
}

#[test]
fn leaves_alone_a_run_that_exits_on_its_own_after_its_result() {
    // case: leaves alone a run that exits on its own after its result
    let script = format!("import sys\nprint({RESULT:?},flush=True)\nsys.stdin.read()");
    let mut runtime = started(&script, Limits { linger_after_result: Duration::from_millis(300), ..Limits::default() });
    let events = run_out(&mut runtime);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(last(&events).kind, "run.completed");
    assert_eq!(process(&events)["exitCode"], 0);
    assert!(process(&events).get("endedAfterResult").is_none());
    assert_eq!(process(&events)["forcedTerminationAttempted"], false);
}

#[test]
fn does_not_take_text_that_merely_mentions_a_result_for_the_result() {
    // case: does not take text that merely mentions a result for the result
    let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"the \"result\" is 4"}]}}"#;
    assert!(!is_turn_result(line));
    assert!(!starts_as_turn_result(line.as_bytes()));
    let script = format!(
        "import sys\nprint({line:?},flush=True)\nprint('{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s\"}}',flush=True)\nsys.stdin.readline()\nsys.stdin.readline()\nprint({RESULT:?},flush=True)"
    );
    let mut runtime = started(&script, Limits::default());
    let deadline = Instant::now() + Duration::from_secs(10);
    while runtime.next_event().unwrap().is_none_or(|e| e.kind != "run.started") {
        assert!(Instant::now() < deadline);
    }
    // Input is still open after the text that mentions a result.
    assert!(runtime.steer("still listening"));
    assert_eq!(last(&run_out(&mut runtime)).kind, "run.completed");
}

#[test]
fn keeps_a_result_line_past_the_ordinary_cap_and_ends_the_turn_on_it_arriving_in_pieces() {
    // case: keeps a result line past the ordinary cap, and ends the turn on it, arriving in pieces
    let line = json!({"type":"result","subtype":"success","result":"x".repeat(300),"session_id":"s1"}).to_string();
    let mut lines = Lines::new(&small(64));
    let mut pieces = lines.feed(&line.as_bytes()[..100]);
    pieces.extend(lines.feed(&line.as_bytes()[100..250]));
    pieces.extend(lines.feed(format!("{}\n", &line[250..]).as_bytes()));
    assert_eq!(records(&pieces), [line.as_str()]);
    assert_eq!(lines.dropped, 0);
    let script = format!(
        "import sys,time\nline={line:?}\nfor part in (line[:100], line[100:250], line[250:]+'\\n'):\n    sys.stdout.write(part); sys.stdout.flush(); time.sleep(.05)\nsys.stdin.read()"
    );
    let mut runtime = started(&script, small(64));
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.completed", "{:?}", kinds(&events));
    assert_eq!(process(&events)["oversizedRecordsDropped"], 0);
}

#[test]
fn still_ends_the_turn_on_a_result_line_too_large_even_for_its_own_cap() {
    // case: still ends the turn on a result line too large even for its own cap
    let line = json!({"type":"result","subtype":"success","result":"y".repeat(200)}).to_string();
    let limits = Limits { max_record_bytes: 32, max_result_record_bytes: 64, ..Limits::default() };
    let mut lines = Lines::new(&limits);
    let first = lines.feed(&line.as_bytes()[..120]);
    assert!(matches!(first.as_slice(), [Piece::Dropped { ends_turn: true, .. }]), "{first:?}");
    let mut rest = lines.feed(format!("{}\n", &line[120..]).as_bytes());
    rest.extend(lines.end());
    assert!(rest.is_empty());
    assert_eq!(lines.dropped, 1);
    // On a run: input closes on it (the child reads its end), and with no result kept the turn says so.
    let script = format!("import sys\nsys.stdin.readline()\nprint({line:?},flush=True)\nsys.stdin.read()");
    let mut runtime = started(&script, limits);
    let events = run_out(&mut runtime);
    assert_eq!(last(&events).kind, "run.failed", "{:?}", kinds(&events));
    assert_eq!(process(&events)["oversizedRecordsDropped"], 1);
    assert_eq!(process(&events)["recordCount"], 1);
    assert!(!runtime.steer("too late"));
}

#[test]
fn gives_an_ordinary_line_no_larger_cap_whatever_it_says() {
    // case: gives an ordinary line no larger cap, whatever it says
    let line = json!({"type":"assistant","text":format!("\"type\":\"result\" {}", "z".repeat(200))}).to_string();
    let mut lines = Lines::new(&small(64));
    let pieces = lines.feed(format!("{line}\n").as_bytes());
    assert!(matches!(pieces.as_slice(), [Piece::Dropped { ends_turn: false, .. }]), "{pieces:?}");
    assert_eq!(lines.dropped, 1);
    // Even one that opens as a result: `"result"` must close the type (`,` or `}` next), as startsAsTurnResult's `[,}]`.
    let opens_like_one = format!("{{\"type\":\"result\" \"{}\"}}", "z".repeat(200));
    assert!(!starts_as_turn_result(opens_like_one.as_bytes()));
    let pieces = lines.feed(format!("{opens_like_one}\n").as_bytes());
    assert!(matches!(pieces.as_slice(), [Piece::Dropped { ends_turn: false, .. }]), "{pieces:?}");
    assert!(starts_as_turn_result(br#"{ "type" : "result" , "subtype":"success"}"#));
}

#[test]
fn offers_no_send_on_any_other_transport() {
    // case: offers no send on any other transport
    let mut command = python("import sys\nsys.stdin.read()");
    let mut copilot = crate::runtime::copilot::Copilot::spawn(&mut command).unwrap();
    assert!(!copilot.can_steer());
    assert!(!copilot.steer("take this too"));
    let _ = copilot.stop();
}
