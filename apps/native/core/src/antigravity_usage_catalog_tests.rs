//! Electron's `antigravity-usage-is-read-without-a-turn.test.ts`, the cases not yet named: Antigravity's `/usage` read
//! the way the catalog read (`read_listed_checked`) reads it, through `antigravity_usage::catalog_reading` (the gate on
//! what the sweep found, the cache, the bounded read) and `run_usage` (the real process, here a fake `agy` script),
//! and handed back beside the models in `Listed` / `usage_windows`, with Codex's own reading read by the app's own
//! Codex session (`codex_session`) against the fake app server.
use super::*;
use crate::antigravity_usage::{
    AgyFound, MAX_OUTPUT, USAGE_TIMEOUT, UsageCache, UsageExecSpec, antigravity_usage_text, catalog_reading,
    read_antigravity_usage_with, run_usage,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const RESET: &str = "2099-10-10T20:49:36Z";

fn bucket(window: &str, remaining_fraction: f64) -> Value {
    json!({"window": window, "remaining_fraction": remaining_fraction, "reset_time": RESET})
}

fn answer(other: f64) -> Value {
    json!({"status": "SUCCESS", "num_turns": 0, "command": {"name": "usage", "data": {"groups": [
        {"name": "Gemini Models", "buckets": [bucket("weekly", 0.67), bucket("5h", 0.95)]},
        {"name": "Claude and GPT models", "buckets": [bucket("weekly", other), bucket("5h", 1.0)]}
    ]}}})
}

fn version(text: &str) -> Option<crate::runtime::version::Version> {
    crate::runtime::version::parse(text)
}

fn found(prefix: &[&str], at: Option<&str>) -> AgyFound {
    AgyFound {
        command_name: "agy".into(),
        spec: UsageExecSpec {
            executable_path: "/tools/agy".into(),
            prefix_args: prefix.iter().map(|a| (*a).to_owned()).collect(),
            env: HashMap::from([("AGY_TEST".to_owned(), "yes".to_owned())]),
        },
        ready: true,
        version: at.and_then(version),
    }
}

/// A cache of the test's own, so no test sees another's reading (the app's is `antigravity_usage::usage_cache`).
fn fresh_cache() -> &'static UsageCache {
    Box::leak(Box::new(Mutex::new(None)))
}

/// A runner that says what it answers and counts each run.
fn counted(reply: Option<String>) -> (std::sync::Arc<AtomicUsize>, impl FnOnce(&str, &[String], &HashMap<String, String>) -> Option<String> + Send + 'static) {
    let runs = std::sync::Arc::new(AtomicUsize::new(0));
    let seen = runs.clone();
    (runs, move |_: &str, _: &[String], _: &HashMap<String, String>| {
        seen.fetch_add(1, Ordering::SeqCst);
        reply
    })
}

fn wait() -> Duration {
    Duration::from_secs(5)
}

/// What the catalog reads from `agy models`, which the usage read never touches.
fn listed_with(usage: Option<String>) -> Listed {
    Listed {
        antigravity: parse_agy_model_list("gemini-flash\tGemini Flash\n"),
        antigravity_usage: usage,
        ..Default::default()
    }
}

fn has_gemini_flash(listed: &Listed) -> bool {
    catalog(listed).iter().any(|m| m["id"] == "gemini-flash" && m["runtime"] == "antigravity")
}

/// A fake `agy` that writes each argument it was given on a line of `args`, and its AGY_TEST, then prints `stdout`
/// and exits with `code`.
struct FakeAgy {
    dir: PathBuf,
}

