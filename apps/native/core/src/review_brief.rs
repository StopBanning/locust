//! Handing a finished turn to another teammate to be challenged (Electron `renderer/src/reviewBrief.ts`).
//!
//! What a reviewer is given is what the record holds and nothing else: the request the person made on this turn, what
//! the teammate said back, what changed by path, what ran and what came back, what the conversation built on earlier
//! turns, and the conditions it ran under. A reviewer cannot see another teammate's conversation, so the facts travel.
//!
//! THE BRIEF MAKES NO CLAIM ABOUT QUALITY. It never says the work looks done or that the commands prove anything:
//! telling a reviewer the work passed and then asking it to check would be handing it the answer.

use crate::hand_off::{STEP_BUDGET, head16, len16};

/// One command the turn ran: its first line, and its exit code where one was recorded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub exit_code: Option<i64>,
}

/// What the brief is built from (`ReviewMaterial`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewMaterial {
    /// What the person asked for ON THIS TURN, in their words.
    pub request: String,
    /// What the conversation opened with, when this turn asked for something else: context, labelled as such.
    pub opened_with: Option<String>,
    /// What the teammate said back. For research or a question it is the whole deliverable.
    pub said: String,
    /// Paths this turn changed.
    pub changed: Vec<String>,
    /// Commands it ran, in order.
    pub commands: Vec<Command>,
    /// Paths changed EARLIER in the same conversation. Kept apart from `changed`, so a reviewer can still tell which
    /// turn is its job: the verification range is the whole conversation, the thing judged is still this turn.
    pub changed_earlier: Vec<String>,
    /// The conditions, from `ranOnLine`.
    pub ran_on: String,
    /// Who did the work.
    pub author: String,
}

/// Long enough to carry a real request, short enough not to be the whole turn.
const MAX_REQUEST: usize = 1_200;
/// The whole brief fits where it is sent: a mission prompt is at most `STEP_BUDGET`.
pub const MAX_BRIEF: usize = STEP_BUDGET - 200;
const MAX_PATHS: usize = 20;
const MAX_COMMANDS: usize = 12;

/// 12345 as en-US says it: "12,345".
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Where the reply's middle was, said where it was.
fn trimmed_note(count: usize) -> String {
    format!("[... {} characters from the middle of the reply left out; the whole reply is in their thread ...]", grouped(count))
}

/// `s.slice(start)` in UTF-16 units.
fn from16(s: &str, start: usize) -> &str {
    let head = head16(s, start);
    &s[head.len()..]
}

fn clean(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").trim().to_owned()
}

fn bounded(text: &str, limit: usize) -> String {
    let clean = clean(text);
    if len16(&clean) <= limit { clean } else { format!("{}\u{2026}", head16(&clean, limit - 1)) }
}

/// The reply cut to `limit` from the MIDDLE: how it opened and how it ended both stay, and the gap says how much.
pub fn trimmed_from_the_middle(text: &str, limit: usize) -> String {
    let clean = clean(text);
    let length = len16(&clean);
    if length <= limit {
        return clean;
    }
    let room = limit.saturating_sub(len16(&trimmed_note(length)) + 2);
    let head = room * 4 / 10;
    let tail = room - head;
    format!(
        "{}\n{}\n{}",
        head16(&clean, head).trim_end(),
        trimmed_note(length - head - tail),
        from16(&clean, length - tail).trim_start()
    )
}

/// The brief, as the reviewer's mission prompt: a job, not a question.
pub fn review_brief(material: &ReviewMaterial) -> String {
    const SAID: &str = "\u{0}SAID\u{0}";
    let brief = brief_around(material, SAID);
    if !brief.contains(SAID) {
        return brief;
    }
    let room = 400.max(MAX_BRIEF.saturating_sub(len16(&brief) - len16(SAID)));
    let said = crate::protocol_tags::defang_protocol_blocks(&material.said);
    brief.replacen(SAID, &trimmed_from_the_middle(&said, room), 1)
}

