//! WHAT THE AI AGENTS SCREEN SAYS ABOUT EACH AGENT (Electron: `renderer/src/status.ts` routeRowStatus,
//! `shared/runtime-install.ts`, `shared/runtime-integration.ts`).
//!
//! An agent found on this computer is shown with a tag (READY, SIGN IN, NOT INSTALLED, ...), whether it can be picked,
//! and one sentence that says what to do. The words are Electron's: `scripts/agents-goldens.mjs` runs Electron's own
//! functions over a grid of made-up discovery results, and `agents_tests.rs` holds this file to every answer.

/// How far Locust's support for an agent goes (RUNTIME_INTEGRATION).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integration {
    /// Can own a live turn end to end.
    Live,
    /// Found and selectable, the adapter not finished.
    Preview,
    /// Runs end to end through a surface its maker did not publish.
    Experimental,
    /// Drawn in the design, not built.
    Planned,
}

impl Integration {
    pub fn as_str(self) -> &'static str {
        match self {
            Integration::Live => "live",
            Integration::Preview => "preview",
            Integration::Experimental => "experimental",
            Integration::Planned => "planned",
        }
    }
}

/// integrationOf: an agent Locust does not know is planned.
pub fn integration_of(id: &str) -> Integration {
    match id {
        "codex" | "claude" | "cursor" | "opencode" | "copilot" | "muse" | "apple" => Integration::Live,
        "antigravity" => Integration::Experimental,
        "kilo" => Integration::Preview,
        _ => Integration::Planned,
    }
}

/// Whether the agent's account is signed in (RuntimeAuthState).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Auth {
    Authenticated,
    Unauthenticated,
    /// It needs no account (OpenCode's free model).
    NotApplicable,
    Unknown,
}

/// What its last check found (RuntimeProbeStatus).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    Ready,
    AuthRequired,
    Unsupported,
    Offline,
    ProbeFailed,
    Missing,
    Unknown,
}

/// One agent as discovery found it (PublicRuntimeStatus).
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub id: String,
    pub display_name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub auth: Auth,
    pub ready: bool,
    pub status: Probe,
    /// Why an installed version cannot run yet, and what to do (only with `Probe::Unsupported`).
    pub unsupported_note: Option<String>,
    /// Antigravity through its own CLI: a published, measured route.
    pub through_cli: bool,
    /// Installed, and its check has not finished.
    pub checking: bool,
}

/// What one agent's row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowStatus {
    pub tag: &'static str,
    pub selectable: bool,
    pub detail: String,
}

pub const SIGNED_IN_DETAIL: &str = "Signed in on this machine, using your own account.";
pub const NO_ACCOUNT_DETAIL: &str = "Ready. Its free model needs no account.";

/// routeRowStatus: the row for `found`. A route whose last run ended on the account's usage limit says so, and stays
/// selectable: the limit may lift on the provider's clock.
pub fn row_status(found: &Found, integration: Integration, active: bool, limited: Option<&str>) -> RowStatus {
    let status = base_row_status(found, integration, active);
    match limited {
        Some(limit) if status.selectable => RowStatus {
            tag: "AT LIMIT",
            selectable: true,
            detail: format!("Signed in, but the last run on it ended on your account's usage limit: {limit}"),
        },
        _ => status,
    }
}

fn base_row_status(found: &Found, integration: Integration, active: bool) -> RowStatus {
    let name = &found.display_name;
    let row = |tag, selectable, detail: String| RowStatus { tag, selectable, detail };
    if integration == Integration::Planned {
        return row("PLANNED", false, "Not built yet. Shown so the roadmap is visible, not because it works.".into());
    }
    if !found.installed {
        return row("NOT INSTALLED", false, install_sentence(&found.id, name));
    }
    if found.status == Probe::AuthRequired || found.auth == Auth::Unauthenticated {
        return row("SIGN IN", false, format!("Installed, but {name} is not signed in."));
    }
    if found.status == Probe::Unsupported {
        let detail = found.unsupported_note.clone().unwrap_or_else(|| format!("This version of {name} is not supported yet."));
        return row("UNAVAILABLE", false, detail);
    }
    if !(found.ready && found.status == Probe::Ready) {
        // Installed but not answering is a moment, not a verdict: asked again shortly.
        let reason = if found.checking {
            "is still being checked"
        } else {
            match found.status {
                Probe::Offline => "could not be reached",
                Probe::ProbeFailed => "did not answer its version probe in time",
                _ => "is not ready",
            }
        };
        return row("CHECKING", false, format!("{name} {reason}. Locust asks again shortly."));
    }
    if integration == Integration::Experimental && !found.through_cli {
        return row(
            "EXPERIMENTAL",
            true,
            format!("{name} is driven through an unpublished interface of the running app. It works today and may break with an update."),
        );
    }
    if integration == Integration::Preview {
        let detail = if found.id == "kilo" {
            "Preview. Auto Free needs no account. A turn through Kilo did not finish, and paid models and sign-in have not been measured.".to_string()
        } else {
            format!("Signed in and found. Locust's support for {name} has not been tried through a whole run yet, so parts of a run may not show.")
        };
        return row("PREVIEW", true, detail);
    }
    let detail = if found.auth == Auth::NotApplicable { NO_ACCOUNT_DETAIL } else { SIGNED_IN_DETAIL };
    row(if active { "ACTIVE" } else { "READY" }, true, detail.to_string())
}

/// The runtime a person can start using free with no account (`FREE_START_RUNTIME` in Electron).
pub const FREE_START_RUNTIME: &str = "opencode";

/// How an agent is installed (`RuntimeInstall` in Electron).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeInstall {
    Npm(&'static str),
    Vendor(&'static str),
}

/// How to get an agent and sign in to it (`RuntimeInstallFacts` in Electron).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeInstallFacts {
    pub install: RuntimeInstall,
    pub sign_in: Option<&'static str>,
    pub sign_in_again: Option<&'static str>,
    pub sign_in_again_note: Option<&'static str>,
    pub account: Option<&'static str>,
    pub sign_in_optional: bool,
}

/// Facts for installing and signing in to one runtime, or `None` when it names no installable agent.
pub fn runtime_install_facts(id: &str) -> Option<RuntimeInstallFacts> {
    let f = |install, sign_in: Option<&'static str>, again: Option<&'static str>, account: Option<&'static str>| RuntimeInstallFacts {
        install,
        sign_in,
        sign_in_again: again,
        sign_in_again_note: None,
        account,
        sign_in_optional: false,
    };
    Some(match id {
        "opencode" => f(RuntimeInstall::Npm("opencode-ai"), None, None, None),
        "claude" => f(RuntimeInstall::Npm("@anthropic-ai/claude-code"), Some("claude"), Some("claude auth login"), Some("an Anthropic account")),
        "codex" => f(RuntimeInstall::Npm("@openai/codex"), Some("codex"), Some("codex login"), Some("a ChatGPT account")),
        // Its own /login, inside the window this opens.
        "copilot" => f(RuntimeInstall::Npm("@github/copilot"), Some("copilot"), Some("copilot"), Some("a GitHub Copilot subscription")),
        "cursor" => f(RuntimeInstall::Vendor("https://cursor.com/cli"), Some("cursor-agent login"), Some("cursor-agent login"), Some("a Cursor account")),
        "muse" => f(RuntimeInstall::Vendor("https://dev.meta.ai"), Some("muse login"), Some("muse login"), Some("a Muse Code subscription")),
        // Signed into inside `agy` itself: it has no login command; /logout then /login switches accounts.
        "antigravity" => RuntimeInstallFacts {
            sign_in_again_note: Some("then type /logout, then /login, and choose the account"),
            ..f(RuntimeInstall::Vendor("https://antigravity.google/product/antigravity-cli/"), Some("agy"), Some("agy"), Some("a Google account"))
        },
        // Auto Free needs no account; `kilo auth login` is only for a paid model.
        "kilo" => RuntimeInstallFacts {
            sign_in_optional: true,
            ..f(RuntimeInstall::Npm("@kilocode/cli"), Some("kilo auth login"), Some("kilo auth login"), Some("a Kilo account, only for a paid model"))
        },
        _ => return None,
    })
}

/// installCommand: the one npm line, for an agent npm installs.
pub fn install_command(id: &str) -> Option<String> {
    match runtime_install_facts(id)?.install {
        RuntimeInstall::Npm(package) => Some(format!("npm install -g {package}")),
        RuntimeInstall::Vendor(_) => None,
    }
}

/// The npm package an agent installs from, for an agent npm installs.
pub fn install_package(id: &str) -> Option<String> {
    match runtime_install_facts(id)?.install {
        RuntimeInstall::Npm(package) => Some(package.to_owned()),
        RuntimeInstall::Vendor(_) => None,
    }
}

