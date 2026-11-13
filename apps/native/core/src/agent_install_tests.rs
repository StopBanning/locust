//! Electron's main/runtime-installer.test.ts: what a person sees when Install does not work, and that it runs the line
//! the row showed.
use super::*;
use std::sync::Mutex;
use std::sync::mpsc;

type Answer = (Option<i32>, String);

/// Answers each run in turn (the last one again once they run out), printing `lines` first; keeps what it was asked.
struct Fake {
    answers: Mutex<Vec<Answer>>,
    lines: Vec<String>,
    asked: Mutex<Vec<(String, Vec<String>)>>,
    /// When set, a run waits here until the test lets it go.
    hold: Mutex<Option<mpsc::Receiver<()>>>,
}

impl Fake {
    fn new(answers: Vec<Answer>, lines: &[&str]) -> Arc<Fake> {
        Arc::new(Fake { answers: Mutex::new(answers), lines: lines.iter().map(|s| s.to_string()).collect(), asked: Mutex::new(vec![]), hold: Mutex::new(None) })
    }
    fn asked(&self) -> Vec<(String, Vec<String>)> {
        self.asked.lock().unwrap().clone()
    }
}

impl Run for Fake {
    fn run(&self, command: &str, args: &[String], on_line: &mut dyn FnMut(&str)) -> Answer {
        self.asked.lock().unwrap().push((command.to_owned(), args.to_vec()));
        if let Some(rx) = self.hold.lock().unwrap().take() {
            let _ = rx.recv();
        }
        for line in &self.lines {
            on_line(line);
        }
        let mut answers = self.answers.lock().unwrap();
        if answers.len() > 1 { answers.remove(0) } else { answers[0].clone() }
    }
}

fn installer(run: Arc<Fake>, found: bool, platform: &'static str, prefix: Option<&str>) -> Installer {
    Installer::new(run, Box::new(move |_| found), prefix.map(str::to_owned), platform)
}

fn ok(output: &str) -> Answer {
    (Some(0), output.into())
}

fn quiet(i: &Installer, id: &str) -> Outcome {
    i.install(id, "OpenCode", None, &mut |_| {})
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| s.to_string()).collect()
}

// ---- a global folder that is the system's (0.515) ----

const EACCES: &str = "npm ERR! code EACCES\nnpm ERR! Error: EACCES: permission denied, mkdir '/usr/local/lib/node_modules/opencode-ai'";

#[test]
fn off_windows_tries_once_more_into_the_persons_own_folder_and_says_so() {
    let run = Fake::new(vec![(Some(243), EACCES.into()), ok("added 1 package")], &[]);
    let i = installer(run.clone(), true, "macos", Some("/Users/ian/.npm-global"));
    let mut lines = vec![];
    assert_eq!(i.install("opencode", "OpenCode", None, &mut |l| lines.push(l.to_owned())), Ok(()));
    assert_eq!(run.asked()[1], ("npm".into(), strings(&["install", "-g", "opencode-ai", "--prefix", "/Users/ian/.npm-global"])));
    assert!(lines.contains(&"npm's global folder belongs to the system; installing into /Users/ian/.npm-global instead.".to_owned()));
}

#[test]
fn not_on_windows_where_a_permission_failure_is_a_lock_or_a_policy_and_not_for_any_other_failure() {
    let on_windows = Fake::new(vec![(Some(1), EACCES.into())], &[]);
    let _ = quiet(&installer(on_windows.clone(), true, "windows", Some("C:/x")), "opencode");
    assert_eq!(on_windows.asked().len(), 1);
    let other = Fake::new(vec![(Some(1), "npm ERR! 404 Not Found".into())], &[]);
    let _ = quiet(&installer(other.clone(), true, "macos", Some("/Users/ian/.npm-global")), "opencode");
    assert_eq!(other.asked().len(), 1);
}

#[test]
fn and_when_the_second_try_fails_too_it_says_what_the_first_said() {
    let run = Fake::new(vec![(Some(243), EACCES.into())], &[]);
    let said = quiet(&installer(run.clone(), true, "macos", Some("/Users/ian/.npm-global")), "opencode").unwrap_err();
    assert_eq!(run.asked().len(), 2);
    assert_eq!(said.what, "npm could not write to its global folder.");
}

// ---- running the install the screen showed ----