impl FakeAgy {
    fn new(stdout: &str, code: i32) -> FakeAgy {
        let dir = std::env::temp_dir().join(format!("locust-fake-agy-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{args}'; done\nprintf '%s' \"$AGY_TEST\" > '{env}'\nprintf '%s' '{out}'\nexit {code}\n",
            args = dir.join("args").display(),
            env = dir.join("env").display(),
            out = stdout.replace('\'', ""),
        );
        let path = dir.join("agy");
        std::fs::write(&path, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        FakeAgy { dir }
    }
    fn found(&self, prefix: &[&str]) -> AgyFound {
        let mut f = found(prefix, Some("1.1.11"));
        f.spec.executable_path = self.dir.join("agy").to_string_lossy().into_owned();
        f
    }
    fn args(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("args")).unwrap_or_default().lines().map(str::to_owned).collect()
    }
    fn env(&self) -> String {
        std::fs::read_to_string(self.dir.join("env")).unwrap_or_default()
    }
}

impl Drop for FakeAgy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The app's own Codex session against the fake app server: its models, its reading, and what it was asked.
fn codex_read(scenario: &str) -> (Option<(Value, Option<String>)>, String) {
    let asked = std::env::temp_dir().join(format!("locust-agy-codex-asked-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let mut command = Command::new("python3");
    command.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py")).arg(scenario).env("M1_ASKED_FILE", &asked);
    let read = codex_session(command, Duration::from_secs(20));
    let said = std::fs::read_to_string(&asked).unwrap_or_default();
    let _ = std::fs::remove_file(&asked);
    (read, said)
}

#[test]
fn agy_usage_refuses_a_prefix_that_turns_slash_commands_off() {
    // case: refuses even a discovered prefix that would disable built-in slash commands
    let refusing = found(&["--disable-slash-commands"], Some("1.1.11"));
    let (runs, run) = counted(Some(answer(0.2).to_string()));
    assert_eq!(read_antigravity_usage_with(&refusing.spec, refusing.version.as_ref(), run), None);
    let (catalog_runs, run) = counted(Some(answer(0.2).to_string()));
    assert_eq!(catalog_reading(fresh_cache(), Some(&refusing), wait(), run), None);
    assert_eq!(runs.load(Ordering::SeqCst) + catalog_runs.load(Ordering::SeqCst), 0, "nothing was started");
}

#[test]
fn agy_usage_catalog_keeps_models_without_a_safe_version() {
    // case: the catalog keeps models but does not spawn usage without a safe version (%s)
    for at in [None, Some("1.1.10")] {
        let (runs, run) = counted(Some(answer(0.2).to_string()));
        let reading = catalog_reading(fresh_cache(), Some(&found(&[], at)), wait(), run);
        assert_eq!(reading, None, "{at:?}");
        assert_eq!(runs.load(Ordering::SeqCst), 0, "{at:?}: nothing was started");
        let listed = listed_with(reading);
        assert!(has_gemini_flash(&listed), "{at:?}");
        assert!(!usage_windows(&listed).contains_key("antigravity"), "{at:?}");
    }
}

#[test]
fn agy_usage_leaves_out_the_other_group_when_not_lower() {
    // case: omits the other group when it is not lower
    let said = antigravity_usage_text(&answer(0.9)).unwrap();
    assert!(!said.contains("Claude and GPT"), "{said}");
}

#[test]
fn agy_usage_says_no_reset_it_cannot_read() {
    // case: does not invent a reset when its date cannot be read
    let mut value = answer(0.2);
    value["command"]["data"]["groups"][0]["buckets"][0]["reset_time"] = json!("unknown");
    let said = antigravity_usage_text(&value).unwrap();
    assert!(said.contains("Gemini: weekly window 67% left \u{b7} Claude"), "{said}");
}

#[test]
fn agy_usage_refuses_a_repeated_bucket_or_group() {
    // case: rejects duplicate buckets and a group named twice
    let mut duplicate = answer(0.2);
    duplicate["command"]["data"]["groups"][0]["buckets"].as_array_mut().unwrap().push(bucket("weekly", 0.4));
    assert_eq!(antigravity_usage_text(&duplicate), None);
    let mut twice = answer(0.2);
    twice["command"]["data"]["groups"].as_array_mut().unwrap().push(json!({"name": "Gemini Models", "buckets": [bucket("weekly", 0.5), bucket("5h", 0.5)]}));
    assert_eq!(antigravity_usage_text(&twice), None);
}

#[test]
fn agy_usage_runs_only_the_discovered_executable_and_argv() {
    // case: uses only the discovered executable and usage argv, with no shell and a bounded hidden process
    let seen: std::sync::Arc<Mutex<Option<(String, Vec<String>, HashMap<String, String>)>>> = Default::default();
    let keep = seen.clone();
    let reply = answer(0.2);
    let run = move |file: &str, args: &[String], env: &HashMap<String, String>| {
        *keep.lock().unwrap() = Some((file.to_owned(), args.to_vec(), env.clone()));
        Some(answer(0.2).to_string())
    };
    let f = found(&["prefix"], Some("1.1.11"));
    assert_eq!(read_antigravity_usage_with(&f.spec, f.version.as_ref(), run), Some(reply));
    let (file, args, env) = seen.lock().unwrap().clone().unwrap();
    assert_eq!(file, "/tools/agy");
    assert_eq!(args, ["prefix", "-p", "/usage", "--output-format", "json", "--print-timeout", "20s"]);
    assert!(!args.iter().any(|a| a == "--disable-slash-commands"));
    assert_eq!(env.get("AGY_TEST").map(String::as_str), Some("yes"));
    assert_eq!((USAGE_TIMEOUT, MAX_OUTPUT), (Duration::from_secs(30), 256 * 1024));
    // And for real, through the catalog: the executable is run itself, each argument as one argument, no shell between.
    let fake = FakeAgy::new(&answer(0.2).to_string(), 0);
    let reading = catalog_reading(fresh_cache(), Some(&fake.found(&["prefix; echo shell"])), wait(), run_usage).expect("a reading");
    assert!(reading.starts_with("Gemini: 5-hour window 95% left"), "{reading}");
    assert_eq!(fake.args(), ["prefix; echo shell", "-p", "/usage", "--output-format", "json", "--print-timeout", "20s"]);
    assert_eq!(fake.env(), "yes");
}

#[test]
fn agy_usage_fails_softly_when_refused_or_unreadable() {
    // case: fails softly on a refused or unreadable CLI response
    let f = found(&[], Some("1.1.11"));
    assert_eq!(read_antigravity_usage_with(&f.spec, f.version.as_ref(), |_, _, _| None), None);
    assert_eq!(read_antigravity_usage_with(&f.spec, f.version.as_ref(), |_, _, _| Some("not json".into())), None);
    let refused = FakeAgy::new("", 1);
    assert_eq!(catalog_reading(fresh_cache(), Some(&refused.found(&[])), wait(), run_usage), None);
    let garbled = FakeAgy::new("not json", 0);
    assert_eq!(catalog_reading(fresh_cache(), Some(&garbled.found(&[])), wait(), run_usage), None);
}

#[test]
fn agy_usage_kept_with_models_without_codex_and_cached() {
    // case: keeps the reading and models even when Codex is absent, and caches the read
    let cache = fresh_cache();
    let (runs, run) = counted(Some(answer(0.2).to_string()));
    let first = catalog_reading(cache, Some(&found(&[], Some("1.1.11"))), wait(), run).expect("a reading");
    let listed = listed_with(Some(first.clone()));
    assert!(listed.codex.is_none());
    let reading = usage_windows(&listed).remove("antigravity").unwrap_or_default();
    assert!(reading.starts_with("Gemini: 5-hour window 95% left"), "{reading}");
    let as_of = reading.split(" \u{b7} as of ").nth(1).expect("as of a time");
    assert!(chrono::DateTime::parse_from_rfc3339(as_of).is_ok(), "{as_of}");
    assert!(has_gemini_flash(&listed));
    let (again, run) = counted(Some(answer(0.9).to_string()));
    assert_eq!(catalog_reading(cache, Some(&found(&[], Some("1.1.11"))), wait(), run), Some(first));
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(again.load(Ordering::SeqCst), 0, "the second read is the kept one");
}

#[test]
fn agy_usage_refused_still_keeps_the_models() {
    // case: keeps models when the usage read refuses
    let (_, run) = counted(None);
    let listed = listed_with(catalog_reading(fresh_cache(), Some(&found(&[], Some("1.1.11"))), wait(), run));
    assert!(has_gemini_flash(&listed));
    assert!(!usage_windows(&listed).contains_key("antigravity"));
}

#[test]
fn agy_usage_and_codex_usage_sit_beside_their_models() {
    // case: keeps both readings beside their models without asking either runtime for a turn
    let (_, run) = counted(Some(answer(0.2).to_string()));
    let agy = catalog_reading(fresh_cache(), Some(&found(&[], Some("1.1.11"))), wait(), run);
    let (read, asked) = codex_read("models-usage");
    let (models, codex_usage) = read.expect("Codex's models");
    let listed = Listed { codex: Some(models), codex_usage, ..listed_with(agy) };
    let windows = usage_windows(&listed);
    assert!(windows.get("antigravity").is_some_and(|r| r.contains("95% left")), "{windows:?}");
    assert!(windows.get("codex").is_some_and(|r| r.contains("% used")), "{windows:?}");
    assert!(has_gemini_flash(&listed));
    assert!(catalog(&listed).iter().any(|m| m["runtime"] == "codex"));
    // Neither was asked for a turn: Codex only for its models and its account's limits.
    assert_eq!(asked, "initialize\nmodel/list\naccount/rateLimits/read\n");
}

#[test]
fn agy_usage_survives_a_failed_codex_probe() {
    // case: keeps Antigravity usage even when the Codex model probe fails
    let (_, run) = counted(Some(answer(0.2).to_string()));
    let agy = catalog_reading(fresh_cache(), Some(&found(&[], Some("1.1.11"))), wait(), run);
    let codex = codex_session(Command::new("false"), Duration::from_secs(5));
    let (codex, codex_usage) = codex.map_or((None, None), |(m, u)| (Some(m), u));
    let listed = Listed { codex, codex_usage, ..listed_with(agy) };
    assert!(usage_windows(&listed).get("antigravity").is_some_and(|r| r.contains("95% left")));
    assert!(has_gemini_flash(&listed));
}

#[test]
fn agy_usage_not_run_for_missing_unready_or_other_discoveries() {
    // case: does not run usage for missing, unready or non-agy discoveries
    let unready = AgyFound { ready: false, ..found(&[], Some("1.1.11")) };
    let other = AgyFound { command_name: "antigravity".into(), ..found(&[], Some("1.1.11")) };
    for entry in [None, Some(&unready), Some(&other)] {
        let (runs, run) = counted(Some(answer(0.2).to_string()));
        assert_eq!(catalog_reading(fresh_cache(), entry, wait(), run), None);
        assert_eq!(runs.load(Ordering::SeqCst), 0);
    }
}

// ---- Electron's `antigravity-usage-does-not-hold-the-models.test.ts` (0.709): the list waits a moment for the
// reading and goes on without it; a reading that comes later is kept for the next read ----

/// `agy /usage` answering one Gemini window with no reset, so it says exactly the words Electron's reading says.
fn one_window(window: &str, remaining_fraction: f64) -> String {
    json!({"status": "SUCCESS", "num_turns": 0, "command": {"name": "usage", "data": {"groups": [
        {"name": "Gemini Models", "buckets": [{"window": window, "remaining_fraction": remaining_fraction}]}
    ]}}})
    .to_string()
}

/// The catalog's read, as `read_listed_checked` makes it (the app's own wait), on a thread of its own: it must come
/// back within `within`, or the test says the list was held instead of hanging on it.
fn catalog_read_within(cache: &'static UsageCache, run: impl FnOnce(&str, &[String], &HashMap<String, String>) -> Option<String> + Send + 'static, within: Duration) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(catalog_reading(cache, Some(&found(&[], Some("1.1.11"))), crate::antigravity_usage::default_catalog_wait(), run));
    });
    rx.recv_timeout(within).expect("the model list was held for Antigravity's usage")
}

