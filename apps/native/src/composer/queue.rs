//! Busy-turn follow-up queue — port of `steering.ts` + `conversationQueue.ts` rules.

pub const QUEUE_RESTORED_NOTE: &str = "saved before Locust closed — press Send to continue";
pub const QUEUE_SEPARATOR: &str = "\n\n";
pub const NOT_SENT_HELD: &str = "not sent — press Send to try again";

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Person,
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
        let rows = vec![person("1", "a", "a1"), person("2", "b", "b1"), person("3", "a", "a2")];
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
