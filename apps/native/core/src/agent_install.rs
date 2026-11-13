//! Install an agent from its row (Electron `main/runtime-installer.ts`): run `npm install -g <package>`, exactly the
//! line the row shows, for someone who has nothing on their machine yet.
//!
//! What it is careful about, as Electron's is:
//!
//! - **It runs exactly the command the screen showed.** The line a person read and the arguments spawned come from one
//!   place ([`crate::agents::install_command`]), split from the same string.
//! - **One install at a time**, so a line of output belongs to one install.
//! - **A failure is classified, not dumped**: what happened and what to do, each a sentence, and the command stays on
//!   screen to run by hand.
//! - **A clean exit with nothing to run is detected**: the caller asks the machine again after npm says fine.
//! - Off Windows, a permission failure (npm's global folder is the system's) is tried once more into `~/.npm-global`,
//!   which the PATH Locust runs with already searches, and says so.
//!
//! Electron also carries an npm of its own for a machine with none; this app carries no Node, so a machine without npm
//! gets the failure's words and the line to run.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// How an install ended, in the shape the row draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// One sentence: what happened.
    pub what: String,
    /// One sentence: what to do about it.
    pub next: String,
    /// Whether looking again (Electron: restarting Locust) is the action, rather than running it again.
    pub restart: bool,
    /// The command to show when it is not the one that just failed (a permission failure's remedy).
    pub command: Option<String>,
}

pub type Outcome = Result<(), Failure>;

/// Run a command with its arguments, handing each printed line over as it arrives; how it ended and what it printed.
pub trait Run: Send + Sync {
    fn run(&self, command: &str, args: &[String], on_line: &mut dyn FnMut(&str)) -> (Option<i32>, String);
}

/// The most output kept. Bounded: this is not a terminal.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

fn matches(pattern: &'static str, text: &str) -> bool {
    crate::invariants::regex(pattern).is_match(text)
}

const NETWORK: &str = r"(?i)ENOTFOUND|EAI_AGAIN|ECONNREFUSED|ECONNRESET|ETIMEDOUT|network|getaddrinfo";
const PROXY: &str = r"(?i)407|proxy|tunneling socket|ERR_PROXY";
const PERMISSION: &str = r"(?i)EACCES|EPERM|permission denied|operation not permitted";
const IN_USE: &str = r"(?i)EBUSY|EPERM[^\n]{0,80}\b(?:rename|unlink|rmdir|scandir|open|copyfile)\b";
const PACKAGE: &str = r"(?i)E404|ETARGET|notarget|is not in this registry|No matching version";

fn failure(what: impl Into<String>, next: impl Into<String>) -> Failure {
    Failure { what: what.into(), next: next.into(), restart: false, command: None }
}

/// Read npm's own words and say which of the six this was. A proxy failure usually looks like a network one too, and
/// naming the proxy is the more useful, so it is asked first.
pub fn classify(package: &str, display_name: &str, output: &str, seconds: u64, platform: &str) -> Failure {
    let windows = platform == "windows";
    if matches(PROXY, output) {
        return failure("The npm registry refused the connection.", "If this machine uses a proxy, npm needs to be told about it. The command below works in a terminal that already has one configured.");
    }
    if matches(NETWORK, output) {
        return failure("npm could not reach the registry.", "Check this machine's connection and try again.");
    }
    if windows && matches(IN_USE, output) {
        return failure(
            format!("A running program is holding {display_name}'s files, so npm could not replace them."),
            format!("Close anything using {display_name} -- a terminal running it, or a Locust mission on it -- and try again."),
        );
    }
    if matches(PERMISSION, output) {
        let prefix = if windows { "%LOCALAPPDATA%\\npm-global" } else { "~/.npm-global" };
        return Failure {
            command: Some(format!("npm config set prefix \"{prefix}\"\nnpm install -g {package}")),
            ..failure(
                "npm could not write to its global folder.",
                format!("Point npm at a folder you own and install again. Add {prefix}{} to your PATH afterwards so Locust can find it.", if windows { "" } else { "/bin" }),
            )
        };
    }
    if matches(PACKAGE, output) {
        return failure(format!("npm could not install {package}."), "The package name may have changed. The command below is the one Locust would have run.");
    }
    failure(format!("npm stopped with an error after {seconds}s."), "Show the output below \u{2014} the last lines usually name the cause.")
}

/// npm said fine and the command is still not there.
pub fn installed_but_not_found(display_name: &str) -> Failure {
    Failure {
        restart: true,
        ..failure(
            format!("{display_name} installed, but Locust still cannot find the command."),
            "It may need a new terminal session, or it may not be on this machine\u{2019}s PATH. Restart Locust and it will look again.",
        )
    }
}

/// The only shape an install argument may take: what an npm package or flag needs (`@ / . _ - :`), nothing a shell
/// reads as syntax. Nothing here goes through a shell, and every argument comes from a constant table today; this is
/// for the day one does not.
pub fn safe_to_spawn(token: &str) -> bool {
    matches(r"^[A-Za-z0-9@/._:-]+$", token)
}

