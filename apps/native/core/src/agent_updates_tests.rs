//! Electron's main/runtime-updates.test.ts, main/the-in-use-check-reads-any-path.test.ts and
//! renderer/src/agent-updates-say-what-happened.test.ts: keeping Codex and Copilot current, on fakes.
//!
//! Colin, 2026-09-23: "new gpt-6 models released we need those added". 0.303 made it ask first, suspecting a 159 MB
//! download for a beta tester's dropped connection; the drops went on, only on Codex runs, and Colin: "automatic updating
//! for the models should be fine". So it updates on its own by default, and asks only when the person turned that off.
use super::*;
use chrono::TimeZone;
use std::sync::Arc;
use std::sync::mpsc;

const HOUR: i64 = 60 * 60 * 1000;
const MAC_ROOT: &str = "/opt/homebrew/lib/node_modules";

fn now() -> i64 {
    parse_ms("2026-09-24T12:00:00.000Z").unwrap()
}

fn released(version: &str, hours_ago: i64) -> Release {
    Release { version: version.into(), published_at: Some(iso(now() - hours_ago * HOUR)) }
}

fn exe(shim: &str) -> Executable {
    Executable { discovered_path: shim.into(), executable_path: shim.into(), prefix_args: vec![] }
}

/// Codex as npm put it on a Mac: the shim is a link in `<prefix>/bin`, run by node from inside the package.
fn codex() -> Discovered {
    Discovered {
        id: "codex".into(),
        version: Some("0.153.0".into()),
        executable: Some(Executable {
            discovered_path: "/opt/homebrew/bin/codex".into(),
            executable_path: "/opt/homebrew/bin/node".into(),
            prefix_args: vec![format!("{MAC_ROOT}/@openai/codex/bin/codex.js")],
        }),
    }
}

fn no_link(_: &str) -> Option<String> {
    None
}

// ---- versions ----

#[test]
fn versions_compare_as_numbers_a_prerelease_below_its_release() {
    use std::cmp::Ordering::{Equal, Greater, Less};
    assert_eq!(compare_versions("0.156.1", "0.153.0"), Greater);
    assert_eq!(compare_versions("0.153.0", "0.156.1"), Less);
    assert_eq!(compare_versions("0.10.0", "0.9.9"), Greater);
    // Prereleases by their numbers (2026-10-10 sweep).
    assert_eq!(compare_versions("0.158.0-alpha.10", "0.158.0-alpha.9"), Greater);
    assert_eq!(compare_versions("0.158.0-alpha.9", "0.158.0-beta.1"), Less);
    assert_eq!(compare_versions("0.158.0-alpha", "0.158.0-alpha.1"), Less);
    assert_eq!(compare_versions("0.158.0-alpha.4", "0.158.0"), Less);
    assert_eq!(compare_versions("1.0.88", "1.0.88"), Equal);
    // A leading v, and a part that is not a number, read as Electron's parseInt does.
    assert_eq!(compare_versions("v1.2.3", "1.2.3"), Equal);
    assert_eq!(compare_versions("1.x.3", "1.0.3"), Equal);
}

// ---- what to do about one agent ----

fn base<'a>(latest: Option<&'a Release>) -> DecideInput<'a> {
    DecideInput { installed: Some("0.153.0"), latest, automatic: true, in_use: false, now: now(), held: false }
}

fn waiting(version: &str, why: Why) -> Decided {
    Decided::Say(Status::Waiting { version: version.into(), why })
}

#[test]
fn asks_first_a_newer_release_waits_for_the_person_to_press_update_however_old_and_however_idle() {
    let r = released("0.156.1", 400);
    assert_eq!(decide(&DecideInput { automatic: false, ..base(Some(&r)) }), waiting("0.156.1", Why::Ask));
}

#[test]
fn updates_to_a_newer_release_that_has_been_out_long_enough_when_nothing_is_using_it() {
    let r = released("0.156.1", 14);
    assert_eq!(decide(&base(Some(&r))), Decided::Update("0.156.1".into()));
}

#[test]
fn waits_while_something_is_using_it_a_mission_or_another_session_running_that_copy() {
    let r = released("0.156.1", 14);
    assert_eq!(decide(&DecideInput { in_use: true, ..base(Some(&r)) }), waiting("0.156.1", Why::InUse));
}

