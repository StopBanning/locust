//! Electron's tests: your-github-is-connected-through-gh, the-github-cli-installs-from-the-card,
//! the-github-cli-updates-from-the-card. The shapes are gh 2.96.0's own (measured by Electron, 10 Oct 2026); the
//! code here is made up.
use super::*;
use std::collections::VecDeque;

const SIGNED_IN: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"octo-cat","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"}]}}"#;
const PRINTED: &str = "\n! First copy your one-time code: AB12-CD34\nOpen this URL to continue in your web browser: https://github.com/login/device\n";

fn signed_in() -> GithubAccount {
    GithubAccount::SignedIn { login: "octo-cat".into(), scopes: vec!["gist".into(), "read:org".into(), "repo".into(), "workflow".into()] }
}

// ---- who the GitHub CLI is signed in as ----

#[test]
fn reads_the_active_account_its_scopes_and_an_account_github_no_longer_takes() {
    assert_eq!(account_of(SIGNED_IN), signed_in());
    assert_eq!(account_of(r#"{"hosts":{}}"#), GithubAccount::SignedOut);
    assert_eq!(account_of(r#"{"hosts":{"github.com":[{"state":"error","active":true,"login":"octo-cat"}]}}"#), GithubAccount::Expired { login: Some("octo-cat".into()) });
    assert_eq!(
        account_of(r#"{"hosts":{"github.com":[{"state":"success","active":false,"login":"old-one"},{"state":"success","active":true,"login":"octo-cat","scopes":"repo"}]}}"#),
        GithubAccount::SignedIn { login: "octo-cat".into(), scopes: vec!["repo".into()] }
    );
    assert!(matches!(account_of("not json"), GithubAccount::Unknown { .. }));
}

#[test]
fn never_names_a_login_github_could_not_have_issued() {
    assert_eq!(account_of(r#"{"hosts":{"github.com":[{"state":"success","active":true,"login":"<img src=x>"}]}}"#), GithubAccount::Expired { login: None });
    assert!(!is_login("-cat") && !is_login("octo--cat") && !is_login("cat-") && !is_login(&"a".repeat(40)) && is_login("octo-cat"));
}

#[test]
fn reads_an_older_gh_by_its_exit_and_first_account_line() {
    assert_eq!(account_of_text("github.com\n  ✓ Logged in to github.com account octo-cat (keyring)", true), GithubAccount::SignedIn { login: "octo-cat".into(), scopes: vec![] });
    assert_eq!(account_of_text("You are not logged into any GitHub hosts.", false), GithubAccount::SignedOut);
}

#[test]
fn takes_the_code_only_with_githubs_own_device_page() {
    assert_eq!(sign_in_code_of(PRINTED), Some(SignInCode { code: "AB12-CD34".into(), url: GITHUB_DEVICE_PAGE.into() }));
    assert_eq!(
        sign_in_code_of("! One-time code (AB12-CD34) copied to clipboard\nOpen this URL to continue in your web browser: https://github.com/login/device"),
        Some(SignInCode { code: "AB12-CD34".into(), url: GITHUB_DEVICE_PAGE.into() })
    );
    assert_eq!(sign_in_code_of("! One-time code (AB12-CD34) copied to clipboard\nOpen this URL to continue in your web browser: https://evil.example/login/device"), None);
    assert_eq!(sign_in_code_of("! First copy your one-time code: AB12-CD34\nOpen https://evil.example/login/device"), None);
    assert_eq!(sign_in_code_of("! First copy your one-time code: AB12"), None);
}

#[test]
fn says_each_state_in_a_line() {
    assert_eq!(account_line(&GithubAccount::SignedIn { login: "octo-cat".into(), scopes: vec![] }), "Signed in to GitHub. Teammates push and open pull requests as you.");
    assert!(account_line(&GithubAccount::NoCli).contains("not on this computer"));
    assert_eq!(account_line(&GithubAccount::Expired { login: Some("octo-cat".into()) }), "The sign-in for octo-cat no longer works. Sign in again.");
}

// ---- the fakes ----

#[derive(Default)]
struct Fake {
    answers: Mutex<VecDeque<GhAnswer>>,
    asked: Mutex<Vec<Vec<String>>>,
    login: Mutex<Option<FakeLoginHandle>>,
    started: Mutex<Vec<String>>,
    installs: Mutex<Vec<Vec<String>>>,
    installer: Mutex<Option<GhAnswer>>,
    platform: &'static str,
    brew: Option<&'static str>,
}

#[derive(Clone)]
struct FakeLoginHandle {
    events: Arc<Mutex<VecDeque<LoginEvent>>>,
    stopped: Arc<AtomicBool>,
}

impl FakeLoginHandle {
    fn new() -> Self {
        FakeLoginHandle { events: Arc::new(Mutex::new(VecDeque::new())), stopped: Arc::new(AtomicBool::new(false)) }
    }
    fn print(&self, text: &str) {
        self.events.lock().unwrap().push_back(LoginEvent::Text(text.into()));
    }
    fn end(&self, code: i64) {
        self.events.lock().unwrap().push_back(LoginEvent::Ended(Exit::Code(code)));
    }
}

struct FakeLogin(FakeLoginHandle);

impl Login for FakeLogin {
    fn next(&mut self, wait: Duration) -> LoginEvent {
        if let Some(event) = self.0.events.lock().unwrap().pop_front() {
            return event;
        }
        if self.0.stopped.load(Ordering::SeqCst) {
            return LoginEvent::Ended(Exit::Code(1));
        }
        std::thread::sleep(wait.min(Duration::from_millis(5)));
        LoginEvent::Nothing
    }
    fn stop(&mut self) {
        self.0.stopped.store(true, Ordering::SeqCst);
    }
}

impl Gh for Fake {
    fn run_gh(&self, args: &[&str]) -> GhAnswer {
        self.asked.lock().unwrap().push(args.iter().map(|a| a.to_string()).collect());
        self.answers.lock().unwrap().pop_front().unwrap_or_else(|| GhAnswer::code(1, ""))
    }
    fn start_login(&self, args: &[&str]) -> Box<dyn Login> {
        *self.started.lock().unwrap() = args.iter().map(|a| a.to_string()).collect();
        Box::new(FakeLogin(self.login.lock().unwrap().clone().expect("a login")))
    }
    fn run_installer(&self, command: &str, args: &[&str]) -> GhAnswer {
        self.installs.lock().unwrap().push(std::iter::once(command).chain(args.iter().copied()).map(str::to_owned).collect());
        self.installer.lock().unwrap().clone().unwrap_or_else(|| GhAnswer::code(0, ""))
    }
    fn exists(&self, path: &str) -> bool {
        self.brew == Some(path)
    }
    fn platform(&self) -> &'static str {
        self.platform
    }
}

fn fake(answers: Vec<GhAnswer>) -> Arc<Fake> {
    Arc::new(Fake { answers: Mutex::new(answers.into()), platform: "darwin", ..Fake::default() })
}

// ---- signing in without a terminal ----

#[test]
fn asks_gh_by_json_and_says_when_gh_is_not_installed() {
    let gh = fake(vec![GhAnswer::code(0, SIGNED_IN)]);
    assert!(matches!(Account::new(gh.clone()).read(), GithubAccount::SignedIn { login, .. } if login == "octo-cat"));
    assert_eq!(gh.asked.lock().unwrap()[0], ["auth", "status", "--json", "hosts", "--hostname", "github.com"]);
    assert_eq!(Account::new(fake(vec![GhAnswer::missing()])).read(), GithubAccount::NoCli);
}

#[test]
fn shows_the_code_gh_prints_even_split_across_two_writes_and_answers_with_the_account_once_github_says_yes() {
    let gh = fake(vec![GhAnswer::code(0, SIGNED_IN)]);
    let handle = FakeLoginHandle::new();
    *gh.login.lock().unwrap() = Some(handle.clone());
    handle.print("\n! First copy your one-time code: AB1");
    handle.print("2-CD34\nOpen this URL to continue in your web browser: https://github.com/login/device\n");
    handle.print(PRINTED);
    handle.end(0);
    let mut codes = Vec::new();
    let result = Account::new(gh.clone()).sign_in(|code| codes.push(code));
    assert_eq!(codes, vec![SignInCode { code: "AB12-CD34".into(), url: GITHUB_DEVICE_PAGE.into() }]);
    assert_eq!(*gh.started.lock().unwrap(), ["auth", "login", "--web", "--hostname", "github.com", "--git-protocol", "https", "--skip-ssh-key"]);
    assert_eq!(result, SignInResult::Ok(signed_in()));
}

#[test]
fn runs_one_sign_in_at_a_time_and_stop_ends_it() {
    let gh = fake(vec![]);
    let handle = FakeLoginHandle::new();
    *gh.login.lock().unwrap() = Some(handle.clone());
    let account = Arc::new(Account::new(gh));
    let first = {
        let account = account.clone();
        std::thread::spawn(move || account.sign_in(|_| {}))
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while account.running.lock().unwrap().is_none() {
        assert!(Instant::now() < deadline, "the first sign-in never started");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(account.sign_in(|_| {}), SignInResult::Refused("A GitHub sign-in is already waiting. Finish it on GitHub, or stop it.".into()));
    account.cancel();
    assert!(handle.stopped.load(Ordering::SeqCst));
    assert_eq!(first.join().unwrap(), SignInResult::Refused("Sign-in stopped.".into()));
}

#[test]
fn stops_waiting_when_the_code_runs_out() {
    let gh = fake(vec![]);
    let handle = FakeLoginHandle::new();
    *gh.login.lock().unwrap() = Some(handle.clone());
    let account = Account::with_timeout(gh, Duration::from_millis(5));
    assert_eq!(account.sign_in(|_| {}), SignInResult::Refused("The code ran out before GitHub was told yes. Sign in again for a new one.".into()));
    assert!(handle.stopped.load(Ordering::SeqCst));
}

#[test]
fn says_what_gh_said_when_github_refused() {
    let gh = fake(vec![]);
    let handle = FakeLoginHandle::new();
    *gh.login.lock().unwrap() = Some(handle.clone());
    handle.print(PRINTED);
    handle.print("error: the device code was denied\n");
    handle.end(1);
    assert_eq!(Account::new(gh).sign_in(|_| {}), SignInResult::Refused("GitHub did not sign you in: error: the device code was denied".into()));
}

// ---- installing and updating from the card ----

fn installer(platform: &'static str, brew: Option<&'static str>, ran: GhAnswer, after: Vec<GhAnswer>) -> Arc<Fake> {
    Arc::new(Fake { answers: Mutex::new(after.into()), platform, brew, installer: Mutex::new(Some(ran)), ..Fake::default() })
}

const SIGNED_OUT: &str = r#"{"hosts":{}}"#;

#[test]
fn uses_winget_on_windows_githubs_own_package_and_then_asks_gh_who_is_signed_in() {
    let gh = installer("win32", None, GhAnswer::code(0, ""), vec![GhAnswer::code(0, SIGNED_OUT)]);
    assert_eq!(Account::new(gh.clone()).install(), InstallResult::Ok(GithubAccount::SignedOut));
    assert_eq!(gh.installs.lock().unwrap()[0], std::iter::once("winget").chain(WINGET_GH_ARGS).collect::<Vec<_>>());
    assert!(WINGET_GH_ARGS.contains(&"GitHub.cli") && WINGET_GH_ARGS.contains(&"--exact"));
}

#[test]
fn takes_wingets_already_installed_and_no_newer_version_signed_or_as_reported_unsigned() {
    for code in [-1978335135_i64, -1978335189, 2316632161, 2316632107] {
        let gh = installer("win32", None, GhAnswer::code(code, ""), vec![GhAnswer::code(0, SIGNED_OUT)]);
        assert_eq!(Account::new(gh).install(), InstallResult::Ok(GithubAccount::SignedOut), "exit {code}");
    }
}

#[test]
fn uses_homebrew_on_a_mac_that_has_it_and_offers_githubs_page_on_one_that_does_not() {
    let brewed = installer("darwin", Some("/opt/homebrew/bin/brew"), GhAnswer::code(0, ""), vec![GhAnswer::code(0, SIGNED_OUT)]);
    assert!(matches!(Account::new(brewed.clone()).install(), InstallResult::Ok(_)));
    assert_eq!(brewed.installs.lock().unwrap()[0], ["/opt/homebrew/bin/brew", "install", "gh"]);
    let bare = installer("darwin", None, GhAnswer::code(0, ""), vec![]);
    assert!(matches!(Account::new(bare.clone()).install(), InstallResult::Refused { get_it_yourself: true, .. }));
    assert!(bare.installs.lock().unwrap().is_empty());
    assert!(matches!(Account::new(installer("linux", None, GhAnswer::code(0, ""), vec![])).install(), InstallResult::Refused { get_it_yourself: true, .. }));
}

#[test]
fn says_why_when_the_installer_is_missing_or_fails_and_offers_githubs_page() {
    assert_eq!(
        Account::new(installer("win32", None, GhAnswer::missing(), vec![])).install(),
        InstallResult::Refused { message: "winget is not on this computer, so Locust cannot install it for you.".into(), get_it_yourself: true }
    );
    assert_eq!(
        Account::new(installer("win32", None, GhAnswer::code(1, "Installer failed with exit code: 1602\n"), vec![])).install(),
        InstallResult::Refused { message: "The GitHub CLI was not installed: Installer failed with exit code: 1602".into(), get_it_yourself: true }
    );
}

#[test]
fn finds_a_gh_installed_while_locust_runs_where_its_installer_put_it() {
    let fresh = "C:\\Program Files\\GitHub CLI\\gh.exe";
    assert_eq!(gh_command("win32", "C:\\Windows", Some("C:\\Program Files"), None, &|p| p == fresh), fresh);
    assert_eq!(gh_command("win32", "C:\\tools", Some("C:\\Program Files"), None, &|p| p == "C:\\tools\\gh.exe"), "gh");
    assert_eq!(gh_command("darwin", "/usr/bin", None, None, &|p| p == "/opt/homebrew/bin/gh"), "/opt/homebrew/bin/gh");
    assert_eq!(gh_command("win32", "", None, None, &|_| false), "gh");
}

#[test]
fn reads_its_version_and_the_newest_github_has_released() {
    let gh = fake(vec![GhAnswer::code(0, "gh version 2.96.0 (2026-07-02)\nhttps://github.com/cli/cli/releases/tag/v2.96.0\n"), GhAnswer::code(0, "v2.102.0\n")]);
    assert_eq!(Account::new(gh).versions(), CliVersions { installed: Some("2.96.0".into()), latest: Some("2.102.0".into()) });
}

#[test]
fn leaves_the_newest_unsaid_when_github_could_not_be_asked_and_asks_again_next_time() {
    let version = || GhAnswer::code(0, "gh version 2.96.0 (2026-07-02)\n");
    let gh = fake(vec![version(), GhAnswer::code(1, ""), version(), GhAnswer::code(1, "")]);
    let account = Account::new(gh.clone());
    assert_eq!(account.versions(), CliVersions { installed: Some("2.96.0".into()), latest: None });
    assert_eq!(account.versions(), CliVersions { installed: Some("2.96.0".into()), latest: None });
    let asked = gh.asked.lock().unwrap().iter().filter(|a| a[0] == "api").count();
    assert_eq!(asked, 2);
}

#[test]
fn says_nothing_for_a_computer_with_no_github_cli() {
    assert_eq!(Account::new(fake(vec![GhAnswer::missing()])).versions(), CliVersions::default());
}

#[test]
fn is_behind_only_when_github_has_a_higher_number() {
    let v = |i: &str, l: Option<&str>| CliVersions { installed: Some(i.into()), latest: l.map(str::to_owned) };
    assert!(cli_is_behind(Some(&v("2.96.0", Some("2.102.0")))));
    assert!(!cli_is_behind(Some(&v("2.102.0", Some("2.102.0")))));
    assert!(!cli_is_behind(Some(&v("3.0.0", Some("2.102.0")))));
    assert!(!cli_is_behind(Some(&v("2.96.0", None))));
    assert!(!cli_is_behind(None));
}

#[test]
fn runs_wingets_upgrade_of_githubs_own_package_on_windows_and_homebrews_on_a_mac() {
    let win = installer("win32", None, GhAnswer::code(0, ""), vec![GhAnswer::code(0, SIGNED_OUT)]);
    assert!(matches!(Account::new(win.clone()).update(), InstallResult::Ok(_)));
    let mac = installer("darwin", Some("/usr/local/bin/brew"), GhAnswer::code(0, ""), vec![GhAnswer::code(0, SIGNED_OUT)]);
    assert!(matches!(Account::new(mac.clone()).update(), InstallResult::Ok(_)));
    assert_eq!(win.installs.lock().unwrap()[0], std::iter::once("winget").chain(WINGET_GH_UPGRADE_ARGS).collect::<Vec<_>>());
    assert_eq!(mac.installs.lock().unwrap()[0], ["/usr/local/bin/brew", "upgrade", "gh"]);
    assert_eq!(&WINGET_GH_UPGRADE_ARGS[..3], ["upgrade", "--id", "GitHub.cli"]);
}

#[test]
fn says_why_when_it_could_not_update() {
    let gh = installer("win32", None, GhAnswer::code(1, "The installer was cancelled by the user.\n"), vec![]);
    assert_eq!(
        Account::new(gh).update(),
        InstallResult::Refused { message: "The GitHub CLI was not updated: The installer was cancelled by the user.".into(), get_it_yourself: true }
    );
}