fn is_version(v: &str) -> bool {
    matches(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?$", v)
}

pub struct Installer {
    run: Arc<dyn Run>,
    /// Whether the agent can now be found; asked only after a clean exit.
    now_installed: Box<dyn Fn(&str) -> bool + Send + Sync>,
    /// Off Windows: where an install goes when npm's global folder is not this user's (`~/.npm-global`, absolute).
    user_prefix: Option<String>,
    platform: &'static str,
    running: AtomicBool,
}

struct Running<'a>(&'a AtomicBool);
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Installer {
    pub fn new(run: Arc<dyn Run>, now_installed: Box<dyn Fn(&str) -> bool + Send + Sync>, user_prefix: Option<String>, platform: &'static str) -> Self {
        Installer { run, now_installed, user_prefix, platform, running: AtomicBool::new(false) }
    }

    /// True while an install runs; every other Install waits on it.
    pub fn busy(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Install `id` (or, with `version`, that release over the one there), handing npm's lines to `on_line`.
    pub fn install(&self, id: &str, display_name: &str, version: Option<&str>, on_line: &mut dyn FnMut(&str)) -> Outcome {
        if let Some(v) = version
            && !is_version(v)
        {
            return Err(failure(format!("\"{v}\" is not a version Locust installs."), "Nothing was changed."));
        }
        let (Some(plain), Some(package)) = (crate::agents::install_command(id), crate::agents::install_package(id)) else {
            return Err(failure("That runtime does not install from a package manager.", "Its own page has the installer."));
        };
        let line = match version {
            Some(v) => format!("{plain}@{v}"),
            None => plain,
        };
        if self.running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return Err(failure("Another install is already running.", "Wait for it to finish, then try this one."));
        }
        let _running = Running(&self.running);
        let started = std::time::Instant::now();
        // Split from the same string the row showed, so the arguments and the line cannot drift apart.
        let mut words = line.split(' ').map(str::to_owned);
        let command = words.next().unwrap_or_else(|| "npm".into());
        let args: Vec<String> = words.collect();
        let (mut code, mut output) = self.run.run(&command, &args, on_line);
        if code != Some(0)
            && self.platform != "windows"
            && let Some(prefix) = self.user_prefix.as_deref().filter(|p| safe_to_spawn(p))
            && matches(PERMISSION, &output)
        {
            on_line(&format!("npm's global folder belongs to the system; installing into {prefix} instead."));
            let mut again = args.clone();
            again.extend(["--prefix".to_owned(), prefix.to_owned()]);
            (code, output) = self.run.run(&command, &again, on_line);
        }
        if code != Some(0) {
            let seconds = started.elapsed().as_secs_f64().round().max(1.0) as u64;
            return Err(classify(&package, display_name, &output, seconds, self.platform));
        }
        // A clean exit is not the same as something to run.
        if (self.now_installed)(id) { Ok(()) } else { Err(installed_but_not_found(display_name)) }
    }
}

/// npm on this machine, spawned with no shell (the PATH Locust adopted at launch finds it), its lines handed over as
/// they arrive. Refuses an argument shaped like shell syntax.
pub struct Npm;

impl Run for Npm {
    fn run(&self, command: &str, args: &[String], on_line: &mut dyn FnMut(&str)) -> (Option<i32>, String) {
        use std::io::{BufRead, BufReader};
        if let Some(unsafe_token) = std::iter::once(command).chain(args.iter().map(String::as_str)).find(|t| !safe_to_spawn(t)) {
            return (None, format!("Locust would not run this install: {unsafe_token:?} contains characters a shell reads as syntax."));
        }
        if cfg!(test) {
            return (None, "no install runs in a test".into());
        }
        let child = std::process::Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => return (None, format!("npm could not be started: {e}")),
        };
        // stderr on its own thread into a channel, stdout here: both reach on_line in the order they arrive.
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let readers: Vec<_> = [child.stdout.take().map(|o| Box::new(o) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|e| Box::new(e) as Box<dyn std::io::Read + Send>)]
            .into_iter()
            .flatten()
            .map(|stream| {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    for line in BufReader::new(stream).lines().map_while(Result::ok) {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                })
            })
            .collect();
        drop(tx);
        let mut output = String::new();
        for line in rx {
            if output.len() < MAX_OUTPUT_BYTES {
                output.push_str(&line);
                output.push('\n');
            }
            let said = line.trim();
            if !said.is_empty() {
                on_line(said);
            }
        }
        for r in readers {
            let _ = r.join();
        }
        match child.wait() {
            Ok(status) => (status.code(), output),
            Err(e) => (None, format!("{output}{e}")),
        }
    }
}

/// This machine's one installer: npm, `~/.npm-global` for a global folder that is the system's, and the agent's command
/// looked for again after npm says fine.
pub fn for_this_machine() -> &'static Installer {
    static ONE: std::sync::OnceLock<Installer> = std::sync::OnceLock::new();
    ONE.get_or_init(|| {
        let prefix = std::env::var("HOME").ok().map(|home| format!("{home}/.npm-global"));
        let found = |id: &str| crate::agents::command_of(id).is_some_and(|c| crate::runtime::discovery::find_named(c).is_ok());
        Installer::new(Arc::new(Npm), Box::new(found), prefix, if cfg!(windows) { "windows" } else { std::env::consts::OS })
    })
}

#[cfg(test)]
#[path = "agent_install_tests.rs"]
mod tests;
