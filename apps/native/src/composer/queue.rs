//! Busy-turn follow-up queue — port of `steering.ts` + `conversationQueue.ts` rules.

pub const QUEUE_RESTORED_NOTE: &str = "saved before Locust closed — press Send to continue";
pub const QUEUE_SEPARATOR: &str = "\n\n";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueuedVerdict {
    Waiting,
    Send,
    Held { note: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedRow {
    pub id: String,
    pub key: String,
    pub text: String,
    pub origin: Origin,
    pub attachments: Vec<String>,
    pub issue: Option<serde_json::Value>,
    pub restored: bool,
    pub held: Option<String>,
    /// When a row the host refused as busy may be tried again (ms since the epoch). See `retried_after_busy`. Never
    /// saved: Electron's queue file has no such field.
    #[allow(dead_code)]
    pub retry_at: Option<u64>,
    /// How many times the host has refused it as busy.
    #[allow(dead_code)]
    pub tries: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Person,
    /// A row the host queued (a routine's next step, a decision reply, a relay hand-off): never merged, since that would
    /// put two intents in one instruction under whoever typed last. Nothing queues one yet, in either app.
    #[allow(dead_code)]
    Host,
}

pub fn queued_verdict(input: VerdictInput<'_>) -> QueuedVerdict {
    if let Some(held) = input.held {
        return QueuedVerdict::Held {
            note: held.to_owned(),
        };
    }
    if input.restored {
        return QueuedVerdict::Held {
            note: QUEUE_RESTORED_NOTE.into(),
        };
    }
    let Some(phase) = input.phase else {
        return QueuedVerdict::Held {
            note: "held — that conversation is no longer open".into(),
        };
    };
    if input.running {
        return QueuedVerdict::Waiting;
    }
    if phase != "completed" {
        let why = match phase {
            "cancelled" => "was stopped",
            "failed" => "failed",
            _ => "did not finish",
        };
        return QueuedVerdict::Held {
            note: format!("held — the run {why}"),
        };
    }
    if !input.on_screen {
        return QueuedVerdict::Held {
            note: "typed in another conversation — sending puts it here instead".into(),
        };
    }
    QueuedVerdict::Send
}

#[derive(Clone, Copy, Debug)]
pub struct VerdictInput<'a> {
    pub running: bool,
    pub phase: Option<&'a str>,
    pub on_screen: bool,
    pub restored: bool,
    pub held: Option<&'a str>,
}

/// A queued message the host refused as busy goes back in line (Electron's `retriedAfterBusy`, `steering.ts`): pointed
/// at the conversation it was typed in, and tried again after half a second, then one, two, four, and every eight
/// after that.
///
/// Not wired yet: the window hands a queued message to the host and does not hear a busy refusal back for that row
/// (`drain_queue_after_turn` removes it first, so an uncertain result never retries itself). The backoff is here for
/// when it does.
#[allow(dead_code)]
pub fn retried_after_busy(row: &QueuedRow, key: &str, now: u64) -> QueuedRow {
    let tries = row.tries.unwrap_or(0).saturating_add(1);
    let wait = 250u64.saturating_mul(2u64.saturating_pow(tries)).min(8_000);
    QueuedRow { key: key.to_owned(), tries: Some(tries), retry_at: Some(now.saturating_add(wait)), ..row.clone() }
}

pub fn can_merge_front(row: &QueuedRow) -> bool {
    row.origin == Origin::Person && !row.text.trim().is_empty()
}

pub fn can_merge_follower(row: &QueuedRow) -> bool {
    can_merge_front(row) && row.attachments.is_empty()
}

pub fn combine_queued(rows: &[QueuedRow]) -> Vec<QueuedRow> {
    let Some(front) = rows.first() else {
        return Vec::new();
    };
    if !can_merge_front(front) {
        return rows.to_vec();
    }
    let mut taken = 1;
    while taken < rows.len() {
        let next = &rows[taken];
        if next.key != front.key
            || next.restored != front.restored
            || next.held != front.held
            || !can_merge_follower(next)
        {
            break;
        }
        taken += 1;
    }
    if taken == 1 {
        return rows.to_vec();
    }
    let mut merged = front.clone();
    merged.text = rows[..taken]
        .iter()
        .map(|r| r.text.trim())
        .collect::<Vec<_>>()
        .join(QUEUE_SEPARATOR);
    let mut out = vec![merged];
    out.extend(rows[taken..].iter().cloned());
    out
}

pub fn queued_in<'a>(rows: &'a [QueuedRow], key: Option<&str>) -> Vec<&'a QueuedRow> {
    let Some(key) = key else {
        return Vec::new();
    };
    rows.iter().filter(|r| r.key == key).collect()
}

