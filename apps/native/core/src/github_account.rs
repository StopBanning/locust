//! YOUR GITHUB, CONNECTED (Electron `shared/github-account.ts` and `main/github-account.ts`, 0.720 to 0.739).
//!
//! The account is the GitHub CLI's own, never a second one: Locust keeps no token. It asks `gh auth status --json
//! hosts` who is signed in, and signs in with `gh auth login --web`, which prints a one-time code and GitHub's device
//! page and then waits for GitHub to say yes. The code is shown on the card; the person enters it on GitHub's own
//! page, in their own browser. Install and update use the computer's own package manager (Homebrew on a Mac that
//! has it, winget on Windows).
//!
//! Everything that runs a program goes through [`Gh`], so tests never run `gh`, `brew` or `winget`.

use serde_json::Value;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// Where the GitHub CLI comes from, for a computer without it.
pub const GITHUB_CLI_PAGE: &str = "https://cli.github.com/";
/// The only page a sign-in code is entered on.
pub const GITHUB_DEVICE_PAGE: &str = "https://github.com/login/device";
/// A device code lasts fifteen minutes on GitHub; the wait stops there too.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub const WINGET_GH_ARGS: [&str; 9] = [
    "install", "--id", "GitHub.cli", "--exact", "--source", "winget", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity",
];
pub const WINGET_GH_UPGRADE_ARGS: [&str; 9] = [
    "upgrade", "--id", "GitHub.cli", "--exact", "--source", "winget", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity",
];
/// winget's "already installed" and "no newer version" exits, as signed 32-bit numbers: gh is there either way.
const WINGET_ALREADY: [i32; 2] = [-1978335135, -1978335189];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GithubAccount {
    /// No `gh` on this computer.
    NoCli,
    SignedOut,
    /// gh holds an account whose sign-in GitHub no longer takes.
    Expired { login: Option<String> },
    SignedIn { login: String, scopes: Vec<String> },
    /// gh answered with something Locust could not read.
    Unknown { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignInCode {
    pub code: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInResult {
    Ok(GithubAccount),
    Refused(String),
}

/// Install or update from the card. `get_it_yourself`: the card then offers GitHub's own download page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallResult {
    Ok(GithubAccount),
    Refused { message: String, get_it_yourself: bool },
}

/// Which GitHub CLI this computer has, and the newest GitHub has released; either unsaid when unread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliVersions {
    pub installed: Option<String>,
    pub latest: Option<String>,
}

const UNREADABLE: &str = "The GitHub CLI answered in a way Locust could not read.";

/// A GitHub login: letters, digits and single hyphens, as GitHub allows (at most 39 characters).
fn is_login(login: &str) -> bool {
    let bytes = login.as_bytes();
    if bytes.is_empty() || bytes.len() > 39 || !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'-' {
            if !bytes.get(i + 1).is_some_and(u8::is_ascii_alphanumeric) {
                return false;
            }
        } else if !b.is_ascii_alphanumeric() {
            return false;
        }
    }
    true
}

/// `gh auth status --json hosts` -> who is signed in to github.com.
pub fn account_of(json: &str) -> GithubAccount {
    let unknown = || GithubAccount::Unknown { message: UNREADABLE.into() };
    let Ok(parsed) = serde_json::from_str::<Value>(json) else { return unknown() };
    let Some(hosts) = parsed.get("hosts").filter(|h| h.is_object()) else { return unknown() };
    let accounts: Vec<&Value> = hosts.get("github.com").and_then(Value::as_array).map(|a| a.iter().filter(|e| e.is_object()).collect()).unwrap_or_default();
    let Some(&first) = accounts.first() else { return GithubAccount::SignedOut };
    let active = accounts.iter().copied().find(|e| e["active"] == true).unwrap_or(first);
    let login = active["login"].as_str().filter(|l| is_login(l)).map(str::to_owned);
    if active["state"] != "success" || login.is_none() {
        return GithubAccount::Expired { login };
    }
    let scopes = active["scopes"].as_str().map(|s| s.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect()).unwrap_or_default();
    GithubAccount::SignedIn { login: login.unwrap_or_default(), scopes }
}

