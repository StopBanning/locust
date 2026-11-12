//! WHEN LOCUST TAPS YOU ON THE SHOULDER (main/attention.ts, renderer finishedToast.ts).
//!
//! A system notification only while Locust's window is not the one in front: a card waiting for an answer, a question,
//! a plan that ran out, and a long turn you started that ended (finishes inside four seconds of each other said as one).
//! The words are Electron's.

/// shouldNotify: supported, and the window is elsewhere.
pub fn should_notify(focused: bool, supported: bool) -> bool {
    supported && !focused
}

fn clip(text: &str, max: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        return one;
    }
    format!("{}…", one.chars().take(max - 1).collect::<String>().trim_end())
}

/// approvalNotificationText, with the conversation's title after it when there is one.
pub fn approval_text(question: bool, summary: &str, teammate: Option<&str>, conversation: Option<&str>) -> (String, String) {
    let who = teammate.unwrap_or("A teammate");
    let mut title = if question { format!("{who} has a question") } else { format!("{who} needs your approval") };
    if let Some(c) = conversation {
        title = format!("{title} · {}", c.chars().take(100).collect::<String>());
    }
    (title, format!("{summary} · the run is paused until you answer."))
}

/// decisionNotificationText.
pub fn decision_text(teammate: Option<&str>, question: &str) -> (String, String) {
    (
        format!("{} is asking you something", teammate.unwrap_or("A teammate")),
        format!("{} · the run waits for your answer.", clip(question, 140)),
    )
}

/// limitNotificationText.
pub fn limit_text(teammate: Option<&str>, runtime: &str, message: Option<&str>) -> (String, String) {
    let said = message.map(|m| m.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|m| !m.is_empty());
    (
        format!("{} hit the {runtime} limit", teammate.unwrap_or("A teammate")),
        format!("{} · pick where it continues.", said.map(|s| clip(&s, 120)).unwrap_or_else(|| "The run stopped at a checkpoint".into())),
    )
}

/// FINISH_TOAST_WINDOW_MS: finishes this close together are said as one.
pub const FINISH_WINDOW_MS: u64 = 4_000;
/// FINISH_WORTH_TELLING_MS: a turn shorter than a minute was watched; it is not worth a toast.
pub const FINISH_WORTH_TELLING_MS: u64 = 60_000;

/// finishedNotificationText: one finish as it is, several as "N teammates are done" with the last three.
pub fn finished_text(finishes: &[(String, String)]) -> (String, String) {
    if finishes.len() == 1 {
        return finishes[0].clone();
    }
    let shown = &finishes[finishes.len().saturating_sub(3)..];
    let hidden = finishes.len() - shown.len();
    let mut lines = Vec::new();
    if hidden > 0 {
        lines.push(format!("… and {hidden} more"));
    }
    lines.extend(shown.iter().map(|(t, b)| format!("{t}: {b}")));
    (format!("{} teammates are done", finishes.len()), lines.join("\n"))
}

fn took_text(ms: u64) -> String {
    let minutes = ((ms as f64 / 60_000.0).round() as u64).max(1);
    if minutes < 60 { format!("{minutes} min") } else { format!("{} h {} min", minutes / 60, minutes % 60) }
}

/// finishedToast: "Wren finished" for a turn the person started that ran at least a minute and ended, with its first
/// line; "Wren stopped" with why, for one that failed. None for anything else.
pub fn finished_toast(completed: bool, failed: bool, started_by_person: bool, took_ms: u64, teammate: Option<&str>, last_said: Option<&str>, error: Option<&str>) -> Option<(String, String)> {
    if !started_by_person || (!completed && !failed) || took_ms < FINISH_WORTH_TELLING_MS {
        return None;
    }
    let who = teammate.unwrap_or("A teammate");
    if failed {
        let why = error.and_then(|e| e.lines().next()).map(str::trim).unwrap_or("");
        let said = if why.is_empty() { "The run did not finish.".to_owned() } else { clip(why, 140) };
        return Some((format!("{who} stopped"), format!("{said} · after {}", took_text(took_ms))));
    }
    let line = last_said.and_then(|t| t.lines().map(str::trim).find(|l| !l.is_empty())).map(|l| clip(l, 140));
    Some((format!("{who} finished"), format!("{} · took {}", line.unwrap_or_else(|| "Done.".into()), took_text(took_ms))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_say_what_electron_says() {
        assert!(should_notify(false, true) && !should_notify(true, true) && !should_notify(false, false));
        assert_eq!(
            approval_text(false, "Run npm test", Some("Codec"), Some("Fix the build")),
            ("Codec needs your approval · Fix the build".into(), "Run npm test · the run is paused until you answer.".into())
        );
        assert_eq!(approval_text(true, "Which one?", None, None).0, "A teammate has a question");
        assert_eq!(limit_text(Some("Scout"), "OpenCode", None).1, "The run stopped at a checkpoint · pick where it continues.");
        assert_eq!(decision_text(None, "Ship  it?\nnow").1, "Ship it? now · the run waits for your answer.");
    }

    #[test]
    fn long_turns_you_started_say_how_they_ended_and_close_finishes_are_one() {
        assert_eq!(finished_toast(true, false, true, 30_000, Some("Wren"), Some("Done"), None), None, "under a minute is not worth it");
        assert_eq!(finished_toast(true, false, false, 120_000, Some("Wren"), Some("Done"), None), None, "an app's turn is not yours");
        assert_eq!(
            finished_toast(true, false, true, 125_000, Some("Wren"), Some("\nAll tests pass.\nMore."), None),
            Some(("Wren finished".into(), "All tests pass. · took 2 min".into()))
        );
        assert_eq!(finished_toast(false, true, true, 3_900_000, None, None, Some("Codex exited")).unwrap().1, "Codex exited · after 1 h 5 min");
        let many: Vec<(String, String)> = (1..=5).map(|i| (format!("T{i}"), "ok".into())).collect();
        assert_eq!(finished_text(&many), ("5 teammates are done".into(), "… and 2 more\nT3: ok\nT4: ok\nT5: ok".into()));
    }
}
