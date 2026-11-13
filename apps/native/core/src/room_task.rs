//! The room task block (`shared/room-task.ts`): parse `<locust-task>` blocks, strip them for display,
//! format the room's board within budget, and identify an unambiguous open row + member at run start.

use crate::briefing::BLOCK_PLACEMENT;
use crate::protocol_tags::{blocks_outside_code, defang_protocol_blocks, strip_blocks_outside_code};
use regex::Regex;
use std::sync::OnceLock;

pub const TASK_TAG: &str = "locust-task";
pub const MAX_TASK_TEXT_LENGTH: usize = 200;
pub const MAX_TASK_OPS_PER_REPLY: usize = 8;

const EXAMPLE_TEXTS: &[&str] = &["the task text", "a task that should exist and does not"];

fn block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<locust-task\s*>([\s\S]*?)</locust-task>").expect("valid task block regex"))
}

fn line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(new|claim|done|handoff(?:\s+([^:]{1,40}?))?)\s*::\s*(.+?)\s*$")
            .expect("valid task line regex")
    })
}

fn non_word_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[^\p{L}\p{N}]+").expect("valid word key regex"))
}

fn multi_blank_lines_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n{3,}").expect("valid blank line regex"))
}

fn control_chars(c: char) -> bool {
    let u = c as u32;
    matches!(u, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f)
}

pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOp {
    New { text: String },
    Claim { text: String },
    Done { text: String },
    Handoff { text: String, to: String },
}

impl TaskOp {
    pub fn kind_str(&self) -> &'static str {
        match self {
            TaskOp::New { .. } => "new",
            TaskOp::Claim { .. } => "claim",
            TaskOp::Done { .. } => "done",
            TaskOp::Handoff { .. } => "handoff",
        }
    }

    pub fn text(&self) -> &str {
        match self {
            TaskOp::New { text }
            | TaskOp::Claim { text }
            | TaskOp::Done { text }
            | TaskOp::Handoff { text, .. } => text,
        }
    }
}

/// Task text as the board compares it: case, punctuation and spacing blind (`taskKey`).
pub fn task_key(text: &str) -> String {
    let lower = text.to_lowercase();
    non_word_re().replace_all(&lower, " ").trim().to_owned()
}

/// Task text as the board stores it: one line, bounded, no control characters (`boundedTaskText`).
pub fn bounded_task_text(text: &str) -> String {
    let without_ctl: String = text.chars().filter(|c| !control_chars(*c)).collect();
    let mut clean = String::with_capacity(without_ctl.len());
    for word in without_ctl.split_whitespace() {
        if !clean.is_empty() {
            clean.push(' ');
        }
        clean.push_str(word);
    }
    if utf16_len(&clean) <= MAX_TASK_TEXT_LENGTH {
        return clean;
    }
    let limit = MAX_TASK_TEXT_LENGTH - 1;
    let mut used_utf16 = 0;
    let mut end_byte = 0;
    for (idx, ch) in clean.char_indices() {
        let ch_u16 = ch.len_utf16();
        if used_utf16 + ch_u16 > limit {
            break;
        }
        used_utf16 += ch_u16;
        end_byte = idx + ch.len_utf8();
    }
    format!("{}…", clean[..end_byte].trim_end())
}

fn is_example(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    EXAMPLE_TEXTS.iter().any(|ex| *ex == lower)
}

/// Every well-formed operation in every block, in transcript order, capped (`parseTaskBlocks`).
pub fn parse_task_blocks(text: &str) -> Vec<TaskOp> {
    let mut ops = Vec::new();
    for (_, _, body) in blocks_outside_code(text, block_re()) {
        for raw_line in body.split(['\n', '\r']) {
            let Some(caps) = line_re().captures(raw_line) else {
                continue;
            };
            let verb = caps.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
            let task_text = bounded_task_text(caps.get(3).map(|m| m.as_str()).unwrap_or(""));
            if task_text.is_empty() || is_example(&task_text) {
                continue;
            }
            if verb.starts_with("handoff") {
                let to = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                if to.is_empty() {
                    continue;
                }
                ops.push(TaskOp::Handoff {
                    text: task_text,
                    to: to.to_owned(),
                });
            } else if verb == "new" {
                ops.push(TaskOp::New { text: task_text });
            } else if verb == "claim" {
                ops.push(TaskOp::Claim { text: task_text });
            } else if verb == "done" {
                ops.push(TaskOp::Done { text: task_text });
            }
            if ops.len() >= MAX_TASK_OPS_PER_REPLY {
                return ops;
            }
        }
    }
    ops
}