pub fn take_next(rows: &[QueuedRow], key: &str) -> (Option<QueuedRow>, Vec<QueuedRow>) {
    let own: Vec<QueuedRow> = rows.iter().filter(|r| r.key == key).cloned().collect();
    let folded = combine_queued(&own);
    let Some(going) = folded.first().cloned() else {
        return (None, rows.to_vec());
    };
    let went_count = own.len() - (folded.len() - 1);
    let mut skipped = 0usize;
    let rest: Vec<QueuedRow> = rows
        .iter()
        .filter(|row| {
            if row.key == key && skipped < went_count {
                skipped += 1;
                false
            } else {
                true
            }
        })
        .cloned()
        .collect();
    (Some(going), rest)
}

pub fn without_queue_of(rows: &[QueuedRow], key: Option<&str>) -> Vec<QueuedRow> {
    let Some(key) = key else {
        return rows.to_vec();
    };
    rows.iter().filter(|r| r.key != key).cloned().collect()
}

pub fn requeued_rows(rows: &[QueuedRow], from_key: &str, to_key: &str) -> Vec<QueuedRow> {
    rows.iter()
        .map(|row| {
            if row.key == from_key {
                let mut r = row.clone();
                r.key = to_key.to_owned();
                r
            } else {
                row.clone()
            }
        })
        .collect()
}

/// Note shown under the queued bubble (Composer.tsx).
pub fn queue_note(verdict: &QueuedVerdict, working_name: &str, count: usize) -> String {
    let base = match verdict {
        QueuedVerdict::Waiting => format!("Sends when {working_name} finishes"),
        QueuedVerdict::Send => format!("Sends when {working_name} finishes"),
        QueuedVerdict::Held { note } => {
            let mut chars = note.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => note.clone(),
            }
        }
    };
    if count > 1 {
        format!("{count} messages, sent as one · {base}")
    } else {
        base
    }
}

#[cfg(test)]
#[path = "steering_tests.rs"]
mod steering_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn person(id: &str, key: &str, text: &str) -> QueuedRow {
        QueuedRow {
            id: id.into(),
            key: key.into(),
            text: text.into(),
            origin: Origin::Person,
            attachments: Vec::new(),
            issue: None,
            restored: false,
            held: None,
            retry_at: None,
            tries: None,
        }
    }

    #[test]
    fn take_next_folds_same_conversation() {
        let rows = vec![
            person("1", "a", "one"),
            person("2", "a", "two"),
            person("3", "b", "other"),
        ];
        let (going, rest) = take_next(&rows, "a");
        assert_eq!(going.unwrap().text, "one\n\ntwo");
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].key, "b");
    }

    #[test]
    fn take_next_order_preserves_other_conversations() {
        let rows = vec![
            person("1", "a", "a1"),
            person("2", "b", "b1"),
            person("3", "a", "a2"),
        ];
        let (going, rest) = take_next(&rows, "a");
        assert_eq!(going.unwrap().text, "a1\n\na2");
        assert_eq!(rest, vec![person("2", "b", "b1")]);
    }

    #[test]
    fn verdict_restored_and_failed() {
        assert!(matches!(
            queued_verdict(VerdictInput {
                running: false,
                phase: Some("completed"),
                on_screen: true,
                restored: true,
                held: None,
            }),
            QueuedVerdict::Held { .. }
        ));
        let held = queued_verdict(VerdictInput {
            running: false,
            phase: Some("failed"),
            on_screen: true,
            restored: false,
            held: None,
        });
        assert!(matches!(held, QueuedVerdict::Held { note } if note.contains("failed")));
    }

    #[test]
    fn requeue_follows_key() {
        let rows = vec![person("1", "temp", "hi")];
        let next = requeued_rows(&rows, "temp", "real");
        assert_eq!(next[0].key, "real");
    }
}

