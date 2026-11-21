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

/// decisionNotificationText, with the conversation's title after it when there is one.
pub fn decision_text(teammate: Option<&str>, question: &str, conversation: Option<&str>) -> (String, String) {
    let mut title = format!("{} is asking you something", teammate.unwrap_or("A teammate"));
    if let Some(c) = conversation {
        title = format!("{title} · {}", c.chars().take(100).collect::<String>());
    }
    (title, format!("{} · the run waits for your answer.", clip(question, 140)))
}

/// conversationAttentionTitle (index.ts): the conversation's name as the sidebar's titles keep it, the turn's own or,
/// walking back through what each turn continued (500 at most, a loop stopped), the oldest one titled; else the first
/// turn's words; else "Conversation". One line, 100 characters at most. `metadata_of` reads a turn's record.
pub fn conversation_title(mission_id: &str, titles: &serde_json::Value, metadata_of: impl Fn(&str) -> Option<serde_json::Value>) -> String {
    let mut record = metadata_of(mission_id);
    let mut title = titles[mission_id].as_str().map(str::to_owned);
    let mut seen = std::collections::HashSet::new();
    while let Some(parent) = record.as_ref().and_then(|m| m["continuesFrom"]["missionId"].as_str().map(str::to_owned)) {
        if seen.len() >= 500 || !seen.insert(parent.clone()) {
            break;
        }
        if let Some(named) = titles[parent.as_str()].as_str() {
            title = Some(named.to_owned());
        }
        match metadata_of(&parent) {
            Some(older) => record = Some(older),
            None => break,
        }
    }
    let said = title.or_else(|| record.as_ref().and_then(|m| m["prompt"].as_str().map(str::to_owned))).unwrap_or_else(|| "Conversation".into());
    said.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(100).collect()
}

/// approvalArrived: a card waiting for the person, named for its conversation, and a click opens the turn that raised it.
pub fn approval_notice(mission_id: &str, question: bool, summary: &str, teammate: Option<&str>, conversation: Option<&str>) -> Notice {
    let (title, body) = approval_text(question, summary, teammate, conversation);
    Notice { title, body, opens: Some(mission_id.to_owned()), limit: false }
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

/// ROOM_TOAST_WINDOW_MS: a room's changes are gathered this long, then said as one. Twenty seconds was the first cut,
/// and three teammates ending runs thirty seconds apart still made one toast per change; two minutes made a set.
pub const ROOM_TOAST_WINDOW_MS: u64 = 120_000;
/// ROOM_TOAST_LINES: the newest lines a room's toast shows; the rest are counted.
pub const ROOM_TOAST_LINES: usize = 3;

/// roomNotificationText: one change titled with the room's name, several as "Room · N changes" with the newest three
/// and a count of the rest.
pub fn room_text(room_name: &str, messages: &[String]) -> (String, String) {
    let shown = &messages[messages.len().saturating_sub(ROOM_TOAST_LINES)..];
    let hidden = messages.len() - shown.len();
    let title = if messages.len() == 1 { room_name.to_owned() } else { format!("{room_name} · {} changes", messages.len()) };
    let mut lines = Vec::new();
    if hidden > 0 {
        lines.push(format!("… and {hidden} more"));
    }
    lines.extend(shown.iter().cloned());
    (title, lines.join("\n"))
}

/// createAttention's `roomChanged` and its flush: each room's changes are held from its first change for
/// `ROOM_TOAST_WINDOW_MS`, then given back as one toast and forgotten. Whether the toast is shown is asked at that
/// moment (`should_notify`), not when the change came: the room screen already says it to a person looking.
#[derive(Debug, Default)]
pub struct RoomToasts {
    held: Vec<HeldRoom>,
}

#[derive(Debug)]
struct HeldRoom {
    room_id: String,
    room_name: String,
    messages: Vec<String>,
    due: std::time::Instant,
}

impl RoomToasts {
    /// A room changed: added to its held toast, or the start of one.
    pub fn changed(&mut self, room_id: &str, room_name: &str, message: &str, now: std::time::Instant) {
        if let Some(held) = self.held.iter_mut().find(|h| h.room_id == room_id) {
            held.messages.push(message.to_owned());
            return;
        }
        self.held.push(HeldRoom {
            room_id: room_id.to_owned(),
            room_name: room_name.to_owned(),
            messages: vec![message.to_owned()],
            due: now + std::time::Duration::from_millis(ROOM_TOAST_WINDOW_MS),
        });
    }

    /// The toasts whose window ended by `now`, as (title, body), each room's said once and let go.
    pub fn due(&mut self, now: std::time::Instant) -> Vec<(String, String)> {
        let (said, kept): (Vec<HeldRoom>, Vec<HeldRoom>) = std::mem::take(&mut self.held).into_iter().partition(|h| now >= h.due);
        self.held = kept;
        said.into_iter().map(|h| room_text(&h.room_name, &h.messages)).collect()
    }

    /// `pending`: what each room still holds, by room id.
    pub fn pending(&self) -> Vec<(String, Vec<String>)> {
        self.held.iter().map(|h| (h.room_id.clone(), h.messages.clone())).collect()
    }
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

/// What a turn that ended says to a person who stepped away (attention-reader.ts): the two ways a run ends WAITING FOR
/// THE PERSON that no other notification covers. Decided from the record; nothing for a run that ended any other way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEnded {
    /// decisionAsked: the completed reply ends with a decision card, and this is its question.
    Decision { question: String },
    /// limitHit: the run stopped at its account limit; the runtime that recorded it, and its own words.
    Limit { runtime: String, message: Option<String> },
}

/// onRunEnded: the newest quota-exhausted limit the run did not go on to complete past, whatever its phase; else, for a
/// completed run only, the decision card in its last final reply.
pub fn run_ended(events: &[serde_json::Value], completed: bool) -> Option<RunEnded> {
    let mut limit = None;
    for event in events {
        if event["type"] == "route.limit_detected" && event["payload"]["kind"] == "quota-exhausted" {
            limit = Some(RunEnded::Limit {
                runtime: event["sourceAdapter"].as_str().unwrap_or_default().to_owned(),
                message: event["payload"]["message"].as_str().map(str::to_owned),
            });
        } else if event["type"] == "run.completed" {
            limit = None;
        }
    }
    if limit.is_some() {
        return limit;
    }
    if !completed {
        return None;
    }
    let text = crate::carry_on::latest_final(events)?;
    crate::protocol_tags::parse_decision(&text).map(|decision| RunEnded::Decision { question: decision.question })
}

/// A system notification: what it says, and the conversation a click opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub opens: Option<String>,
    /// A run stopped at its limit: said instead of its finish.
    pub limit: bool,
}

