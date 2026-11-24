//! the-context-menu-reads-claudes-own-estimate.test.ts (0.806): every case, by name, against the same fixture the
//! Electron test reads. The window's ring menu is not wired to the reader yet; the last test is the reader as the
//! window will feed it: a remembered launch, then a read after the turn.

use super::*;
use std::sync::atomic::AtomicUsize;

const FIXTURE: &str = include_str!("../../../desktop/src/main/fixtures/claude-context.json");

fn reply(result: &str, extra: Value) -> String {
    let mut v = json!({"type": "result", "is_error": false, "num_turns": 0, "total_cost_usd": 0, "result": result});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v.to_string()
}

fn fixture_result() -> String {
    serde_json::from_str::<Value>(FIXTURE).unwrap()["result"].as_str().unwrap().to_owned()
}

fn rows(list: &[(&str, u64)]) -> Vec<ContextCategory> {
    list.iter().map(|(n, t)| ContextCategory::new(n, *t)).collect()
}

// Claude Code's /context reply

#[test]
fn is_read_from_its_category_table_the_totals_included() {
    let breakdown = parse_claude_context(FIXTURE).expect("the fixture reads");
    assert_eq!((breakdown.used, breakdown.window), (45_200, 200_000));
    assert_eq!(breakdown.categories.iter().find(|r| r.name == "Messages"), Some(&ContextCategory::new("Messages", 24_500)));
    // Not the per-tool table after it.
    assert!(!breakdown.categories.iter().any(|r| r.name == "fixture_tool"));
}

#[test]
fn is_read_on_a_resumed_session_whose_reply_carries_that_sessions_earlier_cost() {
    let raw = reply(&fixture_result(), json!({"total_cost_usd": 0.00107, "duration_api_ms": 0}));
    assert_eq!(parse_claude_context(&raw).map(|b| b.used), Some(45_200));
}

#[test]
fn is_refused_when_it_ran_a_turn_or_called_the_model_failed_or_has_no_table() {
    let result = fixture_result();
    assert_eq!(parse_claude_context(&reply(&result, json!({"num_turns": 1}))), None);
    assert_eq!(parse_claude_context(&reply(&result, json!({"duration_api_ms": 850}))), None);
    assert_eq!(parse_claude_context(&reply(&result, json!({"is_error": true}))), None);
    assert_eq!(parse_claude_context(&reply("**Tokens:** 1k / 200k", json!({}))), None);
    assert_eq!(parse_claude_context("not json"), None);
}

// the menu

#[test]
fn shows_the_biggest_rows_in_the_window_and_free_space_never_deferred_tools_or_the_autocompact_buffer() {
    let breakdown = parse_claude_context(FIXTURE).unwrap();
    let names: Vec<String> = context_categories(&breakdown).into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["Messages", "Skills", "System tools", "System prompt", "Free space"]);
    assert_eq!(
        context_advice(&breakdown).as_deref(),
        Some("Messages take 24.5k. Compact this conversation to make room while keeping a summary.")
    );
}

#[test]
fn says_nothing_when_no_category_the_person_controls_is_a_real_share_of_the_window() {
    let fresh = ClaudeContextBreakdown {
        used: 24_000,
        window: 1_000_000,
        categories: rows(&[
            ("System prompt", 2_500),
            ("MCP tools (deferred)", 200_700),
            ("Skills", 10_000),
            ("MCP tools", 600),
            ("Messages", 10),
            ("Free space", 943_000),
            ("Autocompact buffer", 33_000),
        ]),
    };
    assert_eq!(context_advice(&fresh), None);
    assert!(!context_categories(&fresh).iter().any(|r| r.name == "MCP tools (deferred)"));
}

#[test]
fn points_at_connectors_when_they_are_what_fills_it_server_instructions_counted_with_them() {
    let heavy = ClaudeContextBreakdown {
        used: 190_000,
        window: 200_000,
        categories: rows(&[("MCP tools", 150_000), ("MCP server instructions", 20_000), ("Messages", 15_000), ("Free space", 10_000)]),
    };
    assert_eq!(
        context_advice(&heavy).as_deref(),
        Some("Your connectors take 170k: untick ones this teammate does not need in Edit teammate.")
    );
}

