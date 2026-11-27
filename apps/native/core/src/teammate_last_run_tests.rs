//! Words 1, row 2: a card that says "Runs 167" must not say "Last run never". Runs counts every conversation the record
//! gives a teammate; the last run was read only from the conversations the list shows, which leaves out Scout's looks and
//! side chats. These read the last run from the records themselves, and say "never" only for a teammate with no runs.
use super::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn folder(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-last-run-{label}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A record for `id` in `dir`, last written `at`.
fn record(dir: &Path, id: &str, at: SystemTime) {
    let path = dir.join(format!("{id}.jsonl"));
    std::fs::write(&path, "{}\n").unwrap();
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(at).unwrap();
}

fn at(iso: &str) -> SystemTime {
    SystemTime::from(chrono::DateTime::parse_from_rfc3339(iso).unwrap())
}

fn owners(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(m, t)| ((*m).into(), (*t).into())).collect()
}

#[test]
fn a_teammate_whose_runs_are_never_listed_still_has_its_newest_record_as_its_last_run() {
    let dir = folder("scout");
    record(&dir, "look_1", at("2026-11-20T08:00:00.000Z"));
    record(&dir, "look_2", at("2026-11-27T09:30:00.000Z"));
    record(&dir, "look_3", at("2026-11-25T10:00:00.000Z"));
    let owners = owners(&[("look_1", "tm_scout"), ("look_2", "tm_scout"), ("look_3", "tm_scout")]);
    let last = recorded_last_runs(&owners, std::slice::from_ref(&dir));
    assert_eq!(last.get("tm_scout").map(String::as_str), Some("2026-11-27T09:30:00.000Z"));
    // The same three as Runs counts them: a card with runs now has a last run.
    assert_eq!(owned_runs(&owners, "tm_scout"), 3);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reads_both_folders_and_keeps_each_teammate_to_its_own_records() {
    let (native, electron) = (folder("native"), folder("electron"));
    record(&native, "m_new", at("2026-11-26T12:00:00.000Z"));
    record(&electron, "m_old", at("2026-10-01T12:00:00.000Z"));
    record(&electron, "m_wren", at("2026-11-27T12:00:00.000Z"));
    let owners = owners(&[("m_new", "tm_atlas"), ("m_old", "tm_atlas"), ("m_wren", "tm_wren")]);
    let last = recorded_last_runs(&owners, &[native.clone(), electron.clone()]);
    assert_eq!(last.get("tm_atlas").map(String::as_str), Some("2026-11-26T12:00:00.000Z"));
    assert_eq!(last.get("tm_wren").map(String::as_str), Some("2026-11-27T12:00:00.000Z"));
    let _ = std::fs::remove_dir_all(native);
    let _ = std::fs::remove_dir_all(electron);
}

#[test]
fn a_record_that_is_gone_or_an_id_that_is_not_a_file_name_gives_no_time() {
    let dir = folder("gone");
    let outside = dir.parent().unwrap().join(format!("{}.jsonl", dir.file_name().unwrap().to_string_lossy()));
    std::fs::write(&outside, "{}\n").unwrap();
    let escape = format!("../{}", dir.file_name().unwrap().to_string_lossy());
    let owners = owners(&[("m_trashed", "tm_gone"), (escape.as_str(), "tm_gone")]);
    assert!(recorded_last_runs(&owners, std::slice::from_ref(&dir)).is_empty());
    let _ = std::fs::remove_file(outside);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_last_run_is_the_later_of_the_listed_one_and_the_recorded_one() {
    let listed = Some("2026-11-27T09:00:00.000Z");
    let recorded = Some("2026-11-27T10:00:00.000Z");
    assert_eq!(last_run(listed, recorded), recorded);
    assert_eq!(last_run(recorded, listed), recorded);
    assert_eq!(last_run(listed, None), listed);
    assert_eq!(last_run(None, recorded), recorded);
    assert_eq!(last_run(None, None), None);
    assert_eq!(last_run(Some("not a time"), recorded), recorded, "a time that can be read wins over one that cannot");
}

#[test]
fn says_never_only_for_a_teammate_with_no_runs_and_unknown_rather_than_never_when_runs_have_no_time() {
    let ago = |_: &str| Some("2 hours ago".to_owned());
    assert_eq!(last_run_words(167, Some("2026-11-27T08:00:00.000Z"), ago), "2 hours ago");
    assert_eq!(last_run_words(0, None, ago), "never");
    assert_eq!(last_run_words(167, None, ago), "unknown", "runs and no record left: not never");
    assert_eq!(last_run_words(3, Some("garbled"), |_| None), "unknown", "Electron's agoLabel ?? 'unknown'");
}

#[test]
fn a_fresh_record_reads_as_just_written() {
    let dir = folder("fresh");
    record(&dir, "m_now", SystemTime::now() - Duration::from_secs(5));
    let last = recorded_last_runs(&owners(&[("m_now", "tm_now")]), std::slice::from_ref(&dir));
    let written = chrono::DateTime::parse_from_rfc3339(&last["tm_now"]).unwrap();
    assert!((chrono::Utc::now() - written.with_timezone(&chrono::Utc)).num_seconds() < 60, "{written}");
    let _ = std::fs::remove_dir_all(dir);
}