/// gh's plain `auth status`, for a gh too old for `--json`: its exit says whether anyone is signed in, and its first
/// account line names who.
pub fn account_of_text(text: &str, signed_in: bool) -> GithubAccount {
    if !signed_in {
        return GithubAccount::SignedOut;
    }
    let login = ["Logged in to github.com account ", "Logged in to github.com as "].iter().find_map(|marker| {
        let at = text.find(marker)? + marker.len();
        let name: String = text[at..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        Some(name)
    });
    match login.filter(|l| is_login(l)) {
        Some(login) => GithubAccount::SignedIn { login, scopes: vec![] },
        None => GithubAccount::Unknown { message: "The GitHub CLI says it is signed in but did not say as whom.".into() },
    }
}

/// The one-time code and the page gh printed, once both have arrived; GitHub's own device page only. In either of
/// gh's wordings: "! First copy your one-time code: AB12-CD34" until about 2.100, and from 2.102 "! One-time code
/// (AB12-CD34) copied to clipboard" with the page on its own line.
pub fn sign_in_code_of(text: &str) -> Option<SignInCode> {
    let lower = text.to_ascii_lowercase();
    let mut code = None;
    let mut from = 0;
    while let Some(found) = lower[from..].find("one-time code") {
        let mut rest = &text[from + found + "one-time code".len()..];
        if let Some(r) = rest.strip_prefix(':') {
            rest = r.trim_start();
        } else if let Some(r) = rest.trim_start().strip_prefix('(') {
            rest = r.trim_start();
        } else {
            from += found + 1;
            continue;
        }
        let candidate: String = rest.chars().take(9).collect();
        let b = candidate.as_bytes();
        let ok = b.len() == 9
            && b[4] == b'-'
            && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_uppercase() || c.is_ascii_digit())
            && !rest[9..].chars().next().is_some_and(|c| c.is_ascii_alphanumeric());
        if ok {
            code = Some(candidate);
            break;
        }
        from += found + 1;
    }
    let url_at = text.find(GITHUB_DEVICE_PAGE)?;
    let after = text[url_at + GITHUB_DEVICE_PAGE.len()..].chars().next();
    if after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(SignInCode { code: code?, url: GITHUB_DEVICE_PAGE.into() })
}

/// What the card says, in a line.
pub fn account_line(account: &GithubAccount) -> String {
    match account {
        GithubAccount::NoCli => "The GitHub CLI is not on this computer. Install it here, then sign in.".into(),
        GithubAccount::SignedOut => "Not signed in. Sign in so teammates can push and open pull requests as you.".into(),
        GithubAccount::Expired { login } => format!(
            "{} no longer works. Sign in again.",
            login.as_ref().map(|l| format!("The sign-in for {l}")).unwrap_or_else(|| "Your GitHub sign-in".into())
        ),
        // The card's name already says who: the line says what it means.
        GithubAccount::SignedIn { .. } => "Signed in to GitHub. Teammates push and open pull requests as you.".into(),
        GithubAccount::Unknown { message } => message.clone(),
    }
}

/// Whether `latest` is a newer release than `installed`, by their numbers.
pub fn cli_is_behind(versions: Option<&CliVersions>) -> bool {
    let Some(CliVersions { installed: Some(have), latest: Some(out) }) = versions else { return false };
    let parts = |v: &str| v.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (have, out) = (parts(have), parts(out));
    for at in 0..3 {
        let (h, o) = (have.get(at).copied().unwrap_or(0), out.get(at).copied().unwrap_or(0));
        if h != o {
            return o > h;
        }
    }
    false
}

/// Where gh is, PATH first. A gh installed while Locust runs is not on the PATH Locust started with, so the places
/// its installers put it are looked at too: winget's GitHub CLI folders, and Homebrew's two prefixes.
pub fn gh_command(platform: &str, path: &str, program_files: Option<&str>, local_app_data: Option<&str>, exists: &dyn Fn(&str) -> bool) -> String {
    let windows = platform == "win32";
    let (delimiter, slash, name) = if windows { (';', '\\', "gh.exe") } else { (':', '/', "gh") };
    for dir in path.split(delimiter).filter(|d| !d.is_empty()) {
        if exists(&format!("{}{slash}{name}", dir.trim_end_matches(slash))) {
            return "gh".into();
        }
    }
    let known: Vec<String> = if windows {
        program_files.map(|p| format!("{p}\\GitHub CLI\\gh.exe")).into_iter().chain(local_app_data.map(|p| format!("{p}\\Programs\\GitHub CLI\\gh.exe"))).collect()
    } else {
        vec!["/opt/homebrew/bin/gh".into(), "/usr/local/bin/gh".into()]
    };
    known.into_iter().find(|p| exists(p)).unwrap_or_else(|| "gh".into())
}