/// installSentence: what a NOT INSTALLED row says, standing on its own (no terminal in it: the row's Install does it).
pub fn install_sentence(id: &str, display_name: &str) -> String {
    let Some(facts) = runtime_install_facts(id) else {
        return format!("{display_name} was not found on this machine.");
    };
    if let RuntimeInstall::Vendor(url) = facts.install {
        let optional = match (facts.sign_in_optional, facts.sign_in) {
            (true, Some(sign_in)) => format!(" Auto Free needs no account; run {sign_in} only for a paid model."),
            _ => String::new(),
        };
        return format!("{display_name} was not found on this machine. It installs from {url}.{optional}");
    }
    let after = match (facts.sign_in, facts.sign_in_optional) {
        (None, _) => "Press Install. It needs no account and no sign-in, and its free model runs as soon as it is there.".to_string(),
        (Some(sign_in), true) => format!("Press Install. Its free model needs no account; run {sign_in} only for a paid model."),
        (Some(_), false) => format!("Press Install, then Sign in. It needs {}.", facts.account.unwrap_or("an account")),
    };
    format!("{display_name} was not found on this machine. {after}")
}

/// signInCommand: the line a person runs once to sign in, or to sign in again as this account or another.
pub fn sign_in_command(id: &str, again: bool) -> Option<String> {
    let facts = runtime_install_facts(id)?;
    let line = if again { facts.sign_in_again } else { facts.sign_in }?;
    Some(match (again, facts.sign_in_again_note) {
        (true, Some(note)) => format!("run {line}, {note}"),
        _ => format!("run {line}"),
    })
}

/// signInArgs: the words run after the agent's own program to sign in (or sign in again), or `None` for a runtime with
/// no account. Claude Code and Codex sign in from their own first screen, so theirs is no words at all.
pub fn sign_in_args(id: &str, again: bool) -> Option<Vec<String>> {
    let facts = runtime_install_facts(id)?;
    let line = if again { facts.sign_in_again } else { facts.sign_in }?;
    Some(line.split_whitespace().skip(1).map(str::to_owned).collect())
}

/// SignInButton's `SIGNS_OUT_AT_START`: runtimes whose sign-in command removes the current sign-in the moment it starts
/// (measured on Codex, 10/02, on a throwaway CODEX_HOME).
const SIGNS_OUT_AT_START: [&str; 1] = ["codex"];

/// What the row says before signing in again opens a window (SignInButton, 0.545): closing that window unanswered
/// can leave the agent signed out, and for Codex it does.
pub fn sign_in_again_warning(id: &str, display_name: &str) -> String {
    if SIGNS_OUT_AT_START.contains(&id) {
        format!("{display_name} signs out as soon as this opens, and stays signed out until you finish there.")
    } else {
        format!("Until you finish signing in there, {display_name} may stay signed out.")
    }
}

/// signInOpenedLine: what the UI shows after opening the sign-in command in a terminal.
pub fn sign_in_opened_line(id: &str, again: bool) -> String {
    let note = if again {
        runtime_install_facts(id).and_then(|facts| facts.sign_in_again_note)
    } else {
        None
    };
    match note {
        Some(note) => {
            let trimmed = note.strip_prefix("then ").unwrap_or(note);
            format!("In the window that opened, {trimmed}.")
        }
        None => "Finish in the window that opened.".to_owned(),
    }
}

// ---- discovery (runtime-adapters discovery.ts, main's publicStatus) ------------------------------------------------

/// What one check answered. `None` from a runner means the program could not be started at all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    pub exit: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl Answer {
    fn text(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// succeeded: exit 0 and not cut off.
fn succeeded(a: &Option<Answer>) -> bool {
    a.as_ref().is_some_and(|a| a.exit == Some(0) && !a.timed_out)
}

/// What discovery may touch, passed in so a test never runs a real agent or reads a real home folder. Each may be
/// asked from several threads at once: one agent's checks run together.
pub struct Seams<'a> {
    /// Where a command is, if it is installed.
    pub find: &'a (dyn Fn(&str) -> Option<std::path::PathBuf> + Sync),
    /// Run it with these arguments (bounded by the caller).
    pub run: &'a (dyn Fn(&std::path::Path, &[&str]) -> Option<Answer> + Sync),
    /// Whether a file exists. Only ever asked about a credentials file's presence, never its contents.
    pub exists: &'a (dyn Fn(&std::path::Path) -> bool + Sync),
    pub home: Option<std::path::PathBuf>,
    pub xdg_config_home: Option<String>,
    /// What unchanged binaries said before. None asks everything, which is what every caller but the app wants.
    pub recall: Option<Recall<'a>>,
}

/// What a binary said about itself (RuntimeBinaryFacts): its `--version` and its help, each as printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryFacts {
    pub version_text: String,
    pub capability_text: String,
}

/// Where facts about unchanged binaries are kept (RuntimeFactsCache), by fingerprint.
pub trait FactsCache: Send + Sync {
    fn get(&self, fingerprint: &str) -> Option<BinaryFacts>;
    fn set(&self, fingerprint: &str, facts: BinaryFacts);
}

/// A file's size and modification time: the two things a fingerprint is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub size: u64,
    pub modified_ns: u128,
}

/// The recall seam: the cache, how a file is stamped (following links, as `stat` does), and where a launcher really
/// points.
#[derive(Clone, Copy)]
pub struct Recall<'a> {
    pub facts: &'a dyn FactsCache,
    pub stat: &'a (dyn Fn(&std::path::Path) -> Option<Stamp> + Sync),
    pub canonical: &'a (dyn Fn(&std::path::Path) -> Option<std::path::PathBuf> + Sync),
}

/// This Mac's files, for [`Recall`]: `stat` follows a link, as Node's does.
pub fn stamp_of(path: &std::path::Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let modified_ns = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some(Stamp { size: meta.len(), modified_ns })
}

/// Every file a launch depends on, in a stable order (fingerprintPaths): the launcher found on PATH, and the file it
/// really is -- an npm bin on a Mac is a link to the CLI's own script, which is the file an update rewrites. A script
/// launcher (`.cmd`, `.bat`, `.ps1`: a Windows shape) also brings its folder, a `versions` folder in it and Muse's
/// version file, since the CLI behind it updates itself while the launcher stays put.
fn fingerprint_paths(found: &std::path::Path, recall: &Recall) -> Vec<std::path::PathBuf> {
    let mut paths = vec![found.to_path_buf()];
    if let Some(real) = (recall.canonical)(found) {
        paths.push(real);
    }
    let script = found.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "cmd" | "bat" | "ps1"));
    if script && let Some(folder) = found.parent() {
        paths.extend([folder.to_path_buf(), folder.join("versions"), folder.join(".muse-version")]);
    }
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| !p.as_os_str().is_empty() && seen.insert(p.clone()));
    paths
}

/// fingerprintOf: each file's path, size and time. A file that cannot be stamped is skipped; when none can be, there
/// is no fingerprint at all -- the absence of a fact is never taken as the fact being unchanged.
pub fn fingerprint_of(found: &std::path::Path, recall: &Recall) -> Option<String> {
    let parts: Vec<String> = fingerprint_paths(found, recall)
        .iter()
        .filter_map(|p| (recall.stat)(p).map(|s| format!("{}:{}:{}", p.display(), s.size, s.modified_ns)))
        .collect();
    (!parts.is_empty()).then(|| parts.join("|"))
}

/// One agent's discovery definition (IntegrationDefinition).
struct Definition {
    id: &'static str,
    name: &'static str,
    command: &'static str,
    version: &'static [&'static str],
    capability: &'static [&'static str],
    readiness: &'static [&'static str],
    models: Option<&'static [&'static str]>,
    list_answers_readiness: bool,
    ready_when: fn(&str) -> bool,
    /// Where its credentials file is, under home and under XDG_CONFIG_HOME, for an agent with no free sign-in check.
    credentials: Option<(&'static [&'static str], &'static [&'static str])>,
    required: &'static [&'static str],
    unsupported: fn(&crate::runtime::version::Version) -> Option<String>,
}

fn always(_: &str) -> bool {
    true
}

fn never_unsupported(_: &crate::runtime::version::Version) -> Option<String> {
    None
}

fn re(pattern: &'static str) -> regex::Regex {
    crate::invariants::regex(pattern)
}

