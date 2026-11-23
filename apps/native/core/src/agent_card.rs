//! An agent's card: what it says about itself on hover (Electron `AgentMark.tsx`, 0.389; `AgentMark` and `AgentCard`).
//!
//! The mark is a thing to point at. Pointing at it opens a card with the agent's name and state, and one bar for each
//! usage window its last run reported, with when it resets. A bar is amber from 80% and red when the window is spent,
//! the tones the app uses for "may not finish" and "stopped". An agent that reported no windows gets its name and state
//! alone, not an empty meter. The chat box's model chip opens the same card (`Composer.tsx`, 2026-10-30), and reads the
//! reading aloud as its description, with a word more from 80%.
//!
//! This is the card as a value. `src/ui/thread_view.rs` draws it.

use chrono::{DateTime, Utc};

use crate::usage_words::{usage_percent, usage_read_line, usage_window_sentence, usage_windows_of};

/// From this much use a window is amber ("may not finish"), Electron's `window.percent >= 80`.
pub const PRESSING_FROM: u32 = 80;
/// From this much it is red ("stopped"), Electron's `window.percent >= 100`.
pub const SPENT_AT: u32 = 100;

/// How full a window is, as its bar is toned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Calm,
    Pressing,
    Spent,
}

/// The tone of a window `percent` used (`is-spent` at 100, `is-pressing` from 80, else none).
pub fn tone_of(percent: u32) -> Tone {
    if percent >= SPENT_AT {
        Tone::Spent
    } else if percent >= PRESSING_FROM {
        Tone::Pressing
    } else {
        Tone::Calm
    }
}

/// One window on the card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardWindow {
    /// Its reset has passed since the reading: what it said is no longer true, so no bar (0.406). `resets` is the
    /// clock time the reading gave.
    Reset { label: String, resets: Option<String> },
    /// A bar. `value` is "35%" or, for a window reported as quota remaining, "85% left"; `fill` is the share of the bar
    /// drawn (always consumption, even when the runtime reported what is left).
    Bar { label: String, value: String, fill: u32, tone: Tone, resets: Option<String> },
}

impl CardWindow {
    /// The window's name with its first letter capitalised ("5-hour window", "Weekly window").
    pub fn label(&self) -> &str {
        match self {
            CardWindow::Reset { label, .. } | CardWindow::Bar { label, .. } => label,
        }
    }

    /// What a reset window says in place of a bar: "reset since (10:10)" or "reset since".
    pub fn reset_since(&self) -> Option<String> {
        match self {
            CardWindow::Reset { resets, .. } => Some(format!("reset since{}", resets.as_ref().map(|r| format!(" ({r})")).unwrap_or_default())),
            CardWindow::Bar { .. } => None,
        }
    }

    /// What a bar says of its reset: "resets 22:10".
    pub fn resets_line(&self) -> Option<String> {
        match self {
            CardWindow::Bar { resets: Some(r), .. } => Some(format!("resets {r}")),
            _ => None,
        }
    }
}

/// The card: the agent's name, its state, a line for each window, and, when there is a window, where the reading came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentCard {
    pub name: String,
    pub state: String,
    pub windows: Vec<CardWindow>,
    pub read: Option<String>,
}

