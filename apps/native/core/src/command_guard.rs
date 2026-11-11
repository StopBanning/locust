//! Locust's command guard: refuse ending browsers / editors / agents by name,
//! and refuse publishing from a Scout fix copy (0.717 / 0.828).
//!
//! Port of hard-deny-by-name and fix-copy publish from
//! `apps/desktop/resources/locust-command-guard.mjs` (`stoppedBecause`).

use regex::Regex;
use std::sync::LazyLock;

/// Context for a guard check without a live process table.
pub struct GuardContext {
    /// Command runs inside a Scout fix copy (`rt_scoutfix…`).
    pub fix_copy: bool,
    /// Process ids this run started; by-id kills of others can be refused.
    pub started_pids: Vec<u32>,
}

/// Exact refusal when a Scout fix copy would push / add a remote / use gh.
pub const FIX_REFUSAL: &str = "Refused: this is a copy Scout made to prepare a fix, and from here Locust does not let anything push, add a remote or use gh. The person brings the fix into their folder with Apply and opens any pull request themselves. Make the fix in these files, then say what you changed.";

const BROWSER_TIP: &str = "A test browser you start needs a profile of its own (--user-data-dir=<a new folder>), so it never meets theirs.";

struct EndEntry {
    names: &'static [&'static str],
    what: &'static str,
    harm: Harm,
    browser: bool,
    agent: bool,
}

#[derive(Clone, Copy)]
enum Harm {
    Windows,
    Apps,
    Work,
    Desktop,
}

impl Harm {
    fn phrase(self) -> &'static str {
        match self {
            Self::Windows => "the person's own windows",
            Self::Apps => "other apps' windows, which draw in it",
            Self::Work => "the person's own programs and other teammates' work",
            Self::Desktop => "the person's desktop and taskbar",
        }
    }

    fn owner(self) -> &'static str {
        match self {
            Self::Windows | Self::Apps | Self::Desktop => "the person's own",
            Self::Work => "the person's or another teammate's",
        }
    }
}

const ENDS: &[EndEntry] = &[
    EndEntry {
        names: &["google chrome", "chrome", "chromium"],
        what: "Chrome",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["msedgewebview2"],
        what: "Edge web view",
        harm: Harm::Apps,
        browser: false,
        agent: false,
    },
    EndEntry {
        names: &["microsoft edge", "msedge"],
        what: "Edge",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["firefox"],
        what: "Firefox",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["brave browser", "brave"],
        what: "Brave",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["opera"],
        what: "Opera",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["vivaldi"],
        what: "Vivaldi",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["iexplore"],
        what: "Internet Explorer",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["safari"],
        what: "Safari",
        harm: Harm::Windows,
        browser: true,
        agent: false,
    },
    EndEntry {
        names: &["node"],
        what: "Node",
        harm: Harm::Work,
        browser: false,
        agent: false,
    },
    EndEntry {
        names: &["electron"],
        what: "Electron",
        harm: Harm::Work,
        browser: false,
        agent: false,
    },
    EndEntry {
        names: &["locust"],
        what: "Locust",
        harm: Harm::Work,
        browser: false,
        agent: false,
    },
    EndEntry {
        names: &["explorer"],
        what: "File Explorer",
        harm: Harm::Desktop,
        browser: false,
        agent: false,
    },
    EndEntry {
        names: &["code"],
        what: "VS Code",
        harm: Harm::Windows,
        browser: false,
        agent: false,
    },
    EndEntry {
        names: &["cursor-agent", "cursor"],
        what: "Cursor",
        harm: Harm::Work,
        browser: false,
        agent: true,
    },
    EndEntry {
        names: &["claude"],
        what: "Claude Code",
        harm: Harm::Work,
        browser: false,
        agent: true,
    },
    EndEntry {
        names: &["codex"],
        what: "Codex",
        harm: Harm::Work,
        browser: false,
        agent: true,
    },
    EndEntry {
        names: &["gemini"],
        what: "Gemini CLI",
        harm: Harm::Work,
        browser: false,
        agent: true,
    },
    EndEntry {
        names: &["copilot"],
        what: "Copilot CLI",
        harm: Harm::Work,
        browser: false,
        agent: true,
    },
    EndEntry {
        names: &["opencode"],
        what: "OpenCode",
        harm: Harm::Work,
        browser: false,
        agent: true,
    },
    EndEntry {
        names: &["agy", "antigravity"],
        what: "Antigravity",
        harm: Harm::Work,
        browser: false,
        agent: true,
    },
];