const DEFINITIONS: &[Definition] = &[
    Definition {
        id: "codex", name: "Codex CLI", command: "codex", version: &["--version"], capability: &["exec", "--help"],
        readiness: &["login", "status"], models: None, list_answers_readiness: false, ready_when: always, credentials: None,
        required: &["non-interactive", "jsonl-events", "stdin-prompt", "workspace-selection", "read-only-sandbox"],
        unsupported: never_unsupported,
    },
    Definition {
        id: "claude", name: "Claude Code", command: "claude", version: &["--version"], capability: &["--help"],
        readiness: &["auth", "status"], models: None, list_answers_readiness: false, ready_when: always, credentials: None,
        required: &["non-interactive", "jsonl-events", "stdin-prompt", "restricted-mode", "verbose-streaming", "partial-messages", "plan-permission-mode", "tool-allowlist", "tool-denylist"],
        unsupported: never_unsupported,
    },
    Definition {
        id: "cursor", name: "Cursor Agent", command: "cursor-agent", version: &["--version"], capability: &["--help"],
        readiness: &["status"], models: Some(&["--list-models"]), list_answers_readiness: false,
        ready_when: |t| !re(r"(?i)not logged in").is_match(t), credentials: None,
        required: &["non-interactive", "jsonl-events", "partial-messages", "plan-permission-mode", "workspace-selection"],
        unsupported: never_unsupported,
    },
    Definition {
        id: "gemini", name: "Gemini CLI", command: "gemini", version: &["--version"], capability: &["--help"],
        readiness: &["--list-sessions"], models: None, list_answers_readiness: false,
        ready_when: |t| !re(r"(?i)error authenticating|please set an auth method").is_match(t), credentials: None,
        required: &["non-interactive", "jsonl-events", "stdin-prompt", "plan-permission-mode"],
        unsupported: never_unsupported,
    },
    Definition {
        id: "opencode", name: "OpenCode", command: "opencode", version: &["--version"], capability: &["run", "--help"],
        readiness: &["models"], models: Some(&["models", "--verbose"]), list_answers_readiness: true,
        ready_when: |t| crate::models::parse_opencode_model_list(t).is_some(), credentials: None,
        required: &["non-interactive", "jsonl-events"],
        unsupported: |v| {
            (v.major >= 3).then(|| {
                format!(
                    "OpenCode {} is installed; this Locust supports OpenCode 1 and OpenCode 2. Use a supported OpenCode 1 or OpenCode 2 release, or update Locust for this version.",
                    v.version
                )
            })
        },
    },
    Definition {
        id: "copilot", name: "Copilot CLI", command: "copilot", version: &["--version"], capability: &["--help"],
        readiness: &["--version"], models: None, list_answers_readiness: false,
        ready_when: |t| re(r"(?i)copilot cli").is_match(t), credentials: None,
        required: &["non-interactive", "jsonl-events", "tool-allowlist", "tool-denylist"],
        unsupported: never_unsupported,
    },
    Definition {
        id: "muse", name: "Muse Code", command: "muse", version: &["--version"], capability: &["exec", "--help"],
        readiness: &["--version"], models: None, list_answers_readiness: false,
        ready_when: |t| re(r"(?i)muse code\s+\d").is_match(t),
        credentials: Some((&[".config", "muse", "auth.json"], &["muse", "auth.json"])),
        required: &["non-interactive", "jsonl-events"],
        unsupported: never_unsupported,
    },
    Definition {
        id: "kilo", name: "Kilo", command: "kilo", version: &["--version"], capability: &["acp", "--help"],
        readiness: &["--version"], models: None, list_answers_readiness: false,
        ready_when: |t| re(r"\d+\.\d+\.\d+").is_match(t), credentials: None,
        required: &["acp-stdio"],
        unsupported: never_unsupported,
    },
];

/// detectSupportedFeatures: what an agent's own help text says it can do, by the exact flags Locust passes.
fn supported_features(id: &str, help: &str) -> Vec<&'static str> {
    let lower = help.to_lowercase();
    let scan = |token: &str| lower.contains(&token.to_lowercase());
    let mut features = Vec::new();
    let mut add = |feature: &'static str, ok: bool| {
        if ok {
            features.push(feature);
        }
    };
    match id {
        "codex" => {
            add("non-interactive", true);
            add("jsonl-events", scan("--json"));
            add("stdin-prompt", scan("stdin"));
            add("workspace-selection", scan("--cd") || scan("-C"));
            add("read-only-sandbox", scan("--sandbox") && scan("read-only"));
        }
        "claude" => {
            add("non-interactive", scan("--print") || scan("-p"));
            add("jsonl-events", scan("--output-format") && scan("stream-json"));
            add("stdin-prompt", scan("stdin"));
            add("restricted-mode", scan("--restricted"));
            add("verbose-streaming", scan("--verbose"));
            add("partial-messages", scan("--include-partial-messages"));
            add("plan-permission-mode", scan("--permission-mode"));
            add("tool-allowlist", scan("--tools"));
            add("tool-denylist", scan("--disallowedTools"));
        }
        "cursor" => {
            add("non-interactive", scan("--print"));
            add("jsonl-events", scan("--output-format") && scan("stream-json"));
            add("partial-messages", scan("--stream-partial-output"));
            add("plan-permission-mode", scan("--mode") && scan("plan"));
            add("workspace-selection", scan("--workspace"));
        }
        "gemini" => {
            add("non-interactive", scan("--prompt"));
            add("jsonl-events", scan("--output-format") && scan("stream-json"));
            add("stdin-prompt", scan("stdin"));
            add("plan-permission-mode", scan("--approval-mode") && scan("plan"));
        }
        "opencode" => {
            add("non-interactive", scan("run"));
            add("jsonl-events", scan("--format") && scan("json"));
        }
        "copilot" => {
            add("non-interactive", scan("--prompt") || scan("-p"));
            add("jsonl-events", scan("--output-format") && scan("json"));
            add("tool-allowlist", scan("--allow-all-tools"));
            add("tool-denylist", scan("--deny-tool"));
        }
        "muse" => {
            add("non-interactive", scan("exec"));
            add("jsonl-events", scan("--json"));
            add("workspace-selection", scan("--workspace"));
            add("read-only-sandbox", scan("--disable-write") && scan("--disable-shell"));
        }
        "kilo" => add("acp-stdio", re(r"(?i)\bagent[\s-]*client[\s-]*protocol\b").is_match(help)),
        _ => add("json-health-check", scan("--json")),
    }
    features
}

/// saysSignedOut: for the agents whose readiness command IS a sign-in check, its failing is the statement; for the
/// rest, only the agent's own words about signing in. OpenCode has no account to be out of.
fn says_signed_out(id: &str, answer: &Answer) -> bool {
    if id == "opencode" {
        return false;
    }
    if matches!(id, "codex" | "claude" | "cursor" | "gemini") {
        return true;
    }
    re(r"(?i)sign(?:ed)?[ -]?(?:in|out)|log(?:ged)?[ -]?(?:in|out)|login|auth|credential|api[ _-]?key|unauthori[sz]ed|subscription|not entitled|account|ineligible")
        .is_match(&answer.text())
}

/// Agents Locust cannot start a turn on are not asked past their version (readinessFromVersion: the planned ones).
fn readiness_from_version(id: &str) -> bool {
    integration_of(id) == Integration::Planned
}

/// The agents discovery looks for, in Electron's order.
pub fn known() -> impl Iterator<Item = (&'static str, &'static str)> {
    DEFINITIONS.iter().map(|d| (d.id, d.name))
}

/// The command discovery looks for, for an agent it knows.
pub fn command_of(id: &str) -> Option<&'static str> {
    DEFINITIONS.iter().find(|d| d.id == id).map(|d| d.command)
}