// running /context: through a real process, not a stand-in.

#[cfg(unix)]
fn command(script: &str) -> ContextCommand {
    ContextCommand { program: "sh".into(), args: vec!["-c".into(), script.into()], cwd: std::env::temp_dir(), env: Vec::new() }
}

#[cfg(unix)]
#[test]
fn returns_what_the_process_printed() {
    let raw = run_claude_context(&command(r#"printf '%s\n' '{"type":"result","num_turns":0}'"#), &Stop::default()).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), json!({"type": "result", "num_turns": 0}));
}

#[cfg(unix)]
#[test]
fn throws_when_the_process_fails_so_the_menu_shows_nothing_from_it() {
    assert_eq!(run_claude_context(&command("exit 3"), &Stop::default()), Err(NOT_AVAILABLE.to_owned()));
}

// the reader

fn launch() -> ClaudeContextLaunch {
    ClaudeContextLaunch {
        cwd: "C:/project".into(),
        executable_path: "claude".into(),
        args: vec!["--print".into()],
        sandbox: "read-only".into(),
        permission: None,
        browser: None,
        plugins: Vec::new(),
    }
}

struct Setup {
    reader: Arc<ClaudeContextReader>,
    runs: Arc<AtomicUsize>,
    disposed: Arc<AtomicUsize>,
    prepared_with: Arc<Mutex<Vec<ClaudeContextLaunch>>>,
}

fn setup(busy: bool, run: Option<Arc<dyn Fn(&Stop) -> Result<String, String> + Send + Sync>>) -> Setup {
    let dir = std::env::temp_dir().join(format!("locust-context-{}", crate::invariants::unique_nanos()));
    let runs = Arc::new(AtomicUsize::new(0));
    let disposed = Arc::new(AtomicUsize::new(0));
    let prepared_with = Arc::new(Mutex::new(Vec::new()));
    let resolve: Arc<Resolve> = Arc::new(move |_mission: &str| {
        Some(ClaudeContextTurn { mission_id: "mission_a".into(), session_id: "session_a".into(), busy, account: None, fallback: Some(launch()) })
    });
    let prepare: Arc<Prepare> = {
        let (disposed, prepared_with) = (disposed.clone(), prepared_with.clone());
        Arc::new(move |l: &ClaudeContextLaunch, _turn: &ClaudeContextTurn| {
            prepared_with.lock().unwrap().push(l.clone());
            let disposed = disposed.clone();
            Ok(PreparedContext {
                command: ContextCommand { program: "claude".into(), args: Vec::new(), cwd: "C:/project".into(), env: Vec::new() },
                dispose: Box::new(move || {
                    disposed.fetch_add(1, Ordering::SeqCst);
                }),
            })
        })
    };
    let runner: Arc<Run> = {
        let runs = runs.clone();
        Arc::new(move |_command: &ContextCommand, stop: &Stop| {
            runs.fetch_add(1, Ordering::SeqCst);
            match &run {
                Some(run) => run(stop),
                None => Ok(FIXTURE.to_owned()),
            }
        })
    };
    let reader = ClaudeContextReader::new(ReaderOptions {
        file: dir.join("launches.json"),
        resolve,
        prepare,
        run: Some(runner),
        timeout: Duration::from_secs(10),
    });
    Setup { reader: Arc::new(reader), runs, disposed, prepared_with }
}

#[test]
fn runs_context_once_for_a_finished_turn_then_answers_from_what_it_read() {
    let s = setup(false, None);
    assert_eq!(s.reader.read("mission_a").map(|b| b.used), Some(45_200));
    assert_eq!(s.reader.read("mission_a").map(|b| b.used), Some(45_200));
    assert_eq!(s.runs.load(Ordering::SeqCst), 1);
    assert_eq!(s.disposed.load(Ordering::SeqCst), 1);
}

#[test]
fn does_not_run_while_a_turn_is_running() {
    let s = setup(true, None);
    assert_eq!(s.reader.read("mission_a"), None);
    assert_eq!(s.runs.load(Ordering::SeqCst), 0);
}

#[test]
fn gives_way_to_a_starting_turn_the_inspection_is_stopped_and_the_menu_shows_nothing_from_it() {
    // A run that only ends when it is told to stop.
    let s = setup(
        false,
        Some(Arc::new(|stop: &Stop| {
            while !stop.is_stopped() {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err("stopped".into())
        })),
    );
    let reading = {
        let reader = s.reader.clone();
        std::thread::spawn(move || reader.read("mission_a"))
    };
    std::thread::sleep(Duration::from_millis(20));
    let started = Instant::now();
    s.reader.before_start();
    assert!(started.elapsed() < Duration::from_secs(2), "the turn waited {:?}: the inspection was not stopped", started.elapsed());
    assert_eq!(reading.join().unwrap(), None);
    assert_eq!(s.disposed.load(Ordering::SeqCst), 1);
}

#[test]
fn shows_nothing_and_says_nothing_wrong_when_context_fails() {
    let s = setup(false, Some(Arc::new(|_stop: &Stop| Err("exit 1".into()))));
    assert_eq!(s.reader.read("mission_a"), None);
}

#[test]
fn the_reader_runs_context_with_the_launch_the_turn_was_remembered_with() {
    let s = setup(false, None);
    let mut remembered = launch();
    remembered.args = vec!["--print".into(), "--model".into(), "opus".into()];
    remembered.permission = Some("plan".into());
    s.reader.remember("mission_a", remembered.clone()).unwrap();
    assert_eq!(s.reader.read("mission_a").map(|b| b.used), Some(45_200));
    assert_eq!(*s.prepared_with.lock().unwrap(), vec![remembered.clone()], "the remembered flags, not the fallback");
    // Kept on disk for the next start, paths and flags only.
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&s.reader.options.file).unwrap()).unwrap();
    assert_eq!(saved["mission_a"]["args"], json!(["--print", "--model", "opus"]));
    assert_eq!(saved["mission_a"]["permission"], json!("plan"));
}

// The /context run is built by Runtime's `claude::context_command`, from the launch with its host paths made again.

fn resources_counting<'a>(
    permission: crate::claude_context::MakeAgain<'a>,
    browser: crate::claude_context::MakeAgain<'a>,
    skills: &'a (dyn Fn(bool, bool) -> Result<(Vec<(String, String)>, Held), String> + Sync),
) -> ContextResources<'a> {
    ContextResources { env: vec![("CLAUDE_CONFIG_DIR".into(), "/accounts/work".into())], permission, browser, skills }
}

