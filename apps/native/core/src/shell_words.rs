//! How a command is written on a row (Electron `missionView.ts` `shellCommandText`, `railLabel`).
//!
//! A row is one line wide. A Windows run hands over its command wrapped in the host that ran it
//! (`powershell.exe -Command "npm test"`, `cmd.exe /d /s /c "..."`); with the host in front every command read as the
//! same truncated path and the work itself was cut off. So the row shows what ran: the host taken off, one layer of
//! its quoting undone, and the doubling that quoting introduced undone too.

use crate::invariants;

/// The command as the shell got it, without the Windows host that ran it. A command that was never wrapped is
/// returned as it is.
pub fn shell_command_text(command: &str) -> String {
    let host = invariants::regex(
        r#"(?is)^\s*"?[^"]*(?:powershell|pwsh|cmd)\.exe"?\s+(?:-NoProfile\s+|-NonInteractive\s+|/d\s+|/s\s+)*(?:-Command|/c)\s+(.+)$"#,
    );
    let Some(caps) = host.captures(command) else {
        return command.to_owned();
    };
    let inner = caps[1].trim();
    // The host quotes the whole command: unwrap one matched layer.
    let double_quoted = inner.starts_with('"') && inner.ends_with('"');
    let single_quoted = inner.starts_with('\'') && inner.ends_with('\'');
    // A lone quote is both ends at once and unwraps to nothing, as Electron's `slice(1, -1)` does.
    let unquoted = if double_quoted || single_quoted { inner.get(1..inner.len().saturating_sub(1)).unwrap_or("") } else { inner };
    // AND THE BACKSLASHES (0.610). Codex reports a Windows command with every backslash escaped, its own
    // powershell.exe path included. When every backslash in the quoted payload is half of an escape pair, the payload
    // was escaped as a whole and is shown as the shell got it; one lone backslash means it was not.
    if double_quoted && let Some(unescaped) = without_escapes(unquoted) {
        return unescaped.replace("''", "'").trim().to_owned();
    }
    unquoted.replace("\\\"", "\"").replace("\"\"", "\"").replace("''", "'").trim().to_owned()
}

/// A running step names the command, not the thing that launched it (Electron `missionView.ts` `withoutShellWrapper`):
/// `cmd /c`, `powershell -Command`, `bash -lc`, `/bin/sh -c` come off the front, and one pair of quotes comes off only
/// when it encloses the whole rest. Anything else comes back trimmed and otherwise untouched; a wrapper with nothing
/// after it is kept, since a strange row beats an empty one.
pub fn without_shell_wrapper(command: &str) -> String {
    let trimmed = command.trim();
    let wrapper = invariants::regex(
        r"^(?:cmd(?:\.exe)?\s+/[cCkK]|powershell(?:\.exe)?(?:\s+-\w+)*\s+-[cC]ommand|pwsh(?:\s+-\w+)*\s+-[cC]ommand|(?:/bin/)?(?:ba|z)?sh\s+-[lic]*c)\s+",
    );
    let Some(found) = wrapper.find(trimmed) else {
        return trimmed.to_owned();
    };
    let rest = trimmed[found.end()..].trim();
    if rest.is_empty() {
        return trimmed.to_owned();
    }
    for quote in ['"', '\''] {
        if rest.len() > 1 && rest.starts_with(quote) && rest.ends_with(quote) {
            let inner = &rest[1..rest.len() - 1];
            // Only when the quotes wrap the whole thing: an inner quote is punctuation inside a larger line.
            if !inner.contains(quote) {
                return inner.trim().to_owned();
            }
        }
    }
    rest.to_owned()
}

/// A doubled backslash to one, and an escaped quote to a quote, when every backslash in the text is half of such a
/// pair; otherwise nothing.
fn without_escapes(text: &str) -> Option<String> {
    if !text.contains('\\') {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(next @ ('\\' | '"')) => out.push(next),
            _ => return None,
        }
    }
    Some(out)
}

/// The most characters a rail line takes.
pub const RAIL_LABEL_LIMIT: usize = 72;

/// One line, whitespace collapsed, bounded with an ellipsis: a multi-line command never becomes several rail lines.
pub fn rail_label(value: &str) -> String {
    let single = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if single.chars().count() <= RAIL_LABEL_LIMIT {
        return single;
    }
    let mut out: String = single.chars().take(RAIL_LABEL_LIMIT - 1).collect();
    out.push('\u{2026}');
    out
}

#[cfg(test)]
#[path = "shell_words_tests.rs"]
mod tests;