/// An agent that is not on this machine, as publicStatus says it.
fn not_installed(d: &Definition) -> Found {
    Found {
        id: d.id.into(),
        display_name: d.name.into(),
        installed: false,
        version: None,
        auth: if matches!(d.id, "opencode" | "kilo") { Auth::NotApplicable } else { Auth::Unknown },
        ready: false,
        status: Probe::Missing,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

/// One agent, found or not, as main's publicStatus says it.
pub fn discover_one(id: &str, seams: &Seams) -> Option<Found> {
    let d = DEFINITIONS.iter().find(|d| d.id == id)?;
    let not_applicable = matches!(d.id, "opencode" | "kilo");
    let Some(path) = (seams.find)(d.command) else {
        return Some(not_installed(d));
    };
    let run = |args: &[&str]| (seams.run)(&path, args);
    // What this binary already said, if it is the same binary: only version and help, the two answers that are a
    // function of the file. Readiness is always asked, because signing in changes nothing on disk.
    let fingerprint = seams.recall.as_ref().and_then(|r| fingerprint_of(&path, r));
    let remembered = seams.recall.as_ref().zip(fingerprint.as_ref()).and_then(|(r, fp)| r.facts.get(fp));
    let said = |text: &str| Some(Answer { exit: Some(0), stdout: text.to_owned(), stderr: String::new(), timed_out: false });
    let same_command = d.models == Some(d.readiness);
    let readiness_is_version = readiness_from_version(d.id) || d.readiness == d.version;
    // All of one agent's checks at once (discoverOne's Promise.all): none needs another's output, so together they cost
    // the slowest, not the sum. A list that is also the readiness answer is asked first on its own thread, and the
    // plain readiness command only when the list did not answer.
    let (version_answer, capability_answer, _listed, readiness_answer) = std::thread::scope(|s| {
        let version = s.spawn(|| match &remembered {
            Some(f) => said(&f.version_text),
            None => run(d.version),
        });
        let capability = s.spawn(|| match &remembered {
            Some(f) => said(&f.capability_text),
            None => run(d.capability),
        });
        let list_then_ready = d.models.filter(|_| !same_command).map(|models| {
            let reads_ready = d.list_answers_readiness && !readiness_is_version;
            s.spawn(move || {
                let list = run(models);
                let ready = reads_ready.then(|| {
                    if succeeded(&list) && list.as_ref().is_some_and(|a| (d.ready_when)(&a.text())) { list.clone() } else { run(d.readiness) }
                });
                (list, ready)
            })
        });
        let reads_from_list = d.list_answers_readiness && list_then_ready.is_some();
        let readiness = (!readiness_is_version && !reads_from_list).then(|| s.spawn(|| run(d.readiness)));
        let version_answer = version.join().ok().flatten();
        let capability_answer = capability.join().ok().flatten();
        let (listed, from_list) = match list_then_ready.map(|h| h.join()) {
            Some(Ok((list, ready))) => (Some(list), ready),
            _ => (None, None),
        };
        let readiness_answer = if readiness_is_version {
            version_answer.clone()
        } else if let Some(h) = readiness {
            h.join().ok().flatten()
        } else {
            from_list.flatten()
        };
        (version_answer, capability_answer, listed, readiness_answer)
    });
    // Kept only when BOTH answered, and only from a real check: a CLI that timed out once is not versionless for good.
    if remembered.is_none()
        && let (Some(r), Some(fp)) = (seams.recall.as_ref(), fingerprint.as_ref())
        && succeeded(&version_answer)
        && succeeded(&capability_answer)
    {
        let text = |a: &Option<Answer>| a.as_ref().map(Answer::text).unwrap_or_default();
        r.facts.set(fp, BinaryFacts { version_text: text(&version_answer), capability_text: text(&capability_answer) });
    }
    let version = succeeded(&version_answer)
        .then(|| crate::runtime::version::parse(&version_answer.as_ref().map(Answer::text).unwrap_or_default()))
        .flatten();
    let features = if succeeded(&capability_answer) {
        supported_features(d.id, &capability_answer.as_ref().map(Answer::text).unwrap_or_default())
    } else {
        Vec::new()
    };
    let missing = d.required.iter().any(|f| !features.contains(f));
    let refused = version.as_ref().and_then(d.unsupported);

    #[derive(PartialEq)]
    enum Readiness {
        Ready,
        AuthRequired,
        Unsupported,
        Unhealthy,
    }
    let readiness = if missing || refused.is_some() {
        Readiness::Unsupported
    } else if missing_credentials(d, seams) {
        Readiness::AuthRequired
    } else if succeeded(&readiness_answer) && readiness_answer.as_ref().is_some_and(|a| (d.ready_when)(&a.text())) {
        Readiness::Ready
    } else if readiness_answer.as_ref().is_some_and(|a| !a.timed_out && says_signed_out(d.id, a)) {
        Readiness::AuthRequired
    } else {
        Readiness::Unhealthy
    };
    let auth = if not_applicable {
        Auth::NotApplicable
    } else {
        match readiness {
            Readiness::Ready => Auth::Authenticated,
            Readiness::AuthRequired => Auth::Unauthenticated,
            _ => Auth::Unknown,
        }
    };
    let status = if refused.is_some() {
        Probe::Unsupported
    } else {
        match readiness {
            Readiness::Ready => Probe::Ready,
            Readiness::AuthRequired => Probe::AuthRequired,
            Readiness::Unhealthy => Probe::Offline,
            Readiness::Unsupported => Probe::ProbeFailed,
        }
    };
    Some(Found {
        id: d.id.into(),
        display_name: d.name.into(),
        installed: true,
        version: version.map(|v| v.version),
        auth,
        ready: status == Probe::Ready,
        status,
        unsupported_note: refused,
        through_cli: false,
        checking: false,
    })
}

/// missingCredentials: the credentials file of an agent with no free sign-in check is not there. Asked only whether
/// the file exists.
fn missing_credentials(d: &Definition, seams: &Seams) -> bool {
    let Some((home_path, xdg_path)) = d.credentials else { return false };
    let path = match seams.xdg_config_home.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
        Some(xdg) => xdg_path.iter().fold(std::path::PathBuf::from(xdg), |p, part| p.join(part)),
        None => match &seams.home {
            Some(home) => home_path.iter().fold(home.clone(), |p, part| p.join(part)),
            None => return false,
        },
    };
    !(seams.exists)(&path)
}

/// Antigravity, found apart from the others as Electron's antigravity-host does: `agy --version` and `agy models`,
/// each on its own ten-second clock. A model list is "ready"; an answer with no list is "signed out"; no answer in
/// time is "not confirmed", never "signed out". It runs through its own CLI, a published route.
pub fn discover_antigravity(seams: &Seams) -> Found {
    let Some(path) = (seams.find)("agy") else { return antigravity_found(false, None, Auth::Unknown, Probe::Missing, false) };
    let version_ans = (seams.run)(&path, &["--version"]);
    let models_ans = (seams.run)(&path, &["models"]);
    antigravity_from_answers(version_ans, models_ans)
}

fn antigravity_found(installed: bool, version: Option<String>, auth: Auth, status: Probe, checking: bool) -> Found {
    Found {
        id: "antigravity".into(),
        display_name: "Antigravity".into(),
        installed,
        version,
        auth,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: installed,
        checking,
    }
}

fn antigravity_from_answers(version_ans: Option<Answer>, models_ans: Option<Answer>) -> Found {
    let version = version_ans
        .filter(|a| a.exit == Some(0) && !a.timed_out)
        .and_then(|a| a.stdout.trim().lines().next().and_then(|l| crate::runtime::version::parse(l)).map(|v| v.version));
    match models_ans {
        None => antigravity_found(true, version, Auth::Unknown, Probe::ProbeFailed, false),
        Some(a) if a.timed_out => antigravity_found(true, version, Auth::Unknown, Probe::ProbeFailed, false),
        Some(a) if crate::models::parse_agy_model_list(&a.stdout).is_some() => {
            antigravity_found(true, version, Auth::Authenticated, Probe::Ready, false)
        }
        Some(_) => antigravity_found(true, version, Auth::Unauthenticated, Probe::AuthRequired, false),
    }
}

/// Runs `agy --version` and `agy models` concurrently so the probe costs the slower one, not both.
pub fn discover_antigravity_concurrent(
    find: &(dyn Fn(&str) -> Option<std::path::PathBuf> + Sync),
    run: &(dyn Fn(&std::path::Path, &[&str]) -> Option<Answer> + Sync),
) -> Found {
    let Some(path) = find("agy") else {
        return antigravity_found(false, None, Auth::Unknown, Probe::Missing, false);
    };
    let (version_ans, models_ans) = std::thread::scope(|s| {
        let v = s.spawn(|| run(&path, &["--version"]));
        let m = s.spawn(|| run(&path, &["models"]));
        (v.join().ok().flatten(), m.join().ok().flatten())
    });
    antigravity_from_answers(version_ans, models_ans)
}

/// Coalesces concurrent Antigravity checks so a second caller shares the in-flight check instead of spawning another `agy`.
#[derive(Default)]
pub struct AntigravityProbe {
    in_flight: std::sync::Mutex<Option<std::sync::Arc<OnceProbe>>>,
}

struct OnceProbe {
    done: std::sync::Mutex<Option<Found>>,
    cv: std::sync::Condvar,
}

impl AntigravityProbe {
    pub fn pending_record(&self, installed: bool) -> Found {
        antigravity_found(installed, None, Auth::Unknown, Probe::ProbeFailed, installed)
    }

    pub fn discovery_record(
        &self,
        find: &(dyn Fn(&str) -> Option<std::path::PathBuf> + Sync),
        run: &(dyn Fn(&std::path::Path, &[&str]) -> Option<Answer> + Sync),
    ) -> Found {
        let (cell, leader) = {
            let mut guard = crate::invariants::lock(&self.in_flight);
            if let Some(existing) = guard.as_ref() {
                (existing.clone(), false)
            } else {
                let cell = std::sync::Arc::new(OnceProbe {
                    done: std::sync::Mutex::new(None),
                    cv: std::sync::Condvar::new(),
                });
                *guard = Some(cell.clone());
                (cell, true)
            }
        };
        if !leader {
            let mut done = crate::invariants::lock(&cell.done);
            while done.is_none() {
                done = cell.cv.wait(done).unwrap_or_else(|e| e.into_inner());
            }
            return done.clone().expect("notified with result");
        }
        let result = discover_antigravity_concurrent(find, run);
        {
            let mut done = crate::invariants::lock(&cell.done);
            *done = Some(result.clone());
            cell.cv.notify_all();
        }
        let mut guard = crate::invariants::lock(&self.in_flight);
        if guard.as_ref().is_some_and(|g| std::sync::Arc::ptr_eq(g, &cell)) {
            *guard = None;
        }
        result
    }
}

/// How long one check may take before it counts as not answering (PROBE_TIMEOUT_MS).
pub const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// How long the first screen waits for agents, from the sweep starting (Electron `FIRST_SCREEN_DEADLINE_MS`).
pub const FIRST_SCREEN_DEADLINE: std::time::Duration = std::time::Duration::from_millis(2_000);

/// Placeholder for an agent that is installed and whose check has not finished yet.
pub fn pending_found(id: &str, display_name: &str) -> Found {
    Found {
        id: id.into(),
        display_name: display_name.into(),
        installed: true,
        version: None,
        auth: Auth::Unknown,
        ready: false,
        status: Probe::ProbeFailed,
        unsupported_note: None,
        through_cli: false,
        checking: true,
    }
}

/// One agent's check in a bounded sweep (Electron `SweepCheck`).
pub struct SweepCheck {
    pub id: String,
    pub run: Box<dyn FnOnce(Box<dyn FnOnce() + Send>) -> Option<Found> + Send>,
    pub pending: Box<dyn Fn() -> Found + Send + Sync>,
}

/// Handle to a check that is still running after the deadline.
#[derive(Clone)]
pub struct CheckHandle {
    pub id: String,
    rx: std::sync::Arc<(std::sync::Mutex<Option<Option<Found>>>, std::sync::Condvar)>,
}

impl CheckHandle {
    pub fn wait(&self) -> Option<Found> {
        let (lock, cv) = &*self.rx;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        while guard.is_none() {
            guard = cv.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
        guard.as_ref().cloned().flatten()
    }
}

/// Result of [`settle_within`]: immediate answers in order, and handles for checks that ran past the deadline.
pub struct Settled {
    pub answers: Vec<Found>,
    pub left: std::collections::HashMap<String, CheckHandle>,
}

/// Wait up to `deadline` for `checks` to finish, or as soon as all have answered. Any check still running past the
/// deadline waits only for its PATH lookup (`located`), is shown via `pending()`, and delivers its eventual answer to
/// `late` once.
pub fn settle_within(
    checks: Vec<SweepCheck>,
    deadline: std::time::Duration,
    late: impl Fn(&str, Option<&Found>) + Send + Sync + 'static,
) -> Settled {
    use std::sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, Ordering}};

    struct EntryState {
        id: String,
        pending: Box<dyn Fn() -> Found + Send + Sync>,
        done: AtomicBool,
        gone: AtomicBool,
        located: Arc<(Mutex<bool>, Condvar)>,
        result: Arc<(Mutex<Option<Option<Found>>>, Condvar)>,
    }

    let late = Arc::new(late);
    let all_done = Arc::new((Mutex::new(0usize), Condvar::new()));
    let total = checks.len();
    let mut entries = Vec::with_capacity(total);

    for check in checks {
        let state = Arc::new(EntryState {
            id: check.id,
            pending: check.pending,
            done: AtomicBool::new(false),
            gone: AtomicBool::new(false),
            located: Arc::new((Mutex::new(false), Condvar::new())),
            result: Arc::new((Mutex::new(None), Condvar::new())),
        });
        entries.push(Arc::clone(&state));

        let entry = Arc::clone(&state);
        let all_done = Arc::clone(&all_done);
        let late = Arc::clone(&late);
        let run = check.run;
        std::thread::spawn(move || {
            let loc = Arc::clone(&entry.located);
            let mark_located = Box::new(move || {
                let (lock, cv) = &*loc;
                let mut g = lock.lock().unwrap_or_else(|e| e.into_inner());
                *g = true;
                cv.notify_all();
            });
            let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(mark_located))).unwrap_or(None);
            // Ensure `located` is unblocked even if the check returned/panicked before calling it.
            {
                let (lock, cv) = &*entry.located;
                let mut g = lock.lock().unwrap_or_else(|e| e.into_inner());
                if !*g {
                    *g = true;
                    cv.notify_all();
                }
            }
            {
                let (lock, cv) = &*entry.result;
                let mut g = lock.lock().unwrap_or_else(|e| e.into_inner());
                *g = Some(answer.clone());
                cv.notify_all();
            }
            entry.done.store(true, Ordering::Release);
            {
                let (lock, cv) = &*all_done;
                let mut count = lock.lock().unwrap_or_else(|e| e.into_inner());
                *count += 1;
                if *count >= total {
                    cv.notify_all();
                }
            }
            if entry.gone.load(Ordering::Acquire) {
                late(&entry.id, answer.as_ref());
            }
        });
    }

    // Wait until either all checks finish or the deadline expires.
    if total > 0 {
        let (lock, cv) = &*all_done;
        let mut count = lock.lock().unwrap_or_else(|e| e.into_inner());
        let start = std::time::Instant::now();
        while *count < total {
            let elapsed = start.elapsed();
            if elapsed >= deadline {
                break;
            }
            let (next, res) = cv.wait_timeout(count, deadline - elapsed).unwrap_or_else(|e| e.into_inner());
            count = next;
            if res.timed_out() {
                break;
            }
        }
    }

    // Wait only for PATH lookups of checks that have not finished yet.
    for entry in &entries {
        if !entry.done.load(Ordering::Acquire) {
            let (lock, cv) = &*entry.located;
            let mut g = lock.lock().unwrap_or_else(|e| e.into_inner());
            while !*g {
                g = cv.wait(g).unwrap_or_else(|e| e.into_inner());
            }
        }
    }

    let mut answers = Vec::with_capacity(total);
    let mut left = std::collections::HashMap::new();
    for entry in entries {
        if entry.done.load(Ordering::Acquire) {
            let answer = entry.result.0.lock().unwrap_or_else(|e| e.into_inner()).clone().flatten();
            if let Some(found) = answer {
                answers.push(found);
            }
        } else {
            entry.gone.store(true, Ordering::Release);
            // If it finished in the small window between `done` check and `gone` store, deliver `late` once.
            if entry.done.load(Ordering::Acquire) {
                let answer = entry.result.0.lock().unwrap_or_else(|e| e.into_inner()).clone().flatten();
                late(&entry.id, answer.as_ref());
                if let Some(found) = answer {
                    answers.push(found);
                }
            } else {
                answers.push((entry.pending)());
                left.insert(
                    entry.id.clone(),
                    CheckHandle {
                        id: entry.id.clone(),
                        rx: Arc::clone(&entry.result),
                    },
                );
            }
        }
    }

    Settled { answers, left }
}