#[test]
fn waits_on_a_release_out_for_less_than_twelve_hours_or_one_npm_gives_no_date_for() {
    let young = released("0.156.1", 2);
    assert_eq!(decide(&base(Some(&young))), waiting("0.156.1", Why::TooNew));
    let undated = Release { version: "0.156.1".into(), published_at: None };
    assert_eq!(decide(&base(Some(&undated))), waiting("0.156.1", Why::TooNew));
    // A date that is not one is as good as none.
    let garbled = Release { version: "0.156.1".into(), published_at: Some("last tuesday".into()) };
    assert_eq!(decide(&base(Some(&garbled))), waiting("0.156.1", Why::TooNew));
    assert_eq!(RELEASE_AGE_MS, 12 * HOUR);
    // Exactly twelve hours is old enough.
    assert_eq!(decide(&base(Some(&released("0.156.1", 12)))), Decided::Update("0.156.1".into()));
}

#[test]
fn does_nothing_when_it_is_current_or_ahead_of_what_npm_calls_newest() {
    let same = released("0.153.0", 40);
    assert_eq!(decide(&base(Some(&same))), Decided::Say(Status::Current));
    let behind = released("0.156.1", 40);
    assert_eq!(decide(&DecideInput { installed: Some("0.157.0"), ..base(Some(&behind)) }), Decided::Say(Status::Current));
    // Nothing installed, or nothing known of npm's newest: nothing to say.
    assert_eq!(decide(&DecideInput { installed: None, ..base(Some(&behind)) }), Decided::Say(Status::Current));
    assert_eq!(decide(&base(None)), Decided::Say(Status::Current));
}

#[test]
fn a_release_the_check_could_not_read_waits_as_held_however_old_but_after_the_ask_rule() {
    let r = released("0.156.1", 400);
    assert_eq!(decide(&DecideInput { held: true, ..base(Some(&r)) }), waiting("0.156.1", Why::Held));
    // Turned off, the reason is that the person is asked, whatever the check said.
    assert_eq!(decide(&DecideInput { held: true, automatic: false, ..base(Some(&r)) }), waiting("0.156.1", Why::Ask));
}

// ---- only what npm put there ----

// npm's Windows layout (codex.cmd beside node_modules, run by node.exe), which only a Windows host joins this way:
// these two cannot apply on a Mac, where the path rules differ, so they exist only in a Windows build (Electron skips
// them off win32 too).
#[cfg(windows)]
#[test]
fn windows_is_npms_its_shim_in_npms_own_folder_or_its_script_inside_the_package() {
    let root = "C:\\Users\\me\\AppData\\Roaming\\npm\\node_modules";
    let d = Discovered {
        id: "codex".into(),
        version: Some("0.153.0".into()),
        executable: Some(Executable {
            discovered_path: "C:\\Users\\me\\AppData\\Roaming\\npm\\codex.cmd".into(),
            executable_path: "C:\\Program Files\\nodejs\\node.exe".into(),
            prefix_args: vec![format!("{root}\\@openai\\codex\\bin\\codex.js")],
        }),
    };
    assert_eq!(npm_package_dir(&d, root, &no_link), Some(format!("{root}\\@openai\\codex")));
}

#[cfg(windows)]
#[test]
fn windows_is_not_an_agent_found_somewhere_else_npm_would_put_a_second_copy_beside_it() {
    let root = "C:\\Users\\me\\AppData\\Roaming\\npm\\node_modules";
    let d = Discovered { id: "codex".into(), version: Some("0.153.0".into()), executable: Some(exe("C:\\tools\\codex\\codex.exe")) };
    assert_eq!(npm_package_dir(&d, root, &no_link), None);
}

#[test]
fn a_shim_in_npms_own_prefix_folder_is_npms_where_the_prefix_is_node_modules_parent() {
    // The Windows shape on a Mac's paths: the shim sits in the prefix itself, the folder above node_modules.
    let d = Discovered { id: "codex".into(), version: Some("0.153.0".into()), executable: Some(exe("/Users/me/.npm-global/codex")) };
    assert_eq!(npm_package_dir(&d, "/Users/me/.npm-global/node_modules", &no_link), Some("/Users/me/.npm-global/node_modules/@openai/codex".into()));
    let elsewhere = Discovered { executable: Some(exe("/usr/local/tools/codex")), ..d };
    assert_eq!(npm_package_dir(&elsewhere, "/Users/me/.npm-global/node_modules", &no_link), None);
}

