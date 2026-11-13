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
        "codex" | "claude" | "cursor" | "opencode" | "copilot" | "muse" => Integration::Live,
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

/// How an agent is installed (RuntimeInstall).
enum Install {
    Npm(&'static str),
    Vendor(&'static str),
}

/// RuntimeInstallFacts: how to get it and sign in to it.
struct Facts {
    install: Install,
    sign_in: Option<&'static str>,
    sign_in_again: Option<&'static str>,
    sign_in_again_note: Option<&'static str>,
    account: Option<&'static str>,
    sign_in_optional: bool,
}

fn facts(id: &str) -> Option<Facts> {
    let f = |install, sign_in: Option<&'static str>, again: Option<&'static str>, account: Option<&'static str>| Facts {
        install,
        sign_in,
        sign_in_again: again,
        sign_in_again_note: None,
        account,
        sign_in_optional: false,
    };
    Some(match id {
        "opencode" => f(Install::Npm("opencode-ai"), None, None, None),
        "claude" => f(Install::Npm("@anthropic-ai/claude-code"), Some("claude"), Some("claude auth login"), Some("an Anthropic account")),
        "codex" => f(Install::Npm("@openai/codex"), Some("codex"), Some("codex login"), Some("a ChatGPT account")),
        // Its own /login, inside the window this opens.
        "copilot" => f(Install::Npm("@github/copilot"), Some("copilot"), Some("copilot"), Some("a GitHub Copilot subscription")),
        "cursor" => f(Install::Vendor("https://cursor.com/cli"), Some("cursor-agent login"), Some("cursor-agent login"), Some("a Cursor account")),
        "muse" => f(Install::Vendor("https://dev.meta.ai"), Some("muse login"), Some("muse login"), Some("a Muse Code subscription")),
        // Signed into inside `agy` itself: it has no login command; /logout then /login switches accounts.
        "antigravity" => Facts {
            sign_in_again_note: Some("then type /logout, then /login, and choose the account"),
            ..f(Install::Vendor("https://antigravity.google/product/antigravity-cli/"), Some("agy"), Some("agy"), Some("a Google account"))
        },
        // Auto Free needs no account; `kilo auth login` is only for a paid model.
        "kilo" => Facts {
            sign_in_optional: true,
            ..f(Install::Npm("@kilocode/cli"), Some("kilo auth login"), Some("kilo auth login"), Some("a Kilo account, only for a paid model"))
        },
        _ => return None,
    })
}

/// installCommand: the one npm line, for an agent npm installs.
pub fn install_command(id: &str) -> Option<String> {
    match facts(id)?.install {
        Install::Npm(package) => Some(format!("npm install -g {package}")),
        Install::Vendor(_) => None,
    }
}

/// The npm package an agent installs from, for an agent npm installs.
pub fn install_package(id: &str) -> Option<String> {
    match facts(id)?.install {
        Install::Npm(package) => Some(package.to_owned()),
        Install::Vendor(_) => None,
    }
}

/// installSentence: what a NOT INSTALLED row says, standing on its own (no terminal in it: the row's Install does it).
pub fn install_sentence(id: &str, display_name: &str) -> String {
    let Some(facts) = facts(id) else {
        return format!("{display_name} was not found on this machine.");
    };
    if let Install::Vendor(url) = facts.install {
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
    let facts = facts(id)?;
    let line = if again { facts.sign_in_again } else { facts.sign_in }?;
    Some(match (again, facts.sign_in_again_note) {
        (true, Some(note)) => format!("run {line}, {note}"),
        _ => format!("run {line}"),
    })
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

/// What discovery may touch, passed in so a test never runs a real agent or reads a real home folder.
pub struct Seams<'a> {
    /// Where a command is, if it is installed.
    pub find: &'a dyn Fn(&str) -> Option<std::path::PathBuf>,
    /// Run it with these arguments (bounded by the caller).
    pub run: &'a dyn Fn(&std::path::Path, &[&str]) -> Option<Answer>,
    /// Whether a file exists. Only ever asked about a credentials file's presence, never its contents.
    pub exists: &'a dyn Fn(&std::path::Path) -> bool,
    pub home: Option<std::path::PathBuf>,
    pub xdg_config_home: Option<String>,
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

/// One agent, found or not, as main's publicStatus says it.
pub fn discover_one(id: &str, seams: &Seams) -> Option<Found> {
    let d = DEFINITIONS.iter().find(|d| d.id == id)?;
    let not_applicable = matches!(d.id, "opencode" | "kilo");
    let Some(path) = (seams.find)(d.command) else {
        return Some(Found {
            id: d.id.into(),
            display_name: d.name.into(),
            installed: false,
            version: None,
            auth: if not_applicable { Auth::NotApplicable } else { Auth::Unknown },
            ready: false,
            status: Probe::Missing,
            unsupported_note: None,
            through_cli: false,
            checking: false,
        });
    };
    let run = |args: &[&str]| (seams.run)(&path, args);
    let version_answer = run(d.version);
    let capability_answer = run(d.capability);
    let same_command = d.models == Some(d.readiness);
    let listed = d.models.filter(|_| !same_command).map(run);
    let readiness_is_version = readiness_from_version(d.id) || d.readiness == d.version;
    let readiness_answer = if readiness_is_version {
        version_answer.clone()
    } else if d.list_answers_readiness && let Some(list) = &listed {
        if succeeded(list) && list.as_ref().is_some_and(|a| (d.ready_when)(&a.text())) { list.clone() } else { run(d.readiness) }
    } else {
        run(d.readiness)
    };
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
    let base = |installed, version, auth, status| Found {
        id: "antigravity".into(),
        display_name: "Antigravity".into(),
        installed,
        version,
        auth,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: installed,
        checking: false,
    };
    let Some(path) = (seams.find)("agy") else { return base(false, None, Auth::Unknown, Probe::Missing) };
    let version = (seams.run)(&path, &["--version"])
        .filter(|a| a.exit == Some(0) && !a.timed_out)
        .and_then(|a| a.stdout.trim().lines().next().and_then(|l| crate::runtime::version::parse(l)).map(|v| v.version));
    match (seams.run)(&path, &["models"]) {
        None => base(true, version, Auth::Unknown, Probe::ProbeFailed),
        Some(a) if a.timed_out => base(true, version, Auth::Unknown, Probe::ProbeFailed),
        Some(a) if crate::models::parse_agy_model_list(&a.stdout).is_some() => base(true, version, Auth::Authenticated, Probe::Ready),
        Some(_) => base(true, version, Auth::Unauthenticated, Probe::AuthRequired),
    }
}

/// How long one check may take before it counts as not answering (PROBE_TIMEOUT_MS).
pub const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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

/// Every agent, checked for real and all at once: each one found on PATH (the window adopts the terminal's PATH at
/// launch) is asked its version, its help and its sign-in check, ten seconds each. Reads no file but the presence of
/// a credentials file; sends nothing anywhere.
pub fn discover() -> Vec<Found> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let checks: Vec<_> = DEFINITIONS
        .iter()
        .map(|d| {
            let (home, xdg) = (home.clone(), xdg.clone());
            std::thread::spawn(move || {
                let find = |c: &str| crate::runtime::discovery::find_named(c).ok();
                let run = |p: &std::path::Path, a: &[&str]| run_bounded(p, a, PROBE_TIMEOUT);
                let exists = |p: &std::path::Path| p.exists();
                discover_one(d.id, &Seams { find: &find, run: &run, exists: &exists, home, xdg_config_home: xdg })
            })
        })
        .collect();
    let agy = {
        let (home, xdg) = (home.clone(), xdg.clone());
        std::thread::spawn(move || {
            let find = |c: &str| crate::runtime::discovery::find_named(c).ok();
            let run = |p: &std::path::Path, a: &[&str]| run_bounded(p, a, PROBE_TIMEOUT);
            let exists = |p: &std::path::Path| p.exists();
            discover_antigravity(&Seams { find: &find, run: &run, exists: &exists, home, xdg_config_home: xdg })
        })
    };
    let mut found: Vec<Found> = checks.into_iter().filter_map(|c| c.join().ok().flatten()).collect();
    // Electron lists Antigravity after Copilot.
    if let Ok(a) = agy.join() {
        let at = found.iter().position(|f| f.id == "copilot").map_or(found.len(), |i| i + 1);
        found.insert(at, a);
    }
    found
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