/// Keep an agent's last definite answer when a later sweep goes on without it (Electron `keepWhatWasAnswered`).
pub fn keep_what_was_answered(settled: &Settled, before: &[Found]) -> Vec<Found> {
    let last: std::collections::HashMap<&str, &Found> = before.iter().map(|f| (f.id.as_str(), f)).collect();
    settled
        .answers
        .iter()
        .map(|entry| {
            if !settled.left.contains_key(&entry.id) {
                return entry.clone();
            }
            if let Some(was) = last.get(entry.id.as_str())
                && was.installed
                && matches!(was.status, Probe::Ready | Probe::AuthRequired)
                && !was.checking
            {
                return (*was).clone();
            }
            entry.clone()
        })
        .collect()
}

/// Whether the sweep still has an installed agent that has not answered yet (Electron `listsItsModelsButListedNone`).
pub fn sweep_has_pending_agent(runtimes: &[Found]) -> bool {
    runtimes.iter().any(|entry| {
        entry.installed
            && ((entry.id == "antigravity" && entry.status == Probe::Unknown) || entry.checking)
    })
}

/// Checks the sweep went on without, held so a start on that agent waits for its own check and no other.
#[derive(Default)]
pub struct OutstandingChecks {
    held: std::collections::HashMap<String, CheckHandle>,
}