/// Counts how many of what was made again have been let go of.
struct Let(Arc<AtomicUsize>);
impl Drop for Let {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn turn_of(session: &str) -> ClaudeContextTurn {
    ClaudeContextTurn { mission_id: "mission_a".into(), session_id: session.into(), busy: false, account: None, fallback: None }
}

#[test]
fn a_remembered_launch_becomes_context_with_the_turns_own_flags_and_new_host_paths() {
    let gone = Arc::new(AtomicUsize::new(0));
    let (g1, g2, g3) = (gone.clone(), gone.clone(), gone.clone());
    let permission = move |run: &str, mission: &str, cwd: &str| -> Result<(String, Held), String> {
        assert!(run.starts_with("context_"), "{run}");
        assert_eq!((mission, cwd), ("mission_a", "/work/shop"));
        Ok(("/fresh/permission/mcp.json".into(), Box::new(Let(g1.clone()))))
    };
    let browser = move |_: &str, _: &str, _: &str| -> Result<(String, Held), String> { Ok(("/fresh/browser/mcp.json".into(), Box::new(Let(g2.clone())))) };
    let skills = move |own: bool, auto: bool| -> Result<(Vec<(String, String)>, Held), String> {
        assert!(own && !auto, "a personal plugin asks for own skills; workspace-write is not Auto");
        Ok((vec![("project".into(), "/fresh/skills/project".into()), ("personal".into(), "/fresh/skills/personal".into())], Box::new(Let(g3.clone()))))
    };
    let launch = ClaudeContextLaunch {
        cwd: "/work/shop".into(),
        executable_path: "/usr/local/bin/claude".into(),
        args: [
            "--print", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--model", "opus",
            "--mcp-config", "/old/permission/mcp.json", "/old/browser/mcp.json", "--plugin-dir", "/old/skills/project",
            "--plugin-dir", "/old/skills/personal", "--resume", "sess-old",
        ]
        .map(String::from)
        .to_vec(),
        sandbox: "workspace-write".into(),
        permission: Some("/old/permission/mcp.json".into()),
        browser: Some("/old/browser/mcp.json".into()),
        plugins: vec![("project".into(), "/old/skills/project".into()), ("personal".into(), "/old/skills/personal".into())],
    };
    let prepared = prepare_claude_context_launch(&launch, &turn_of("sess-1"), &resources_counting(&permission, &browser, &skills)).unwrap();
    let c = &prepared.command;
    assert_eq!(c.program, "/usr/local/bin/claude");
    assert_eq!(c.cwd, PathBuf::from("/work/shop"));
    assert_eq!(
        c.args,
        [
            "--print", "--model", "opus", "--mcp-config", "/fresh/permission/mcp.json", "/fresh/browser/mcp.json",
            "--plugin-dir", "/fresh/skills/project", "--plugin-dir", "/fresh/skills/personal", "--output-format", "json",
            "--no-session-persistence", "--resume", "sess-1", "--", "/context",
        ]
        .map(String::from)
        .to_vec()
    );
    assert_eq!(c.env, vec![("CLAUDE_CONFIG_DIR".to_owned(), "/accounts/work".to_owned())]);
    assert_eq!(gone.load(Ordering::SeqCst), 0, "held while the run needs them");
    (prepared.dispose)();
    assert_eq!(gone.load(Ordering::SeqCst), 3, "all three let go of after the run");
}

#[test]
fn a_launch_that_cannot_be_made_again_lets_go_of_what_was_made_and_runs_nothing() {
    let gone = Arc::new(AtomicUsize::new(0));
    let g = gone.clone();
    let permission = move |_: &str, _: &str, _: &str| -> Result<(String, Held), String> { Ok(("/fresh/p.json".into(), Box::new(Let(g.clone())))) };
    let browser = |_: &str, _: &str, _: &str| -> Result<(String, Held), String> { Err("no browser".into()) };
    let skills = |_: bool, _: bool| -> Result<(Vec<(String, String)>, Held), String> { Ok((Vec::new(), Box::new(()))) };
    let mut launch = self::launch();
    launch.executable_path = "/usr/local/bin/claude".into();
    launch.permission = Some("/old/p.json".into());
    launch.browser = Some("/old/b.json".into());
    assert!(prepare_claude_context_launch(&launch, &turn_of("sess-1"), &resources_counting(&permission, &browser, &skills)).is_err());
    assert_eq!(gone.load(Ordering::SeqCst), 1, "the bridge made before the browser failed is let go of");

    // A skill plugin the fresh copies lack: refused, never run without it.
    let mut launch = self::launch();
    launch.plugins = vec![("library".into(), "/old/skills/library".into())];
    launch.executable_path = "/usr/local/bin/claude".into();
    let fine = |_: &str, _: &str, _: &str| -> Result<(String, Held), String> { Ok((String::new(), Box::new(()))) };
    assert_eq!(
        prepare_claude_context_launch(&launch, &turn_of("sess-1"), &resources_counting(&fine, &fine, &skills)).err().as_deref(),
        Some("The turn's skills are unavailable.")
    );
    // Not Claude's launch, or a session that is not one: Runtime's builder refuses it.
    let mut launch = self::launch();
    launch.executable_path = "/usr/local/bin/codex".into();
    assert_eq!(
        prepare_claude_context_launch(&launch, &turn_of("sess-1"), &resources_counting(&fine, &fine, &skills)).err().as_deref(),
        Some("The Claude launch flags are unavailable.")
    );
    let mut launch = self::launch();
    launch.executable_path = "/usr/local/bin/claude".into();
    assert_eq!(
        prepare_claude_context_launch(&launch, &turn_of("not a session"), &resources_counting(&fine, &fine, &skills)).err().as_deref(),
        Some("The Claude session is unavailable.")
    );
}