#[test]
fn answers_with_the_models_without_waiting_for_a_slow_usage_reading_and_shows_the_reading_when_it_comes() {
    // case: answers with OpenCode's free model without waiting for a slow usage reading, and shows the reading when it comes
    let cache = fresh_cache();
    let (finish, slow) = std::sync::mpsc::channel::<()>();
    let first = catalog_read_within(
        cache,
        move |_: &str, _: &[String], _: &HashMap<String, String>| {
            let _ = slow.recv_timeout(Duration::from_secs(60));
            Some(one_window("weekly", 0.67))
        },
        Duration::from_secs(10),
    );
    assert_eq!(first, None, "the list went on without the reading");
    let listed = Listed { opencode: Some(json!({"aliases": [], "efforts": [], "models": [{"id": "opencode/space-bunny-free", "displayName": "space-bunny-free"}]})), ..listed_with(first) };
    assert!(catalog(&listed).iter().any(|m| m["id"] == "opencode/space-bunny-free"), "{:?}", catalog(&listed));
    assert!(!usage_windows(&listed).contains_key("antigravity"));

    // The reading comes: it is kept, and the list read again carries it without asking agy again.
    finish.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while crate::invariants::lock(cache).is_none() {
        assert!(std::time::Instant::now() < deadline, "the late reading was not kept");
        std::thread::sleep(Duration::from_millis(10));
    }
    let (again, run) = counted(Some(one_window("weekly", 0.1)));
    let read_again = catalog_reading(cache, Some(&found(&[], Some("1.1.11"))), wait(), run).unwrap_or_default();
    assert!(read_again.starts_with("Gemini: weekly window 67% left \u{b7} as of "), "{read_again}");
    assert_eq!(again.load(Ordering::SeqCst), 0);
    assert_eq!(usage_windows(&listed_with(Some(read_again.clone()))).get("antigravity"), Some(&read_again));
}

