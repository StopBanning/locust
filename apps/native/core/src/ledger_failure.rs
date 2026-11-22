//! What to say when Locust cannot write its own record (Electron's `ledgerFailure.ts`). The record is the product's
//! central claim, so the one moment it breaks is said exactly: the run was STOPPED, never "held" (the engine ends the
//! run when a write fails: `native::drive_runtime` returns and the runtime's drop ends its process); what is safe and
//! what is at risk, split because both are knowable (the record only ever appends); and no checkpoint number made up.

/// The card's title: what happened to the person's run, not the app's fault (design, 2026-09-08).
pub const TITLE: &str = "Stopped — the mission ledger could not be written";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// `safe` or `risk`.
    pub tone: &'static str,
    pub label: &'static str,
    pub text: String,
}

/// The rows the card draws (`ledgerFailureRows`). `checkpoints` is a recovered conversation's count when there is one,
/// and None during a live run; `nothing_written` is a turn whose record could not even be made, so nothing ran.
pub fn rows(checkpoints: Option<usize>, nothing_written: bool) -> Vec<Row> {
    if nothing_written {
        return vec![
            Row {
                tone: "safe",
                label: "Safe",
                text: "Everything. The record is written before the runtime starts, so nothing ran, nothing was changed, and your project files are exactly as they were.".into(),
            },
            Row {
                tone: "risk",
                label: "At risk",
                text: "Nothing from this turn — it never began. Locust refused to start work it could not record. Fix the ledger and send it again.".into(),
            },
        ];
    }
    let kept = match checkpoints {
        None => "Everything written before the failure.".to_owned(),
        Some(n) => format!("The {n} checkpoint{} already written.", if n == 1 { "" } else { "s" }),
    };
    vec![
        Row {
            tone: "safe",
            label: "Safe",
            text: format!("{kept} The ledger only ever appends, so what reached the disk is intact and this conversation can be reopened from it. Your project files are untouched by this failure — nothing was rolled back."),
        },
        Row {
            tone: "risk",
            label: "At risk",
            text: "This turn. Anything the runtime did after the last successful write was not recorded, so Locust cannot tell you what it was, and it will not be here when you reopen this conversation — check the folder yourself before trusting it.".into(),
        },
        Row {
            tone: "risk",
            label: "Not undone",
            text: "Files the run already changed are still changed. Stopping the run stops new work; it does not roll back finished work.".into(),
        },
    ]
}

/// The sentence under the title (`ledgerFailureSentence`): the host's own reason quoted, never a guessed cause, and
/// Locust named as the one that stopped the run.
pub fn sentence(reason: Option<&str>) -> String {
    match reason.map(str::trim).filter(|r| !r.is_empty()) {
        None => "Locust could not write this run to its record on this machine, so it stopped the run rather than go on without one.".into(),
        Some(said) => format!("{said} Locust stopped the run rather than continue without a durable record."),
    }
}

/// The whole of it as lines, for a place that draws text: title, sentence, then each row as "Label: text".
pub fn lines(reason: Option<&str>, checkpoints: Option<usize>, nothing_written: bool) -> Vec<String> {
    let mut out = vec![TITLE.to_owned(), sentence(reason)];
    out.extend(rows(checkpoints, nothing_written).into_iter().map(|row| format!("{}: {}", row.label, row.text)));
    out
}

#[cfg(test)]
#[path = "ledger_failure_tests.rs"]
mod tests;
