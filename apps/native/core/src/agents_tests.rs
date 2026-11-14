//! Every answer Electron's own AI agents screen gives (parity/agents.json, from scripts/agents-goldens.mjs).
use super::*;
use serde_json::Value;

fn goldens() -> Value {
    serde_json::from_str(include_str!("../../parity/agents.json")).expect("parity/agents.json")
}

fn found(id: &str, display_name: &str, state: &Value) -> Found {
    let text = |k: &str| state[k].as_str().map(str::to_owned);
    Found {
        id: id.into(),
        display_name: display_name.into(),
        installed: state["installed"] == true,
        version: text("version"),
        auth: match state["auth"].as_str().unwrap() {
            "authenticated" => Auth::Authenticated,
            "unauthenticated" => Auth::Unauthenticated,
            "not-applicable" => Auth::NotApplicable,
            _ => Auth::Unknown,
        },
        ready: state["ready"] == true,
        status: match state["status"].as_str().unwrap() {
            "ready" => Probe::Ready,
            "auth-required" => Probe::AuthRequired,
            "unsupported" => Probe::Unsupported,
            "offline" => Probe::Offline,
            "probe-failed" => Probe::ProbeFailed,
            "missing" => Probe::Missing,
            _ => Probe::Unknown,
        },
        unsupported_note: text("unsupportedNote"),
        through_cli: state["throughCli"] == true,
        checking: state["checking"] == true,
    }
}

#[test]
fn every_row_says_what_electrons_agents_screen_says() {
    let g = goldens();
    let rows = g["rows"].as_array().unwrap();
    assert!(rows.len() > 300, "the grid is whole");
    let mut differ = Vec::new();
    for r in rows {
        let (id, name) = (r["id"].as_str().unwrap(), r["displayName"].as_str().unwrap());
        let state = &g["states"][r["state"].as_str().unwrap()];
        let integration = integration_of(id);
        assert_eq!(integration.as_str(), r["integration"], "{id}'s integration");
        let got = row_status(&found(id, name, state), integration, r["active"] == true, r["limited"].as_str());
        let want = &r["status"];
        if got.tag != want["tag"] || got.selectable != want["selectable"].as_bool().unwrap() || got.detail != want["detail"] {
            differ.push(format!("{id}/{}/active={}/limited={}: got {got:?}, Electron {want}", r["state"], r["active"], !r["limited"].is_null()));
        }
    }
    assert!(differ.is_empty(), "{} of {} rows differ:\n{}", differ.len(), rows.len(), differ.join("\n"));
}

#[test]
fn install_and_sign_in_lines_are_electrons() {
    let g = goldens();
    for (id, want) in g["install"].as_object().unwrap() {
        let name = g["rows"].as_array().unwrap().iter().find(|r| r["id"] == *id).unwrap()["displayName"].as_str().unwrap();
        assert_eq!(install_command(id).as_deref(), want["command"].as_str(), "{id} install command");
        assert_eq!(install_sentence(id, name), want["sentence"].as_str().unwrap(), "{id} install sentence");
        assert_eq!(sign_in_command(id, false).as_deref(), want["signIn"].as_str(), "{id} sign in");
        assert_eq!(sign_in_command(id, true).as_deref(), want["signInAgain"].as_str(), "{id} sign in again");
    }
}

fn answer_from(v: &Value) -> Answer {
    Answer {
        exit: v["exitCode"].as_i64().map(|c| c as i32),
        stdout: v["stdout"].as_str().unwrap_or("").into(),
        stderr: v["stderr"].as_str().unwrap_or("").into(),
        timed_out: v["timedOut"] == true,
    }
}

fn ok(stdout: &str) -> Answer {
    Answer { exit: Some(0), stdout: stdout.into(), stderr: String::new(), timed_out: false }
}

fn probe_word(p: Probe) -> &'static str {
    match p {
        Probe::Ready => "ready",
        Probe::AuthRequired => "auth-required",
        Probe::Unsupported => "unsupported",
        Probe::Offline => "offline",
        Probe::ProbeFailed => "probe-failed",
        Probe::Missing => "not-installed",
        Probe::Unknown => "unknown",
    }
}

fn auth_word(a: Auth) -> &'static str {
    match a {
        Auth::Authenticated => "authenticated",
        Auth::Unauthenticated => "unauthenticated",
        Auth::NotApplicable => "not-applicable",
        Auth::Unknown => "unknown",
    }
}