#[test]
fn still_includes_a_reading_that_is_quick_and_asks_nothing_more_of_the_window() {
    // case: still includes a reading that is quick, and asks nothing more of the window
    let cache = fresh_cache();
    let (runs, run) = counted(Some(one_window("5h", 0.95)));
    let reading = catalog_read_within(cache, run, Duration::from_secs(10)).unwrap_or_default();
    assert!(reading.starts_with("Gemini: 5-hour window 95% left \u{b7} as of "), "{reading}");
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    let listed = listed_with(Some(reading.clone()));
    assert_eq!(usage_windows(&listed).get("antigravity"), Some(&reading));
    assert!(has_gemini_flash(&listed));
    // Nothing comes after it: the late path is the slow one's alone.
    let late = std::sync::Arc::new(AtomicUsize::new(0));
    let seen = late.clone();
    let quick = crate::antigravity_usage::read_usage_without_holding(
        crate::antigravity_usage::default_catalog_wait(),
        || Some("Gemini: 5-hour window 95% left".to_owned()),
        move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
        },
    );
    assert_eq!(quick.as_deref(), Some("Gemini: 5-hour window 95% left"));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(late.load(Ordering::SeqCst), 0);
}

#[test]
fn a_failed_reading_costs_the_list_nothing() {
    // case: a failed reading costs the list nothing
    let agy = FakeAgy::new("agy said no", 1);
    let cache = fresh_cache();
    let reading = catalog_reading(cache, Some(&agy.found(&[])), crate::antigravity_usage::default_catalog_wait(), run_usage);
    assert_eq!(reading, None);
    assert!(crate::invariants::lock(cache).is_none(), "nothing was kept");
    let listed = Listed { opencode: Some(json!({"aliases": [], "efforts": [], "models": [{"id": "opencode/space-bunny-free", "displayName": "space-bunny-free"}]})), ..listed_with(reading) };
    assert!(catalog(&listed).iter().any(|m| m["id"] == "opencode/space-bunny-free"));
    assert!(has_gemini_flash(&listed));
    assert!(!usage_windows(&listed).contains_key("antigravity"));
}
