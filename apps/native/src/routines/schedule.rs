//! Port of shared/routine-schedule.ts: wall-clock days, not 24-hour arithmetic.
use chrono::{DateTime, Datelike, Duration, Local, NaiveDateTime, NaiveTime, TimeZone};
use serde_json::Value;
fn moment(s: &str) -> Option<DateTime<Local>> {
    if s.len() != 16 {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").ok()?;
    local(naive)
}
fn local(naive: NaiveDateTime) -> Option<DateTime<Local>> {
    // JS chooses the earlier overlap, and shifts a spring-forward gap by its size.
    if let Some(at) = Local.from_local_datetime(&naive).earliest() {
        return Some(at);
    }
    let before = Local
        .from_local_datetime(&(naive - Duration::hours(3)))
        .earliest()?;
    let after = Local
        .from_local_datetime(&(naive + Duration::hours(3)))
        .earliest()?;
    let gap = after.offset().local_minus_utc() - before.offset().local_minus_utc();
    Local
        .from_local_datetime(&(naive + Duration::seconds(i64::from(gap))))
        .earliest()
}
fn time(s: &str) -> Option<NaiveTime> {
    if s.len() != 5 {
        None
    } else {
        NaiveTime::parse_from_str(s, "%H:%M").ok()
    }
}
pub fn valid(v: &Value) -> bool {
    match v["kind"].as_str().unwrap_or("") {
        "every" => v["hours"].as_u64().is_some_and(|h| (1..=168).contains(&h)),
        "daily" => v["at"].as_str().and_then(time).is_some(),
        "weekly" => {
            v["at"].as_str().and_then(time).is_some()
                && v["days"].as_array().is_some_and(|ds| {
                    !ds.is_empty()
                        && ds.iter().all(|d| d.as_u64().is_some_and(|d| d <= 6))
                        && ds.windows(2).all(|w| w[0].as_u64() < w[1].as_u64())
                })
        }
        "once" => v["on"].as_str().and_then(moment).is_some(),
        "hook" => v.as_object().is_some_and(|o| o.len() == 1),
        "files" => v["folder"].as_str().is_some_and(|s| {
            // Mirror Electron watchedFolderValid: relative project paths only.
            let absolute = s.starts_with(['/', '\\'])
                || s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                    && s.chars().nth(1) == Some(':');
            s.len() <= 200
                && !absolute
                && !s
                    .split(['/', '\\'])
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .is_empty()
                && s.split(['/', '\\']).filter(|p| !p.is_empty()).all(|p| {
                    p != "." && p != ".." && !p.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c))
                })
        }),
        _ => false,
    }
}
pub fn base(r: &Value) -> Option<DateTime<Local>> {
    let parse = |key| {
        r[key]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Local))
    };
    let ran = parse("lastRunAt").or_else(|| parse("createdAt"))?;
    Some(parse("resumedAt").map_or(ran, |at| at.max(ran)))
}
pub fn next(v: &Value, last: DateTime<Local>, now: DateTime<Local>) -> Option<DateTime<Local>> {
    if !valid(v) {
        return None;
    }
    let slot = |offset| {
        local((now.date_naive() + Duration::days(offset)).and_time(time(v["at"].as_str()?)?))
    };
    match v["kind"].as_str()? {
        "every" => Some((last + Duration::hours(v["hours"].as_i64()?)).max(now)),
        "once" => moment(v["on"].as_str()?).filter(|d| *d > last),
        "daily" => {
            let today = slot(0)?;
            if last >= today { slot(1) } else { Some(today) }
        }
        "weekly" => {
            let days = v["days"].as_array()?;
            let slots: Vec<_> = (-7..=7)
                .filter_map(slot)
                .filter(|d| {
                    days.iter()
                        .any(|v| v.as_u64() == Some(u64::from(d.weekday().num_days_from_sunday())))
                })
                .collect();
            slots
                .iter()
                .rev()
                .find(|d| **d <= now && **d > last)
                .copied()
                .or_else(|| slots.into_iter().find(|d| *d > now && *d > last))
        }
        _ => None,
    }
}
pub fn missed(
    v: &Value,
    last: DateTime<Local>,
    opened: DateTime<Local>,
    now: DateTime<Local>,
) -> Option<DateTime<Local>> {
    let due = next(v, last, now)?;
    if due > now {
        return None;
    }
    if v["kind"] == "every" {
        let seconds = v["hours"].as_i64()? * 3600;
        if (opened - last).num_seconds() < seconds {
            return None;
        }
        Some(last + Duration::seconds((opened - last).num_seconds() / seconds * seconds))
    } else {
        Some(due).filter(|d| *d <= opened)
    }
}
/// A clock slot that passed while a run was still going (Electron `slotPassedDuring`).
/// Kept under test until native completion reconciliation records it on run end.
#[cfg(test)]
pub fn slot_passed_during(
    v: &Value,
    started: DateTime<Local>,
    ended: DateTime<Local>,
) -> Option<DateTime<Local>> {
    match v["kind"].as_str()? {
        "every" | "files" | "hook" => None,
        _ => {
            let next = next(v, started, started)?;
            Some(next).filter(|d| *d > started && *d <= ended)
        }
    }
}
const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
pub fn label(v: &Value) -> String {
    let at = v["at"].as_str().unwrap_or("");
    match v["kind"].as_str().unwrap_or("") {
        "every" => {
            if v["hours"] == 1 {
                "every hour".into()
            } else {
                format!("every {} hours", v["hours"])
            }
        }
        "daily" => format!("daily at {at}"),
        "weekly" => {
            let days = v["days"].as_array().cloned().unwrap_or_default();
            let has = |n| days.iter().any(|d| d == n);
            let words = if days.len() == 7 {
                "every day".into()
            } else if days.len() == 5 && (1..=5).all(has) {
                "weekdays".into()
            } else if days.len() == 2 && has(0) && has(6) {
                "weekends".into()
            } else {
                [1, 2, 3, 4, 5, 6, 0]
                    .iter()
                    .filter(|&&n| has(n))
                    .map(|n| DAY_NAMES[*n as usize])
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            format!("{words} at {at}")
        }
        // Electron uses the raw `on` time slice so a spring-forward gap keeps 02:30 in the card.
        "once" => moment(v["on"].as_str().unwrap_or("")).map_or("once".into(), |d| {
            let on = v["on"].as_str().unwrap_or("");
            format!(
                "once, {} {} {} at {}",
                DAY_NAMES[d.weekday().num_days_from_sunday() as usize],
                MONTH_NAMES[d.month0() as usize],
                d.day(),
                on.get(11..).unwrap_or("")
            )
        }),
        "files" => format!(
            "when a new file arrives in {}",
            v["folder"].as_str().unwrap_or("").replace('\\', "/")
        ),
        "hook" => "when a link is called".into(),
        _ => "when you press Run".into(),
    }
}