/// The reply without its task blocks; the board shows what they did (`stripTaskBlocks`).
pub fn strip_task_blocks(text: &str) -> String {
    let stripped = strip_blocks_outside_code(text, block_re());
    multi_blank_lines_re()
        .replace_all(&stripped, "\n\n")
        .trim_end()
        .to_owned()
}

/// Defang the tags in text quoted into another runtime's prompt -- every protocol tag (`sanitizeTaskTags`).
pub fn sanitize_task_tags(text: &str) -> String {
    defang_protocol_blocks(text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskBoardState {
    Open,
    InHand,
    Done,
}

impl TaskBoardState {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskBoardState::Open => "open",
            TaskBoardState::InHand => "in-hand",
            TaskBoardState::Done => "done",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskBoardLine {
    pub text: String,
    pub state: TaskBoardState,
    pub owner_name: Option<String>,
    pub handed_by_name: Option<String>,
    pub handed_without_note: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct BoardTaskRef<'a> {
    pub text: &'a str,
    pub state: TaskBoardState,
    pub owner_id: Option<&'a str>,
    pub handed_by: Option<&'a str>,
    pub handed_without_note: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RosterNameRef<'a> {
    pub teammate_id: &'a str,
    pub name: &'a str,
}

/// The board's rows as a member is briefed with them: names, not ids (`boardLines`).
pub fn board_lines(tasks: &[BoardTaskRef<'_>], roster: &[RosterNameRef<'_>]) -> Vec<TaskBoardLine> {
    let name_of = |id: &str| -> String {
        roster
            .iter()
            .find(|entry| entry.teammate_id == id)
            .map(|entry| entry.name.to_owned())
            .unwrap_or_else(|| id.to_owned())
    };
    tasks
        .iter()
        .map(|task| TaskBoardLine {
            text: task.text.to_owned(),
            state: task.state,
            owner_name: task.owner_id.map(name_of),
            handed_by_name: task.handed_by.map(name_of),
            handed_without_note: task.handed_without_note,
        })
        .collect()
}

pub struct TaskSectionInput<'a> {
    pub room_name: &'a str,
    pub self_name: &'a str,
    pub member_names: &'a [&'a str],
    pub tasks: &'a [TaskBoardLine],
}

/// The board, fitted beside a post in the room's per-member prompt (`fittedTaskSection`).
pub fn fitted_task_section(input: &TaskSectionInput<'_>, budget: usize) -> String {
    let full = task_section(input);
    if utf16_len(&full) <= budget {
        return full;
    }
    let mut kept: Vec<TaskBoardLine> = input
        .tasks
        .iter()
        .filter(|t| t.state != TaskBoardState::Done)
        .cloned()
        .collect();
    let with_note = |rows: &[TaskBoardLine]| -> String {
        let left = input.tasks.len().saturating_sub(rows.len());
        let text = task_section(&TaskSectionInput {
            room_name: input.room_name,
            self_name: input.self_name,
            member_names: input.member_names,
            tasks: rows,
        });
        if left == 0 {
            text
        } else {
            let s = if left == 1 { "" } else { "s" };
            format!("{text}\n({left} more row{s} on the board did not fit beside this post.)")
        }
    };
    let mut text = with_note(&kept);
    while utf16_len(&text) > budget && !kept.is_empty() {
        kept.pop();
        text = with_note(&kept);
    }
    if utf16_len(&text) <= budget {
        return text;
    }
    let none = "The room\u{2019}s task board did not fit beside this post.";
    if utf16_len(none) <= budget {
        none.to_owned()
    } else {
        String::new()
    }
}

pub fn task_section(input: &TaskSectionInput<'_>) -> String {
    let others: Vec<&str> = input
        .member_names
        .iter()
        .copied()
        .filter(|name| *name != input.self_name)
        .collect();
    let board = if input.tasks.is_empty() {
        "The board is empty.".to_owned()
    } else {
        input
            .tasks
            .iter()
            .map(|task| {
                let owner = match task.owner_name.as_deref() {
                    None => " (unassigned)".to_owned(),
                    Some(name) if name == input.self_name => match task.handed_by_name.as_deref() {
                        None => " (yours)".to_owned(),
                        Some(by) if task.handed_without_note => format!(
                            " (yours, handed over by {by} with no note: ask them what is done and what is left before you start)"
                        ),
                        Some(by) => format!(" (yours, handed over by {by})"),
                    },
                    Some(name) => format!(" ({name}'s)"),
                };
                format!(
                    "- [{}] {}{}",
                    task.state.as_str(),
                    defang_protocol_blocks(&task.text),
                    owner
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let handoff_example = others.first().copied().unwrap_or("Name");
    let with_whom = if others.is_empty() {
        "nobody else".to_owned()
    } else {
        others.join(", ")
    };
    [
        format!(
            "You are {}, answering in the room \"{}\" with {}. The room keeps a task board:",
            input.self_name, input.room_name, with_whom
        ),
        board,
        format!(
            "If your work claims, finishes, hands off or adds a task on that board, use exactly this block and {BLOCK_PLACEMENT} -- one line per task, quoting the task text as it appears above:"
        ),
        format!("<{TASK_TAG}>"),
        "claim :: the task text".to_owned(),
        "done :: the task text".to_owned(),
        format!("handoff {handoff_example} :: the task text"),
        "new :: a task that should exist and does not".to_owned(),
        format!("</{TASK_TAG}>"),
        "A row marked with another teammate's name is already theirs: leave it alone, and do not claim, redo or report on it. Work an unassigned row, or one marked yours.".to_owned(),
        "If you begin work that a row on that board describes -- because the person asked you to, or because you picked it up -- claim it in the same reply. An unassigned row and a row you are working look the same to everyone else, and the rule above sends them at unassigned rows.".to_owned(),
        "When you hand a task over, send that teammate a message in the same reply: what you did, how you checked it, and what is left.".to_owned(),
        "Never hand a task back to the teammate who handed it to you: if you cannot finish it, say in your reply what is stopping you, and keep it.".to_owned(),
        "Claim only what you are actually doing, mark done only what is finished, and if you touched no task, end with no block.".to_owned(),
    ]
    .join("\n")
}

#[derive(Debug, Clone, Copy)]
pub struct ClaimCandidateTask<'a> {
    pub task_id: &'a str,
    pub text: &'a str,
    pub state: &'a str,
    pub owner_id: Option<&'a str>,
}

/// The one open row a post is plainly about, or nothing (`rowNamedByPost`).
pub fn row_named_by_post<'a>(post_text: &str, tasks: &[ClaimCandidateTask<'a>]) -> Option<&'a str> {
    let lower = post_text.to_lowercase();
    let said = non_word_re().replace_all(&lower, " ");
    let mut named: Vec<&'a str> = Vec::new();
    for task in tasks {
        if task.state != "open" || task.owner_id.is_some() {
            continue;
        }
        let words = task_key(task.text);
        if utf16_len(&words) >= 8 && said.contains(words.as_str()) {
            named.push(task.task_id);
        }
    }
    if named.len() == 1 {
        Some(named[0])
    } else {
        None
    }
}

/// The one member a post is plainly addressed to, or nothing (`memberNamedByPost`).
pub fn member_named_by_post<'a>(
    post_text: &str,
    members: &[RosterNameRef<'a>],
) -> Option<&'a str> {
    if members.len() == 1 {
        return Some(members[0].teammate_id);
    }
    let said = format!(" {} ", task_key(post_text));
    let mut named: Vec<&'a str> = Vec::new();
    for member in members {
        let name = task_key(member.name);
        if utf16_len(&name) >= 2 && said.contains(&format!(" {name} ")) {
            named.push(member.teammate_id);
        }
    }
    if named.len() == 1 {
        Some(named[0])
    } else {
        None
    }
}

