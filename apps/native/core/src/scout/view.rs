//! Pure window decisions and copy for Scout (`ScoutCards.tsx`, `ScoutSetting.tsx`).
//!
//! All card sorting, visibility slicing (`SHOWN = 3`), relative time wording (`ago`),
//! invite/watch/setting text, and look-log filtering (`SAW_PAGE = 30`) live here so
//! both the window and unit tests share one source of truth without disk I/O or UI.

use chrono::{DateTime, FixedOffset};

use super::log::{ScoutLogEntry, set_something_aside};
use super::types::{
    SCOUT_QUIET_CHOICES, ScoutCard, ScoutQuietHours, ScoutSettings, ScoutState, ScoutUrgency,
    hour_words, in_quiet_hours,
};

/// Cards shown on Home before asking for all (`ScoutCards.tsx` `SHOWN`).
pub const SCOUT_SHOWN_CARDS: usize = 3;
/// Look log entries shown per page (`ScoutSetting.tsx` `SAW_PAGE`).
pub const SCOUT_SAW_PAGE: usize = 30;

pub const SCOUT_INVITE_TITLE: &str = "Meet Scout, a teammate that keeps watch";
pub const SCOUT_INVITE_NAME: &str = "Meet Scout";
pub const SCOUT_INVITE_ROLE: &str = "Keeps watch for you";
pub const SCOUT_INVITE_ROUTE: &str = "Never acts by itself";
pub const SCOUT_INVITE_LEDE: &str = "Scout looks at your pull requests and their checks, the reviews asked of you and your teammates’ turns every 10 minutes, and tells you here what needs you, with the next step drafted. It never sends, merges or changes anything: you press.";
pub const SCOUT_NOT_NOW_LABEL: &str = "Not now: do not offer Scout again";

pub const SCOUT_SETTING_HEADING: &str = "Scout keeps watch";
pub const SCOUT_OFF_LEDE: &str = "Off. Turn it on and Scout joins your team: it watches your pull requests, their checks and reviews, and tells you on Home what needs you.";
pub const SCOUT_NO_MODEL_MESSAGE: &str = "Scout needs a model to read with: install OpenCode for a free one, then turn it on.";
pub const SCOUT_GOAL_HEADING: &str = "What to keep watch for";
pub const SCOUT_GOAL_LEDE: &str = "In your own words. Scout reads it on every look, and says how what it finds bears on it.";
pub const SCOUT_GOAL_PLACEHOLDER: &str = "For example: keep main’s tests passing, and tell me about new issues on the sign-in page.";
pub const SCOUT_NOTES_PLACEHOLDER: &str = "Nothing yet. A look that learns something worth keeping adds a line here.";
pub const SCOUT_SAW_HEADING: &str = "What Scout saw";
pub const SCOUT_SAW_LEDE: &str = "Each look of the last 14 days, newest first: what it raised, what it set aside and why, and what it did not read. Ask Scout in its conversation why it did not tell you about something, and it answers from this.";

/// Relative time label matching `ScoutCards.tsx` `ago`.
pub fn ago(iso: &str, now_ms: i64) -> String {
    let Ok(dt) = DateTime::parse_from_rfc3339(iso) else {
        return String::new();
    };
    let minutes = (((now_ms - dt.timestamp_millis()) as f64) / 60_000.0)
        .round()
        .max(0.0) as i64;
    if minutes < 1 {
        return "just now".to_owned();
    }
    if minutes < 60 {
        return format!("{minutes} min ago");
    }
    let hours = ((minutes as f64) / 60.0).round() as i64;
    if hours < 24 {
        format!("{hours} h ago")
    } else {
        let days = ((hours as f64) / 24.0).round() as i64;
        format!("{days} d ago")
    }
}

/// Cards sorted so `Now` urgency always appears first, preserving relative order within each bucket.
pub fn ordered_cards(cards: &[ScoutCard]) -> Vec<&ScoutCard> {
    let mut ordered = Vec::with_capacity(cards.len());
    ordered.extend(cards.iter().filter(|c| c.fact.urgency == ScoutUrgency::Now));
    ordered.extend(cards.iter().filter(|c| c.fact.urgency != ScoutUrgency::Now));
    ordered
}

/// Visible cards on Home (`SHOWN = 3`), keeping `focus_card_id` visible if it sits past the first three.
pub fn visible_cards<'a>(
    cards: &'a [ScoutCard],
    show_all: bool,
    focus_card_id: Option<&str>,
) -> (Vec<&'a ScoutCard>, usize) {
    let ordered = ordered_cards(cards);
    let total = ordered.len();
    if show_all {
        return (ordered, total);
    }
    let mut shown: Vec<&'a ScoutCard> = ordered.iter().copied().take(SCOUT_SHOWN_CARDS).collect();
    if let Some(focus) = focus_card_id
        && let Some(focused) = ordered.iter().copied().find(|c| c.card_id == focus)
        && !shown.iter().any(|c| c.card_id == focus)
    {
        // ScoutCards.tsx: the card brought into view goes first, above the three (`[focused, ...shown]`).
        shown.insert(0, focused);
    }
    (shown, total)
}

