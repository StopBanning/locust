//! Since-you-were-away — port of `shared/away.ts` and `main/away.ts`.

use crate::store::Store;
use chrono::Datelike;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::Path;

/// Shorter than this and nobody was "away": a coffee is not a morning.
pub const AWAY_MINIMUM_MS: i64 = 20 * 60_000;
pub const AWAY_FILE: &str = "away.json";

pub const HEADING: &str = "Since you were away";
pub const GOT_IT: &str = "Got it";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayRow {
    pub mission_id: String,
    pub title: String,
    pub phase: String,
    pub last_at: Option<String>,
    pub owner_id: Option<String>,
    pub routine_id: Option<String>,
    pub member_ids: Vec<String>,
    pub root_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayRoutineMiss {
    pub due_at: String,
    pub recorded_at: String,
    /// Present when a hook already counted the miss; Electron skips those.
    pub hook_calls: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayRoutine {
    pub routine_id: String,
    pub name: String,
    pub history: Vec<AwayRoutineMiss>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayItem {
    pub mission_id: String,
    pub title: String,
    pub at: String,
    pub owner_id: Option<String>,
    pub routine_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayMissed {
    pub routine_id: String,
    pub name: String,
    pub due_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaySummary {
    pub since: String,
    pub ran: Vec<AwayItem>,
    pub failed: Vec<AwayItem>,
    pub missed: Vec<AwayMissed>,
    pub waiting: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwaySummaryCounts {
    pub ran: usize,
    pub failed: usize,
    pub missed: usize,
    pub waiting: usize,
}

fn time_ms(value: Option<&str>) -> Option<i64> {
    let value = value?;
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

pub fn since_you_were_away(
    since: &str,
    rows: &[AwayRow],
    routines: &[AwayRoutine],
    needs_you: &HashSet<String>,
) -> Option<AwaySummary> {
    let since_ms = time_ms(Some(since))?;
    let ended = |row: &AwayRow| -> Option<AwayItem> {
        let at = time_ms(row.last_at.as_deref())?;
        if at <= since_ms {
            return None;
        }
        Some(AwayItem {
            mission_id: row.mission_id.clone(),
            title: row.title.clone(),
            at: row.last_at.clone()?,
            owner_id: row.owner_id.clone(),
            routine_id: row.routine_id.clone(),
        })
    };
    let mut ran: Vec<AwayItem> = rows
        .iter()
        .filter(|row| row.phase == "completed")
        .filter_map(ended)
        .collect();
    ran.sort_by(|a, b| {
        time_ms(Some(&b.at))
            .unwrap_or(0)
            .cmp(&time_ms(Some(&a.at)).unwrap_or(0))
    });
    let mut failed: Vec<AwayItem> = rows
        .iter()
        .filter(|row| row.phase == "failed")
        .filter_map(ended)
        .collect();
    failed.sort_by(|a, b| {
        time_ms(Some(&b.at))
            .unwrap_or(0)
            .cmp(&time_ms(Some(&a.at)).unwrap_or(0))
    });
    let waiting = if needs_you.is_empty() {
        0
    } else {
        rows.iter()
            .filter(|row| {
                std::iter::once(row.mission_id.as_str())
                    .chain(row.root_id.as_deref())
                    .chain(row.member_ids.iter().map(String::as_str))
                    .any(|id| needs_you.contains(id))
            })
            .count()
    };
    let mut seen = HashSet::new();
    let mut missed = Vec::new();
    for routine in routines {
        for entry in &routine.history {
            if entry.hook_calls.is_some() {
                continue;
            }
            let recorded = time_ms(Some(&entry.recorded_at)).unwrap_or(0);
            let due = time_ms(Some(&entry.due_at)).unwrap_or(0);
            if !(recorded > since_ms || due > since_ms) {
                continue;
            }
            let key = format!("{}\n{}", routine.routine_id, entry.due_at);
            if !seen.insert(key) {
                continue;
            }
            missed.push(AwayMissed {
                routine_id: routine.routine_id.clone(),
                name: routine.name.clone(),
                due_at: entry.due_at.clone(),
            });
        }
    }
    missed.sort_by(|a, b| {
        time_ms(Some(&a.due_at))
            .unwrap_or(0)
            .cmp(&time_ms(Some(&b.due_at)).unwrap_or(0))
    });
    if ran.is_empty() && failed.is_empty() && missed.is_empty() && waiting == 0 {
        return None;
    }
    Some(AwaySummary {
        since: since.to_owned(),
        ran,
        failed,
        missed,
        waiting,
    })
}

pub fn away_counts(summary: &AwaySummary) -> AwaySummaryCounts {
    AwaySummaryCounts {
        ran: summary.ran.len(),
        failed: summary.failed.len(),
        missed: summary.missed.len(),
        waiting: summary.waiting,
    }
}

/// "Since you were away: 2 ran · 1 failed · 1 missed · 1 waiting on you"
pub fn away_line(counts: Option<AwaySummaryCounts>) -> Option<String> {
    let counts = counts?;
    let mut parts = Vec::new();
    if counts.ran > 0 {
        parts.push(format!("{} ran", counts.ran));
    }
    if counts.failed > 0 {
        parts.push(format!("{} failed", counts.failed));
    }
    if counts.missed > 0 {
        parts.push(format!("{} missed", counts.missed));
    }
    if counts.waiting > 0 {
        parts.push(format!("{} waiting on you", counts.waiting));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("Since you were away: {}", parts.join(" · ")))
}

pub fn away_counts_from(value: &Value) -> Option<AwaySummaryCounts> {
    let count = |name: &str| -> Option<usize> {
        let held = value.get(name)?.as_u64()?;
        if held > 1_000_000 {
            return None;
        }
        Some(held as usize)
    };
    let ran = count("ran")?;
    let failed = count("failed")?;
    let missed = count("missed")?;
    let waiting = count("waiting")?;
    if ran + failed + missed + waiting == 0 {
        return None;
    }
    Some(AwaySummaryCounts {
        ran,
        failed,
        missed,
        waiting,
    })
}

pub fn was_away(last_at: Option<&str>, now_ms: i64, minimum_ms: i64) -> Option<String> {
    let at = time_ms(last_at)?;
    if now_ms - at >= minimum_ms {
        last_at.map(str::to_owned)
    } else {
        None
    }
}

pub fn attention_mark_from(value: &Value) -> Option<String> {
    let at = value.get("at")?.as_str()?;
    time_ms(Some(at))?;
    Some(at.to_owned())
}

pub fn read_attention_mark(store: &Store) -> Option<String> {
    let path = store.root().join(AWAY_FILE);
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    attention_mark_from(&value)
}

pub fn write_attention_mark(store: &Store, at: &str) -> Result<(), String> {
    store.save_json(AWAY_FILE, &json!({ "at": at }))
}

pub fn waiting_blurb(waiting: usize) -> Option<String> {
    match waiting {
        0 => None,
        1 => Some(
            "One conversation is waiting on you -- the sidebar marks it.".into(),
        ),
        n => Some(format!(
            "{n} conversations are waiting on you -- the sidebar marks them."
        )),
    }
}

pub fn when_label(iso: &str, now_ms: i64) -> String {
    let Some(at) = chrono::DateTime::parse_from_rfc3339(iso).ok() else {
        return String::new();
    };
    let Some(now) = chrono::DateTime::from_timestamp_millis(now_ms) else {
        return String::new();
    };
    let at_utc = at.with_timezone(&chrono::Utc);
    let time = at_utc.format("%H:%M").to_string();
    if at_utc.date_naive() == now.date_naive() {
        return time;
    }
    if let Some(yesterday) = now.date_naive().pred_opt()
        && at_utc.date_naive() == yesterday
    {
        return format!("yesterday {time}");
    }
    format!("{} {}", at_utc.format("%b"), at_utc.day())
}

/// Refuse writing the attention mark into Electron's folder (path check only).
pub fn attention_path_is_native(store: &Store, path: &Path) -> bool {
    let Ok(path) = store.check(path) else {
        return false;
    };
    path.starts_with(store.root()) && !path.starts_with(store.electron())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINCE: &str = "2026-10-04T01:00:00.000Z";

    fn row(mission_id: &str, phase: &str, last_at: &str) -> AwayRow {
        AwayRow {
            mission_id: mission_id.into(),
            title: format!("turn {mission_id}"),
            phase: phase.into(),
            last_at: Some(last_at.into()),
            owner_id: None,
            routine_id: None,
            member_ids: Vec::new(),
            root_id: None,
        }
    }

    #[test]
    fn lists_runs_that_ended_after_you_left_newest_first() {
        let mut m1 = row("m1", "completed", "2026-10-04T03:00:00.000Z");
        m1.owner_id = Some("tm_wren".into());
        m1.routine_id = Some("rt_1".into());
        let summary = since_you_were_away(
            SINCE,
            &[
                m1,
                row("m2", "completed", "2026-10-04T05:00:00.000Z"),
                row("m3", "completed", "2026-10-04T00:30:00.000Z"),
                row("m4", "failed", "2026-10-04T02:00:00.000Z"),
                row("m5", "running", "2026-10-04T06:00:00.000Z"),
                row("m6", "cancelled", "2026-10-04T04:00:00.000Z"),
            ],
            &[],
            &HashSet::new(),
        )
        .expect("summary");
        assert_eq!(
            summary.ran,
            vec![
                AwayItem {
                    mission_id: "m2".into(),
                    title: "turn m2".into(),
                    at: "2026-10-04T05:00:00.000Z".into(),
                    owner_id: None,
                    routine_id: None,
                },
                AwayItem {
                    mission_id: "m1".into(),
                    title: "turn m1".into(),
                    at: "2026-10-04T03:00:00.000Z".into(),
                    owner_id: Some("tm_wren".into()),
                    routine_id: Some("rt_1".into()),
                },
            ]
        );
        assert_eq!(
            summary.failed,
            vec![AwayItem {
                mission_id: "m4".into(),
                title: "turn m4".into(),
                at: "2026-10-04T02:00:00.000Z".into(),
                owner_id: None,
                routine_id: None,
            }]
        );
        assert!(summary.missed.is_empty());
        assert_eq!(summary.waiting, 0);
        assert_eq!(summary.since, SINCE);
    }

    #[test]
    fn counts_waiting_and_missed_slots() {
        let mut m1 = row("m1", "running", "2026-10-04T06:00:00.000Z");
        m1.member_ids = vec!["m1".into(), "m1b".into()];
        let mut m2 = row("m2", "running", "2026-10-04T06:00:00.000Z");
        m2.root_id = Some("r2".into());
        let routines = vec![
            AwayRoutine {
                routine_id: "rt_1".into(),
                name: "Morning digest".into(),
                history: vec![
                    AwayRoutineMiss {
                        due_at: "2026-10-04T05:00:00.000Z".into(),
                        recorded_at: "2026-10-04T07:00:00.000Z".into(),
                        hook_calls: None,
                    },
                    AwayRoutineMiss {
                        due_at: "2026-10-03T05:00:00.000Z".into(),
                        recorded_at: "2026-10-03T07:00:00.000Z".into(),
                        hook_calls: None,
                    },
                    AwayRoutineMiss {
                        due_at: "2026-10-04T05:00:00.000Z".into(),
                        recorded_at: "2026-10-04T07:00:00.000Z".into(),
                        hook_calls: None,
                    },
                ],
            },
            AwayRoutine {
                routine_id: "rt_2".into(),
                name: "Nightly tests".into(),
                history: vec![AwayRoutineMiss {
                    due_at: "2026-10-04T02:00:00.000Z".into(),
                    recorded_at: "2026-10-04T07:00:00.000Z".into(),
                    hook_calls: None,
                }],
            },
            AwayRoutine {
                routine_id: "rt_3".into(),
                name: "No history".into(),
                history: vec![],
            },
        ];
        let needs: HashSet<String> = ["m1b", "r2"].into_iter().map(str::to_owned).collect();
        let summary = since_you_were_away(
            SINCE,
            &[
                m1,
                m2,
                row("m3", "running", "2026-10-04T06:00:00.000Z"),
            ],
            &routines,
            &needs,
        )
        .expect("summary");
        assert_eq!(summary.waiting, 2);
        assert_eq!(
            summary.missed,
            vec![
                AwayMissed {
                    routine_id: "rt_2".into(),
                    name: "Nightly tests".into(),
                    due_at: "2026-10-04T02:00:00.000Z".into(),
                },
                AwayMissed {
                    routine_id: "rt_1".into(),
                    name: "Morning digest".into(),
                    due_at: "2026-10-04T05:00:00.000Z".into(),
                },
            ]
        );
        assert!(summary.ran.is_empty());
    }

    #[test]
    fn says_nothing_when_nothing_happened() {
        assert!(
            since_you_were_away(
                SINCE,
                &[row("m3", "completed", "2026-10-04T00:30:00.000Z")],
                &[],
                &HashSet::new(),
            )
            .is_none()
        );
        assert!(
            since_you_were_away(
                "yesterday-ish",
                &[row("m1", "completed", "2026-10-04T05:00:00.000Z")],
                &[],
                &HashSet::new(),
            )
            .is_none()
        );
    }

    #[test]
    fn words_the_counts_and_gap() {
        let summary = since_you_were_away(
            SINCE,
            &[
                row("m1", "completed", "2026-10-04T05:00:00.000Z"),
                row("m2", "completed", "2026-10-04T05:30:00.000Z"),
                row("m4", "failed", "2026-10-04T02:00:00.000Z"),
            ],
            &[],
            &HashSet::new(),
        )
        .expect("summary");
        assert_eq!(
            away_counts(&summary),
            AwaySummaryCounts {
                ran: 2,
                failed: 1,
                missed: 0,
                waiting: 0
            }
        );
        assert_eq!(
            away_line(Some(away_counts(&summary))).as_deref(),
            Some("Since you were away: 2 ran · 1 failed")
        );
        assert_eq!(
            away_line(Some(AwaySummaryCounts {
                ran: 0,
                failed: 0,
                missed: 1,
                waiting: 3
            }))
            .as_deref(),
            Some("Since you were away: 1 missed · 3 waiting on you")
        );
        assert!(away_line(Some(AwaySummaryCounts {
            ran: 0,
            failed: 0,
            missed: 0,
            waiting: 0
        }))
        .is_none());
        assert!(away_line(None).is_none());
    }

    #[test]
    fn counts_from_wire_and_was_away() {
        assert_eq!(
            away_counts_from(&json!({"ran":2,"failed":0,"missed":1,"waiting":0})),
            Some(AwaySummaryCounts {
                ran: 2,
                failed: 0,
                missed: 1,
                waiting: 0
            })
        );
        assert!(away_counts_from(&json!({"ran":0,"failed":0,"missed":0,"waiting":0})).is_none());
        assert!(away_counts_from(&json!({"ran":"2","failed":0,"missed":0,"waiting":0})).is_none());
        assert!(away_counts_from(&json!({"ran":1})).is_none());
        assert!(away_counts_from(&Value::Null).is_none());

        let now = chrono::DateTime::parse_from_rfc3339("2026-10-04T08:00:00.000Z")
            .expect("now")
            .timestamp_millis();
        assert_eq!(AWAY_MINIMUM_MS, 20 * 60_000);
        assert_eq!(
            was_away(Some("2026-10-04T07:40:00.000Z"), now, AWAY_MINIMUM_MS).as_deref(),
            Some("2026-10-04T07:40:00.000Z")
        );
        assert!(was_away(Some("2026-10-04T07:41:00.000Z"), now, AWAY_MINIMUM_MS).is_none());
        assert_eq!(
            was_away(Some("2026-10-03T22:00:00.000Z"), now, AWAY_MINIMUM_MS).as_deref(),
            Some("2026-10-03T22:00:00.000Z")
        );
        assert!(was_away(None, now, AWAY_MINIMUM_MS).is_none());
        assert!(was_away(Some("not a time"), now, AWAY_MINIMUM_MS).is_none());
    }

    #[test]
    fn attention_mark_round_trip_on_native_store() {
        let root = std::env::temp_dir().join(format!(
            "locust-away-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let electron = root.join("electron");
        let native = root.join("native");
        std::fs::create_dir_all(&electron).expect("electron");
        std::fs::create_dir_all(&native).expect("native");
        let store = Store::new(native.clone(), electron).expect("store");
        assert!(read_attention_mark(&store).is_none());
        write_attention_mark(&store, "2026-10-04T01:00:00.000Z").expect("write");
        assert_eq!(
            read_attention_mark(&store).as_deref(),
            Some("2026-10-04T01:00:00.000Z")
        );
        assert!(attention_path_is_native(
            &store,
            &native.join(AWAY_FILE)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn waiting_and_got_it_words() {
        assert_eq!(GOT_IT, "Got it");
        assert_eq!(HEADING, "Since you were away");
        assert_eq!(
            waiting_blurb(1).as_deref(),
            Some("One conversation is waiting on you -- the sidebar marks it.")
        );
        assert_eq!(
            waiting_blurb(3).as_deref(),
            Some("3 conversations are waiting on you -- the sidebar marks them.")
        );
    }
}