#[test]
fn never_names_an_agent_that_updates_itself() {
    let cursor = Discovered { id: "cursor".into(), ..codex() };
    assert_eq!(npm_package_dir(&cursor, MAC_ROOT, &no_link), None);
    // Nor an agent discovery found no program for.
    assert_eq!(npm_package_dir(&Discovered { executable: None, ..codex() }, MAC_ROOT, &no_link), None);
}

// The first Mac run (2026-10-30): codex from npm, found as Homebrew's /opt/homebrew/bin/codex, was never updated.
#[test]
fn is_npms_on_a_mac_or_linux_too_where_its_shim_is_a_link_into_the_package_and_homebrews_own_copy_is_not() {
    let shim = exe("/opt/homebrew/bin/codex");
    let on_mac = Discovered { executable: Some(shim), ..codex() };
    let links = |path: &str| (path == "/opt/homebrew/bin/codex").then(|| "/opt/homebrew/lib/node_modules/@openai/codex/bin/codex.js".to_owned());
    assert_eq!(npm_package_dir(&on_mac, MAC_ROOT, &links), Some("/opt/homebrew/lib/node_modules/@openai/codex".into()));
    assert_eq!(npm_package_dir(&on_mac, MAC_ROOT, &|_| Some("/opt/homebrew/Cellar/codex/0.166.0/bin/codex".into())), None);
    assert_eq!(npm_package_dir(&on_mac, MAC_ROOT, &no_link), None);
    // A folder that merely starts with the package's name is not inside it.
    assert_eq!(npm_package_dir(&on_mac, MAC_ROOT, &|_| Some("/opt/homebrew/lib/node_modules/@openai/codex-other/bin/x".into())), None);
}

// ---- a scripted launch ----

#[test]
fn a_scripted_launch_never_changes_the_machine_drives_run_on_throwaway_profiles_and_the_agents_are_everyones() {
    let args = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let none = |_: &str| None;
    let on = |k: &str| (k == "LOCUST_UPDATE_AGENTS").then(|| "1".to_owned());
    assert!(may_update_agents(&args(&["locust"]), &none, true));
    assert!(!may_update_agents(&args(&["locust", "--remote-debugging-port=9443"]), &none, true));
    assert!(may_update_agents(&args(&["locust", "--remote-debugging-port=9443"]), &on, true));
    // A copy that is not the installed Locust leaves the machine alone (0.514); the drive about updating still may.
    assert!(!may_update_agents(&args(&["locust"]), &none, false));
    assert!(may_update_agents(&args(&["locust"]), &on, false));
}

// ---- what is kept on disk ----

#[test]
fn what_is_kept_on_disk_reads_back_what_was_written_and_nothing_else() {
    let saved = Saved {
        automatic: false,
        chosen: true,
        checked_at: Some(now()),
        latest: [("@openai/codex".to_owned(), released("0.156.1", 14))].into(),
        last: [("codex".to_owned(), Status::Updated { from: "0.153.0".into(), to: "0.156.1".into(), at: "2026-09-24T11:00:00.000Z".into() })].into(),
        held: None,
    };
    let text = saved.to_json().to_string();
    assert_eq!(saved_updates_from(&serde_json::from_str(&text).unwrap()), Some(saved.clone()));
    // The file is Electron's own shape: a file either app wrote reads in the other.
    assert!(text.contains("\"checkedAt\"") && text.contains("\"publishedAt\"") && text.contains("\"kind\":\"updated\""), "{text}");
    assert_eq!(saved_updates_from(&json!({ "automatic": "yes", "latest": { "x": 3 }, "last": { "codex": { "kind": "updating" } } })), Some(nothing_saved()));
    assert_eq!(saved_updates_from(&json!("nonsense")), None);
    assert_eq!(saved_updates_from(&Value::Null), None);
}

#[test]
fn updates_on_its_own_unless_the_person_turned_that_off_a_default_0303_saved_is_not_a_choice() {
    let automatic = |v: Value| saved_updates_from(&v).map(|s| s.automatic);
    assert!(nothing_saved().automatic);
    assert_eq!(automatic(json!({ "automatic": false, "checkedAt": now() })), Some(true));
    assert_eq!(automatic(json!({ "enabled": false, "checkedAt": now() })), Some(true));
    assert_eq!(automatic(json!({ "automatic": false, "chosen": true, "checkedAt": now() })), Some(false));
}