/// Card kind label matching `ScoutCards.tsx` `Card`.
pub fn card_kind(card: &ScoutCard) -> &'static str {
    if card.fact.urgency == ScoutUrgency::Now {
        "Needs you"
    } else if card.fact.key.starts_with("brief:") {
        "Morning brief"
    } else if card.fact.key.starts_with("week:") {
        "Your week"
    } else if card.fact.draft.is_some() {
        "Noticed, and drafted"
    } else {
        "Noticed"
    }
}

pub fn cards_heading(count: usize) -> String {
    format!("Scout noticed · {count}")
}

pub fn model_said_banner(why: &str) -> String {
    format!("{why} The cards below are in Locust’s words.")
}

pub fn watching_line(last_look_at: Option<&str>, now_ms: i64) -> String {
    match last_look_at {
        None => "its first look is on its way".to_owned(),
        Some(iso) => format!("looked {} · nothing needs you", ago(iso, now_ms)),
    }
}

pub fn looked_team_line(last_look_at: Option<&str>, looking: bool, now_ms: i64) -> String {
    if looking {
        "Looking…".to_owned()
    } else if let Some(iso) = last_look_at {
        format!("Looked {} · all clear", ago(iso, now_ms))
    } else {
        "First look on its way".to_owned()
    }
}

pub fn invite_sends_line(model: &str) -> String {
    format!(
        "When something is new, Scout’s model reads it: titles, check names, a waiting teammate’s question. That model is {model}. Change it like any teammate’s."
    )
}

/// Whether Home should offer `Meet Scout` on the team grid (`App.tsx:10018-10025`).
pub fn should_offer_scout_invite(
    settings: &ScoutSettings,
    dismissed_not_now: bool,
    has_team: bool,
    has_folder: bool,
    has_default_model: bool,
) -> bool {
    !settings.on
        && settings.teammate_id.is_none()
        && !dismissed_not_now
        && has_team
        && has_folder
        && has_default_model
}

pub fn quiet_words(quiet: ScoutQuietHours) -> String {
    format!("{} to {}", hour_words(quiet.from), hour_words(quiet.to))
}

pub fn quiet_choices_with_saved(saved: Option<ScoutQuietHours>) -> Vec<(Option<ScoutQuietHours>, String)> {
    let mut out = vec![(None, "Off".to_owned())];
    for choice in SCOUT_QUIET_CHOICES {
        out.push((Some(choice), quiet_words(choice)));
    }
    if let Some(q) = saved
        && !SCOUT_QUIET_CHOICES.contains(&q)
    {
        out.push((Some(q), quiet_words(q)));
    }
    out
}

pub fn setting_on_lede(every_minutes: u32, model: Option<&str>) -> String {
    let model_part = model
        .map(|m| format!(" It reads with {m}."))
        .unwrap_or_default();
    format!(
        "On. Scout looks every {every_minutes} minutes while Locust runs, and tells you on Home what needs you, with the next step drafted. It never sends, merges or changes anything by itself.{model_part}"
    )
}