impl OutstandingChecks {
    pub fn set(&mut self, handle: CheckHandle) {
        self.held.insert(handle.id.clone(), handle);
    }

    pub fn get(&self, id: &str) -> Option<&CheckHandle> {
        self.held.get(id)
    }

    pub fn has(&self, id: &str) -> bool {
        self.held.contains_key(id)
    }

    pub fn delete(&mut self, id: &str) {
        self.held.remove(id);
    }

    pub fn for_start(&self, runtime_id: Option<&str>, found: &[Found]) -> Vec<Found> {
        let Some(runtime_id) = runtime_id else {
            return found.to_vec();
        };
        let Some(checking) = self.held.get(runtime_id) else {
            return found.to_vec();
        };
        if let Some(shown) = found.iter().find(|entry| entry.id == runtime_id)
            && !shown.checking
        {
            return found.to_vec();
        }
        let Some(answer) = checking.wait() else {
            return found.to_vec();
        };
        found
            .iter()
            .map(|entry| if entry.id == runtime_id { answer.clone() } else { entry.clone() })
            .collect()
    }
}

/// Run a check, bounded: its output, or a timed-out answer once `limit` passes (the check is then ended).
fn run_bounded(path: &std::path::Path, args: &[&str], limit: std::time::Duration) -> Option<Answer> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(path).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let (mut out, mut err) = (child.stdout.take()?, child.stderr.take()?);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut o, mut e) = (String::new(), String::new());
        let _ = out.read_to_string(&mut o);
        let _ = err.read_to_string(&mut e);
        let _ = tx.send((o, e));
    });
    match rx.recv_timeout(limit) {
        Ok((stdout, stderr)) => {
            let exit = child.wait().ok().and_then(|s| s.code());
            Some(Answer { exit, stdout, stderr, timed_out: false })
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Some(Answer { exit: None, stdout: String::new(), stderr: String::new(), timed_out: true })
        }
    }
}

/// One sweep's watcher (RuntimeProbeWatcher), told as each agent's check begins and ends so a screen can show the
/// wait. `started` comes BEFORE the agent is looked for or any CLI is started: told both at once, a screen has nothing
/// to draw during the wait. `bin` is the command, which is not always the id (Cursor's is `cursor-agent`).
pub trait Watch: Send + Sync {
    fn started(&self, id: &str, display_name: &str, bin: &str);
    fn finished(&self, id: &str, found: &Found);
}

/// What a sweep's checks may touch, shared by every check's own thread: this Mac ([`Machine::this_mac`]) or a test's.
#[derive(Clone)]
pub struct Machine {
    pub find: std::sync::Arc<dyn Fn(&str) -> Option<std::path::PathBuf> + Send + Sync>,
    pub run: std::sync::Arc<dyn Fn(&std::path::Path, &[&str]) -> Option<Answer> + Send + Sync>,
    pub exists: std::sync::Arc<dyn Fn(&std::path::Path) -> bool + Send + Sync>,
    pub home: Option<std::path::PathBuf>,
    pub xdg_config_home: Option<String>,
    /// What unchanged binaries said before; None asks every binary everything.
    pub facts: Option<std::sync::Arc<dyn FactsCache>>,
    pub stat: std::sync::Arc<dyn Fn(&std::path::Path) -> Option<Stamp> + Send + Sync>,
    pub canonical: std::sync::Arc<dyn Fn(&std::path::Path) -> Option<std::path::PathBuf> + Send + Sync>,
}

impl<T: FactsCache + ?Sized> FactsCache for &T {
    fn get(&self, fingerprint: &str) -> Option<BinaryFacts> {
        (**self).get(fingerprint)
    }
    fn set(&self, fingerprint: &str, facts: BinaryFacts) {
        (**self).set(fingerprint, facts)
    }
}

impl Machine {
    /// This Mac: PATH as the window adopted it, every check bounded by PROBE_TIMEOUT, and what the CLIs said kept in
    /// `runtime-facts.json`.
    pub fn this_mac() -> Self {
        Machine {
            find: std::sync::Arc::new(|c: &str| crate::runtime::discovery::find_named(c).ok()),
            run: std::sync::Arc::new(|p: &std::path::Path, a: &[&str]| run_bounded(p, a, PROBE_TIMEOUT)),
            exists: std::sync::Arc::new(|p: &std::path::Path| p.exists()),
            home: std::env::var_os("HOME").map(std::path::PathBuf::from),
            xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
            facts: crate::runtime_facts::shared().map(|s| std::sync::Arc::new(s) as std::sync::Arc<dyn FactsCache>),
            stat: std::sync::Arc::new(stamp_of),
            canonical: std::sync::Arc::new(|p: &std::path::Path| std::fs::canonicalize(p).ok()),
        }
    }

    fn seams<'a>(&'a self, find: &'a (dyn Fn(&str) -> Option<std::path::PathBuf> + Sync)) -> Seams<'a> {
        Seams {
            find,
            run: &*self.run,
            exists: &*self.exists,
            home: self.home.clone(),
            xdg_config_home: self.xdg_config_home.clone(),
            recall: self.facts.as_deref().map(|facts| Recall { facts, stat: &*self.stat, canonical: &*self.canonical }),
        }
    }
}

/// How a sweep asks (DiscoverInstalledRuntimesOptions): the beat between starts, who is told, and which agents.
#[derive(Clone, Default)]
pub struct EachOptions {
    /// Between one check STARTING and the next: each waits for the one before it to finish OR for this, whichever comes
    /// first, so slow agents still begin a readable beat apart and instant ones chain straight through. Zero starts
    /// them all at once.
    pub stagger: std::time::Duration,
    pub watch: Option<std::sync::Arc<dyn Watch>>,
    /// Ask only these.
    pub only: Option<std::collections::HashSet<String>>,
    /// Do not ask these: a check for them is already going, and a second beside it would start the same CLI twice.
    pub skip: std::collections::HashSet<String>,
}

/// A gate a check waits on, opened by the check before it.
struct Gate {
    open: std::sync::Mutex<bool>,
    cv: std::sync::Condvar,
}

impl Gate {
    fn new(open: bool) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Gate { open: std::sync::Mutex::new(open), cv: std::sync::Condvar::new() })
    }
    fn open(&self) {
        *crate::invariants::lock(&self.open) = true;
        self.cv.notify_all();
    }
    fn wait(&self) {
        let mut open = crate::invariants::lock(&self.open);
        while !*open {
            open = self.cv.wait(open).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// Opens the next check's gate however this one ends, a panic included: no check is ever left waiting on another.
struct OpensOnDrop(std::sync::Arc<Gate>);

impl Drop for OpensOnDrop {
    fn drop(&mut self) {
        self.0.open();
    }
}

/// discoverInstalledRuntimesEach: each agent's check on its own, for [`settle_within`], in Electron's order. A check
/// that has not answered can still be SHOWN -- installed, being checked -- because the one cheap step, finding the
/// file, is done first; one that is not installed says so at once.
pub fn discover_each(machine: &Machine, options: &EachOptions) -> Vec<SweepCheck> {
    use std::sync::{Arc, Mutex};
    let mut gate = Gate::new(true);
    let mut checks = Vec::new();
    for d in DEFINITIONS
        .iter()
        .filter(|d| options.only.as_ref().is_none_or(|only| only.contains(d.id)))
        .filter(|d| !options.skip.contains(d.id))
    {
        let mine = gate.clone();
        let next = Gate::new(false);
        gate = next.clone();
        // What the PATH search found, kept so a check still going is shown as installed, and one not found as not.
        let seen: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
        let shown = seen.clone();
        let (machine, watch, stagger) = (machine.clone(), options.watch.clone(), options.stagger);
        checks.push(SweepCheck {
            id: d.id.into(),
            run: Box::new(move |located| {
                let opens = OpensOnDrop(next.clone());
                mine.wait();
                if let Some(w) = &watch {
                    w.started(d.id, d.name, d.command);
                }
                if stagger.is_zero() {
                    next.open();
                } else {
                    let beat = next.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(stagger);
                        beat.open();
                    });
                }
                let located = Mutex::new(Some(located));
                let find = |c: &str| {
                    let found = (machine.find)(c);
                    *crate::invariants::lock(&seen) = Some(found.is_some());
                    if let Some(cb) = crate::invariants::lock(&located).take() {
                        cb();
                    }
                    found
                };
                let found = discover_one(d.id, &machine.seams(&find));
                drop(opens);
                if let (Some(w), Some(f)) = (&watch, &found) {
                    w.finished(d.id, f);
                }
                found
            }),
            pending: Box::new(move || {
                if *crate::invariants::lock(&shown) == Some(false) { not_installed(d) } else { pending_found(d.id, d.name) }
            }),
        });
    }
    checks
}