#[test]
fn runs_exactly_the_command_the_person_read_not_a_reconstruction_of_it() {
    let run = Fake::new(vec![ok("added 213 packages")], &[]);
    let _ = quiet(&installer(run.clone(), true, "macos", None), "opencode");
    assert_eq!(run.asked(), vec![("npm".to_owned(), strings(&["install", "-g", "opencode-ai"]))]);
    let shown = crate::agents::install_command("opencode").unwrap();
    let (command, args) = &run.asked()[0];
    assert_eq!(format!("{command} {}", args.join(" ")), shown, "the line on the row, word for word");
}

#[test]
fn an_update_installs_that_release_and_refuses_what_is_not_a_version() {
    let run = Fake::new(vec![ok("")], &[]);
    let i = installer(run.clone(), true, "macos", None);
    assert_eq!(i.install("codex", "Codex CLI", Some("0.168.0"), &mut |_| {}), Ok(()));
    assert_eq!(run.asked()[0].1, strings(&["install", "-g", "@openai/codex@0.168.0"]));
    let refused = i.install("codex", "Codex CLI", Some("latest; rm -rf ~"), &mut |_| {}).unwrap_err();
    assert_eq!((refused.what.as_str(), refused.next.as_str()), ("\"latest; rm -rf ~\" is not a version Locust installs.", "Nothing was changed."));
    assert_eq!(run.asked().len(), 1, "nothing ran");
}

#[test]
fn hands_every_line_npm_prints_to_the_screen_as_it_arrives() {
    let run = Fake::new(vec![ok("")], &["npm warn deprecated x", "added 213 packages in 12s"]);
    let mut seen = vec![];
    let _ = installer(run, true, "macos", None).install("opencode", "OpenCode", None, &mut |l| seen.push(l.to_owned()));
    assert_eq!(seen, vec!["npm warn deprecated x", "added 213 packages in 12s"]);
}

#[test]
fn refuses_a_second_install_while_one_is_running() {
    let run = Fake::new(vec![ok("")], &[]);
    let (release, wait) = mpsc::channel();
    *run.hold.lock().unwrap() = Some(wait);
    let i = Arc::new(installer(run.clone(), true, "macos", None));
    let first = {
        let i = i.clone();
        std::thread::spawn(move || quiet(&i, "opencode"))
    };
    while run.asked().is_empty() {
        std::thread::yield_now();
    }
    assert!(i.busy());
    let second = i.install("claude", "Claude Code", None, &mut |_| {}).unwrap_err();
    assert!(second.what.contains("already running"));
    release.send(()).unwrap();
    assert_eq!(first.join().unwrap(), Ok(()));
    assert!(!i.busy());
}

#[test]
fn will_not_offer_to_install_something_that_does_not_come_from_a_package_manager() {
    let run = Fake::new(vec![ok("")], &[]);
    assert!(quiet(&installer(run.clone(), true, "macos", None), "cursor").is_err());
    assert!(run.asked().is_empty());
}

// ---- a clean exit is not the same as something to run ----

#[test]
fn says_so_when_npm_succeeded_and_the_command_still_is_not_there() {
    let said = quiet(&installer(Fake::new(vec![ok("added 1 package")], &[]), false, "macos", None), "opencode").unwrap_err();
    assert!(said.what.contains("still cannot find"));
    assert!(said.restart);
}

#[test]
fn reports_success_when_the_command_really_is_there_afterwards() {
    assert_eq!(quiet(&installer(Fake::new(vec![ok("added 1 package")], &[]), true, "macos", None), "opencode"), Ok(()));
}

// ---- the six ways an install fails, each said in a person's words ----

fn classified(output: &str) -> Failure {
    classify("opencode-ai", "OpenCode", output, 9, "macos")
}

#[test]
fn no_network() {
    assert!(classified("npm error code ENOTFOUND\nnpm error getaddrinfo ENOTFOUND registry.npmjs.org").what.contains("could not reach the registry"));
}

#[test]
fn a_proxy_refusing_which_also_looks_like_a_network_failure_and_is_not() {
    let seen = classified("npm error tunneling socket could not be established, statusCode=407\nECONNRESET");
    assert!(seen.what.contains("refused the connection"));
    assert!(seen.next.contains("proxy"));
}

#[test]
fn permissions() {
    assert!(classified("npm error code EACCES\nnpm error syscall mkdir").what.contains("global folder"));
}