pub fn setting_status_line(
    state: &ScoutState,
    model: Option<&str>,
    now: DateTime<FixedOffset>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(iso) = &state.last_look_at {
        let suffix = if state.cards.is_empty() {
            ", and nothing needed you."
        } else {
            "."
        };
        parts.push(format!("Last looked {}{suffix}", ago(iso, now.timestamp_millis())));
    }
    if let Some(today) = &state.today {
        let look_word = if today.looks == 1 { "look" } else { "looks" };
        let read_part = if today.asked == 0 {
            "none of them needing its model".to_owned()
        } else {
            format!("{} of them read by {}", today.asked, model.unwrap_or("its model"))
        };
        parts.push(format!("Today: {} {look_word}, {read_part}.", today.looks));
    }
    if let Some(quiet) = state.settings.quiet
        && in_quiet_hours(Some(quiet), now)
    {
        parts.push(format!("Quiet until {}.", hour_words(quiet.to)));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

pub fn notes_heading(count: usize) -> String {
    if count == 0 {
        "Scout’s notes".to_owned()
    } else {
        format!("Scout’s notes · {count}")
    }
}

pub fn tray_word(platform: &str) -> &'static str {
    if platform == "darwin" {
        "menu bar"
    } else {
        "tray"
    }
}

pub fn notify_setting_lede(platform: &str) -> String {
    format!(
        "Only for a card that needs you now, while Locust is minimized, in the {} or behind another app.",
        tray_word(platform)
    )
}

/// Filter and reverse log entries (`ScoutSetting.tsx` `ScoutSaw`).
pub fn saw_entries<'a>(
    entries: &'a [ScoutLogEntry],
    aside_only: bool,
) -> Vec<&'a ScoutLogEntry> {
    entries
        .iter()
        .rev()
        .filter(|entry| !aside_only || set_something_aside(entry))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scout::types::{ScoutFact, ScoutToday};

    fn make_card(id: &str, urgency: ScoutUrgency, key: &str, with_draft: bool) -> ScoutCard {
        let mut fact = ScoutFact::simple(key, urgency, format!("News {id}"));
        if with_draft {
            fact.draft = Some(crate::scout::types::ScoutDraft {
                teammate_id: "tm_atlas".into(),
                name: "Atlas".into(),
                message: "Check PR".into(),
            });
        }
        ScoutCard {
            card_id: id.into(),
            at: "2026-10-12T06:00:00.000Z".into(),
            reading: false,
            take: None,
            raw_fix: None,
            fact,
        }
    }

    #[test]
    fn ago_formats_minutes_hours_days_and_just_now() {
        let now = DateTime::parse_from_rfc3339("2026-10-12T06:06:00.000Z")
            .unwrap()
            .timestamp_millis();
        assert_eq!(ago("2026-10-12T06:05:50.000Z", now), "just now");
        assert_eq!(ago("2026-10-12T06:00:00.000Z", now), "6 min ago");
        assert_eq!(ago("2026-10-12T04:06:00.000Z", now), "2 h ago");
        assert_eq!(ago("2026-10-10T06:06:00.000Z", now), "2 d ago");
        assert_eq!(ago("not-an-iso", now), "");
    }

    #[test]
    fn visible_cards_puts_now_first_caps_at_three_and_keeps_focused_card() {
        let c1 = make_card("c1", ScoutUrgency::Soon, "pr:1", false);
        let c2 = make_card("c2", ScoutUrgency::Soon, "pr:2", true);
        let c3 = make_card("c3", ScoutUrgency::Later, "pr:3", false);
        let c4 = make_card("c4", ScoutUrgency::Now, "wait:1", false);
        let c5 = make_card("c5", ScoutUrgency::Later, "pr:5", false);
        let all = vec![c1, c2, c3, c4, c5];

        let (shown, total) = visible_cards(&all, false, None);
        assert_eq!(total, 5);
        assert_eq!(
            shown.iter().map(|c| c.card_id.as_str()).collect::<Vec<_>>(),
            vec!["c4", "c1", "c2"]
        );

        let (with_focus, _) = visible_cards(&all, false, Some("c5"));
        assert_eq!(
            with_focus
                .iter()
                .map(|c| c.card_id.as_str())
                .collect::<Vec<_>>(),
            vec!["c5", "c4", "c1", "c2"],
            "the card brought into view goes first, as ScoutCards.tsx draws it"
        );
        // One already among the three stays where it is, once.
        let (already, _) = visible_cards(&all, false, Some("c1"));
        assert_eq!(already.iter().map(|c| c.card_id.as_str()).collect::<Vec<_>>(), vec!["c4", "c1", "c2"]);
    }

    #[test]
    fn quiet_hours_words_and_custom_choice_preservation() {
        let midnight = ScoutQuietHours { from: 0, to: 8 };
        assert_eq!(quiet_words(midnight), "midnight to 8 AM");
        assert_eq!(
            quiet_words(ScoutQuietHours { from: 22, to: 7 }),
            "10 PM to 7 AM"
        );
        let custom = ScoutQuietHours { from: 19, to: 6 };
        let choices = quiet_choices_with_saved(Some(custom));
        assert_eq!(choices.len(), 7);
        assert_eq!(choices.last().unwrap().1, "7 PM to 6 AM");
    }

    #[test]
    fn card_kind_and_setting_status_match_expected_copy() {
        assert_eq!(
            card_kind(&make_card("1", ScoutUrgency::Now, "w:1", false)),
            "Needs you"
        );
        assert_eq!(
            card_kind(&make_card("2", ScoutUrgency::Soon, "brief:2026-10-22", false)),
            "Morning brief"
        );
        assert_eq!(
            card_kind(&make_card("3", ScoutUrgency::Later, "week:2026-10-22", false)),
            "Your week"
        );
        assert_eq!(
            card_kind(&make_card("4", ScoutUrgency::Soon, "pr:1", true)),
            "Noticed, and drafted"
        );
        assert_eq!(
            card_kind(&make_card("5", ScoutUrgency::Soon, "pr:1", false)),
            "Noticed"
        );

        let now = DateTime::parse_from_rfc3339("2026-10-22T23:05:00+00:00").unwrap();
        let st = ScoutState {
            settings: ScoutSettings {
                on: true,
                quiet: Some(ScoutQuietHours { from: 22, to: 7 }),
                ..Default::default()
            },
            cards: vec![],
            look_ids: vec![],
            last_look_at: Some("2026-10-22T23:02:00.000Z".into()),
            model_said: None,
            today: Some(ScoutToday {
                day: "2026-10-22".into(),
                looks: 14,
                asked: 2,
                fixes: None,
            }),
            notes: vec![],
        };
        assert_eq!(
            setting_status_line(&st, Some("Deepseek V4.1 Flash Free"), now).as_deref(),
            Some(
                "Last looked 3 min ago, and nothing needed you. Today: 14 looks, 2 of them read by Deepseek V4.1 Flash Free. Quiet until 7 AM."
            )
        );
    }
}
