//! Settle and snooze — port of `renderer/src/settled.ts`.

use chrono::{Datelike, Local, TimeZone, Timelike};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettledEntry {
    pub at: String,
    pub until: Option<String>,
}

/// Whether the sidebar leaves this conversation out right now.
pub fn is_put_away(last_at: Option<&str>, entry: Option<&SettledEntry>, now_ms: i64) -> bool {
    let Some(entry) = entry else {
        return false;
    };
    if let Some(last) = last_at
        && parse_ms(last).is_some_and(|t| t > parse_ms(&entry.at).unwrap_or(0))
    {
        return false;
    }
    match &entry.until {
        None => true,
        Some(until) => parse_ms(until).is_some_and(|t| t > now_ms),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnoozeChoice {
    pub label: String,
    /// ISO.
    pub until: String,
}

/// The times a conversation can be snoozed until, from now, in the person's own clock.
pub fn snooze_choices(now: chrono::DateTime<Local>) -> Vec<SnoozeChoice> {
    let mut choices = vec![SnoozeChoice {
        label: "For an hour".into(),
        until: (now + chrono::Duration::hours(1))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    }];
    if now.hour() < 17 {
        choices.push(SnoozeChoice {
            label: "Until this evening (6 PM)".into(),
            until: at_local(now, 0, 18),
        });
    }
    choices.push(SnoozeChoice {
        label: "Until tomorrow (9 AM)".into(),
        until: at_local(now, 1, 9),
    });
    let to_monday = {
        let d = now.weekday().num_days_from_sunday(); // Sun=0 … Sat=6
        let n = (8 - d) % 7;
        if n == 0 { 7 } else { n }
    };
    if to_monday > 1 {
        choices.push(SnoozeChoice {
            label: "Until Monday (9 AM)".into(),
            until: at_local(now, to_monday as i64, 9),
        });
    }
    choices
}

fn at_local(base: chrono::DateTime<Local>, days: i64, hour: u32) -> String {
    let next = base + chrono::Duration::days(days);
    let local = Local
        .with_ymd_and_hms(next.year(), next.month(), next.day(), hour, 0, 0)
        .single()
        .unwrap_or(next);
    local.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn parse_ms(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // a-conversation-can-be-put-away.test.tsx
    #[test]
    fn a_conversation_can_be_put_away() {
        let settled = SettledEntry {
            at: "2026-10-10T12:00:00.000Z".into(),
            until: None,
        };
        let snoozed = SettledEntry {
            at: "2026-10-10T12:00:00.000Z".into(),
            until: Some("2026-10-10T18:00:00.000Z".into()),
        };
        let noon = chrono::DateTime::parse_from_rfc3339("2026-10-10T13:00:00.000Z")
            .unwrap()
            .timestamp_millis();
        assert!(is_put_away(
            Some("2026-10-10T11:00:00.000Z"),
            Some(&settled),
            noon
        ));
        assert!(is_put_away(
            Some("2026-10-10T11:00:00.000Z"),
            Some(&snoozed),
            noon
        ));
        let after = chrono::DateTime::parse_from_rfc3339("2026-10-10T18:00:01.000Z")
            .unwrap()
            .timestamp_millis();
        assert!(!is_put_away(
            Some("2026-10-10T11:00:00.000Z"),
            Some(&snoozed),
            after
        ));
        assert!(!is_put_away(Some("2026-10-10T11:00:00.000Z"), None, noon));
        assert!(!is_put_away(
            Some("2026-10-10T12:30:00.000Z"),
            Some(&settled),
            noon
        ));
    }

    #[test]
    fn snooze_times_offered() {
        // Friday 2026-10-09 10:30 local
        let friday = Local
            .with_ymd_and_hms(2026, 10, 9, 10, 30, 0)
            .single()
            .unwrap();
        let labels: Vec<_> = snooze_choices(friday)
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(
            labels,
            [
                "For an hour",
                "Until this evening (6 PM)",
                "Until tomorrow (9 AM)",
                "Until Monday (9 AM)",
            ]
        );
        let sunday_night = Local
            .with_ymd_and_hms(2026, 10, 11, 20, 0, 0)
            .single()
            .unwrap();
        let labels: Vec<_> = snooze_choices(sunday_night)
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(labels, ["For an hour", "Until tomorrow (9 AM)"]);
    }
}