// ---- keeping current, end to end on fakes ----

#[derive(Default)]
struct Calls {
    latest: usize,
    installs: Vec<String>,
    updated: usize,
    in_use: usize,
}

struct Fake {
    saved: Mutex<Option<Saved>>,
    in_use: bool,
    latest: Release,
    install_ok: bool,
    discovered: Discovered,
    held: Option<BTreeMap<String, Vec<String>>>,
    calls: Mutex<Calls>,
    /// A run of the install waits here until the test lets it go.
    gate: Mutex<Option<mpsc::Receiver<()>>>,
}

impl Effects for Arc<Fake> {
    fn discover(&self) -> Vec<Discovered> {
        vec![self.discovered.clone()]
    }
    fn npm_root(&self) -> Option<String> {
        Some(MAC_ROOT.into())
    }
    fn real_path(&self, _: &str) -> Option<String> {
        None
    }
    fn latest(&self, _: &str) -> Option<Release> {
        self.calls.lock().unwrap().latest += 1;
        Some(self.latest.clone())
    }
    fn held_versions(&self) -> Option<BTreeMap<String, Vec<String>>> {
        self.held.clone()
    }
    fn in_use(&self, _: &str) -> Result<bool, String> {
        self.calls.lock().unwrap().in_use += 1;
        Ok(self.in_use)
    }
    fn install(&self, runtime: &str, version: &str) -> Result<(), String> {
        self.calls.lock().unwrap().installs.push(format!("{runtime}@{version}"));
        let gate = self.gate.lock().unwrap().take();
        if let Some(rx) = gate {
            let _ = rx.recv();
        }
        if self.install_ok { Ok(()) } else { Err("npm could not replace the files.".into()) }
    }
    fn load(&self) -> Option<Saved> {
        self.saved.lock().unwrap().clone()
    }
    fn save(&self, saved: &Saved) {
        *self.saved.lock().unwrap() = Some(saved.clone());
    }
    fn updated(&self, _: &str) {
        self.calls.lock().unwrap().updated += 1;
    }
    fn changed(&self) {}
    fn now(&self) -> i64 {
        now()
    }
}

struct Setup {
    saved: Option<Saved>,
    in_use: bool,
    latest: Option<Release>,
    install_ok: bool,
    held: Option<BTreeMap<String, Vec<String>>>,
}

fn setup() -> Setup {
    Setup { saved: None, in_use: false, latest: None, install_ok: true, held: None }
}

fn automatic() -> Saved {
    nothing_saved()
}

fn asks() -> Saved {
    Saved { automatic: false, chosen: true, ..nothing_saved() }
}

fn run(s: Setup) -> (Updates, Arc<Fake>) {
    let fake = Arc::new(Fake {
        saved: Mutex::new(s.saved),
        in_use: s.in_use,
        latest: s.latest.unwrap_or_else(|| released("0.156.1", 14)),
        install_ok: s.install_ok,
        discovered: codex(),
        held: s.held,
        calls: Mutex::new(Calls::default()),
        gate: Mutex::new(None),
    });
    (Updates::new(Box::new(fake.clone())), fake)
}

fn first(u: &Updates) -> Status {
    u.state().agents[0].status.clone()
}

fn installs(f: &Fake) -> Vec<String> {
    f.calls.lock().unwrap().installs.clone()
}

fn held_codex(version: &str) -> Option<BTreeMap<String, Vec<String>>> {
    Some([("@openai/codex".to_owned(), vec![version.to_owned()])].into())
}

#[test]
fn holds_back_a_version_locusts_release_check_could_not_read_and_installs_it_only_when_asked() {
    let (u, f) = run(Setup { held: held_codex("0.156.1"), ..setup() });
    u.tick();
    assert!(installs(&f).is_empty());
    assert_eq!(first(&u), Status::Waiting { version: "0.156.1".into(), why: Why::Held });
    // Update is the person's choice, and still installs it.
    u.update_now("codex");
    assert_eq!(installs(&f), vec!["codex@0.156.1"]);
}

#[test]
fn holds_nothing_back_that_the_check_passed_or_when_it_cannot_be_reached() {
    let (passed, pf) = run(Setup { held: held_codex("0.155.0"), ..setup() });
    passed.tick();
    assert_eq!(installs(&pf), vec!["codex@0.156.1"]);
    let (unreachable, uf) = run(setup());
    unreachable.tick();
    assert_eq!(installs(&uf), vec!["codex@0.156.1"]);
}