fn tip_for(entry: &EndEntry) -> &'static str {
    if entry.browser {
        BROWSER_TIP
    } else {
        "End only the processes this run started; Locust checks."
    }
}

fn escape_regex(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        if matches!(
            ch,
            '.' | '*' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\\'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Longest name first, so "google chrome" wins over "chrome".
fn all_names() -> Vec<(&'static str, &'static EndEntry)> {
    let mut all: Vec<_> = ENDS
        .iter()
        .flat_map(|entry| entry.names.iter().map(move |name| (*name, entry)))
        .collect();
    all.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
    all
}

fn name_alternation() -> String {
    all_names()
        .into_iter()
        .map(|(name, _)| escape_regex(name).replace(' ', r"\s+"))
        .collect::<Vec<_>>()
        .join("|")
}

static BY_NAME: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let tick = '\u{60}'; // backtick, kept out of raw strings
    let name = name_alternation();
    // A protected name as a word of its own, with or without `.exe` / `.app`.
    let name_group = format!("(?:^|[^\\w-])({name})(?:\\.exe|\\.app)?(?:$|[^\\w-])");
    let same = "[^\\n;&|]*?";
    let pipe = "[^\\n;&]*?";
    let line = "[^\\n]*?";
    let kills = "(?:taskkill|tskill|stop-process|spps|pkill|killall|\\bkill\\b|\\.kill\\s*\\()";
    let patterns = [
        // taskkill / tskill with /IM or /FI
        format!(
            "\\b(?:taskkill|tskill)(?:\\.exe)?\\b{same}(?:[\\/-]{{1,2}}(?:im|fi)\\b){same}{name_group}"
        ),
        format!("\\btskill(?:\\.exe)?\\s+['\"]?{name_group}"),
        // Stop-Process -Name
        format!(
            "\\b(?:stop-process|spps|kill)\\b{same}-(?:processname|name|n)\\b{same}{name_group}"
        ),
        // Get-Process <name> | Stop-Process
        format!(
            "\\b(?:get-process|gps|ps)\\b{same}{name_group}{pipe}(?:\\|\\s*(?:stop-process|spps|kill)\\b|\\.kill\\s*\\(|foreach-object\\s+kill\\b)"
        ),
        // pkill / killall
        format!("\\b(?:pkill|killall)\\b{same}{name_group}"),
        // pgrep | xargs kill
        format!(
            "\\b(?:pgrep|pidof|grep)\\b{same}{name_group}{pipe}\\|\\s*(?:xargs\\s+(?:-\\S+\\s+)*)?(?:kill|taskkill)\\b"
        ),
        // kill $(pgrep <name>) / kill `pidof <name>`
        format!("\\bkill\\b{same}(?:\\$\\(|{tick})\\s*(?:pgrep|pidof)\\b[^){tick}]*?{name_group}"),
        // wmic … delete / terminate
        format!("\\bwmic\\b{same}{name_group}{same}\\b(?:delete|terminate)\\b"),
        // Win32_Process … Terminate
        format!(
            "\\bwin32_process\\b{pipe}{name_group}{pipe}\\b(?:terminate|remove-ciminstance)\\b"
        ),
        // osascript quit
        format!(
            "\\bosascript\\b{same}(?:quit\\s+app(?:lication)?\\s+['\"\\\\]*{name_group}|tell\\s+app(?:lication)?\\s+['\"\\\\]*{name_group}['\"\\\\]*\\s+to\\s+quit)"
        ),
        // find by name then kill by variable pid in the same line
        format!(
            "\\b(?:tasklist|get-process|gps|pgrep|pidof|wmic|win32_process|get-ciminstance)\\b{line}{name_group}{line}(?:\\b(?:taskkill|tskill)\\b{same}[\\/-]{{1,2}}pid\\s+[\"']?[$%]|\\b(?:stop-process|spps)\\b|(?:^|[^\\w.-])kill\\s+(?:-\\S+\\s+)*[\"']?[$%]|\\.kill\\s*\\(|\\bxargs\\s+(?:-\\S+\\s+)*(?:kill|taskkill)\\b)"
        ),
        format!(
            "{kills}{line}(?:\\$\\(|{tick})[^){tick}]*\\b(?:tasklist|get-process|gps|pgrep|pidof|wmic)\\b[^){tick}]*?{name_group}"
        ),
    ];
    patterns
        .into_iter()
        .map(|p| Regex::new(&format!("(?i){p}")).expect("guard by-name pattern"))
        .collect()
});

