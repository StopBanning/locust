//! Antigravity's live `/usage` reading (`apps/desktop/src/main/antigravity-usage.ts`).
//!
//! Reads only `agy`'s built-in `/usage` slash command on versions `>= 1.1.11`
//! (never a prerelease, never when `--disable-slash-commands` is present),
//! formats Gemini and any lower-remaining model group, and never holds up
//! the model catalog when `agy` is slow.

use crate::runtime::version::Version;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

/// How long `/usage` may run, and how much it may print, before it is given up (antigravity-usage.ts `timeout`,
/// `maxBuffer`).
pub const USAGE_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_OUTPUT: usize = 256 * 1024;
const CATALOG_WAIT: Duration = Duration::from_millis(1_500);
const CACHE_TTL: Duration = Duration::from_secs(600);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageExecSpec {
    pub executable_path: String,
    pub prefix_args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// Only `>= 1.1.11` (and never a prerelease) has built-in `/usage`;
/// unknown versions fail closed before starting any process.
pub fn is_supported_usage_version(version: Option<&Version>, prefix_args: &[String]) -> bool {
    let Some(v) = version else {
        return false;
    };
    if prefix_args.iter().any(|a| a == "--disable-slash-commands") {
        return false;
    }
    let has_prerelease = v
        .version
        .split('+')
        .next()
        .is_some_and(|core| core.contains('-'));
    if has_prerelease {
        return false;
    }
    v.major > 1 || (v.major == 1 && (v.minor > 1 || (v.minor == 1 && v.patch >= 11)))
}

pub fn usage_args(prefix_args: &[String]) -> Vec<String> {
    let mut args = prefix_args.to_vec();
    args.extend([
        "-p".into(),
        "/usage".into(),
        "--output-format".into(),
        "json".into(),
        "--print-timeout".into(),
        "20s".into(),
    ]);
    args
}

/// Run `agy -p /usage --output-format json --print-timeout 20s` via a testable runner seam.
pub fn read_antigravity_usage_with<F>(
    spec: &UsageExecSpec,
    version: Option<&Version>,
    run: F,
) -> Option<Value>
where
    F: FnOnce(&str, &[String], &HashMap<String, String>) -> Option<String>,
{
    if !is_supported_usage_version(version, &spec.prefix_args) {
        return None;
    }
    let args = usage_args(&spec.prefix_args);
    let stdout = run(&spec.executable_path, &args, &spec.env)?;
    serde_json::from_str(&stdout).ok()
}

/// Read live `/usage` from `agy` on disk (no shell, bounded time and output).
pub fn read_antigravity_usage(spec: &UsageExecSpec, version: Option<&Version>) -> Option<Value> {
    read_antigravity_usage_with(spec, version, |exe, args, env| {
        run_bounded_json(Path::new(exe), args, env, USAGE_TIMEOUT)
    })
}

fn run_bounded_json(
    path: &Path,
    args: &[String],
    env: &HashMap<String, String>,
    limit: Duration,
) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().ok()?;
    let mut out = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut out).take((MAX_OUTPUT + 1) as u64).read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    match rx.recv_timeout(limit) {
        Ok(bytes) => {
            let status = child.wait().ok()?;
            if !status.success() || bytes.len() > MAX_OUTPUT {
                return None;
            }
            String::from_utf8(bytes).ok()
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

fn group_label(name: &Value) -> Option<String> {
    let raw = name.as_str()?;
    match raw {
        "Gemini Models" => Some("Gemini".into()),
        "Claude and GPT models" => Some("Claude and GPT".into()),
        _ => {
            let trimmed = raw.trim();
            let len = trimmed.chars().count();
            if len > 0
                && len <= 40
                && !trimmed
                    .chars()
                    .any(|c| c == '·' || c == '%' || c.is_control())
            {
                Some(trimmed.to_owned())
            } else {
                None
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Bucket {
    remaining: f64,
    reset: Option<String>,
}

/// Gemini's windows first (5-hour before weekly); any other group is included when its
/// lowest remaining fraction is strictly lower than the first group's (`antigravityUsageText`).
pub fn antigravity_usage_text(answer: &Value) -> Option<String> {
    let obj = answer.as_object()?;
    if obj.get("status").and_then(Value::as_str) != Some("SUCCESS")
        || obj.get("num_turns").and_then(Value::as_u64) != Some(0)
    {
        return None;
    }
    let command = obj.get("command")?.as_object()?;
    if command.get("name").and_then(Value::as_str) != Some("usage") {
        return None;
    }
    let groups_arr = command.get("data")?.as_object()?.get("groups")?.as_array()?;

    let mut seen_labels: Vec<String> = Vec::new();
    let mut rendered: Vec<(String, String, f64)> = Vec::new();

    for group in groups_arr {
        let group_obj = group.as_object()?;
        let buckets_arr = group_obj.get("buckets")?.as_array()?;
        let label = group_label(group_obj.get("name").unwrap_or(&Value::Null));
        if let Some(ref l) = label {
            if seen_labels.contains(l) {
                return None;
            }
            seen_labels.push(l.clone());
        }
        let mut buckets: HashMap<&'static str, Bucket> = HashMap::new();
        for bucket in buckets_arr {
            let b_obj = bucket.as_object()?;
            let window = match b_obj.get("window").and_then(Value::as_str) {
                Some("5h") => "5h",
                Some("weekly") => "weekly",
                _ => continue,
            };
            let remaining = b_obj.get("remaining_fraction").and_then(Value::as_f64)?;
            if !remaining.is_finite() || !(0.0..=1.0).contains(&remaining) || buckets.contains_key(window) {
                return None;
            }
            let reset = if remaining == 1.0 {
                None
            } else {
                b_obj
                    .get("reset_time")
                    .and_then(Value::as_str)
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| {
                        dt.with_timezone(&Utc)
                            .to_rfc3339_opts(SecondsFormat::Millis, true)
                    })
            };
            buckets.insert(window, Bucket { remaining, reset });
        }
        let Some(label) = label else { continue };
        if buckets.is_empty() {
            continue;
        }
        let mut parts = Vec::new();
        for window in ["5h", "weekly"] {
            if let Some(b) = buckets.get(window) {
                let w_name = if window == "5h" { "5-hour" } else { "weekly" };
                let pct = (b.remaining * 100.0).round() as i64;
                let reset_part = match &b.reset {
                    Some(r) => format!(" · resets {r}"),
                    None => String::new(),
                };
                parts.push(format!("{label}: {w_name} window {pct}% left{reset_part}"));
            }
        }
        let min_rem = buckets
            .values()
            .map(|b| b.remaining)
            .fold(f64::INFINITY, f64::min);
        rendered.push((label, parts.join(" · "), min_rem));
    }

    let first_idx = rendered
        .iter()
        .position(|(label, _, _)| label == "Gemini")
        .or_else(|| (!rendered.is_empty()).then_some(0))?;
    let first_min = rendered[first_idx].2;
    let mut out = vec![rendered[first_idx].1.clone()];
    for (idx, (_, text, min_rem)) in rendered.iter().enumerate() {
        if idx != first_idx && *min_rem < first_min {
            out.push(text.clone());
        }
    }
    Some(out.join(" · "))
}

static USAGE_CACHE: OnceLock<Mutex<Option<(Instant, String)>>> = OnceLock::new();

/// Where a reading is kept between catalog reads, and when it was read.
pub type UsageCache = Mutex<Option<(Instant, String)>>;

/// The app's one reading of Antigravity's usage, which `limits` reads too.
pub fn usage_cache() -> &'static Mutex<Option<(Instant, String)>> {
    USAGE_CACHE.get_or_init(|| Mutex::new(None))
}

/// Returns the cached live Antigravity usage text if fresh, without spawning `agy`.
pub fn cached_usage_text() -> Option<String> {
    let guard = crate::invariants::lock(usage_cache());
    guard
        .as_ref()
        .filter(|(at, _)| at.elapsed() < CACHE_TTL)
        .map(|(_, text)| text.clone())
}

pub fn store_cached_usage_text(text: String) {
    *crate::invariants::lock(usage_cache()) = Some((Instant::now(), text));
}

/// Wait up to `deadline` (default `CATALOG_WAIT` = 1.5s) for `read_fn`.
/// If it finishes in time, returns `Some(reading)` without calling `on_late`.
/// If slower, returns `None` immediately and invokes `on_late(reading)` once `read_fn` finishes.
pub fn read_usage_without_holding<F, L>(
    deadline: Duration,
    read_fn: F,
    on_late: L,
) -> Option<String>
where
    F: FnOnce() -> Option<String> + Send + 'static,
    L: FnOnce(String) + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::sync_channel::<Option<String>>(1);
    std::thread::spawn(move || {
        let result = read_fn();
        if tx.send(result.clone()).is_err()
            && let Some(reading) = result
        {
            on_late(reading);
        }
    });
    match rx.recv_timeout(deadline) {
        Ok(res) => res,
        Err(_) => {
            drop(rx);
            None
        }
    }
}

pub fn default_catalog_wait() -> Duration {
    CATALOG_WAIT
}

/// `/usage` run for real: the discovered executable and its argv, no shell, bounded in time and output.
pub fn run_usage(executable: &str, args: &[String], env: &HashMap<String, String>) -> Option<String> {
    run_bounded_json(Path::new(executable), args, env, USAGE_TIMEOUT)
}

/// What the sweep discovered for Antigravity, as the catalog needs it to read `/usage` (model-catalog.ts reads the
/// discovery entry): its command (`agy` for the CLI), how to run it, whether it is ready, and the version it said.
#[derive(Clone, Debug)]
pub struct AgyFound {
    pub command_name: String,
    pub spec: UsageExecSpec,
    pub ready: bool,
    pub version: Option<Version>,
}

/// Antigravity's usage as the model catalog reads it (model-catalog.ts 0.685/0.709): only for a ready `agy` the sweep
/// discovered; a fresh reading in `cache` is used without starting anything; otherwise `/usage` is read through `run`
/// (never on a version without it), said in words with when it was read, and kept in `cache`. The catalog waits `wait`
/// for it and goes on without it after that; a late reading is still kept for the next read.
pub fn catalog_reading<F>(cache: &'static UsageCache, found: Option<&AgyFound>, wait: Duration, run: F) -> Option<String>
where
    F: FnOnce(&str, &[String], &HashMap<String, String>) -> Option<String> + Send + 'static,
{
    let found = found.filter(|f| f.ready && f.command_name == "agy")?;
    let held = crate::invariants::lock(cache).as_ref().filter(|(at, _)| at.elapsed() < CACHE_TTL).map(|(_, text)| text.clone());
    if held.is_some() {
        return held;
    }
    let (spec, version) = (found.spec.clone(), found.version.clone());
    read_usage_without_holding(
        wait,
        move || {
            let answer = read_antigravity_usage_with(&spec, version.as_ref(), run)?;
            let reading = format!("{} \u{b7} as of {}", antigravity_usage_text(&answer)?, Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true));
            *crate::invariants::lock(cache) = Some((Instant::now(), reading.clone()));
            Some(reading)
        },
        |_| {},
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::version::parse as parse_version;
    use serde_json::json;

    const RESET: &str = "2099-10-10T20:49:36Z";

    fn bucket(window: &str, remaining_fraction: f64) -> Value {
        json!({ "window": window, "remaining_fraction": remaining_fraction, "reset_time": RESET })
    }

    fn answer(other: f64) -> Value {
        json!({
            "status": "SUCCESS",
            "num_turns": 0,
            "command": {
                "name": "usage",
                "data": {
                    "groups": [
                        { "name": "Gemini Models", "buckets": [bucket("weekly", 0.67), bucket("5h", 0.95)] },
                        { "name": "Claude and GPT models", "buckets": [bucket("weekly", other), bucket("5h", 1.0)] },
                    ]
                }
            }
        })
    }

    fn spec() -> UsageExecSpec {
        UsageExecSpec {
            executable_path: "/tools/agy".into(),
            prefix_args: Vec::new(),
            env: HashMap::from([("AGY_TEST".into(), "yes".into())]),
        }
    }

    #[test]
    fn never_runs_usage_on_old_or_unknown_or_prerelease_version() {
        for raw in [None, Some("unknown"), Some("0.9.99"), Some("1.0.99"), Some("1.1.10"), Some("1.1.11-rc.1")] {
            let ver = raw.and_then(parse_version);
            let mut called = false;
            let res = read_antigravity_usage_with(&spec(), ver.as_ref(), |_, _, _| {
                called = true;
                Some(answer(0.2).to_string())
            });
            assert_eq!(res, None, "raw={raw:?}");
            assert!(!called, "raw={raw:?}");
        }
    }

    #[test]
    fn runs_built_in_command_on_supported_versions() {
        for raw in ["1.1.11", "1.1.11+build.1", "1.1.12", "1.2.0", "2.0.0"] {
            let ver = parse_version(raw).expect("parse");
            let mut called = false;
            let res = read_antigravity_usage_with(&spec(), Some(&ver), |exe, args, env| {
                called = true;
                assert_eq!(exe, "/tools/agy");
                assert_eq!(args, ["-p", "/usage", "--output-format", "json", "--print-timeout", "20s"]);
                assert_eq!(env.get("AGY_TEST").map(String::as_str), Some("yes"));
                Some(answer(0.2).to_string())
            });
            assert_eq!(res, Some(answer(0.2)), "raw={raw}");
            assert!(called);
        }
    }

    #[test]
    fn refuses_prefix_that_disables_slash_commands() {
        let ver = parse_version("1.1.11").unwrap();
        let mut s = spec();
        s.prefix_args = vec!["--disable-slash-commands".into()];
        let mut called = false;
        let res = read_antigravity_usage_with(&s, Some(&ver), |_, _, _| {
            called = true;
            Some(answer(0.2).to_string())
        });
        assert_eq!(res, None);
        assert!(!called);
    }

    #[test]
    fn puts_short_window_first_and_includes_lower_claude_and_gpt_limit() {
        assert_eq!(
            antigravity_usage_text(&answer(0.2)).as_deref(),
            Some("Gemini: 5-hour window 95% left · resets 2099-10-10T20:49:36.000Z · Gemini: weekly window 67% left · resets 2099-10-10T20:49:36.000Z · Claude and GPT: 5-hour window 100% left · Claude and GPT: weekly window 20% left · resets 2099-10-10T20:49:36.000Z")
        );
        assert!(!antigravity_usage_text(&answer(0.9)).unwrap().contains("Claude and GPT"));
    }

    #[test]
    fn does_not_call_full_bucket_a_future_reset() {
        let mut full = answer(1.0);
        full["command"]["data"]["groups"][0]["buckets"] = json!([bucket("weekly", 1.0), bucket("5h", 1.0)]);
        assert_eq!(
            antigravity_usage_text(&full).as_deref(),
            Some("Gemini: 5-hour window 100% left · Gemini: weekly window 100% left")
        );
    }

    #[test]
    fn rejects_malformed_or_invalid_fractions_or_duplicates() {
        assert_eq!(antigravity_usage_text(&json!({})), None);
        assert_eq!(antigravity_usage_text(&json!({ "status": "ERROR" })), None);

        let mut bad_frac = answer(0.2);
        bad_frac["command"]["data"]["groups"][0]["buckets"][0]["remaining_fraction"] = json!(1.1);
        assert_eq!(antigravity_usage_text(&bad_frac), None);

        let mut dup_bucket = answer(0.2);
        dup_bucket["command"]["data"]["groups"][0]["buckets"]
            .as_array_mut()
            .unwrap()
            .push(bucket("weekly", 0.4));
        assert_eq!(antigravity_usage_text(&dup_bucket), None);

        let mut dup_group = answer(0.2);
        dup_group["command"]["data"]["groups"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "name": "Gemini Models", "buckets": [bucket("weekly", 0.5), bucket("5h", 0.5)] }));
        assert_eq!(antigravity_usage_text(&dup_group), None);
    }

    #[test]
    fn an_antigravity_model_with_its_own_limit_is_shown() {
        let b = |w, r| json!({ "window": w, "remaining_fraction": r, "reset_time": "2099-10-23T02:27:29Z" });
        let ans = |groups: Vec<Value>| json!({ "status": "SUCCESS", "num_turns": 0, "command": { "name": "usage", "data": { "groups": groups } } });
        let gemini = |r| json!({ "name": "Gemini Models", "buckets": [b("weekly", r), b("5h", r)] });
        let claude = |r| json!({ "name": "Claude and GPT models", "buckets": [b("weekly", r), b("5h", r)] });
        let argon = |r| json!({ "name": "Gemini 4 Argon", "buckets": [b("weekly", r)] });
        let reset = " · resets 2099-10-23T02:27:29.000Z";

        assert_eq!(
            antigravity_usage_text(&ans(vec![gemini(1.0), claude(1.0), argon(0.84)])).as_deref(),
            Some(format!("Gemini: 5-hour window 100% left · Gemini: weekly window 100% left · Gemini 4 Argon: weekly window 84% left{reset}").as_str())
        );
        assert_eq!(
            antigravity_usage_text(&ans(vec![gemini(0.5), claude(1.0), argon(0.9)])).as_deref(),
            Some(format!("Gemini: 5-hour window 50% left{reset} · Gemini: weekly window 50% left{reset}").as_str())
        );
        let daily = json!({ "name": "Gemini Models", "buckets": [b("weekly", 0.6), b("daily", 0.1)] });
        assert_eq!(
            antigravity_usage_text(&ans(vec![daily])).as_deref(),
            Some(format!("Gemini: weekly window 60% left{reset}").as_str())
        );
        assert_eq!(
            antigravity_usage_text(&ans(vec![argon(0.3), claude(0.2)])).as_deref(),
            Some(format!("Gemini 4 Argon: weekly window 30% left{reset} · Claude and GPT: 5-hour window 20% left{reset} · Claude and GPT: weekly window 20% left{reset}").as_str())
        );
        for bad_name in [json!(42), json!(""), json!(" "), json!("x".repeat(41)), json!("Gemini · Pro"), json!("50% Pro"), json!("line\nbreak")] {
            assert_eq!(
                antigravity_usage_text(&ans(vec![gemini(0.9), json!({ "name": bad_name, "buckets": [b("weekly", 0.1)] })])).as_deref(),
                Some(format!("Gemini: 5-hour window 90% left{reset} · Gemini: weekly window 90% left{reset}").as_str())
            );
        }
        assert_eq!(antigravity_usage_text(&ans(vec![])), None);
    }

    #[test]
    fn antigravity_usage_does_not_hold_the_models() {
        use std::sync::{Arc, Mutex};
        let late_called = Arc::new(Mutex::new(None));
        let late_clone = late_called.clone();
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let quick = read_usage_without_holding(
            Duration::from_millis(200),
            || Some("Gemini: 5-hour window 95% left".into()),
            |_| unreachable!("quick reading must not fire on_late"),
        );
        assert_eq!(quick.as_deref(), Some("Gemini: 5-hour window 95% left"));

        let slow = read_usage_without_holding(
            Duration::from_millis(10),
            move || {
                let _ = rx.recv_timeout(Duration::from_secs(2));
                Some("Gemini: weekly window 67% left".into())
            },
            move |reading| {
                *crate::invariants::lock(&late_clone) = Some(reading);
            },
        );
        assert_eq!(slow, None);
        let _ = tx.send(());
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if crate::invariants::lock(&late_called).is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            crate::invariants::lock(&late_called).as_deref(),
            Some("Gemini: weekly window 67% left")
        );
    }
}