/// discoverInstalledRuntimes: every check to its end, answered exactly as [`discover_each`]'s checks answer.
pub fn discover_installed(machine: &Machine, options: &EachOptions) -> Vec<Found> {
    let checks = discover_each(machine, options);
    std::thread::scope(|s| {
        let running: Vec<_> = checks.into_iter().map(|c| s.spawn(move || (c.run)(Box::new(|| {})))).collect();
        running.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    })
}

/// The name a row carries while its check is still going, for any agent a sweep asks.
fn display_name_of(id: &str) -> &'static str {
    match id {
        "antigravity" => "Antigravity",
        _ if id == crate::runtime::apple::APPLE_RUNTIME => crate::runtime::apple::APPLE_DISPLAY_NAME,
        _ => DEFINITIONS.iter().find(|d| d.id == id).map_or("", |d| d.name),
    }
}

/// Where an agent sits in the list: Electron's order, Antigravity after Copilot, Apple's model last.
fn list_rank(id: &str) -> usize {
    let mut order: Vec<&str> = DEFINITIONS.iter().map(|d| d.id).collect();
    let agy = order.iter().position(|i| *i == "copilot").map_or(order.len(), |i| i + 1);
    order.insert(agy, "antigravity");
    order.push(crate::runtime::apple::APPLE_RUNTIME);
    order.iter().position(|i| *i == id).unwrap_or(usize::MAX)
}

/// How one sweep is asked (index.ts `askRuntimes`).
pub struct SweepPlan {
    pub deadline: std::time::Duration,
    pub each: EachOptions,
    /// Checks a sweep went on without, still going: the next sweep joins them rather than starting the CLI again.
    pub outstanding: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, CheckHandle>>>,
    /// A check that answered after its sweep had gone on (index.ts `answeredLate`).
    pub late: std::sync::Arc<dyn Fn(&str, Option<&Found>) + Send + Sync>,
}

/// One sweep: the checks still going joined, every other agent asked (`extra` are the ones not on PATH's list), each
/// given until `deadline`, and those that run past it held in `outstanding` so their answers are not lost.
pub fn sweep(machine: &Machine, extra: Vec<SweepCheck>, plan: &SweepPlan) -> Settled {
    let going: Vec<CheckHandle> = crate::invariants::lock(&plan.outstanding).values().cloned().collect();
    let going_ids: std::collections::HashSet<String> = going.iter().map(|h| h.id.clone()).collect();
    let mut checks: Vec<SweepCheck> = going
        .into_iter()
        .map(|handle| {
            let id = handle.id.clone();
            SweepCheck {
                id: id.clone(),
                run: Box::new(move |located| {
                    located();
                    handle.wait()
                }),
                pending: Box::new(move || pending_found(&id, display_name_of(&id))),
            }
        })
        .collect();
    let each = EachOptions { skip: plan.each.skip.union(&going_ids).cloned().collect(), ..plan.each.clone() };
    checks.extend(discover_each(machine, &each));
    checks.extend(extra.into_iter().filter(|c| !going_ids.contains(&c.id)));
    let outstanding = plan.outstanding.clone();
    let late = plan.late.clone();
    let mut settled = settle_within(checks, plan.deadline, move |id, found| {
        // Only one check per agent is ever outstanding, so this is that one.
        crate::invariants::lock(&outstanding).remove(id);
        late(id, found);
    });
    {
        let mut held = crate::invariants::lock(&plan.outstanding);
        for (id, handle) in &settled.left {
            held.insert(id.clone(), handle.clone());
        }
    }
    settled.answers.sort_by_key(|f| list_rank(&f.id));
    settled
}

/// The held answer with one agent's late answer in it, in its place (answeredLate's patch of `discoveryCache`).
pub fn patch_held(held: &std::sync::Mutex<Option<HeldAnswer>>, found: &Found) {
    let mut held = crate::invariants::lock(held);
    if let Some(h) = held.as_mut() {
        match h.value.iter_mut().find(|e| e.id == found.id) {
            Some(entry) => *entry = found.clone(),
            None => h.value.push(found.clone()),
        }
    }
}

/// Checks this process's sweeps went on without.
fn outstanding() -> std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, CheckHandle>>> {
    static HELD: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, CheckHandle>>>> = std::sync::OnceLock::new();
    HELD.get_or_init(Default::default).clone()
}

/// The beat between one agent's check starting and the next (index.ts `staggerMs: 70`).
pub const SWEEP_STAGGER: std::time::Duration = std::time::Duration::from_millis(70);

/// Every agent, checked for real: each one found on PATH (the window adopts the terminal's PATH at launch) is asked
/// its version, its help and its sign-in check, a beat apart and overlapping, bounded by `FIRST_SCREEN_DEADLINE` so
/// one slow agent never stalls the first screen. One that answers later is patched into the held answer, and the
/// window told (`discovery_service`).
pub fn discover() -> Vec<Found> {
    discover_with_deadline(FIRST_SCREEN_DEADLINE)
}

/// Like [`discover`], with an explicit first-screen deadline.
pub fn discover_with_deadline(deadline: std::time::Duration) -> Vec<Found> {
    let machine = Machine::this_mac();
    let mut extra: Vec<SweepCheck> = Vec::with_capacity(2);
    let agy = machine.clone();
    extra.push(SweepCheck {
        id: "antigravity".into(),
        run: Box::new(move |located| {
            let located = std::sync::Mutex::new(Some(located));
            let find = |c: &str| {
                let found = (agy.find)(c);
                if let Some(cb) = crate::invariants::lock(&located).take() {
                    cb();
                }
                found
            };
            // agy's two questions at once (exec/native-agy), inside the sweep's first-screen deadline.
            Some(discover_antigravity_concurrent(&find, &*agy.run))
        }),
        pending: Box::new(|| pending_found("antigravity", "Antigravity")),
    });
    // Apple's on-device model, last: nothing to install or sign in to, it is part of the Mac.
    extra.push(SweepCheck {
        id: crate::runtime::apple::APPLE_RUNTIME.into(),
        run: Box::new(|located| {
            located();
            apple_found(&crate::runtime::apple::probe())
        }),
        pending: Box::new(|| pending_found(crate::runtime::apple::APPLE_RUNTIME, crate::runtime::apple::APPLE_DISPLAY_NAME)),
    });
    let plan = SweepPlan {
        deadline,
        each: EachOptions { stagger: SWEEP_STAGGER, ..Default::default() },
        outstanding: outstanding(),
        late: std::sync::Arc::new(answered_late),
    };
    let before = crate::invariants::lock(&HELD_DISCOVERY).as_ref().map(|h| h.value.clone()).unwrap_or_default();
    let settled = sweep(&machine, extra, &plan);
    // An agent the sweep went on without keeps its last definite answer (keepWhatWasAnswered).
    let answers = keep_what_was_answered(&settled, &before);
    held_a_sweep(&answers);
    answers
}

/// index.ts `answeredLate`: the held answer gets the late one, a start reads it as asked now, and the window is told
/// so it reads the held answer again -- with no new check of the agent that just answered.
fn answered_late(id: &str, found: Option<&Found>) {
    let Some(found) = found else { return };
    patch_held(&HELD_DISCOVERY, found);
    start_readiness().swept(std::slice::from_ref(found), now_ms());
    crate::discovery_service::shared().answered_late(id);
}

/// One agent asked alone, for real: the same lookups and bounded checks a sweep gives it (discoverInstalledRuntimes
/// with `only`).
pub fn discover_one_now(id: &str) -> Option<Found> {
    let machine = Machine::this_mac();
    let find = |c: &str| (machine.find)(c);
    discover_one(id, &machine.seams(&find))
}

/// The last full sweep's answer, as Electron's `discoveryCache` holds it: what a start reads first.
static HELD_DISCOVERY: std::sync::Mutex<Option<HeldAnswer>> = std::sync::Mutex::new(None);

/// The held answer as it stands, late answers patched in: a plain read, which asks no agent anything.
pub fn held_answer() -> Option<Vec<Found>> {
    crate::invariants::lock(&HELD_DISCOVERY).as_ref().map(|h| h.value.clone())
}

