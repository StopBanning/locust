//! The command grid: Electron's own builders against Rust's, for every agent, mode and option a turn can carry.
//!
//! `parity/commands.json` is written by `scripts/command-grid.mjs` from Electron's TypeScript (it names the Electron
//! commit). For each case this test builds what the Rust engine would send, the way `native.rs` puts a turn's command
//! together, and compares. Cases that differ are listed in `parity/commands-not-yet.txt`. The list may only shrink:
//! a new difference fails, and so does a listed case that now matches until it is taken off the list.
//!
//! `LOCUST_WRITE_NOT_YET=1 cargo test -p locust-core command_grid` rewrites the list, with each case's first
//! difference beside it.
//!
//! The adapter below mirrors `native.rs` (`command_for_turn`, Claude's Approve-each swap, the permission bridge and
//! the browser registration). Paths those make at run time are replaced by the grid's placeholders. When B3 moves
//! that composition into one pure builder, this test calls it instead.

use crate::approval::Mode;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const WORKSPACE: &str = "/work/project";
const BRIDGE: &str = "/locust/bridge/mcp.json";
const BROWSER: &str = "/locust/browser/mcp.json";
const GUARD_SETTINGS: &str = "/locust/guard/settings.json";
const BROWSER_COPILOT: &str = "/locust/browser/copilot-mcp.json";
const OPENCODE_GUARD: &str = "file:///guard/locust-command-guard.mjs";
const COPILOT_SESSION: &str = "00000000-0000-4000-8000-000000000000";
/// Both placeholders are replaced by the prompt file's path before Muse starts.
const TS_PROMPT_FILE: &str = "<locust:prompt-file>";

type Env = BTreeMap<String, String>;

fn text<'a>(case: &'a Value, key: &str) -> Option<&'a str> {
    case[key].as_str()
}

/// What the Rust engine would run for this case: its arguments and the environment it sets, or its refusal.
fn rust_command(case: &Value) -> Result<(Vec<String>, Env), String> {
    let runtime = text(case, "runtime").unwrap();
    let mode = Mode::parse(text(case, "mode").unwrap()).unwrap();
    let model = text(case, "model");
    let effort = text(case, "effort");
    let resume = text(case, "resume");
    let browser = case["browser"].as_bool().unwrap();
    let folder = Path::new(WORKSPACE);
    let mut env = Env::new();
    let mut args: Vec<String> = match runtime {
        // native.rs claude_args. Skills, reference folders, forks and ticked connectors are not wired in the Rust
        // engine yet, so those cases stay listed; a case without the guard is one Electron's host never builds.
        "claude" => crate::native::claude_args(
            mode,
            Some(BRIDGE),
            browser.then_some(BROWSER),
            GUARD_SETTINGS,
            model,
            effort,
            resume,
        )?,
        "codex" => super::codex::app_server_args(Some("0.160.0")),
        "cursor" => {
            if mode == Mode::ApproveEach {
                return Err("Cursor does not support Approve-each in the native driver.".into());
            }
            super::cursor::print_args(folder, mode, model, resume).map_err(|e| e.to_string())?
        }
        "copilot" => {
            if mode == Mode::ApproveEach {
                let mut a = vec!["--acp".to_string(), super::copilot::EXCLUDED_TOOLS.into()];
                if let Some(m) = model {
                    a.extend(["--model".into(), m.into()]);
                }
                if let Some(e) = effort {
                    a.extend(["--effort".into(), e.into()]);
                }
                a
            } else {
                let session = resume.is_none().then_some(COPILOT_SESSION);
                super::copilot::prompt_args("Say hi.", mode, model, effort, resume, session)
            }
        }
        "antigravity" => {
            if mode == Mode::ApproveEach {
                return Err("Antigravity does not support Approve-each in the native driver.".into());
            }
            super::antigravity::print_args(mode, model, effort, resume)
        }
        "muse" => {
            if mode == Mode::ApproveEach {
                return Err("Muse does not support Approve-each in the native driver.".into());
            }
            super::muse::exec_args(folder, mode, model, effort, resume)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|a| if a == super::muse::PROMPT_FILE_PLACEHOLDER { TS_PROMPT_FILE.into() } else { a })
                .collect()
        }
        // OpenCode 1 through its local server (opencode.rs), configured by opencode_config::configure.
        "opencode" => {
            let config = super::opencode_config::legacy_config(mode, Some(OPENCODE_GUARD));
            env.insert("OPENCODE_CONFIG_CONTENT".into(), config.to_string());
            if matches!(mode, Mode::Ask | Mode::Plan) {
                env.insert("OPENCODE_PURE".into(), "1".into());
            }
            ["serve", "--port", "0", "--hostname", "127.0.0.1"].map(String::from).to_vec()
        }
        other => panic!("no Rust adapter for {other}"),
    };
    // browser_tools::Registration::prepare
    if browser {
        match runtime {
            // Claude's browser file is in print_args' one --mcp-config.
            "copilot" => args.extend(["--additional-mcp-config".into(), format!("@{BROWSER_COPILOT}")]),
            "opencode" => {
                let mut op: Value = serde_json::from_str(&env["OPENCODE_CONFIG_CONTENT"]).unwrap();
                op["mcp"]["locust_browser"] = serde_json::json!({"type":"local","command":["/locust/locust-native","--browser-bridge","/locust/browser/host"],"environment":{},"enabled":true});
                op["permission"]["locust_browser_*"] = serde_json::json!("allow");
                env.insert("OPENCODE_CONFIG_CONTENT".into(), op.to_string());
            }
            _ => {}
        }
    }
    Ok((args, env))
}