fn split_sentences(post_text: &str) -> Vec<&str> {
    post_text
        .split(['.', '!', '?', '\n', '\r'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

/// The row a teammate starting right now should be given, or nothing (`rowToClaimAtStart`).
pub fn row_to_claim_at_start<'a>(
    post_text: &str,
    tasks: &[ClaimCandidateTask<'a>],
    members: &[RosterNameRef<'a>],
    started_teammate_id: &str,
) -> Option<&'a str> {
    let row = row_named_by_post(post_text, tasks)?;
    let text = tasks.iter().find(|t| t.task_id == row)?.text;
    let wanted = task_key(text);
    let about: Vec<&str> = split_sentences(post_text)
        .into_iter()
        .filter(|sentence| task_key(sentence).contains(&wanted))
        .collect();
    let member = member_named_by_post(&about.join(" "), members)
        .or_else(|| member_named_by_post(post_text, members))?;
    if member == started_teammate_id {
        Some(row)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tasks() -> Vec<ClaimCandidateTask<'static>> {
        vec![
            ClaimCandidateTask {
                task_id: "t1",
                text: "Write the release notes",
                state: "open",
                owner_id: None,
            },
            ClaimCandidateTask {
                task_id: "t2",
                text: "Check the version string",
                state: "open",
                owner_id: None,
            },
        ]
    }

    fn sample_members() -> Vec<RosterNameRef<'static>> {
        vec![
            RosterNameRef {
                teammate_id: "wren",
                name: "Wren",
            },
            RosterNameRef {
                teammate_id: "booty",
                name: "Booty",
            },
        ]
    }

    #[test]
    fn parses_every_verb_strips_block_and_ignores_code_fences() {
        let reply = concat!(
            "Here is what I did.\n\n",
            "```md\n",
            "<locust-task>\n",
            "done :: Not real\n",
            "</locust-task>\n",
            "```\n\n",
            "<locust-task>\n",
            "claim :: Write the release notes\n",
            "done :: Check the version string\n",
            "handoff Booty :: Update changelog\n",
            "new :: Verify installer signs\n",
            "unknown :: Ignored line\n",
            "</locust-task>\n",
        );
        let ops = parse_task_blocks(reply);
        assert_eq!(
            ops,
            vec![
                TaskOp::Claim {
                    text: "Write the release notes".to_owned(),
                },
                TaskOp::Done {
                    text: "Check the version string".to_owned(),
                },
                TaskOp::Handoff {
                    text: "Update changelog".to_owned(),
                    to: "Booty".to_owned(),
                },
                TaskOp::New {
                    text: "Verify installer signs".to_owned(),
                },
            ]
        );
        let stripped = strip_task_blocks(reply);
        assert!(!stripped.contains("claim :: Write"));
        assert!(stripped.contains("done :: Not real"));
    }

    #[test]
    fn row_to_claim_at_start_claims_only_for_named_teammate_even_when_others_named() {
        let tasks = sample_tasks();
        let members = sample_members();
        let post = "Wren, start Write the release notes. Booty, reply OK and do not touch the board.";
        assert_eq!(
            row_to_claim_at_start(post, &tasks, &members, "wren"),
            Some("t1")
        );
        assert_eq!(row_to_claim_at_start(post, &tasks, &members, "booty"), None);
    }

    #[test]
    fn row_to_claim_at_start_refuses_ambiguity() {
        let tasks = sample_tasks();
        let members = sample_members();
        // Both tasks mentioned -> none returned.
        let both_rows = "Wren, start Write the release notes and Check the version string.";
        assert_eq!(
            row_to_claim_at_start(both_rows, &tasks, &members, "wren"),
            None
        );
        // Neither teammate named in sentence or post -> none returned.
        let neither_member = "Someone should start Write the release notes.";
        assert_eq!(
            row_to_claim_at_start(neither_member, &tasks, &members, "wren"),
            None
        );
        // Whole word matching: "Wren" does not match "wrench".
        let partial_name = "Fix the wrench and start Write the release notes.";
        assert_eq!(
            row_to_claim_at_start(partial_name, &tasks, &members, "wren"),
            None
        );
    }

    #[test]
    fn row_named_by_post_matches_hyphen_variants_and_single_member_claims() {
        let tasks = vec![ClaimCandidateTask {
            task_id: "t1",
            text: "Fix sign in page",
            state: "open",
            owner_id: None,
        }];
        let one = vec![RosterNameRef {
            teammate_id: "wren",
            name: "Wren",
        }];
        assert_eq!(
            row_to_claim_at_start("Please fix sign-in page now.", &tasks, &one, "wren"),
            Some("t1")
        );
    }

    #[test]
    fn fitted_task_section_sheds_done_then_trailing_rows_to_fit_budget() {
        let lines: Vec<TaskBoardLine> = vec![
            TaskBoardLine {
                text: "Open first task".to_owned(),
                state: TaskBoardState::Open,
                owner_name: None,
                handed_by_name: None,
                handed_without_note: false,
            },
            TaskBoardLine {
                text: "Open second task".to_owned(),
                state: TaskBoardState::Open,
                owner_name: Some("Wren".to_owned()),
                handed_by_name: None,
                handed_without_note: false,
            },
            TaskBoardLine {
                text: "Already finished task with a long description so shedding the done row shrinks the prompt".to_owned(),
                state: TaskBoardState::Done,
                owner_name: Some("Booty".to_owned()),
                handed_by_name: None,
                handed_without_note: false,
            },
        ];
        let input = TaskSectionInput {
            room_name: "Shipping",
            self_name: "Wren",
            member_names: &["Wren", "Booty"],
            tasks: &lines,
        };
        let full = task_section(&input);
        let tighter = utf16_len(&full) - 5;
        let fitted = fitted_task_section(&input, tighter);
        assert!(utf16_len(&fitted) <= tighter);
        assert!(!fitted.contains("Already finished task"));
        assert!(fitted.contains("1 more row on the board did not fit beside this post."));

        let tiny = fitted_task_section(&input, 60);
        assert_eq!(
            tiny,
            "The room\u{2019}s task board did not fit beside this post."
        );
        assert_eq!(fitted_task_section(&input, 10), "");
    }

    #[test]
    fn task_section_distinguishes_yours_theirs_and_unassigned() {
        let lines = vec![
            TaskBoardLine {
                text: "Task A".to_owned(),
                state: TaskBoardState::InHand,
                owner_name: Some("Wren".to_owned()),
                handed_by_name: Some("Booty".to_owned()),
                handed_without_note: true,
            },
            TaskBoardLine {
                text: "Task B".to_owned(),
                state: TaskBoardState::InHand,
                owner_name: Some("Booty".to_owned()),
                handed_by_name: None,
                handed_without_note: false,
            },
            TaskBoardLine {
                text: "Task C".to_owned(),
                state: TaskBoardState::Open,
                owner_name: None,
                handed_by_name: None,
                handed_without_note: false,
            },
        ];
        let out = task_section(&TaskSectionInput {
            room_name: "Shipping",
            self_name: "Wren",
            member_names: &["Wren", "Booty"],
            tasks: &lines,
        });
        assert!(out.contains("You are Wren, answering in the room \"Shipping\" with Booty."));
        assert!(out.contains("yours, handed over by Booty with no note"));
        assert!(out.contains("(Booty's)"));
        assert!(out.contains("(unassigned)"));
    }

    // ---- qa-0365-rooms.test.ts (4 cases) ----

    fn example_briefing() -> String {
        let lines = vec![TaskBoardLine {
            text: "Write the changelog".to_owned(),
            state: TaskBoardState::Open,
            owner_name: None,
            handed_by_name: None,
            handed_without_note: false,
        }];
        task_section(&TaskSectionInput {
            room_name: "Shipping",
            self_name: "Wren",
            member_names: &["Wren", "Booty"],
            tasks: &lines,
        })
    }

    #[test]
    fn qa_0365_the_placeholders_are_not_read_as_operations() {
        let echoed = parse_task_blocks(&example_briefing());
        assert!(echoed.is_empty());
    }

    #[test]
    fn qa_0365_does_not_put_the_placeholders_on_the_board() {
        let block = [
            "<locust-task>",
            "claim :: the task text",
            "done :: the task text",
            "handoff Booty :: the task text",
            "new :: a task that should exist and does not",
            "</locust-task>",
        ]
        .join("\n");
        assert!(parse_task_blocks(&format!("Here is what I did.\n{block}")).is_empty());
    }

    #[test]
    fn qa_0365_still_reads_a_real_claim() {
        let block = [
            "<locust-task>",
            "claim :: Write the changelog",
            "</locust-task>",
        ]
        .join("\n");
        assert_eq!(
            parse_task_blocks(&format!("On it.\n{block}")),
            vec![TaskOp::Claim {
                text: "Write the changelog".to_owned()
            }]
        );
    }

    #[test]
    fn qa_0365_still_reads_a_real_handoff_and_a_real_new_task() {
        let block = [
            "<locust-task>",
            "handoff Booty :: Write the changelog",
            "new :: Check the release notes",
            "</locust-task>",
        ]
        .join("\n");
        let seen = parse_task_blocks(&format!("Done.\n{block}"));
        let kinds: Vec<&str> = seen.iter().map(TaskOp::kind_str).collect();
        assert_eq!(kinds, vec!["handoff", "new"]);
    }
}