#[test]
fn what_the_check_held_is_remembered_when_a_later_look_cannot_reach_it() {
    // A look that cannot reach the verdicts keeps the last ones (Electron spreads `held` only when it has one).
    let saved = Saved { held: held_codex("0.156.1"), ..nothing_saved() };
    let (u, f) = run(Setup { saved: Some(saved), ..setup() });
    u.tick();
    assert!(installs(&f).is_empty());
    assert_eq!(first(&u), Status::Waiting { version: "0.156.1".into(), why: Why::Held });
}

#[test]
fn turned_off_only_looks_0156_is_out_and_nothing_is_downloaded_until_the_person_asks() {
    let (u, f) = run(Setup { saved: Some(asks()), ..setup() });
    u.tick();
    assert_eq!(f.calls.lock().unwrap().latest, 1);
    assert_eq!(f.calls.lock().unwrap().in_use, 0);
    assert!(installs(&f).is_empty());
    assert_eq!(first(&u), Status::Waiting { version: "0.156.1".into(), why: Why::Ask });
}

#[test]
fn updates_when_the_person_presses_update_and_asks_for_the_models_again() {
    let (u, f) = run(Setup { saved: Some(asks()), ..setup() });
    u.tick();
    u.update_now("codex");
    assert_eq!(installs(&f), vec!["codex@0.156.1"]);
    assert_eq!(f.calls.lock().unwrap().updated, 1);
    assert!(matches!(first(&u), Status::Updated { from, to, .. } if from == "0.153.0" && to == "0.156.1"));
    // The window is told to ask discovery again, once.
    assert!(u.take_updated());
    assert!(!u.take_updated());
}

#[test]
fn does_not_update_on_the_press_while_something_is_using_it_and_says_why() {
    let (u, f) = run(Setup { saved: Some(asks()), in_use: true, ..setup() });
    u.tick();
    u.update_now("codex");
    assert!(installs(&f).is_empty());
    assert_eq!(first(&u), Status::Waiting { version: "0.156.1".into(), why: Why::InUse });
}

#[test]
fn a_press_with_nothing_newer_or_nothing_looked_at_does_nothing() {
    // No look yet: no release is known, so there is nothing to install.
    let (u, f) = run(setup());
    u.update_now("codex");
    assert!(installs(&f).is_empty());
    // Current: the same.
    let (u, f) = run(Setup { saved: Some(Saved { checked_at: Some(now()), latest: [("@openai/codex".to_owned(), released("0.153.0", 40))].into(), ..nothing_saved() }), ..setup() });
    u.update_now("codex");
    assert!(installs(&f).is_empty());
    // An agent that is not kept current at all.
    let (u, f) = run(Setup { saved: Some(asks()), ..setup() });
    u.tick();
    u.update_now("claude");
    assert!(installs(&f).is_empty());
}

#[test]
fn by_default_looks_finds_0156_updates_codex_and_asks_for_the_models_again() {
    let (u, f) = run(setup());
    u.tick();
    assert_eq!(installs(&f), vec!["codex@0.156.1"]);
    assert_eq!(f.calls.lock().unwrap().updated, 1);
    assert!(matches!(first(&u), Status::Updated { ref from, ref to, .. } if from == "0.153.0" && to == "0.156.1"));
    assert!(matches!(f.saved.lock().unwrap().as_ref().unwrap().last.get("codex"), Some(Status::Updated { .. })));
}

#[test]
fn turned_on_leaves_it_be_while_something_is_using_it_and_says_so() {
    let (u, f) = run(Setup { saved: Some(automatic()), in_use: true, ..setup() });
    u.tick();
    assert!(installs(&f).is_empty());
    assert_eq!(first(&u), Status::Waiting { version: "0.156.1".into(), why: Why::InUse });
}