/// How a program ended: its exit, or that it is not on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Code(i64),
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhAnswer {
    pub exit: Exit,
    pub stdout: String,
    pub stderr: String,
}

impl GhAnswer {
    pub fn code(code: i64, stdout: &str) -> Self {
        GhAnswer { exit: Exit::Code(code), stdout: stdout.into(), stderr: String::new() }
    }
    pub fn missing() -> Self {
        GhAnswer { exit: Exit::Missing, stdout: String::new(), stderr: String::new() }
    }
}

/// What a running `gh auth login` gave, read with a deadline.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginEvent {
    Text(String),
    Ended(Exit),
    Nothing,
}

/// A running `gh auth login --web`.
pub trait Login: Send {
    fn next(&mut self, wait: Duration) -> LoginEvent;
    fn stop(&mut self);
}

/// The programs this needs. The real one runs them; tests answer for them.
pub trait Gh: Send + Sync {
    fn run_gh(&self, args: &[&str]) -> GhAnswer;
    fn start_login(&self, args: &[&str]) -> Box<dyn Login>;
    fn run_installer(&self, command: &str, args: &[&str]) -> GhAnswer;
    fn exists(&self, path: &str) -> bool;
    fn platform(&self) -> &'static str;
}

pub struct Account {
    gh: Arc<dyn Gh>,
    sign_in_timeout: Duration,
    running: Mutex<Option<Arc<Mutex<Box<dyn Login>>>>>,
    installing: AtomicBool,
    latest: Mutex<Option<String>>,
}

impl Account {
    pub fn new(gh: Arc<dyn Gh>) -> Self {
        Self::with_timeout(gh, SIGN_IN_TIMEOUT)
    }

    pub fn with_timeout(gh: Arc<dyn Gh>, sign_in_timeout: Duration) -> Self {
        Account { gh, sign_in_timeout, running: Mutex::new(None), installing: AtomicBool::new(false), latest: Mutex::new(None) }
    }

    pub fn read(&self) -> GithubAccount {
        let answer = self.gh.run_gh(&["auth", "status", "--json", "hosts", "--hostname", "github.com"]);
        if answer.exit == Exit::Missing {
            return GithubAccount::NoCli;
        }
        if answer.exit == Exit::Code(0) && answer.stdout.trim().starts_with('{') {
            return account_of(&answer.stdout);
        }
        // A gh from before `--json`: its plain status, by exit and first line.
        let plain = self.gh.run_gh(&["auth", "status", "--hostname", "github.com"]);
        if plain.exit == Exit::Missing {
            return GithubAccount::NoCli;
        }
        account_of_text(&format!("{}\n{}", plain.stdout, plain.stderr), plain.exit == Exit::Code(0))
    }

