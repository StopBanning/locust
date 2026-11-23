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
/// The tray's "next routine" line (`nextRoutineDueLine` in `shared/routine-schedule.ts`):
/// the soonest run still ahead, or `"No routine due"`. A paused routine has none (0.705).
pub fn next_routine_due_line(routines: &[Value], now: DateTime<Local>) -> String {
    let mut best: Option<(&str, DateTime<Local>)> = None;
    for r in routines {
        let Some(sched) = r.get("schedule").filter(|s| !s.is_null()) else {
            continue;
        };
        if sched["kind"] == "files" || r["paused"] == true {
            continue;
        }
        let Some(last) = base(r) else { continue };
        let Some(due) = next(sched, last, now).filter(|d| *d > now) else {
            continue;
        };
        if best.is_none_or(|(_, at)| due < at) {
            best = Some((r["name"].as_str().unwrap_or(""), due));
        }
    }
    match best {
        Some((name, at)) => format!("Next: {name} at {}", clock(at)),
        None => "No routine due".into(),
    }
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
/// Recorded as missed when a run is counted (`routines::progress::record_run`).
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
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn clock(at: DateTime<Local>) -> String {
    crate::time_marker::clock(&at, false)
}

/// "09:30", as a schedule keeps it, said as "9:30 AM" (`saidAt`, 0.882: no 24-hour times on screen). A time that does
/// not read is said as kept.
fn said_at(at: &str) -> String {
    NaiveTime::parse_from_str(at, "%H:%M").map_or_else(|_| at.to_owned(), |t| crate::time_marker::clock(&t, false))
}

/// "Missed, 08:00 today", "... yesterday", or "Missed, Tue 08:00" (`missedLine`). None for a time that does not read.
pub fn missed_line(missed_at: &str, now: DateTime<Local>) -> Option<String> {
    let at = DateTime::parse_from_rfc3339(missed_at).ok()?.with_timezone(&Local);
    let when = clock(at);
    if at.date_naive() == now.date_naive() {
        return Some(format!("Missed, {when} today"));
    }
    if Some(at.date_naive()) == now.date_naive().pred_opt() {
        return Some(format!("Missed, {when} yesterday"));
    }
    Some(format!("Missed, {} {when}", WEEKDAYS[at.weekday().num_days_from_sunday() as usize]))
}

/// What a routine's card says about its schedule (`routineScheduleSummary`): paused, in progress, waiting for your OK,
/// held for review, missed, watching, done, due now, or its next time in the person's own clock. None without one.
pub fn summary(r: &Value, now: DateTime<Local>) -> Option<String> {
    let schedule = r.get("schedule").filter(|s| !s.is_null())?;
    let rule = label(schedule);
    let execution = r.get("execution").filter(|e| !e.is_null());
    let abandoned = execution.is_some_and(|e| e["status"] == "abandoned");
    if r["paused"] == true && (execution.is_none() || abandoned) {
        return Some(format!("{rule} · paused"));
    }
    if let Some(e) = execution.filter(|_| !abandoned) {
        let state = if e["status"] == "running" {
            "in progress"
        } else if e["waitsForOk"] == true {
            "waiting for your OK"
        } else {
            "held for review; no automatic retry"
        };
        return Some(format!("{rule} · {state}"));
    }
    if let Some(missed) = r["missedAt"].as_str().and_then(|at| missed_line(at, now)) {
        return Some(missed);
    }
    if schedule["kind"] == "files" {
        return Some(format!("{rule} · watching"));
    }
    let Some(next) = base(r).and_then(|last| self::next(schedule, last, now)) else {
        return Some(format!("{rule} · done"));
    };
    let when = if next <= now {
        "due now".to_owned()
    } else if next.date_naive() == now.date_naive() {
        format!("next {}", clock(next))
    } else if Some(next.date_naive()) == now.date_naive().succ_opt() {
        format!("next tomorrow {}", clock(next))
    } else {
        format!("next {} {}", WEEKDAYS[next.weekday().num_days_from_sunday() as usize], clock(next))
    };
    Some(format!("{rule} · {when}"))
}

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
        "daily" => format!("daily at {}", said_at(at)),
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
            format!("{words} at {}", said_at(at))
        }
        // Electron says the raw `on` time slice (11..16) so a spring-forward gap keeps 2:30 AM in the card.
        "once" => moment(v["on"].as_str().unwrap_or("")).map_or("once".into(), |d| {
            let on = v["on"].as_str().unwrap_or("");
            format!(
                "once, {} {} {} at {}",
                DAY_NAMES[d.weekday().num_days_from_sunday() as usize],
                MONTH_NAMES[d.month0() as usize],
                d.day(),
                said_at(on.get(11..16).unwrap_or(""))
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