#[test]
fn the_package_itself() {
    assert!(classified("npm error code E404\nnpm error 404 Not Found").what.contains("opencode-ai"));
}

#[test]
fn anything_else_with_the_elapsed_time_rather_than_a_guess_at_the_cause() {
    let seen = classified("npm error something nobody has seen before");
    assert!(seen.what.contains("9s"));
    assert!(seen.next.contains("output"));
}

#[test]
fn and_the_one_that_is_not_npms_fault_at_all() {
    let seen = installed_but_not_found("OpenCode");
    assert!(seen.what.contains("OpenCode installed"));
    assert!(seen.restart);
}

#[test]
fn a_file_held_open_on_windows_is_a_lock_not_a_permission() {
    let seen = classify("opencode-ai", "OpenCode", "npm error EBUSY: resource busy or locked, rename", 3, "windows");
    assert!(seen.what.contains("holding OpenCode's files"));
    assert!(classify("opencode-ai", "OpenCode", "npm error EBUSY: resource busy or locked, rename", 3, "macos").what.contains("stopped with an error"));
}

#[test]
fn every_failure_says_both_what_happened_and_what_to_do() {
    for output in ["ENOTFOUND", "407 proxy", "EACCES", "E404", "who knows"] {
        let seen = classified(output);
        assert!(seen.what.len() > 10 && seen.next.len() > 10, "{output}: {seen:?}");
    }
}

// ---- a permission failure offers the remedy, not a re-run of itself ----

fn eacces(platform: &str) -> Failure {
    classify("opencode-ai", "OpenCode", "npm ERR! code EACCES\nnpm ERR! syscall mkdir", 3, platform)
}

#[test]
fn sets_a_prefix_the_user_owns_then_installs() {
    let command = eacces("linux").command.unwrap();
    assert!(command.contains("npm config set prefix") && command.contains("~/.npm-global") && command.contains("npm install -g opencode-ai"));
}

#[test]
fn uses_a_windows_shaped_path_on_windows() {
    assert!(eacces("windows").command.unwrap().contains("%LOCALAPPDATA%"));
}

#[test]
fn says_to_put_it_on_path_because_installing_is_only_half_of_it() {
    assert!(eacces("linux").next.contains("PATH"));
}

#[test]
fn no_longer_tells_them_to_re_run_the_thing_that_just_failed() {
    assert!(!eacces("linux").next.to_lowercase().contains("terminal with permission"));
}

#[test]
fn leaves_every_other_failure_showing_the_command_locust_ran() {
    let offline = classify("opencode-ai", "OpenCode", "npm ERR! network request to https://registry.npmjs.org failed, reason: getaddrinfo ENOTFOUND", 2, "linux");
    assert_eq!(offline.command, None);
}

// ---- what may be handed to an install ----

#[test]
fn accepts_the_shapes_a_real_npm_install_needs() {
    for token in ["npm", "install", "-g", "opencode-ai", "@anthropic-ai/claude-code", "@openai/codex", "some.pkg_v2", "npm:alias@1.2.3"] {
        assert!(safe_to_spawn(token), "{token}");
    }
}

#[test]
fn refuses_every_character_a_shell_would_read_as_syntax() {
    for token in ["pkg; rm -rf /", "pkg && curl evil.sh", "pkg | tee out", "pkg $(whoami)", "pkg `whoami`", "pkg > file", "pkg\nnext-line", "pkg with space", "pkg'quoted'", "pkg\"quoted\"", ""] {
        assert!(!safe_to_spawn(token), "{token:?}");
    }
    let (code, said) = Npm.run("npm", &strings(&["install", "-g", "pkg; rm -rf /"]), &mut |_| {});
    assert_eq!(code, None);
    assert!(said.contains("characters a shell reads as syntax"));
}

#[test]
fn passes_every_install_line_the_app_can_actually_build_today() {
    for id in ["opencode", "claude", "codex", "copilot", "kilo"] {
        let line = crate::agents::install_command(id).unwrap_or_else(|| panic!("{id}"));
        for token in line.split(' ') {
            assert!(safe_to_spawn(token), "{id}: {token}");
        }
    }
}

#[test]
fn no_install_runs_in_a_test() {
    assert_eq!(Npm.run("npm", &strings(&["install", "-g", "opencode-ai"]), &mut |_| {}), (None, "no install runs in a test".to_owned()));
}