    /// One sign-in at a time; `on_code` hears the code once gh prints it.
    pub fn sign_in(&self, mut on_code: impl FnMut(SignInCode)) -> SignInResult {
        let login = {
            let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
            if running.is_some() {
                return SignInResult::Refused("A GitHub sign-in is already waiting. Finish it on GitHub, or stop it.".into());
            }
            let login = Arc::new(Mutex::new(self.gh.start_login(&["auth", "login", "--web", "--hostname", "github.com", "--git-protocol", "https", "--skip-ssh-key"])));
            *running = Some(login.clone());
            login
        };
        let deadline = Instant::now() + self.sign_in_timeout;
        let mut printed = String::new();
        let mut told = false;
        let ended = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                login.lock().unwrap_or_else(|e| e.into_inner()).stop();
                self.forget(&login);
                return SignInResult::Refused("The code ran out before GitHub was told yes. Sign in again for a new one.".into());
            }
            let event = login.lock().unwrap_or_else(|e| e.into_inner()).next(left.min(Duration::from_millis(100)));
            match event {
                LoginEvent::Text(text) => {
                    // Kept after the code too: a refusal's reason is printed last.
                    printed.push_str(&text);
                    if printed.chars().count() > 4000 {
                        printed = printed.chars().rev().take(4000).collect::<Vec<_>>().into_iter().rev().collect();
                    }
                    if !told && let Some(code) = sign_in_code_of(&printed) {
                        told = true;
                        on_code(code);
                    }
                }
                LoginEvent::Ended(exit) => break exit,
                LoginEvent::Nothing => {}
            }
        };
        let stopped = !self.is_running(&login);
        self.forget(&login);
        match ended {
            Exit::Missing => SignInResult::Refused("The GitHub CLI is not on this computer.".into()),
            Exit::Code(0) if !stopped => SignInResult::Ok(self.read()),
            _ if stopped => SignInResult::Refused("Sign-in stopped.".into()),
            Exit::Code(_) => {
                let said = printed.lines().map(str::trim).filter(|l| !l.is_empty() && !l.to_ascii_lowercase().contains("one-time code")).last();
                SignInResult::Refused(match said {
                    Some(said) => format!("GitHub did not sign you in: {}", said.chars().take(200).collect::<String>()),
                    None => "GitHub did not sign you in.".into(),
                })
            }
        }
    }

    fn is_running(&self, login: &Arc<Mutex<Box<dyn Login>>>) -> bool {
        self.running.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|r| Arc::ptr_eq(r, login))
    }

    fn forget(&self, login: &Arc<Mutex<Box<dyn Login>>>) {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if running.as_ref().is_some_and(|r| Arc::ptr_eq(r, login)) {
            *running = None;
        }
    }

    /// Stop the sign-in that is waiting, if one is.
    pub fn cancel(&self) {
        let login = self.running.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(login) = login {
            login.lock().unwrap_or_else(|e| e.into_inner()).stop();
        }
    }

    pub fn install(&self) -> InstallResult {
        self.via_package_manager(false)
    }

    pub fn update(&self) -> InstallResult {
        self.via_package_manager(true)
    }

    /// One install or update at a time, with the computer's own package manager; then gh is asked again, from
    /// wherever the installer put it.
    fn via_package_manager(&self, update: bool) -> InstallResult {
        let (doing, verb) = if update { ("updated", "update") } else { ("installed", "install") };
        if self.installing.swap(true, Ordering::SeqCst) {
            return InstallResult::Refused { message: "The GitHub CLI is already being installed or updated.".into(), get_it_yourself: false };
        }
        let result = (|| {
            let platform = self.gh.platform();
            let brew = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"].into_iter().find(|p| self.gh.exists(p));
            let (command, args): (&str, Vec<&str>) = if platform == "win32" {
                ("winget", if update { WINGET_GH_UPGRADE_ARGS.to_vec() } else { WINGET_GH_ARGS.to_vec() })
            } else if let (true, Some(brew)) = (platform == "darwin", brew) {
                (brew, vec![if update { "upgrade" } else { "install" }, "gh"])
            } else {
                return InstallResult::Refused { message: format!("This computer has no package manager Locust can {verb} it with."), get_it_yourself: true };
            };
            let ran = self.gh.run_installer(command, &args);
            let code = match ran.exit {
                Exit::Missing => {
                    let manager = if platform == "win32" { "winget" } else { "Homebrew" };
                    return InstallResult::Refused { message: format!("{manager} is not on this computer, so Locust cannot {verb} it for you."), get_it_yourself: true };
                }
                Exit::Code(code) => code,
            };
            // winget's codes, read as 32 bits: an unsigned report of a signed exit still matches.
            if code != 0 && !(platform == "win32" && WINGET_ALREADY.contains(&(code as i32))) {
                let said = format!("{}\n{}", ran.stdout, ran.stderr).lines().map(str::trim).filter(|l| !l.is_empty()).last().map(str::to_owned);
                return InstallResult::Refused {
                    message: match said {
                        Some(said) => format!("The GitHub CLI was not {doing}: {}", said.chars().take(200).collect::<String>()),
                        None => format!("The GitHub CLI was not {doing}."),
                    },
                    get_it_yourself: true,
                };
            }
            match self.read() {
                GithubAccount::NoCli => InstallResult::Refused { message: format!("It {doing}, but Locust cannot find it yet. Restart Locust and it will."), get_it_yourself: false },
                account => InstallResult::Ok(account),
            }
        })();
        self.installing.store(false, Ordering::SeqCst);
        result
    }

    /// Which gh this is, and the newest GitHub has released: `gh --version`, and `gh api
    /// repos/cli/cli/releases/latest`. The newest is asked once a launch; anything unread leaves it unsaid, and it is
    /// asked again next time.
    pub fn versions(&self) -> CliVersions {
        let said = self.gh.run_gh(&["--version"]);
        if said.exit == Exit::Missing {
            return CliVersions::default();
        }
        let installed = version_after(&said.stdout, "gh version ");
        let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        if latest.is_none() {
            let answer = self.gh.run_gh(&["api", "repos/cli/cli/releases/latest", "--jq", ".tag_name"]);
            if answer.exit == Exit::Code(0) {
                let tag = answer.stdout.trim();
                let tag = tag.strip_prefix('v').unwrap_or(tag);
                if is_version(tag) {
                    *latest = Some(tag.to_owned());
                }
            }
        }
        CliVersions { installed, latest: latest.clone() }
    }
}

fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn version_after(text: &str, marker: &str) -> Option<String> {
    let at = text.find(marker)? + marker.len();
    let candidate: String = text[at..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    is_version(&candidate).then_some(candidate)
}

/// The real programs: `gh` found as [`gh_command`] finds it, run quiet, with a time limit.
pub struct RealGh;

fn quiet(command: &mut std::process::Command) -> &mut std::process::Command {
    command.env("GH_PROMPT_DISABLED", "1").env("GH_NO_UPDATE_NOTIFIER", "1").env("NO_COLOR", "1")
}

fn real_gh_command() -> String {
    let path = std::env::var("PATH").unwrap_or_default();
    gh_command(RealGh.platform(), &path, std::env::var("ProgramFiles").ok().as_deref(), std::env::var("LOCALAPPDATA").ok().as_deref(), &|p| Path::new(p).exists())
}

fn run_bounded(command: &mut std::process::Command, limit: Duration) -> GhAnswer {
    use std::process::Stdio;
    let child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return GhAnswer::missing(),
        Err(_) => return GhAnswer::code(1, ""),
    };
    let (tx, rx) = mpsc::channel();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut out = String::new();
        let mut err = String::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_string(&mut out);
        }
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_string(&mut err);
        }
        let _ = tx.send((out, err));
    });
    let deadline = Instant::now() + limit;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            let (stdout, stderr) = rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
            return GhAnswer { exit: Exit::Code(status.code().unwrap_or(1).into()), stdout, stderr };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return GhAnswer { exit: Exit::Code(1), stdout: String::new(), stderr: "It took too long.".into() };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct RealLogin {
    child: std::process::Child,
    text: mpsc::Receiver<String>,
}

impl Login for RealLogin {
    fn next(&mut self, wait: Duration) -> LoginEvent {
        match self.text.recv_timeout(wait) {
            Ok(text) => LoginEvent::Text(text),
            Err(_) => match self.child.try_wait() {
                Ok(Some(status)) => {
                    // What it printed last is read before its ending is said.
                    if let Ok(text) = self.text.try_recv() {
                        return LoginEvent::Text(text);
                    }
                    LoginEvent::Ended(Exit::Code(status.code().unwrap_or(1).into()))
                }
                _ => LoginEvent::Nothing,
            },
        }
    }
    fn stop(&mut self) {
        let _ = self.child.kill();
    }
}

struct EndedLogin(Exit);

impl Login for EndedLogin {
    fn next(&mut self, _wait: Duration) -> LoginEvent {
        LoginEvent::Ended(self.0)
    }
    fn stop(&mut self) {}
}

impl Gh for RealGh {
    fn run_gh(&self, args: &[&str]) -> GhAnswer {
        run_bounded(quiet(std::process::Command::new(real_gh_command()).args(args)), Duration::from_secs(20))
    }
    fn start_login(&self, args: &[&str]) -> Box<dyn Login> {
        use std::process::Stdio;
        let spawned = quiet(std::process::Command::new(real_gh_command()).args(args)).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Box::new(EndedLogin(Exit::Missing)),
            Err(_) => return Box::new(EndedLogin(Exit::Code(1))),
        };
        let (tx, rx) = mpsc::channel();
        for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)].into_iter().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buffer = [0u8; 1024];
                while let Ok(n) = std::io::Read::read(&mut stream, &mut buffer) {
                    if n == 0 || tx.send(String::from_utf8_lossy(&buffer[..n]).into_owned()).is_err() {
                        break;
                    }
                }
            });
        }
        Box::new(RealLogin { child, text: rx })
    }
    fn run_installer(&self, command: &str, args: &[&str]) -> GhAnswer {
        run_bounded(std::process::Command::new(command).args(args), Duration::from_secs(15 * 60))
    }
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
    fn platform(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(windows) {
            "win32"
        } else {
            "linux"
        }
    }
}

#[cfg(test)]
#[path = "github_account_tests.rs"]
mod tests;
