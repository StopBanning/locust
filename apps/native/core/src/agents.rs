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

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