fn capitalised(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The card for an agent (`AgentCard`): `usage` is the latest reading for its account as the ledger keeps it.
pub fn agent_card(name: &str, state: &str, usage: Option<&str>, now: DateTime<Utc>) -> AgentCard {
    let windows: Vec<CardWindow> = usage
        .map(|said| usage_windows_of(said, now))
        .unwrap_or_default()
        .into_iter()
        .map(|w| {
            let label = capitalised(&w.name);
            if w.expired {
                return CardWindow::Reset { label, resets: w.resets };
            }
            let value = format!("{}%{}", w.remaining.unwrap_or(w.percent), if w.remaining.is_some() { " left" } else { "" });
            CardWindow::Bar { label, value, fill: w.percent, tone: tone_of(w.percent), resets: w.resets }
        })
        .collect();
    // The read line is drawn only under a meter: an agent with no windows shows no source for nothing.
    let read = if windows.is_empty() { None } else { usage.and_then(|said| usage_read_line(said, now)) };
    AgentCard { name: name.to_owned(), state: state.to_owned(), windows, read }
}

/// What a mark is called for a screen reader (`AgentMark`'s `aria-label`): its name, its state and, when there are
/// windows, the usage sentence, joined by ". " -- everything its card says.
pub fn mark_said(name: &str, state: &str, usage: Option<&str>, now: DateTime<Utc>) -> String {
    let mut parts = vec![name.to_owned(), state.to_owned()];
    if let Some(said) = usage
        && !usage_windows_of(said, now).is_empty()
    {
        parts.push(usage_window_sentence(said, now));
    }
    parts.join(". ")
}

/// The state line of a ready agent: "Ready · 2.1.283", or "Ready" with no version (`FirstLaunch.tsx`).
pub fn ready_state(version: Option<&str>) -> String {
    match version {
        Some(v) if !v.is_empty() => format!("Ready · {v}"),
        _ => "Ready".to_owned(),
    }
}

/// A version as the chat box chip's card says it: a build hash after a dash is dropped (`version.replace(/-[0-9a-f]{6,}$/i, '')`).
pub fn chip_version(version: &str) -> String {
    crate::invariants::regex(r"(?i)-[0-9a-f]{6,}$").replace(version, "").into_owned()
}

/// The chat box chip's description of its account's usage (`Composer.tsx` `usageSentence`): the sentence, and from 80%
/// a word more. `None` with no reading.
pub fn chip_usage_sentence(usage: Option<&str>, now: DateTime<Utc>) -> Option<String> {
    let said = usage?;
    let sentence = usage_window_sentence(said, now);
    Some(match usage_percent(said, now) {
        Some(p) if p >= SPENT_AT => format!("{sentence}. This window's limit is used up."),
        Some(p) if p >= PRESSING_FROM => format!("{sentence}. Long runs may be cut short."),
        _ => sentence,
    })
}

/// Whether the chip wears its amber dot: from 80%, when a long run might not finish (`usagePressing`).
pub fn chip_is_pressing(usage: Option<&str>, now: DateTime<Utc>) -> bool {
    usage.and_then(|said| usage_percent(said, now)).is_some_and(|p| p >= PRESSING_FROM)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Far ahead, as Electron's tests have it: a reset that has passed is drawn as reset, and a date that is "later
    // today" when written stops being so the next morning.
    const RESETS: &str = "2099-09-27T22:10:00.000Z";
    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }
    fn clock(card: &AgentCard, i: usize) -> Option<String> {
        card.windows[i].resets_line()
    }
    fn matches(pattern: &str, text: &str) -> bool {
        regex::Regex::new(pattern).unwrap().is_match(text)
    }

    // an-account-shows-how-full-it-is: "a usage reading reads as windows, fullest first, each with its reset".
    #[test]
    fn a_usage_reading_reads_as_windows_each_with_its_reset() {
        let said = format!("5-hour window 35% used · resets {RESETS} · 7-day window 12% used");
        let card = agent_card("Claude Code", "Ready · 2.1.283", Some(&said), Utc::now());
        let names: Vec<(&str, u32)> = card
            .windows
            .iter()
            .map(|w| match w {
                CardWindow::Bar { label, fill, .. } => (label.as_str(), *fill),
                CardWindow::Reset { .. } => panic!("not reset"),
            })
            .collect();
        // Electron compares the lower-case names; the card capitalises the first letter, so "5-hour window" is unchanged.
        assert_eq!(names, vec![("5-hour window", 35), ("7-day window", 12)]);
        assert!(matches(r"^resets (\d{1,2}:\d{2} [AP]M|\w{3} \d{1,2}:\d{2} [AP]M)$", &clock(&card, 0).unwrap()), "{:?}", clock(&card, 0));
        assert_eq!(clock(&card, 1), None);
    }

    // "is a thing to point at, named with everything its card says".
    #[test]
    fn a_mark_is_named_with_everything_its_card_says() {
        let said = mark_said("Claude Code", "Ready · 2.1.283", Some(&format!("5-hour window 35% used · resets {RESETS}")), Utc::now());
        assert!(matches(r"^Claude Code\. Ready · 2\.1\.283\. 35% of the 5-hour window used, resets .+$", &said), "{said}");
    }

    // "opens a card with its name, its state, and a bar for each window".
    #[test]
    fn the_card_has_its_name_its_state_and_a_bar_for_each_window() {
        let said = format!("5-hour window 35% used · resets {RESETS} · 7-day window 12% used");
        let card = agent_card("Claude Code", "Ready · 2.1.283", Some(&said), Utc::now());
        assert_eq!((card.name.as_str(), card.state.as_str()), ("Claude Code", "Ready · 2.1.283"));
        assert_eq!(card.windows.len(), 2);
        assert_eq!(card.windows[0].label(), "5-hour window");
        match (&card.windows[0], &card.windows[1]) {
            (CardWindow::Bar { value: a, fill: fa, .. }, CardWindow::Bar { fill: fb, .. }) => {
                assert_eq!((a.as_str(), *fa, *fb), ("35%", 35, 12));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches(r"^resets [^<]+$", &clock(&card, 0).unwrap()));
    }

    // "draws a bar amber from 80% and red when spent".
    #[test]
    fn a_bar_is_amber_from_80_and_red_when_spent() {
        let tone = |said: &str| match &agent_card("Claude Code", "Ready", Some(said), Utc::now()).windows[0] {
            CardWindow::Bar { tone, .. } => *tone,
            other => panic!("{other:?}"),
        };
        assert_eq!(tone("5-hour window 79% used"), Tone::Calm);
        assert_eq!(tone("5-hour window 80% used"), Tone::Pressing);
        assert_eq!(tone("weekly window 100% used"), Tone::Spent);
    }

    // "draws no meter for an agent whose runs reported none: its name and state alone".
    #[test]
    fn an_agent_whose_runs_reported_none_has_its_name_and_state_alone() {
        let card = agent_card("Claude Code", "Ready · 2.1.283", None, Utc::now());
        assert!(card.windows.is_empty());
        assert_eq!(card.read, None);
        assert_eq!(mark_said("Claude Code", "Ready · 2.1.283", None, Utc::now()), "Claude Code. Ready · 2.1.283");
    }

    // 0.406, "draws a window reset since the reading as reset, without its old bar, and says when the reading was taken".
    #[test]
    fn a_window_reset_since_the_reading_is_drawn_as_reset_and_says_when_it_was_taken() {
        let said = "7-day window 66% used · resets 2026-09-28T07:00:00.000Z · 5-hour window 20% used · resets 2026-09-27T05:30:00.000Z · from a run at 2026-09-27T01:23:19.233Z";
        let card = agent_card("Claude Code", "Ready", Some(said), at("2026-09-27T15:30:00.000Z"));
        assert!(matches!(&card.windows[0], CardWindow::Bar { fill: 66, .. }));
        assert!(!card.windows.iter().any(|w| matches!(w, CardWindow::Bar { fill: 20, .. })), "the old 20% bar is not drawn");
        assert_eq!(card.windows[1].label(), "5-hour window");
        assert!(matches(r"^reset since \([^)]+\)$", &card.windows[1].reset_since().unwrap()), "{:?}", card.windows[1].reset_since());
        assert!(matches(r"^From Locust's last run on it, [^<]+\. Use outside Locust since then isn't counted\.$", card.read.as_deref().unwrap()), "{:?}", card.read);
    }

    // antigravity-shows-remaining-not-used: names the remaining percentage, bars and tone use consumption.
    #[test]
    fn a_window_of_remaining_quota_names_what_is_left_and_draws_what_is_used() {
        let reading = "Gemini: 5-hour window 95% left · Gemini: weekly window 67% left · Claude and GPT: 5-hour window 100% left · Claude and GPT: weekly window 20% left · resets 2099-10-10T19:09:25Z · as of 2026-10-07T04:00:00Z";
        let now = at("2026-10-08T00:00:00Z");
        let card = agent_card("Antigravity", "Ready", Some(reading), now);
        let rows: Vec<(String, u32, Tone)> = card
            .windows
            .iter()
            .map(|w| match w {
                CardWindow::Bar { value, fill, tone, .. } => (value.clone(), *fill, *tone),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(rows[0], ("95% left".to_owned(), 5, Tone::Calm));
        assert_eq!(rows[3], ("20% left".to_owned(), 80, Tone::Pressing));
        assert!(card.read.as_deref().unwrap().starts_with("Read from your account at"));
        let sentence = usage_window_sentence(reading, now);
        assert!(sentence.contains("95% of the Gemini: 5-hour window left"), "{sentence}");
        assert!(sentence.contains("20% of the Claude and GPT: weekly window left"), "{sentence}");
    }

    // antigravity-shows-remaining-not-used, "does not carry an expired reading into the current warning".
    #[test]
    fn an_expired_reading_is_not_carried_into_the_current_warning() {
        let expired = "Gemini: 5-hour window 0% left · resets 2026-10-06T00:00:00Z";
        let now = at("2026-10-07T00:00:00Z");
        assert_eq!(usage_percent(expired, now), None);
        assert_eq!(usage_window_sentence(expired, now), "the Gemini: 5-hour window has reset since");
    }

    // antigravity-shows-remaining-not-used, "leaves existing used readings unchanged".
    #[test]
    fn existing_used_readings_are_left_unchanged() {
        let windows = usage_windows_of("5-hour window 35% used", Utc::now());
        assert_eq!(windows, vec![crate::usage_words::UsageWindow { name: "5-hour window".into(), percent: 35, remaining: None, resets: None, expired: false }]);
        assert_eq!(usage_window_sentence("5-hour window 35% used", Utc::now()), "35% of the 5-hour window used");
        assert_eq!(usage_percent("Gemini: 5-hour window 95% left · Gemini: weekly window 67% left · Claude and GPT: 5-hour window 100% left · Claude and GPT: weekly window 20% left · resets 2099-10-10T19:09:25Z · as of 2026-10-07T04:00:00Z", Utc::now()), Some(80));
    }

    // the-model-chip-shows-usage-on-hover: the card for the chip has a bar for each window, capitalised as the page does.
    #[test]
    fn the_model_chips_card_has_a_bar_for_each_window() {
        let reading = "weekly window 64% used · resets 2026-11-04T06:36:18.000Z · 5-hour window 12% used · resets 2026-10-31T05:51:36.000Z · from a run at 2026-10-31T02:22:14.418Z";
        let card = agent_card("Codex CLI", "Ready · 0.166.0", Some(reading), at("2026-10-31T03:20:00Z"));
        let rows: Vec<(&str, &str)> = card
            .windows
            .iter()
            .map(|w| match w {
                CardWindow::Bar { label, value, .. } => (label.as_str(), value.as_str()),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(rows, vec![("Weekly window", "64%"), ("5-hour window", "12%")]);
    }

    // The chip reads its account's usage aloud as its description, with a word more from 80% and at 100%.
    #[test]
    fn the_chip_says_a_word_more_from_80_and_at_100() {
        let now = at("2026-10-31T03:20:00Z");
        assert_eq!(chip_usage_sentence(None, now), None);
        assert_eq!(chip_usage_sentence(Some("5-hour window 40% used"), now).unwrap(), "40% of the 5-hour window used");
        assert_eq!(chip_usage_sentence(Some("5-hour window 85% used"), now).unwrap(), "85% of the 5-hour window used. Long runs may be cut short.");
        assert_eq!(chip_usage_sentence(Some("5-hour window 100% used"), now).unwrap(), "100% of the 5-hour window used. This window's limit is used up.");
    }

    // "the model button is quiet with room left, and wears its amber dot from 80%".
    #[test]
    fn the_model_button_wears_its_amber_dot_from_80() {
        let now = at("2026-10-31T03:20:00Z");
        assert!(!chip_is_pressing(Some("5-hour window 40% used"), now));
        assert!(!chip_is_pressing(Some("5-hour window 79% used"), now));
        assert!(chip_is_pressing(Some("5-hour window 80% used"), now));
        assert!(chip_is_pressing(Some("5-hour window 85% used"), now));
        assert!(!chip_is_pressing(None, now));
    }

    #[test]
    fn the_chips_card_drops_a_build_hash_from_the_version() {
        assert_eq!(chip_version("0.166.0-1a2b3c4d"), "0.166.0");
        assert_eq!(chip_version("0.166.0-ABCDEF"), "0.166.0");
        assert_eq!(chip_version("2.1.283"), "2.1.283");
        assert_eq!(chip_version("1.0-beta"), "1.0-beta");
    }

    // Home's line: "Ready · 2.1.283" with a version, "Ready" without.
    #[test]
    fn a_ready_agent_says_its_version_when_it_has_one() {
        assert_eq!(ready_state(Some("2.1.283")), "Ready · 2.1.283");
        assert_eq!(ready_state(None), "Ready");
    }
}