#[test]
fn does_not_go_to_the_network_when_the_last_look_is_recent_and_reads_no_processes_for_an_agent_that_is_current() {
    let recent = Saved { checked_at: Some(now() - (CHECK_EVERY_MS - HOUR)), latest: [("@openai/codex".to_owned(), released("0.153.0", 40))].into(), ..nothing_saved() };
    let (u, f) = run(Setup { saved: Some(recent), ..setup() });
    u.tick();
    assert_eq!(f.calls.lock().unwrap().latest, 0);
    assert_eq!(f.calls.lock().unwrap().in_use, 0);
    assert!(installs(&f).is_empty());
    // And a look that is due goes at six hours exactly.
    let due = Saved { checked_at: Some(now() - CHECK_EVERY_MS), ..nothing_saved() };
    let (u, f) = run(Setup { saved: Some(due), in_use: true, ..setup() });
    u.tick();
    assert_eq!(f.calls.lock().unwrap().latest, 1);
}

#[test]
fn says_when_an_update_failed_and_keeps_saying_it() {
    let (u, f) = run(Setup { saved: Some(automatic()), install_ok: false, ..setup() });
    u.tick();
    assert!(matches!(first(&u), Status::Failed { ref version, ref what, .. } if version == "0.156.1" && what == "npm could not replace the files."));
    assert!(matches!(f.saved.lock().unwrap().as_ref().unwrap().last.get("codex"), Some(Status::Failed { .. })));
    assert!(!u.take_updated(), "a failed update asks nothing again");
    assert_eq!(f.calls.lock().unwrap().updated, 0);
}

#[test]
fn a_past_update_is_said_again_until_something_newer_is_out() {
    // 0.156.1 landed last time; discovery now reports it, and npm's newest is the same: the row keeps saying so.
    let last = Status::Updated { from: "0.153.0".into(), to: "0.156.1".into(), at: "2026-09-24T11:00:00.000Z".into() };
    let saved = Saved { checked_at: Some(now() - HOUR), latest: [("@openai/codex".to_owned(), released("0.156.1", 40))].into(), last: [("codex".to_owned(), last.clone())].into(), ..nothing_saved() };
    let fake = Arc::new(Fake {
        saved: Mutex::new(Some(saved)),
        in_use: false,
        latest: released("0.156.1", 40),
        install_ok: true,
        discovered: Discovered { version: Some("0.156.1".into()), ..codex() },
        held: None,
        calls: Mutex::new(Calls::default()),
        gate: Mutex::new(None),
    });
    let u = Updates::new(Box::new(fake));
    u.tick();
    assert_eq!(first(&u), last);
}

#[test]
fn turning_the_switch_is_a_choice_kept_and_turned_on_it_updates_what_was_waiting_now() {
    let (u, f) = run(Setup { saved: Some(asks()), ..setup() });
    u.tick();
    assert!(installs(&f).is_empty());
    let state = u.set_automatic(true);
    assert!(state.automatic);
    assert_eq!(installs(&f), vec!["codex@0.156.1"]);
    let kept = f.saved.lock().unwrap().clone().unwrap();
    assert!(kept.automatic && kept.chosen);
    // Turned off, it only keeps the choice.
    assert!(!u.set_automatic(false).automatic);
    assert_eq!(installs(&f).len(), 1);
}

