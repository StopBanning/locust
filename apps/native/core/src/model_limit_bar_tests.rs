//! `an-antigravity-model-limit-is-its-own-bar.test.ts`.

use crate::usage_words::usage_windows_of;

#[test]
fn reads_each_groups_windows_from_the_reading_the_models_own_limit_last() {
    let said = "Gemini: 5-hour window 100% left · Gemini: weekly window 100% left · Gemini 4 Argon: weekly window 84% left · resets 2099-10-23T02:27:29.000Z · as of 2099-10-16T13:54:47.000Z";
    let now = chrono::DateTime::parse_from_rfc3339("2099-10-16T14:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let windows = usage_windows_of(said, now);
    let named: Vec<(&str, Option<u32>)> = windows.iter().map(|w| (w.name.as_str(), w.remaining)).collect();
    assert_eq!(named, [("Gemini: 5-hour window", Some(100)), ("Gemini: weekly window", Some(100)), ("Gemini 4 Argon: weekly window", Some(84))]);
    assert!(windows[2].resets.is_some());
    assert!(!windows.iter().any(|w| w.expired));
}