fn brief_around(material: &ReviewMaterial, said_slot: &str) -> String {
    let mut lines: Vec<String> = vec![
        format!("{} finished a piece of work and you are reviewing it. You did not do this work and you cannot see their conversation; everything known about it is below.", material.author),
        String::new(),
        "WHAT WAS ASKED FOR".into(),
        bounded(&material.request, MAX_REQUEST),
        String::new(),
    ];
    if let Some(opened) = material.opened_with.as_deref().filter(|o| !o.trim().is_empty()) {
        // Context, explicitly not the thing being judged.
        lines.push("EARLIER IN THE SAME CONVERSATION (context, not the request)".into());
        lines.push(bounded(opened, MAX_REQUEST));
        lines.push(String::new());
    }
    // Above what changed: for an answer this is the entire deliverable.
    lines.push("WHAT THEY SAID".into());
    if material.said.trim().is_empty() {
        lines.push("This turn recorded no reply text.".into());
    } else {
        // Filled in once everything else is known, so the whole brief fits; defanged, since one model's words become
        // another model's instructions.
        lines.push(said_slot.into());
    }
    lines.push(String::new());

    // Both negatives are said once, as a fact about the kind of work.
    if material.changed.is_empty() && material.commands.is_empty() {
        lines.push("WHAT IT DID IN THE WORKSPACE".into());
        lines.push("This turn answered in the conversation; it changed no files and ran no commands.".into());
        lines.push(String::new());
        // Even here -- especially here: a turn that only answered, after earlier turns built something.
        lines.extend(earlier_work_lines(material));
        lines.push("WHERE IT RAN".into());
        lines.push(material.ran_on.clone());
        lines.push(String::new());
        lines.extend(job_lines().iter().map(|l| (*l).to_owned()));
        return lines.join("\n");
    }

    lines.push("WHAT CHANGED".into());
    if material.changed.is_empty() {
        lines.push("Nothing in the workspace.".into());
    } else {
        lines.extend(material.changed.iter().take(MAX_PATHS).map(|p| format!("- {p}")));
        if material.changed.len() > MAX_PATHS {
            lines.push(format!("- and {} more", material.changed.len() - MAX_PATHS));
        }
    }
    lines.push(String::new());

    lines.push("WHAT RAN".into());
    if material.commands.is_empty() {
        lines.push("No commands were run.".into());
    } else {
        for command in material.commands.iter().take(MAX_COMMANDS) {
            let exit = command.exit_code.map_or_else(|| " (no exit code recorded)".to_owned(), |code| format!(" \u{2014} exit {code}"));
            lines.push(format!("- {}{exit}", command.name));
        }
        if material.commands.len() > MAX_COMMANDS {
            lines.push(format!("- and {} more", material.commands.len() - MAX_COMMANDS));
        }
    }
    lines.push(String::new());

    lines.extend(earlier_work_lines(material));

    lines.push("WHERE IT RAN".into());
    lines.push(material.ran_on.clone());
    lines.push(String::new());

    lines.extend(job_lines().iter().map(|l| (*l).to_owned()));
    lines.join("\n")
}

/// What the conversation built before this turn, labelled as before; a path this turn touched again is this turn's.
fn earlier_work_lines(material: &ReviewMaterial) -> Vec<String> {
    let earlier: Vec<&String> = material.changed_earlier.iter().filter(|p| !material.changed.contains(p)).collect();
    if earlier.is_empty() {
        return Vec::new();
    }
    let mut lines = vec!["CHANGED EARLIER IN THIS CONVERSATION (already done, not this turn)".to_owned()];
    lines.extend(earlier.iter().take(MAX_PATHS).map(|p| format!("- {p}")));
    if earlier.len() > MAX_PATHS {
        lines.push(format!("- and {} more", earlier.len() - MAX_PATHS));
    }
    lines.push(String::new());
    lines
}

/// The job, said the same way whatever kind of work it was.
fn job_lines() -> [&'static str; 8] {
    [
        "YOUR JOB",
        "Say whether the work does what was asked for, and name anything it misses or gets wrong. Read what they said, and the files they changed if there are any; you may run commands to check.",
        "A turn that only answered is a complete piece of work. Judge the answer.",
        "Do not redo the work or make the change yourself, and edit no file: report; do not fix.",
        "What they said about their own work is their account, not evidence: check a claim such as \"the tests pass\" or \"this fixes it\" against the files, or by running it.",
        "Start your answer with a one-line verdict -- Ready, Needs changes, or Start over -- then the reasons.",
        "If it does what was asked, say so plainly and stop \u{2014} a short answer is the right answer for work that is fine. If something is wrong, name the file and what is wrong with it.",
        "The commands above are what was run, not proof that the work is correct: a command exiting 0 says only that it exited 0. Decide for yourself what it does and does not show.",
    ]
}

#[cfg(test)]
#[path = "review_brief_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "review_brief_cases_tests.rs"]
mod cases_tests;