/// The first way the two commands differ, in a line; None when they are the same.
fn difference(case: &Value) -> Option<String> {
    let electron = &case["electron"];
    let rust = rust_command(case);
    match (electron.get("refused").and_then(Value::as_str), rust) {
        (Some(_), Err(_)) => None,
        (Some(why), Ok(_)) => Some(format!("Electron refuses ({why}); Rust builds it")),
        (None, Err(why)) => Some(format!("Rust refuses ({why})")),
        (None, Ok((args, env))) => {
            let want: Vec<String> =
                electron["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()).collect();
            if args != want {
                let have: BTreeSet<&String> = args.iter().collect();
                let need: BTreeSet<&String> = want.iter().collect();
                let missing: Vec<&str> = want.iter().filter(|a| !have.contains(a)).map(String::as_str).collect();
                let extra: Vec<&str> = args.iter().filter(|a| !need.contains(a)).map(String::as_str).collect();
                return Some(match (missing.is_empty(), extra.is_empty()) {
                    (true, true) => "same arguments in another order".into(),
                    _ => format!("Rust lacks {missing:?}; Rust adds {extra:?}"),
                });
            }
            let want_env = electron["env"].as_object().unwrap();
            for (key, value) in want_env {
                let value = value.as_str().unwrap();
                match env.get(key) {
                    None => return Some(format!("Rust does not set {key}")),
                    Some(have) if key.ends_with("CONFIG_CONTENT") => {
                        let (a, b): (Value, Value) =
                            (serde_json::from_str(have).unwrap(), serde_json::from_str(value).unwrap());
                        if a != b {
                            return Some(format!("{key} differs: {}", json_difference(&b, &a, "")));
                        }
                    }
                    Some(have) if have != value => return Some(format!("{key} is {have:?}, Electron {value:?}")),
                    _ => {}
                }
            }
            for key in env.keys() {
                if !want_env.contains_key(key) {
                    return Some(format!("Rust also sets {key}"));
                }
            }
            None
        }
    }
}

/// Where two JSON values first part, as a path.
fn json_difference(electron: &Value, rust: &Value, at: &str) -> String {
    match (electron, rust) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                match b.get(k) {
                    None => return format!("Rust lacks {at}/{k}"),
                    Some(w) if w != v => return json_difference(v, w, &format!("{at}/{k}")),
                    _ => {}
                }
            }
            for k in b.keys() {
                if !a.contains_key(k) {
                    return format!("Rust adds {at}/{k}");
                }
            }
            format!("{at} differs")
        }
        _ => format!("{at}: Electron {electron}, Rust {rust}"),
    }
}

#[test]
fn rust_builds_what_electron_builds_or_the_case_is_listed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity");
    let grid: Value = serde_json::from_str(&std::fs::read_to_string(root.join("commands.json")).unwrap()).unwrap();
    let cases = grid["cases"].as_array().unwrap();
    assert!(cases.len() >= 300, "the grid has {} cases; it should have at least 300", cases.len());
    let differs: BTreeMap<String, String> = cases
        .iter()
        .filter_map(|c| difference(c).map(|d| (c["id"].as_str().unwrap().to_owned(), d)))
        .collect();
    let list_path = root.join("commands-not-yet.txt");
    if std::env::var_os("LOCUST_WRITE_NOT_YET").is_some() {
        let mut out = format!(
            "# Command grid cases where Rust does not yet build what Electron {} builds.\n\
             # Written by LOCUST_WRITE_NOT_YET=1 cargo test -p locust-core command_grid; may only shrink.\n",
            grid["electronCommit"].as_str().unwrap_or("?")
        );
        for (id, why) in &differs {
            out.push_str(&format!("{id}\t{why}\n"));
        }
        std::fs::write(&list_path, out).unwrap();
    }
    let listed: BTreeSet<String> = std::fs::read_to_string(&list_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').next().unwrap().to_owned())
        .collect();
    let new: Vec<String> =
        differs.iter().filter(|(id, _)| !listed.contains(*id)).map(|(id, why)| format!("{id}: {why}")).collect();
    let fixed: Vec<&String> = listed.iter().filter(|id| !differs.contains_key(*id)).collect();
    eprintln!(
        "command grid: {} of {} cases match Electron {}; {} listed as not yet",
        cases.len() - differs.len(),
        cases.len(),
        grid["electronCommit"].as_str().unwrap_or("?"),
        listed.len()
    );
    assert!(new.is_empty(), "Rust now differs from Electron in cases not on the list:\n{}", new.join("\n"));
    assert!(
        fixed.is_empty(),
        "these cases match Electron now; take them off parity/commands-not-yet.txt:\n{}",
        fixed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );
}