static GH_COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:^|[\s"'`(;&|=\\/])gh(?:\.exe)?(?:$|[\s"'`);&|])"#).expect("gh")
});

static GIT_OUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:^|[\s"'`(;&|\\/])git(?:\.exe)?["']?\s+(?:(?:-C\s+(?:"[^"]*"|'[^']*'|\S+)|-c\s+\S+|--?[\w-]+(?:=\S+)?)\s+)*(?:push|remote|credential|send-email|request-pull)(?:$|[\s"'`);&|])"#,
    )
    .expect("git out")
});

/// Whether a command would reach past a fix's copy: push, remote, credential, send-email, request-pull, or gh.
pub fn publishes_from_copy(command: &str) -> bool {
    GH_COMMAND.is_match(command) || GIT_OUT.is_match(command)
}

/// Whether any folder is inside a Scout fix copy (`rt_scoutfix` + 12 hex digits).
pub fn in_fix_copy(folders: &[&str]) -> bool {
    let re = crate::invariants::regex(r"(?i)(?:^|[\\/])rt_scoutfix[a-f0-9]{12}(?:[\\/]|$)");
    folders.iter().any(|folder| re.is_match(folder))
}

fn entry_for_name(named: &str) -> Option<&'static EndEntry> {
    let named = named.to_lowercase().replace(['\t', '\n'], " ");
    let named = named.split_whitespace().collect::<Vec<_>>().join(" ");
    all_names()
        .into_iter()
        .find(|(name, _)| *name == named)
        .map(|(_, e)| e)
}

fn by_name_refusal(entry: &EndEntry) -> String {
    format!(
        "Locust stopped this command before it ran: it ends {} by name, which ends every {} on this computer, {} too. Leave the person's {} running. {}",
        entry.what,
        entry.what,
        entry.harm.phrase(),
        entry.what,
        tip_for(entry)
    )
}

fn by_id_refusal(pid: u32, entry: &EndEntry) -> String {
    if entry.agent {
        format!(
            "Locust stopped this command before it ran: process {pid} is {}, which this run did not start, so it is another teammate's or the person's. Leave it running.",
            entry.what
        )
    } else {
        let tip = if entry.browser {
            format!(" {BROWSER_TIP}")
        } else {
            String::new()
        };
        format!(
            "Locust stopped this command before it ran: process {pid} is {}, which no teammate started, so it is {}. Leave it running.{tip}",
            entry.what,
            entry.harm.owner()
        )
    }
}