/// Electron's exact queue-isolation cases. Busy-elsewhere wiring remains unported.
#[cfg(test)]
mod renderer_conversation_tests {
    use super::*;
    fn row(id: &str, key: &str) -> QueuedRow {
        QueuedRow { id: id.into(), key: key.into(), text: format!("{key}: {id}"), origin: Origin::Person,
            attachments: vec![], issue: None, restored: false, held: None, retry_at: None, tries: None }
    }
    fn said(id: &str, key: &str, text: &str) -> QueuedRow { QueuedRow { text: text.into(), ..row(id, key) } }
    fn ids(rows: &[QueuedRow]) -> Vec<&str> { rows.iter().map(|r| r.id.as_str()).collect() }
    #[test]
    fn is_its_own_rows_in_the_order_they_were_typed() {
        let rows = [row("a", "pip"), row("b", "gem"), row("c", "pip")];
        assert_eq!(queued_in(&rows, Some("pip")).iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        assert_eq!(queued_in(&rows, Some("gem")).iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["b"]);
        assert!(queued_in(&rows, Some("nobody")).is_empty());
        assert!(queued_in(&rows, None).is_empty());
    }
    #[test]
    fn never_folds_into_another_conversations_message() {
        let folded = combine_queued(&[row("a", "pip"), row("b", "gem"), row("c", "pip")]);
        assert_eq!(folded.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["pip: a", "gem: b", "pip: c"]);
    }
    #[test]
    fn still_folds_lines_typed_during_one_run_into_one_message() {
        let folded = combine_queued(&[row("a", "pip"), row("b", "pip")]);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].text, ["pip: a", "pip: b"].join(QUEUE_SEPARATOR));
    }
    #[test]
    fn takes_that_conversations_rows_folded_and_leaves_every_other_row_where_it_was() {
        let (going, rest) = take_next(&[row("a", "pip"), row("b", "gem"), row("c", "pip"), row("d", "gem")], "gem");
        let going = going.unwrap();
        assert_eq!(going.text, ["gem: b", "gem: d"].join(QUEUE_SEPARATOR));
        assert_eq!(going.key, "gem");
        assert_eq!(ids(&rest), ["a", "c"]);
    }
    #[test]
    fn takes_nothing_from_a_conversation_with_nothing_queued() {
        let rows = vec![row("a", "pip")];
        assert_eq!(take_next(&rows, "gem"), (None, rows.clone()));
    }
    #[test]
    fn goes_by_the_row_itself_two_rows_can_be_given_the_same_id() {
        let first = row("q_1_pip", "pip");
        let other = said("q_1_pip", "gem", "gem: same id");
        let (going, rest) = take_next(&[first, other.clone()], "pip");
        assert_eq!(going.unwrap().text, "pip: q_1_pip");
        assert_eq!(rest, [other]);
    }
    #[test]
    fn empty_that_conversations_queue_and_nobody_elses() {
        let rows = vec![row("a", "pip"), row("b", "gem"), row("c", "pip")];
        assert_eq!(ids(&without_queue_of(&rows, Some("gem"))), ["a", "c"]);
        assert_eq!(without_queue_of(&rows, None), rows);
    }
    #[test]
    fn the_reviewers_case_gems_line_edited_pips_untouched_each_sent_to_its_own() {
        let rows = [said("p", "pip", "PIP_QUEUED"), said("g", "gem", "GEM_QUEUED")];
        let mut rows = without_queue_of(&rows, Some("gem"));
        rows.push(said("g2", "gem", "GEM_EDITED"));
        assert_eq!(queued_in(&rows, Some("pip"))[0].text, "PIP_QUEUED");
        let (pip, rest) = take_next(&rows, "pip");
        assert_eq!(pip.unwrap().text, "PIP_QUEUED");
        let (gem, rest) = take_next(&rest, "gem");
        assert_eq!(gem.unwrap().text, "GEM_EDITED");
        assert!(rest.is_empty());
    }
    #[test]
    fn sends_each_its_own_as_each_run_ends_in_either_order_the_others_staying_pending() {
        let rows = [said("p1", "pip", "PIP_1"), said("g1", "gem", "GEM_1"), said("p2", "pip", "PIP_2")];
        let (gem, rest) = take_next(&rows, "gem");
        assert_eq!(gem.unwrap().text, "GEM_1");
        assert_eq!(queued_in(&rest, Some("pip")).iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["PIP_1", "PIP_2"]);
        let (pip, rest) = take_next(&rest, "pip");
        assert_eq!(pip.unwrap().text, ["PIP_1", "PIP_2"].join(QUEUE_SEPARATOR));
        assert!(rest.is_empty());
        let (pip, rest) = take_next(&rows, "pip");
        assert_eq!(pip.unwrap().text, ["PIP_1", "PIP_2"].join(QUEUE_SEPARATOR));
        assert_eq!(queued_in(&rest, Some("gem"))[0].text, "GEM_1");
    }
    #[test]
    fn sends_nothing_the_words_are_back_in_the_box_not_the_queue_and_leaves_the_other_conversation_queued() {
        let rows = [said("g", "gem", "GEM_QUEUED"), said("p", "pip", "PIP_QUEUED")];
        let editing = without_queue_of(&rows, Some("gem"));
        let (going, rest) = take_next(&editing, "gem");
        assert!(going.is_none());
        assert_eq!(rest, editing);
        assert_eq!(queued_in(&rest, Some("pip"))[0].text, "PIP_QUEUED");
    }
}

#[cfg(test)]
#[path = "combine_queued_tests.rs"]
mod combine_queued_tests;

#[cfg(test)]
#[path = "retried_after_busy_tests.rs"]
mod retried_after_busy_tests;

#[cfg(test)]
#[path = "restored_queue_tests.rs"]
mod restored_queue_tests;