// L12 (the code review): a tick that came while a manual Update was installing ran the same install again beside it --
// npm fighting itself -- recorded the refused second one as a failed update, and cleared "updating" while the first was
// still running.
#[test]
fn a_tick_during_a_manual_update_waits_for_it_and_installs_once() {
    let saved = Saved { checked_at: Some(now()), latest: [("@openai/codex".to_owned(), released("0.156.1", 14))].into(), ..nothing_saved() };
    let (u, f) = run(Setup { saved: Some(saved), ..setup() });
    let (release, gate) = mpsc::channel();
    *f.gate.lock().unwrap() = Some(gate);
    let pause = || std::thread::sleep(std::time::Duration::from_millis(60));
    std::thread::scope(|s| {
        let pressed = s.spawn(|| u.update_now("codex"));
        for _ in 0..200 {
            if u.updating().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(u.updating().as_deref(), Some("codex"));
        let ticked = s.spawn(|| u.tick());
        pause();
        // Still the one install, and still updating.
        assert_eq!(installs(&f), vec!["codex@0.156.1"]);
        assert_eq!(u.updating().as_deref(), Some("codex"));
        release.send(()).unwrap();
        pressed.join().unwrap();
        ticked.join().unwrap();
    });
    assert_eq!(installs(&f), vec!["codex@0.156.1"]);
    assert_eq!(u.updating(), None);
}

#[test]
fn an_agent_updating_shows_as_updating_to_a_window_that_peeks_meanwhile() {
    let saved = Saved { checked_at: Some(now()), latest: [("@openai/codex".to_owned(), released("0.156.1", 14))].into(), ..nothing_saved() };
    let (u, f) = run(Setup { saved: Some(saved), ..setup() });
    let (release, gate) = mpsc::channel();
    *f.gate.lock().unwrap() = Some(gate);
    std::thread::scope(|s| {
        let pressed = s.spawn(|| u.update_now("codex"));
        for _ in 0..200 {
            if u.updating().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(u.peek().agents[0].status, Status::Updating { version: "0.156.1".into() });
        release.send(()).unwrap();
        pressed.join().unwrap();
    });
}

// ---- the release check's verdicts (0.501) ----

#[test]
fn holds_only_the_versions_it_ran_and_could_not_read() {
    let v = json!({ "packages": { "@openai/codex": { "0.159.2": { "ok": true }, "0.160.0": { "ok": false } }, "@github/copilot": { "1.0.0": { "ok": true } } } });
    assert_eq!(held_from(&v), BTreeMap::from([("@openai/codex".to_owned(), vec!["0.160.0".to_owned()])]));
}

#[test]
fn reads_anything_else_as_nothing_held() {
    assert!(held_from(&Value::Null).is_empty());
    assert!(held_from(&json!({ "packages": "no" })).is_empty());
    assert!(held_from(&json!({ "packages": { "@openai/codex": { "0.160.0": { "ok": "false" } } } })).is_empty());
}

#[test]
fn keeps_what_it_held_across_a_restart() {
    let held = saved_updates_from(&json!({ "latest": {}, "last": {}, "held": { "@openai/codex": ["0.160.0", 7] } })).unwrap().held;
    assert_eq!(held, Some(BTreeMap::from([("@openai/codex".to_owned(), vec!["0.160.0".to_owned()])])));
    assert_eq!(saved_updates_from(&json!({ "latest": {}, "last": {} })).unwrap().held, None);
}

// ---- npm's answer ----

#[test]
fn npms_answer_is_the_latest_tag_and_the_time_it_went_out() {
    let text = r#"{ "dist-tags.latest": "0.156.1", "time": { "0.153.0": "2026-09-01T00:00:00.000Z", "0.156.1": "2026-09-23T20:00:00.000Z" } }"#;
    assert_eq!(release_from(text), Some(Release { version: "0.156.1".into(), published_at: Some("2026-09-23T20:00:00.000Z".into()) }));
    assert_eq!(release_from(r#"{ "dist-tags.latest": "0.156.1" }"#), Some(Release { version: "0.156.1".into(), published_at: None }));
    assert_eq!(release_from(r#"{ "dist-tags.latest": "" }"#), None);
    assert_eq!(release_from("[]"), None);
    assert_eq!(release_from("not json"), None);
}

// ---- the in-use check ----

// Electron's the-in-use-check-reads-any-path.test.ts (L13) is Windows-only: PowerShell's output was read in the console's
// OEM code page and a folder with a non-ASCII letter never matched, so an agent running from `C:\Users\Jos\u{e9}\...` read
// as not in use. A Mac reads `ps` as UTF-8, so what carries over is the same promise: a real process, with such a
// folder on its command line, is found in the real process table's bytes.
#[test]
fn the_in_use_check_finds_a_process_whose_path_has_a_non_ascii_letter() {
    let folder = format!("{}/locust-in-use-caf\u{e9}-check", std::env::temp_dir().display());
    let mut child = std::process::Command::new("/bin/sh").args(["-c", "sleep 15; true", &format!("{folder}/agent.js")]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    // Read here, not by the real effect, which refuses to run in a test.
    let table = std::process::Command::new("ps").args(["-eo", "command"]).output().unwrap().stdout;
    let found = processes_using_bytes(&folder, &table);
    let found_loud = processes_using_bytes(&folder.to_uppercase(), &table);
    let _ = child.kill();
    let _ = child.wait();
    assert!(found, "the process with {folder} on its command line is in use");
    assert!(found_loud, "case does not matter");
    assert!(!processes_using_bytes(&format!("{folder}-nobody"), &table));
}

#[test]
fn the_real_effects_refuse_to_run_in_a_test() {
    let dir = std::env::temp_dir().join(format!("locust-updates-refuse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let m = Machine::new(&dir);
    assert!(m.discover().is_empty());
    assert_eq!(m.npm_root(), None);
    assert_eq!(m.latest("@openai/codex"), None);
    assert_eq!(m.held_versions(), None);
    assert!(m.in_use("/x").is_err());
    assert!(m.install("codex", "0.156.1").is_err());
    // Its one file is the only thing it touches, and reads back.
    assert_eq!(m.load(), None);
    let saved = Saved { automatic: false, chosen: true, ..nothing_saved() };
    m.save(&saved);
    assert_eq!(m.load(), Some(saved));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the row's words (agent-updates-say-what-happened.test.ts) ----

fn view(status: Status) -> View {
    View { runtime: "codex".into(), installed: "0.153.0".into(), latest: Some("0.156.1".into()), status }
}

fn waiting_view(why: Why) -> View {
    view(Status::Waiting { version: "0.156.1".into(), why })
}

fn at_noon() -> chrono::DateTime<chrono::Local> {
    chrono::Local.with_ymd_and_hms(2026, 9, 24, 12, 0, 0).unwrap()
}

#[test]
fn a_runtimes_row_says_nothing_when_the_agent_is_current() {
    assert_eq!(update_line(Some(&view(Status::Current)), at_noon()), None);
    assert_eq!(update_line(None, at_noon()), None);
}

#[test]
fn a_runtimes_row_says_a_newer_version_is_out_and_what_it_is_waiting_for() {
    let line = |why| update_line(Some(&waiting_view(why)), at_noon());
    assert_eq!(line(Why::Ask).as_deref(), Some("0.156.1 is out."));
    assert_eq!(line(Why::InUse).as_deref(), Some("0.156.1 is out. It can update once nothing is using it."));
    assert_eq!(line(Why::TooNew).as_deref(), Some("0.156.1 is out. It updates once it has been out 12 hours."));
}

#[test]
fn a_runtimes_row_offers_update_when_the_download_is_the_persons_to_start_not_for_a_release_too_new_to_update_on_its_own() {
    assert!(offers_update(Some(&waiting_view(Why::Ask))));
    assert!(offers_update(Some(&waiting_view(Why::InUse))));
    assert!(!offers_update(Some(&waiting_view(Why::TooNew))));
    assert!(!offers_update(Some(&view(Status::Updating { version: "0.156.1".into() }))));
    assert!(!offers_update(Some(&view(Status::Current))));
    assert!(!offers_update(None));
}

#[test]
fn a_runtimes_row_says_an_update_is_under_way_landed_or_failed_and_when() {
    let line = |s| update_line(Some(&view(s)), at_noon()).unwrap();
    assert_eq!(line(Status::Updating { version: "0.156.1".into() }), "Updating to 0.156.1\u{2026}");
    let landed = line(Status::Updated { from: "0.153.0".into(), to: "0.156.1".into(), at: "2026-09-24T11:05:00".into() });
    assert!(landed.starts_with("Updated from 0.153.0 to 0.156.1 at 11:05"), "{landed}");
    assert_eq!(line(Status::Updated { from: "0.153.0".into(), to: "0.156.1".into(), at: "2026-09-22T11:05:00".into() }), "Updated from 0.153.0 to 0.156.1 on Sep 22.");
    let failed = line(Status::Failed { version: "0.156.1".into(), what: "npm could not replace the files.".into(), at: "2026-09-24T11:05:00".into() });
    assert!(failed.starts_with("Could not update to 0.156.1 at 11:05") && failed.ends_with(": npm could not replace the files."), "{failed}");
}

#[test]
fn the_switch_says_what_it_does_either_way_and_that_the_download_is_big() {
    assert!(keep_current_note(true).starts_with("Updating on their own."));
    assert!(keep_current_note(false).starts_with("Updating when you press Update."));
    assert!(keep_current_note(false).contains("160 MB"));
}

#[test]
fn a_version_held_back_by_the_release_check_says_so_and_still_offers_update() {
    let v = View { runtime: "codex".into(), installed: "0.159.0".into(), latest: Some("0.160.0".into()), status: Status::Waiting { version: "0.160.0".into(), why: Why::Held } };
    assert_eq!(update_line(Some(&v), at_noon()).as_deref(), Some("0.160.0 is out, and held back: Locust's check of new versions could not read it yet. Update to try it anyway."));
    assert!(offers_update(Some(&v)));
}
