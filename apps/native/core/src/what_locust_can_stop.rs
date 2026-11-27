//! WHAT LOCUST CAN STOP, AI agent by AI agent (Electron `shared/what-locust-can-stop.ts`, 0.617; the PRD's R9).
//!
//! Locust does not run an agent's tools; each agent runs its own, and Locust can stop one only where that agent asks
//! first. Every sentence here was read off this engine's own code, not copied from Electron's table: the run's flags
//! in `runtime/*.rs`, the approval paths in `approval.rs`, the modes in `mode_facts.rs`. Where this engine differs
//! from Electron the sentence says what this engine does:
//! - Claude Code: this engine loads none of the account's connectors (`--strict-mcp-config`), so no connector call is
//!   made and none is asked about; Electron's connector clauses are left out.
//! - Antigravity: Electron's "what it refused is listed after the run" is not built here, so it is not said.
//! - The on-device model (Apple) only talks, and has a row of its own.

/// How much of a run Locust's card can stop, at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReach {
    EachAction,
    SomeActions,
    QuestionsOnly,
    Nothing,
}

impl StopReach {
    /// `STOP_REACH_LABEL`.
    pub fn label(self) -> &'static str {
        match self {
            StopReach::EachAction => "Each action",
            StopReach::SomeActions => "Some actions",
            StopReach::QuestionsOnly => "Questions only",
            StopReach::Nothing => "Nothing",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StopRow {
    pub runtime: &'static str,
    pub reach: StopReach,
    /// What it asks Locust before doing.
    pub asks: &'static str,
    /// What it does without asking, mode by mode.
    pub without: &'static str,
    /// Who keeps an "Always", where there is one.
    pub always: Option<&'static str>,
}

pub const KEPT_BY_LOCUST: &str = "Locust keeps an Always for the rest of the run, and your saved rules come first.";

pub const WHAT_LOCUST_CAN_STOP_LEDE: &str = "Locust can stop a run only where its AI agent asks first. Where it asks, a card waits for you, your saved rules answer before it reaches you, and the record says who decided. Where it does not, the mode decides what the agent is given.";

pub const WHAT_LOCUST_CAN_STOP: &[StopRow] = &[
    StopRow {
        runtime: "codex",
        reach: StopReach::EachAction,
        asks: "In Approve each action: every command that does more than read, and every file change, before it runs.",
        without: "In Ask and Plan it works in Codex\u{2019}s own sandbox, where its commands can read but change nothing; in Edit, where they can also change files in this folder. In Ask and Plan it starts none of the MCP servers or plugins your Codex runs as programs on this computer, nor its own browser or computer use. It asks nothing in those modes, or in Auto.",
        always: Some(KEPT_BY_LOCUST),
    },
    StopRow {
        runtime: "claude",
        reach: StopReach::EachAction,
        asks: "In Approve each action: every file change and command, before it runs. In Edit, a command it does not run on its own say, and a change to a file outside this folder.",
        without: "Reading files, in every mode. In Edit, changes to files in this folder and the commands Claude Code runs on its own say. In Auto, everything. Your account\u{2019}s connectors are not loaded here, so none is called. In every mode, Locust refuses one kind of command without asking: one that ends a browser, Locust or another AI agent that no teammate started, by name or by its id, which would end yours too.",
        always: Some(KEPT_BY_LOCUST),
    },
    StopRow {
        runtime: "opencode",
        reach: StopReach::EachAction,
        asks: "In Approve each action: every command, file change, web page, place outside this folder and tool from your own MCP servers, before it runs.",
        without: "Web searches, in every mode. In Ask and Plan, no file changes, no commands and none of your own MCP tools; in Edit, commands, the web and your own MCP tools, and nothing outside this folder. In Auto, everything.",
        always: Some(KEPT_BY_LOCUST),
    },
    StopRow {
        runtime: "copilot",
        reach: StopReach::EachAction,
        asks: "In Approve each action: every action Copilot asks about, before it runs: its commands and file changes, and each fetch or place outside this folder.",
        without: "Reading files, in every mode. In Ask and Plan, no file changes, no commands and none of the MCP servers in your Copilot settings. In Edit, commands and changes in this folder. In Auto, everything.",
        always: Some(KEPT_BY_LOCUST),
    },
    StopRow {
        runtime: "cursor",
        reach: StopReach::Nothing,
        asks: "Nothing: Cursor\u{2019}s command line has no way to stop and ask Locust.",
        without: "In Ask and Plan, Cursor\u{2019}s own read-only mode, with no MCP server started that a folder brings; your own MCP tools run only where your Cursor settings already allow them. In Edit, changes in this folder, and commands as Cursor\u{2019}s own settings allow. In Auto, everything.",
        always: None,
    },
    StopRow {
        runtime: "antigravity",
        // Over agy acp (27 Nov 2026, agy 1.5.18): its default mode asks before every command and file change, each a
        // card (antigravity.rs acp_mode); its questions are forms on a question card in every mode.
        reach: StopReach::EachAction,
        asks: "In Approve each action: every command and file change, before it runs. Its questions in every mode, which you answer on a card.",
        without: "In Ask and Plan, no file changes and no commands; in Edit, file changes and no commands; in Auto, everything. An Antigravity CLI without agy acp cannot stop and ask, so Approve each action is refused there.",
        always: Some(KEPT_BY_LOCUST),
    },
    StopRow {
        runtime: "kilo",
        reach: StopReach::Nothing,
        asks: "Nothing: Kilo runs without asking Locust.",
        without: "In Ask and Plan, Kilo\u{2019}s ask mode: no file changes. In Edit and Auto, Kilo\u{2019}s code mode in this folder. It reads its settings from a folder of the run\u{2019}s own, never yours.",
        always: None,
    },
    StopRow {
        runtime: "muse",
        reach: StopReach::Nothing,
        asks: "Nothing: Muse runs without asking Locust.",
        without: "In Ask and Plan, no file changes and no commands. In Edit, changes in this folder and commands inside Muse\u{2019}s own sandbox. In Auto, everything, with commands outside that sandbox.",
        always: None,
    },
    StopRow {
        runtime: "acp",
        reach: StopReach::SomeActions,
        asks: "In Approve each action, its only mode: every action the agent asks about, before it runs. Which actions it asks about is the agent\u{2019}s own setting.",
        without: "Whatever its own settings let it do without asking, in the folder it was given.",
        always: Some(KEPT_BY_LOCUST),
    },
    StopRow {
        runtime: "apple",
        reach: StopReach::Nothing,
        asks: "Nothing: the model on this Mac only talks.",
        without: "It has no tools: it reads no file, changes none and runs no command, in Ask and Plan, its only modes.",
        always: None,
    },
];

/// The page written from this table, beside this app (a source document, not a file in the profile: no `_FILE` name) (Electron's is `docs/WHAT-LOCUST-CAN-STOP.md`, written
/// from Electron's own table; this engine's sentences differ where its code does).
pub const WHAT_LOCUST_CAN_STOP_PAGE: &str = "WHAT-LOCUST-CAN-STOP.md";

/// An agent's name in the document, as Electron's `runtimeDisplayName` writes it; a custom ACP agent is "Your own agent".
fn document_name(runtime: &str) -> &str {
    if runtime == "acp" { "Your own agent" } else { crate::failure_words::runtime_display_name(runtime) }
}

/// `whatLocustCanStopDocument`: the table and each agent's three sentences, as one Markdown page, so the page can never
/// say what this engine does not do.
pub fn what_locust_can_stop_document() -> String {
    let mut lines: Vec<String> = vec![
        "# What Locust can stop".into(),
        String::new(),
        "<!-- Written from apps/native/core/src/what_locust_can_stop.rs. Edit that file, then run".into(),
        "     `LOCUST_WRITE_DOCS=1 cargo test -p locust-core what_locust_can_stop` in apps/native to rewrite this one. -->".into(),
        String::new(),
        WHAT_LOCUST_CAN_STOP_LEDE.into(),
        String::new(),
        "| AI agent | Can stop | Asks first | Always kept by |".into(),
        "| --- | --- | --- | --- |".into(),
    ];
    for row in WHAT_LOCUST_CAN_STOP {
        let brief = match (row.runtime, row.reach) {
            ("acp", _) => "what the agent asks about",
            (_, StopReach::Nothing) => "never",
            (_, StopReach::QuestionsOnly) => "its questions",
            (_, StopReach::EachAction) => "in Approve each action",
            (_, StopReach::SomeActions) => "some of its actions",
        };
        let kept = match row.always {
            None => "\u{2014}",
            Some(KEPT_BY_LOCUST) => "Locust",
            Some(_) => document_name(row.runtime),
        };
        lines.push(format!("| {} | {} | {brief} | {kept} |", document_name(row.runtime), row.reach.label()));
    }
    lines.push(String::new());
    for row in WHAT_LOCUST_CAN_STOP {
        lines.extend([format!("## {}", document_name(row.runtime)), String::new(), format!("**Asks first.** {}", row.asks), String::new(), format!("**Without asking.** {}", row.without), String::new()]);
        if let Some(always) = row.always {
            lines.extend([format!("**Always.** {always}"), String::new()]);
        }
    }
    lines.join("\n")
}

/// The row for one agent.
pub fn row(runtime: &str) -> Option<&'static StopRow> {
    WHAT_LOCUST_CAN_STOP.iter().find(|r| r.runtime == runtime)
}

#[cfg(test)]
#[path = "what_locust_can_stop_tests.rs"]
mod tests;