#[test]
fn discovery_decides_what_electrons_discovery_decides() {
    // parity/discovery.json: Electron's discoverInstalledRuntimes and publicStatus over made-up check answers, with
    // the real help texts this Mac's agents printed. The fake runner here answers exactly as the script's did.
    let g: Value = serde_json::from_str(include_str!("../../parity/discovery.json")).expect("parity/discovery.json");
    let results = g["results"].as_array().unwrap();
    assert!(results.len() >= 72, "the grid is whole");
    let mut differ = Vec::new();
    for r in results {
        let (id, scenario) = (r["id"].as_str().unwrap(), r["scenario"].as_str().unwrap());
        let (well, s) = (&g["well"][id], &g["scenarios"][scenario]);
        let help = std::fs::read_to_string(format!("{}/../parity/agents/{id}-help.txt", env!("CARGO_MANIFEST_DIR"))).unwrap_or_default();
        let command = well["command"].as_str().unwrap().to_owned();
        let find = |c: &str| (s["found"] != false && c == command).then(|| std::path::PathBuf::from(format!("/usr/local/bin/{c}")));
        let run = |_: &std::path::Path, args: &[&str]| -> Option<Answer> {
            let joined = args.join(" ");
            let readiness = s.get("readiness").map(answer_from);
            Some(if joined == "--version" && s.get("version").is_some() {
                answer_from(&s["version"])
            } else if joined == "--version" && matches!(id, "copilot" | "muse" | "kilo") && readiness.is_some() {
                readiness.unwrap()
            } else if joined == "--version" {
                ok(s["versionText"].as_str().unwrap_or_else(|| well["version"].as_str().unwrap()))
            } else if joined.ends_with("--help") {
                s.get("help").map(answer_from).unwrap_or_else(|| ok(&help))
            } else if well.get("models").is_some() && (joined == "--list-models" || joined == "models --verbose") {
                readiness.unwrap_or_else(|| ok(well["models"].as_str().unwrap()))
            } else {
                readiness.unwrap_or_else(|| ok(well["ready"].as_str().unwrap_or("")))
            })
        };
        let exists = |_: &std::path::Path| s["credentials"] != false;
        let seams = Seams { find: &find, run: &run, exists: &exists, home: Some("/home/someone".into()), xdg_config_home: None };
        let got = discover_one(id, &seams).expect("a known agent");
        let want = &r["status"];
        let same = got.installed == (want["installed"] == true)
            && got.version.as_deref() == want["version"].as_str()
            && auth_word(got.auth) == want["auth"]
            && probe_word(got.status) == want["status"]
            && got.ready == (want["ready"] == true)
            && got.unsupported_note.as_deref() == want["unsupportedNote"].as_str();
        if !same {
            differ.push(format!("{id}/{scenario}: got {got:?}\n   Electron {want}"));
        }
    }
    assert!(differ.is_empty(), "{} of {} differ:\n{}", differ.len(), results.len(), differ.join("\n"));
}

#[test]
fn a_check_that_does_not_answer_in_time_is_ended_and_said_so() {
    // A real process: `sleep 5` with a limit of 0.2 s is cut off and reads as timed out, not as a failure with words.
    let sleep = crate::runtime::discovery::find_named("sleep").expect("sleep on PATH");
    let start = std::time::Instant::now();
    let a = run_bounded(&sleep, &["5"], std::time::Duration::from_millis(200)).unwrap();
    assert!(a.timed_out && a.exit.is_none(), "{a:?}");
    assert!(start.elapsed() < std::time::Duration::from_secs(3), "it was ended, not waited out");
    // And one that answers gives its exit code and output.
    let echo = crate::runtime::discovery::find_named("echo").expect("echo on PATH");
    let b = run_bounded(&echo, &["codex-cli", "0.153.0"], PROBE_TIMEOUT).unwrap();
    assert_eq!((b.exit, b.stdout.trim()), (Some(0), "codex-cli 0.153.0"));
}