/// What a turn of the native engine that ended raises (attention_watch::poll): `run_ended` on its record, a decision
/// named for its conversation (`conversation`, asked only then), where the
/// turn's own agent names a limit event that does not say whose it was. A failure at the limit with no limit event of
/// its own is a limit too (Rust's own events end such a turn `run.failed`, kind `quota-exhausted`).
pub fn ended_notice(mission_id: &str, events: &[serde_json::Value], runtime: &str, teammate: Option<&str>, conversation: impl FnOnce() -> String) -> Option<Notice> {
    let end = events.iter().rev().find(|e| matches!(e["type"].as_str(), Some("run.completed" | "run.failed" | "run.cancelled")));
    let kind = end.and_then(|e| e["type"].as_str()).unwrap_or("");
    let says = run_ended(events, kind == "run.completed");
    let limit = match &says {
        Some(RunEnded::Limit { runtime, message }) => Some((runtime.clone(), message.clone())),
        _ if kind == "run.failed" && end.is_some_and(|e| e["payload"]["kind"] == "quota-exhausted") => {
            Some((String::new(), end.and_then(|e| e["payload"]["message"].as_str()).map(str::to_owned)))
        }
        _ => None,
    };
    if let Some((by, message)) = limit {
        let by = if by.is_empty() { runtime.to_owned() } else { by };
        let (title, body) = limit_text(teammate, crate::carry_on::display_name(&by), message.as_deref());
        return Some(Notice { title, body, opens: Some(mission_id.to_owned()), limit: true });
    }
    let Some(RunEnded::Decision { question }) = says else { return None };
    let (title, body) = decision_text(teammate, &question, Some(&conversation()));
    Some(Notice { title, body, opens: Some(mission_id.to_owned()), limit: false })
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
        assert_eq!(decision_text(None, "Ship  it?\nnow", None).1, "Ship it? now · the run waits for your answer.");
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

#[cfg(test)]
#[path = "attention_reader_tests.rs"]
mod attention_reader_tests;

#[cfg(test)]
#[path = "a_waiting_notification_opens_its_conversation_tests.rs"]
mod a_waiting_notification_opens_its_conversation_tests;

#[cfg(test)]
#[path = "room_toasts_tests.rs"]
mod room_toasts_tests;