/// Process ids a command ends BY ID, as written in it.
pub fn ended_ids(command: &str) -> Vec<u32> {
    let mut ids = std::collections::BTreeSet::new();
    let add = |ids: &mut std::collections::BTreeSet<u32>, text: &str| {
        for one in text.split(|c: char| c.is_whitespace() || c == ',') {
            if let Ok(n) = one.parse::<u32>()
                && (1..=999_999_999).contains(&n)
                && one.chars().all(|c| c.is_ascii_digit())
            {
                ids.insert(n);
            }
        }
    };

    for segment in command.split(['\n', ';', '&', '|']) {
        if !crate::invariants::regex(r"(?i)\b(?:taskkill|tskill)\b").is_match(segment) {
            // still check other patterns on whole command below
        } else {
            for m in
                crate::invariants::regex(r#"(?i)[/-]{1,2}pid\s+['"]?(\d+)"#).captures_iter(segment)
            {
                add(&mut ids, &m[1]);
            }
            for m in
                crate::invariants::regex(r"(?i)\btskill(?:\.exe)?\s+(\d+)\b").captures_iter(segment)
            {
                add(&mut ids, &m[1]);
            }
        }
    }

    let each = |ids: &mut std::collections::BTreeSet<u32>, pattern: &str, group: usize| {
        let re = Regex::new(pattern).expect("ended id pattern");
        for m in re.captures_iter(command) {
            if let Some(g) = m.get(group) {
                add(ids, g.as_str());
            }
        }
    };

    each(
        &mut ids,
        r"(?i)\b(?:stop-process|spps)\b[^\n;&|]*?-id\s+([\d,\s]+)",
        1,
    );
    each(&mut ids, r"(?i)\b(?:stop-process|spps)\s+(\d[\d,\s]*)", 1);
    each(&mut ids, r"(?i)\bkill\s+-id\s+([\d,\s]+)", 1);
    each(
        &mut ids,
        r"(?i)\bget-process\s+-id\s+([\d,\s]+)[^\n;&]*?(?:\|\s*(?:stop-process|spps|kill)\b|\.kill\s*\()",
        1,
    );
    each(
        &mut ids,
        r"(?i)getprocessbyid\(\s*(\d+)\s*\)\s*\.kill\s*\(",
        1,
    );
    each(
        &mut ids,
        r"(?i)(?:^|[^\w.-])kill((?:\s+-(?:s\s+\S+|\w+))*)((?:\s+\d+)+)(?:\s|$|[;&|)])",
        2,
    );
    each(&mut ids, r"(?i)process\.kill\(\s*(\d+)", 1);
    each(
        &mut ids,
        r#"(?i)\bwmic\b[^\n;&|]*?processid\s*=\s*['"]?(\d+)[^\n;&|]*?\b(?:delete|terminate)\b"#,
        1,
    );
    each(
        &mut ids,
        r"(?i)\bwmic\s+process\s+(\d+)\s+(?:delete|call\s+terminate)\b",
        1,
    );
    each(
        &mut ids,
        r#"(?i)\bwin32_process\b[^\n;&]*?processid\s*=\s*['"]?(\d+)[^\n;&]*?\b(?:terminate|remove-ciminstance)\b"#,
        1,
    );

    // for p in 1 2 3; do taskkill //PID $p; done
    let for_re = Regex::new(r"(?i)\bfor\s+(\w+)\s+in\s+([\d\s]+?)\s*;\s*do\b([\s\S]*?)\bdone\b")
        .expect("for");
    for m in for_re.captures_iter(command) {
        let name = &m[1];
        let body = &m[3];
        if kills_variable(body, name) {
            add(&mut ids, &m[2]);
        }
    }
    let foreach_re = Regex::new(
        r"(?i)\bforeach\s*\(\s*\$(\w+)\s+in\s+@?\(?\s*([\d,\s]+?)\s*\)?\s*\)\s*\{([^}]*)\}",
    )
    .expect("foreach");
    for m in foreach_re.captures_iter(command) {
        if kills_variable(&m[3], &m[1]) {
            add(&mut ids, &m[2]);
        }
    }
    each(
        &mut ids,
        r#"(?i)\b(?:echo|printf)\s+['"]?([\d\s]+?)['"]?\s*\|\s*xargs\s+(?:-\S+\s+)*(?:kill|taskkill)\b"#,
        1,
    );

    ids.into_iter().collect()
}

fn kills_variable(body: &str, name: &str) -> bool {
    let escaped = regex::escape(name);
    let prefix = r#"(?i)(?:\b(?:taskkill|tskill)\b[^\n;&|]*?[/-]{1,2}pid\s+["']?|(?:^|[^\w.-])kill\s+(?:-\S+\s+)*["']?|\b(?:stop-process|spps)\b[^\n;&|]*?(?:-id\s+)?|process\.kill\(\s*)\$"#;
    let braced = format!("{prefix}\\{{{escaped}\\}}\\b");
    let plain = format!("{prefix}{escaped}\\b");
    Regex::new(&braced).ok().is_some_and(|r| r.is_match(body))
        || Regex::new(&plain).ok().is_some_and(|r| r.is_match(body))
}

/// Protected process name mentioned near a by-id kill in the same command, if any.
fn named_in_id_kill(command: &str) -> Option<&'static EndEntry> {
    // e.g. taskkill /PID 1 /IM chrome.exe — rare, but name + id together.
    for (name, entry) in all_names() {
        let re = Regex::new(&format!(
            r"(?i)(?:^|[^\w-]){}(?:\.exe|\.app)?(?:$|[^\w-])",
            escape_regex(name).replace(' ', r"\s+")
        ))
        .ok()?;
        if re.is_match(command) {
            return Some(entry);
        }
    }
    None
}