/// staleChecking: the agents an answer still calls "being checked" whose checks have in fact finished -- the answer
/// crossed that finish, and the held one is read once more.
pub fn stale_checking(runtimes: &[Found], finished: &std::collections::HashSet<String>) -> Vec<String> {
    runtimes.iter().filter(|e| e.checking && e.status != Probe::Ready && finished.contains(&e.id)).map(|e| e.id.clone()).collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Every full sweep is held, and tells a start's readiness it asked every agent it answered (index.ts: an agent that
/// is still being checked has not been asked yet, as far as a start can tell).
pub(crate) fn held_a_sweep(answers: &[Found]) {
    hold_sweep(&HELD_DISCOVERY, start_readiness(), answers, now_ms());
}

/// A full sweep's answer, held at `at`, and every agent it answered marked asked then.
pub fn hold_sweep(held: &std::sync::Mutex<Option<HeldAnswer>>, readiness: &StartReadiness, answers: &[Found], at: u64) {
    *crate::invariants::lock(held) = Some(HeldAnswer { at, value: answers.to_vec() });
    let asked: Vec<Found> = answers.iter().filter(|entry| !entry.checking).cloned().collect();
    readiness.swept(&asked, at);
}

/// The agents a start never asks alone: Antigravity's readiness is whether its app is open, and Apple's is the Mac's.
pub fn sweep_only_at_start() -> std::collections::HashSet<String> {
    ["antigravity", crate::runtime::apple::APPLE_RUNTIME].into_iter().map(str::to_owned).collect()
}

/// index.ts `startReadiness`: the one a start asks through, held for the whole process.
fn start_readiness() -> &'static StartReadiness {
    static READINESS: std::sync::OnceLock<StartReadiness> = std::sync::OnceLock::new();
    READINESS.get_or_init(|| {
        StartReadiness::new(StartReadinessOptions {
            ttl_ms: READY_TO_START_TTL_MS,
            now: Box::new(now_ms),
            held: Box::new(|| crate::invariants::lock(&HELD_DISCOVERY).clone()),
            // The sweep's own clock is left alone: the other runtimes were not asked.
            store: Box::new(|value| {
                let mut held = crate::invariants::lock(&HELD_DISCOVERY);
                let at = held.as_ref().map_or_else(now_ms, |h| h.at);
                *held = Some(HeldAnswer { at, value });
            }),
            sweep: Box::new(discover),
            ask_one: Box::new(|id| Ok(discover_one_now(id))),
            sweep_only: sweep_only_at_start(),
        })
    })
}

/// index.ts `discoverForStart`: a stale answer is refreshed for the starting runtime alone. A start ON an agent the
/// sweep went on without waits for THAT agent's own check, asked alone; a start on any other agent never does.
pub fn discover_for_start(runtime_id: Option<&str>) -> Vec<Found> {
    let found = start_readiness().for_start(runtime_id);
    waits_for_its_own_check(runtime_id, found, discover_one_now)
}

/// The start's own agent, when the answer still has it being checked: its own check, asked alone (sweep-settle.ts
/// `forStart`). Anything else is the answer as it stands.
pub fn waits_for_its_own_check(runtime_id: Option<&str>, found: Vec<Found>, ask_one: impl Fn(&str) -> Option<Found>) -> Vec<Found> {
    let Some(id) = runtime_id else { return found };
    if !found.iter().any(|entry| entry.id == id && entry.checking) || sweep_only_at_start().contains(id) {
        return found;
    }
    let Some(answer) = ask_one(id) else { return found };
    found.into_iter().map(|entry| if entry.id == id { answer.clone() } else { entry }).collect()
}

/// Apple Intelligence's row, from what its helper's probe said: ready when the model answers; when the Mac says why it
/// does not (turned off, still downloading, a Mac that cannot run it), that, in words, as the row's note; and no row at
/// all where there is no helper to ask (not a Mac, or a build without Swift) or it could not start.
pub fn apple_found(probe: &crate::runtime::apple::Probe) -> Option<Found> {
    use crate::runtime::apple::{APPLE_DISPLAY_NAME, APPLE_RUNTIME, Probe as Said, unavailable_words};
    let row = |ready: bool, note: Option<String>| Found {
        id: APPLE_RUNTIME.into(),
        display_name: APPLE_DISPLAY_NAME.into(),
        installed: true,
        version: None,
        auth: Auth::NotApplicable,
        ready,
        status: if ready { Probe::Ready } else { Probe::Unsupported },
        unsupported_note: note,
        through_cli: false,
        checking: false,
    };
    match probe {
        Said::Available { .. } => Some(row(true, None)),
        Said::Unavailable { reason } if reason != "unknown" => Some(row(false, Some(unavailable_words(reason).into()))),
        Said::Unavailable { .. } => None,
    }
}

/// How long a runtime that was READY is trusted when the only question is whether a turn may start on it
/// (READY_TO_START_TTL_MS): the run itself is the real check, and a CLI signed out since fails at launch, fast.
pub const READY_TO_START_TTL_MS: u64 = 5 * 60_000;

/// The last full answer and when that sweep ran (start-readiness.ts `held`).
#[derive(Clone, Debug, PartialEq)]
pub struct HeldAnswer {
    pub at: u64,
    pub value: Vec<Found>,
}

/// What a start's readiness is asked through (StartReadinessOptions).
pub struct StartReadinessOptions {
    pub ttl_ms: u64,
    pub now: Box<dyn Fn() -> u64 + Send + Sync>,
    /// The last full answer and when that sweep ran, if there is one.
    pub held: Box<dyn Fn() -> Option<HeldAnswer> + Send + Sync>,
    /// Replace the held answer; its `at` stays the sweep's own clock.
    pub store: Box<dyn Fn(Vec<Found>) + Send + Sync>,
    /// A full sweep, as every other caller gets.
    pub sweep: Box<dyn Fn() -> Vec<Found> + Send + Sync>,
    /// Ask one runtime alone.
    pub ask_one: Box<dyn Fn(&str) -> Result<Option<Found>, String> + Send + Sync>,
    /// Runtimes that are not a CLI to ask alone (Antigravity: the app itself).
    pub sweep_only: std::collections::HashSet<String>,
}

/// start-readiness.ts: whether the runtime a turn is about to start on is still ready, asked as cheaply as the answer
/// allows. A stale answer is refreshed for that runtime alone, never by a sweep of every runtime.
pub struct StartReadiness {
    options: StartReadinessOptions,
    /// When each runtime was last ASKED, by a sweep or on its own.
    asked_at: std::sync::Mutex<std::collections::HashMap<String, u64>>,
}

/// `usable`: available, ready and with a program to run. `Found` holds the first and last as `installed`.
fn usable_to_start(entry: Option<&Found>) -> bool {
    entry.is_some_and(|e| e.installed && e.ready)
}

impl StartReadiness {
    pub fn new(options: StartReadinessOptions) -> Self {
        Self { options, asked_at: std::sync::Mutex::new(std::collections::HashMap::new()) }
    }

    /// A full sweep answered every runtime at this moment.
    pub fn swept(&self, value: &[Found], at: u64) {
        let mut asked = crate::invariants::lock(&self.asked_at);
        for runtime in value {
            asked.insert(runtime.id.clone(), at);
        }
    }

    pub fn for_start(&self, runtime_id: Option<&str>) -> Vec<Found> {
        let o = &self.options;
        let held = (o.held)();
        let (Some(runtime_id), Some(held)) = (runtime_id, held) else { return (o.sweep)() };
        let chosen = held.value.iter().find(|e| e.id == runtime_id);
        let asked = crate::invariants::lock(&self.asked_at).get(runtime_id).copied().unwrap_or(held.at);
        if usable_to_start(chosen) && (o.now)().saturating_sub(asked) < o.ttl_ms {
            return held.value;
        }
        // Not ready last time, or not a CLI: the full path, as before. Only a runtime that WAS ready and has merely
        // gone stale is asked alone.
        if !usable_to_start(chosen) || o.sweep_only.contains(runtime_id) {
            return (o.sweep)();
        }
        match (o.ask_one)(runtime_id) {
            Ok(Some(fresh)) => {
                let current = (o.held)().unwrap_or(held);
                let merged: Vec<Found> =
                    current.value.into_iter().map(|e| if e.id == runtime_id { fresh.clone() } else { e }).collect();
                (o.store)(merged.clone());
                crate::invariants::lock(&self.asked_at).insert(runtime_id.to_owned(), (o.now)());
                merged
            }
            _ => (o.sweep)(),
        }
    }
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "a_start_asks_only_its_runtime_tests.rs"]
mod a_start_asks_only_its_runtime_tests;

#[cfg(test)]
#[path = "probe_sweep_tests.rs"]
mod probe_sweep_tests;
