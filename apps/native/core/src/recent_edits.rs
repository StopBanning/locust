//! A2.9: THE OVERLAP NOTE (Electron `recent-edits.ts`). Two teammates working in one folder can each change the same
//! file, a run apart, and neither is told: the second edit lands on top of the first and the brief it starts from says
//! nothing about it. The host already reads every writing run's folder before and after (`record_disk_observation`).
//! This keeps what it read, per folder and teammate, for a few hours, and a teammate's next brief names the files
//! another teammate changed that they changed too.
//!
//! Held in memory on purpose: it is a heads-up about the last few hours, not a record. A restart forgets it, and the
//! ledger still has every change.
use std::collections::HashMap;
use std::sync::Mutex;

/// How long a change counts as recent.
pub const OVERLAP_WINDOW_MS: i64 = 6 * 60 * 60 * 1000;
/// Files named per teammate in the note; the rest are counted.
const NAMED_PER_TEAMMATE: usize = 8;

struct Edits {
    name: String,
    paths: HashMap<String, i64>,
}

/// Per folder, per teammate, in the order teammates were first seen there (Electron's Map order).
#[derive(Default)]
pub struct RecentEdits {
    folders: HashMap<String, Vec<(String, Edits)>>,
}

/// One key per folder, whatever the spelling Windows allows for it.
fn folder_key(folder: &str) -> String {
    let slashed = folder.replace('\\', "/");
    let slashed = slashed.trim_end_matches('/');
    if cfg!(windows) { slashed.to_lowercase() } else { slashed.to_owned() }
}

fn ago(ms: i64) -> String {
    let minutes = ((ms as f64) / 60_000.0).round().max(1.0) as i64;
    if minutes < 60 {
        return if minutes == 1 { "a minute ago".into() } else { format!("{minutes} minutes ago") };
    }
    let hours = ((minutes as f64) / 60.0).round() as i64;
    if hours == 1 { "an hour ago".into() } else { format!("{hours} hours ago") }
}

impl RecentEdits {
    /// A writing run, read by the host, changed these paths in this folder at `at` (milliseconds).
    pub fn record(&mut self, folder: &str, teammate_id: &str, name: &str, paths: &[String], at: i64) {
        if paths.is_empty() {
            return;
        }
        let by_teammate = self.folders.entry(folder_key(folder)).or_default();
        let at_index = match by_teammate.iter().position(|(id, _)| id == teammate_id) {
            Some(i) => i,
            None => {
                by_teammate.push((teammate_id.to_owned(), Edits { name: name.to_owned(), paths: HashMap::new() }));
                by_teammate.len() - 1
            }
        };
        let edits = &mut by_teammate[at_index].1;
        edits.name = name.to_owned();
        for path in paths {
            edits.paths.insert(path.clone(), at);
        }
    }

    /// The note for a teammate about to start in this folder: the files another teammate changed there recently that
    /// this one also changed recently. None when there is none.
    pub fn overlap_for(&mut self, folder: &str, teammate_id: &str, now: i64) -> Option<String> {
        let window = OVERLAP_WINDOW_MS;
        let by_teammate = self.folders.get_mut(&folder_key(folder))?;
        // What is no longer recent is forgotten as it is read.
        let recent = |edits: &mut Edits| -> HashMap<String, i64> {
            edits.paths.retain(|_, at| now - *at <= window && *at <= now + 60_000);
            edits.paths.clone()
        };
        let mine = match by_teammate.iter_mut().find(|(id, _)| id == teammate_id) {
            Some((_, edits)) => recent(edits),
            None => return None,
        };
        if mine.is_empty() {
            return None;
        }
        let mut lines = Vec::new();
        for (id, edits) in by_teammate.iter_mut() {
            if id == teammate_id {
                continue;
            }
            let theirs = recent(edits);
            let mut both: Vec<(String, i64)> = theirs.into_iter().filter(|(path, _)| mine.contains_key(path)).collect();
            if both.is_empty() {
                continue;
            }
            // Newest first; a tie keeps a stable order by path.
            both.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let named: Vec<&str> = both.iter().take(NAMED_PER_TEAMMATE).map(|(path, _)| path.as_str()).collect();
            let more = both.len() - named.len();
            let more = if more > 0 { format!(" and {more} more") } else { String::new() };
            lines.push(format!("{} changed {}{more} (last {})", edits.name, named.join(", "), ago(now - both[0].1)));
        }
        if lines.is_empty() {
            return None;
        }
        let mut note = vec!["Files you changed recently in this folder were also changed by another teammate since, or around the same time -- read their change before you edit these again, so neither of you undoes the other:".to_owned()];
        note.extend(lines.into_iter().map(|line| format!("- {line}.")));
        Some(note.join("\n"))
    }
}

/// The app's one record of recent edits.
static EDITS: Mutex<Option<RecentEdits>> = Mutex::new(None);

pub fn record(folder: &str, teammate_id: &str, name: &str, paths: &[String]) {
    if let Ok(mut edits) = EDITS.lock() {
        edits.get_or_insert_with(RecentEdits::default).record(folder, teammate_id, name, paths, chrono::Utc::now().timestamp_millis());
    }
}

pub fn overlap_for(folder: &str, teammate_id: &str) -> Option<String> {
    EDITS.lock().ok()?.get_or_insert_with(RecentEdits::default).overlap_for(folder, teammate_id, chrono::Utc::now().timestamp_millis())
}

#[cfg(test)]
#[path = "recent_edits_tests.rs"]
mod tests;