/// Why a command is refused, in words the model reads, or `None` when it may run.
pub fn stopped_because(command: &str, ctx: &GuardContext) -> Option<String> {
    if command.is_empty() {
        return None;
    }
    if ctx.fix_copy && publishes_from_copy(command) {
        return Some(FIX_REFUSAL.to_string());
    }

    for pattern in BY_NAME.iter() {
        for found in pattern.captures_iter(command) {
            let named = found
                .iter()
                .skip(1)
                .flatten()
                .map(|m| m.as_str())
                .find(|s| !s.is_empty())
                .map(|s| {
                    s.to_lowercase()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                });
            let Some(named) = named else {
                continue;
            };
            let Some(entry) = entry_for_name(&named) else {
                continue;
            };
            return Some(by_name_refusal(entry));
        }
    }

    let ids = ended_ids(command);
    if ids.is_empty() {
        return None;
    }
    // Without a process table: refuse by-id when the command also names a protected
    // process, or when the id is not one this run started (caller supplied the list).
    if let Some(entry) = named_in_id_kill(command) {
        for pid in &ids {
            if !ctx.started_pids.contains(pid) {
                return Some(by_id_refusal(*pid, entry));
            }
        }
    } else if !ctx.started_pids.is_empty() {
        for pid in &ids {
            if !ctx.started_pids.contains(pid) {
                // Known protected names by common browser/editor id-kill without a table:
                // refuse as not this run's when the caller tracks started pids.
                return Some(format!(
                    "Locust stopped this command before it ran: process {pid} is not one this run started. Leave it running."
                ));
            }
        }
    }
    None
}

/// The hook (resources/locust-command-guard.mjs `main`): one PreToolUse event as JSON; a deny to print, or `None` to
/// let it run. Where it runs is the runtime's own word for it, then `here`, the folder the hook was started in.
///
/// The process table is not read yet, so a by-id kill of a process this run did not start is let through (as the
/// approval cards' guard does); the by-name refusals and the fix-copy refusals are whole.
pub fn hook_reply(input: &str, here: &str) -> Option<String> {
    let event: serde_json::Value = serde_json::from_str(input).ok()?;
    let command = event["tool_input"]["command"].as_str()?;
    let folders = [event["cwd"].as_str().unwrap_or(""), event["tool_input"]["workdir"].as_str().unwrap_or(""), here];
    let reason = stopped_because(command, &GuardContext { fix_copy: in_fix_copy(&folders), started_pids: Vec::new() })?;
    Some(
        serde_json::json!({"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":reason}})
            .to_string(),
    )
}

/// `locust-native --command-guard`: read the event (at most 1 MiB), print the deny if there is one, exit 0 whatever
/// happens, as the TypeScript hook does, so a broken hook never blocks a command by accident.
pub fn hook_main() {
    use std::io::{Read, Write};
    let mut input = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut input);
    let here = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(reply) = hook_reply(&input, &here) {
        let _ = std::io::stdout().write_all(reply.as_bytes());
    }
}

/// command-guard.ts commandGuardSettings: the `--settings` every Claude Code turn gets, naming this binary as a
/// PreToolUse hook on the shell tools. Claude Code runs a hook in a POSIX shell (Git Bash on Windows), so the path is
/// quoted for one, with forward slashes.
pub fn claude_settings(executable: &std::path::Path) -> String {
    let path = executable.to_string_lossy().replace('\\', "/");
    let mut quoted = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '$' | '`') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {"PreToolUse": [{"matcher": "Bash|PowerShell", "hooks": [{"type": "command", "command": format!("{quoted} --command-guard"), "timeout": 15}]}]}
    }))
    .expect("plain JSON")
}