#[test]
fn antigravity_is_ready_with_a_model_list_signed_out_without_one_and_unconfirmed_when_silent() {
    // antigravity-host.ts: a list is ready, an answer with no list is signed out, no answer is not confirmed.
    let check = |models: Option<Answer>| {
        let find = |c: &str| (c == "agy").then(|| std::path::PathBuf::from("/usr/local/bin/agy"));
        let run = |_: &std::path::Path, a: &[&str]| -> Option<Answer> {
            if a == ["--version"] { Some(ok("1.2.14")) } else { models.clone() }
        };
        let exists = |_: &std::path::Path| true;
        discover_antigravity(&Seams { find: &find, run: &run, exists: &exists, home: None, xdg_config_home: None })
    };
    let ready = check(Some(ok("Fetching available models...\ngemini-4-argon-high\tGemini 4 Argon (High)\n")));
    assert_eq!((ready.status, ready.auth, ready.through_cli, ready.version.as_deref()), (Probe::Ready, Auth::Authenticated, true, Some("1.2.14")));
    // Ready through its own CLI reads READY, not EXPERIMENTAL.
    assert_eq!(row_status(&ready, integration_of("antigravity"), false, None).tag, "READY");
    let out = check(Some(Answer { exit: Some(1), stdout: "Please sign in".into(), stderr: String::new(), timed_out: false }));
    assert_eq!((out.status, out.auth), (Probe::AuthRequired, Auth::Unauthenticated));
    let silent = check(Some(Answer { exit: None, stdout: String::new(), stderr: String::new(), timed_out: true }));
    assert_eq!((silent.status, silent.auth), (Probe::ProbeFailed, Auth::Unknown));
    assert_eq!(row_status(&silent, integration_of("antigravity"), false, None).tag, "CHECKING");
}

#[test]
fn names_the_exact_package_for_each_of_the_four_that_install_from_npm() {
    assert_eq!(install_command("claude").as_deref(), Some("npm install -g @anthropic-ai/claude-code"));
    assert_eq!(install_command("codex").as_deref(), Some("npm install -g @openai/codex"));
    assert_eq!(install_command("copilot").as_deref(), Some("npm install -g @github/copilot"));
    assert_eq!(install_command("opencode").as_deref(), Some("npm install -g opencode-ai"));
}

#[test]
fn offers_no_command_for_the_ones_that_do_not_install_from_npm() {
    assert_eq!(install_command("cursor"), None);
    assert_eq!(install_package("cursor"), None);
    assert_eq!(
        runtime_install_facts("cursor").map(|f| f.install),
        Some(RuntimeInstall::Vendor("https://cursor.com/cli")),
    );
    assert_eq!(install_command("antigravity"), None);
    assert_eq!(install_package("antigravity"), None);
}

#[test]
fn knows_the_one_runtime_that_needs_no_account_and_that_the_others_do() {
    assert_eq!(FREE_START_RUNTIME, "opencode");
    let opencode = runtime_install_facts("opencode").unwrap();
    assert_eq!(opencode.sign_in, None);
    assert_eq!(opencode.account, None);
    for id in ["claude", "codex", "copilot", "cursor"] {
        let f = runtime_install_facts(id).unwrap();
        assert!(f.sign_in.is_some_and(|s| !s.is_empty()), "{id} sign_in");
        assert!(f.account.is_some_and(|s| !s.is_empty()), "{id} account");
    }
}

#[test]
fn says_what_to_do_next_not_only_what_is_missing() {
    assert_eq!(
        install_sentence("opencode", "OpenCode"),
        "OpenCode was not found on this machine. Press Install. It needs no account and no sign-in, and its free model runs as soon as it is there.",
    );
    assert_eq!(
        install_sentence("claude", "Claude Code"),
        "Claude Code was not found on this machine. Press Install, then Sign in. It needs an Anthropic account.",
    );
    assert_eq!(
        install_sentence("copilot", "Copilot CLI"),
        "Copilot CLI was not found on this machine. Press Install, then Sign in. It needs a GitHub Copilot subscription.",
    );
    assert_eq!(
        install_sentence("kilo", "Kilo"),
        "Kilo was not found on this machine. Press Install. Its free model needs no account; run kilo auth login only for a paid model.",
    );
    assert_eq!(
        install_sentence("cursor", "Cursor Agent"),
        "Cursor Agent was not found on this machine. It installs from https://cursor.com/cli.",
    );
}

#[test]
fn answers_nothing_for_an_id_that_names_no_installable_thing() {
    assert_eq!(install_command("omniroute"), None);
    assert_eq!(runtime_install_facts("omniroute"), None);
    assert_eq!(install_sentence("omniroute", "Omniroute"), "Omniroute was not found on this machine.");
    assert_eq!(runtime_install_facts("gemini"), None);
}

#[test]
fn sign_in_opened_line_notes_in_window_account_switch_only_when_signing_in_again() {
    assert_eq!(sign_in_opened_line("claude", false), "Finish in the window that opened.");
    assert_eq!(sign_in_opened_line("claude", true), "Finish in the window that opened.");
    assert_eq!(sign_in_opened_line("antigravity", false), "Finish in the window that opened.");
    assert_eq!(
        sign_in_opened_line("antigravity", true),
        "In the window that opened, type /logout, then /login, and choose the account.",
    );
}