/// Writes the settings into `dir` (the native data folder's `guard`) and returns the file's path.
pub fn write_claude_settings(dir: &std::path::Path, executable: &std::path::Path) -> std::io::Result<String> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("claude-settings.json");
    let text = claude_settings(executable);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        // A new name, then a rename, so a turn starting meanwhile never reads half a file.
        let temp = dir.join(format!(".claude-settings.{}.tmp", std::process::id()));
        std::fs::write(&temp, text.as_bytes())?;
        std::fs::rename(&temp, &path)?;
    }
    path.into_os_string().into_string().map_err(|_| std::io::Error::other("Guard settings path is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ctx() -> GuardContext {
        GuardContext {
            fix_copy: false,
            started_pids: Vec::new(),
        }
    }

    #[test]
    fn chrome_by_name() {
        let reason = stopped_because("taskkill //F //IM chrome.exe", &empty_ctx()).unwrap();
        assert_eq!(
            reason,
            "Locust stopped this command before it ran: it ends Chrome by name, which ends every Chrome on this computer, the person's own windows too. Leave the person's Chrome running. A test browser you start needs a profile of its own (--user-data-dir=<a new folder>), so it never meets theirs."
        );
        assert!(!reason.to_lowercase().contains("pid"));
        assert!(
            stopped_because("taskkill /F /IM msedge.exe", &empty_ctx())
                .unwrap()
                .contains("it ends Edge by name")
        );
        assert!(
            stopped_because("Stop-Process -Name chrome -Force", &empty_ctx())
                .unwrap()
                .contains("Chrome")
        );
        assert!(stopped_because("echo hello", &empty_ctx()).is_none());
    }

    #[test]
    fn fix_copy_publish() {
        let ctx = GuardContext {
            fix_copy: true,
            started_pids: Vec::new(),
        };
        for command in [
            "git push origin main",
            "git -C . push",
            "git remote add origin https://github.com/acme/app.git",
            "gh pr create --fill",
            "npm test && gh api repos/acme/app",
        ] {
            let reason = stopped_because(command, &ctx).unwrap();
            assert!(
                reason.starts_with("Refused: this is a copy Scout made to prepare a fix"),
                "{command}: {reason}"
            );
        }
        assert!(stopped_because("git status", &ctx).is_none());
        assert!(stopped_because("npm test", &ctx).is_none());
        assert!(stopped_because("git push origin main", &empty_ctx()).is_none());
    }

    #[test]
    fn by_id_refuses_pids_not_started_here() {
        let ctx = GuardContext {
            fix_copy: false,
            started_pids: vec![7300],
        };
        let reason = stopped_because("taskkill //F //PID 10544", &ctx).unwrap();
        assert!(reason.contains("process 10544"));
        assert!(reason.contains("not one this run started"));
        assert!(stopped_because("taskkill //F //PID 7300", &ctx).is_none());
        // No started_pids list → fail open for pure by-id (no process table).
        assert!(stopped_because("taskkill //F //PID 10544", &empty_ctx()).is_none());
    }

    #[test]
    fn the_hook_denies_ending_a_browser_and_lets_the_rest_run() {
        let ask = |command: &str| {
            hook_reply(&serde_json::json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":command},"cwd":"/work"}).to_string(), "/work")
        };
        let deny: serde_json::Value = serde_json::from_str(&ask("pkill -f 'Google Chrome'").unwrap()).unwrap();
        assert_eq!(deny["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(deny["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap().contains("Chrome"));
        assert!(ask("ls -la").is_none());
        assert!(hook_reply("not json", "/").is_none());
        let fix = serde_json::json!({"tool_input":{"command":"git push origin main"},"cwd":"/tmp/rt_scoutfix0123456789ab"}).to_string();
        assert!(hook_reply(&fix, "/").unwrap().contains("Scout"));
    }

    #[test]
    fn claude_settings_quote_the_binary_for_a_shell() {
        let text = claude_settings(std::path::Path::new("/Apps/Lo$cust \"x\"/locust-native"));
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let hook = &v["hooks"]["PreToolUse"][0];
        assert_eq!(hook["matcher"], "Bash|PowerShell");
        assert_eq!(hook["hooks"][0]["command"], "\"/Apps/Lo\\$cust \\\"x\\\"/locust-native\" --command-guard");
        assert_eq!(hook["hooks"][0]["timeout"], 15);
    }
}
