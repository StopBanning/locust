//! A usage window as a person reads it (Electron `missionView.ts`: `usageWindowLabel`, `usagePercent`, `usageReadOf`,
//! `usageReadLine`, `usageWindowSentence`, `usageWindowsOf`). The ledger keeps the runtime's words with ISO reset
//! instants; a person reads a clock time, the fullest window, whether a window has reset since the reading, and where
//! the reading came from (Locust's last run, or the account itself).

use chrono::{DateTime, Local, Utc};

fn iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s.trim()).ok().map(|t| t.with_timezone(&Utc))
}

/// `14:44`, in this computer's timezone (`clockTime`).
pub fn clock_time(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).format("%H:%M").to_string()
}

fn label_at(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let local = at.with_timezone(&Local);
    if local.date_naive() == now.with_timezone(&Local).date_naive() { clock_time(at) } else { format!("{} {}", local.format("%a"), clock_time(at)) }
}

/// The runtime's words with each ISO instant turned into a local clock time ("resets 22:10", "resets Sun 03:00").
pub fn usage_window_label(said: &str, now: DateTime<Utc>) -> String {
    crate::invariants::regex(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z")
        .replace_all(said, |c: &regex::Captures| iso(&c[0]).map(|at| label_at(at, now)).unwrap_or_else(|| c[0].to_owned()))
        .into_owned()
}

/// One usage window, as a bar draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageWindow {
    pub name: String,
    /// Consumption: bars and warning tones use it.
    pub percent: u32,
    /// The runtime reported remaining quota instead.
    pub remaining: Option<u32>,
    pub resets: Option<String>,
    /// Its reset has passed since the reading: what it said is no longer true.
    pub expired: bool,
}

/// A reading's windows, in the runtime's order (`usageWindowsOf`), read from the raw reading where a reset is still an
/// instant, so whether it has passed can be compared.
pub fn usage_windows_of(said: &str, now: DateTime<Utc>) -> Vec<UsageWindow> {
    // Electron's lookahead `(?= · |$)` is expressed by matching the separator or the end and not consuming it.
    let re = crate::invariants::regex(r"([^·]+?) window (\d{1,3})% (used|left)(?: · resets ([^·]+?))?(?: · |$)");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(c) = re.captures_at(said, at) {
        let whole = c.get(0).unwrap();
        let value: u32 = c[2].parse::<u32>().unwrap_or(0).min(100);
        let left = &c[3] == "left";
        let reset = c.get(4).map(|m| m.as_str().trim().to_owned());
        let expired = reset.as_deref().and_then(iso).is_some_and(|t| t <= now);
        out.push(UsageWindow {
            name: format!("{} window", c[1].trim()),
            percent: if left { 100 - value } else { value },
            remaining: left.then_some(value),
            resets: reset.map(|r| usage_window_label(&r, now)),
            expired,
        });
        // Step back over the separator, so the next window's name starts after it.
        at = if whole.as_str().ends_with(" · ") { whole.end() - " · ".len() + 1 } else { whole.end() };
        if at >= said.len() {
            break;
        }
    }
    out
}

/// The fullest window's percentage, or None. A window that has reset since does not count.
pub fn usage_percent(said: &str, now: DateTime<Utc>) -> Option<u32> {
    usage_windows_of(said, now).into_iter().filter(|w| !w.expired).map(|w| w.percent).max()
}

/// Where and when a reading was taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadOf {
    /// The usage Locust's last run reported: use outside Locust since then is not in it.
    Run(String),
    /// Read from the account itself, so it counts everything.
    Account(String),
}

pub fn usage_read_of(said: &str) -> Option<ReadOf> {
    if let Some(c) = crate::invariants::regex(r" · from a run at (\d{4}-\d{2}-\d{2}T[\d:.]+Z)").captures(said) {
        return Some(ReadOf::Run(c[1].to_owned()));
    }
    crate::invariants::regex(r" · as of (\d{4}-\d{2}-\d{2}T[\d:.]+Z)").captures(said).map(|c| ReadOf::Account(c[1].to_owned()))
}

/// "From Locust's last run on it, Sat 21:23. Use outside Locust since then isn't counted." / "Read from your account
/// at 11:40."
pub fn usage_read_line(said: &str, now: DateTime<Utc>) -> Option<String> {
    Some(match usage_read_of(said)? {
        ReadOf::Run(at) => format!("From Locust's last run on it, {}. Use outside Locust since then isn't counted.", usage_window_label(&at, now)),
        ReadOf::Account(at) => format!("Read from your account at {}.", usage_window_label(&at, now)),
    })
}

/// The reading in the spec's words: "67% of the 5-hour window used, resets 22:10 · 53% of the 7-day window, resets
/// Sun 03:00", a window that has reset said so, and where the reading came from.
pub fn usage_window_sentence(said: &str, now: DateTime<Utc>) -> String {
    let windows = usage_windows_of(said, now);
    if windows.is_empty() {
        return usage_window_label(said, now);
    }
    let mut first = true;
    let parts: Vec<String> = windows
        .iter()
        .map(|w| {
            if w.expired {
                return format!("the {} has reset since", w.name);
            }
            let tail = match w.remaining {
                Some(_) => " left",
                None if first => " used",
                None => "",
            };
            let text = format!("{}% of the {}{tail}{}", w.remaining.unwrap_or(w.percent), w.name, w.resets.as_ref().map(|r| format!(", resets {r}")).unwrap_or_default());
            first = false;
            text
        })
        .collect();
    let read = match usage_read_of(said) {
        None => String::new(),
        Some(ReadOf::Run(at)) => format!(" (as of Locust's last run {})", usage_window_label(&at, now)),
        Some(ReadOf::Account(at)) => format!(" (as of {})", usage_window_label(&at, now)),
    };
    parts.join(" · ") + &read
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        iso(s).unwrap()
    }
    fn matches(pattern: &str, text: &str) -> bool {
        regex::Regex::new(pattern).unwrap().is_match(text)
    }

    // Electron's missionView.test.ts, "a usage window, as a person reads it".
    #[test]
    fn turns_iso_reset_instants_into_clock_times_and_leaves_the_words_alone() {
        let label = usage_window_label("5-hour window 67% used · resets 2026-09-06T02:10:00.000Z · 7-day window 53% used", at("2026-09-06T01:00:00.000Z"));
        assert!(!label.contains("2026-09-06T"), "{label}");
        assert!(matches(r"^5-hour window 67% used · resets .+ · 7-day window 53% used$", &label), "{label}");
        assert_eq!(usage_window_label("nothing to convert", Utc::now()), "nothing to convert");
    }

    // "utilisation, in the words of the spec".
    #[test]
    fn reads_the_fullest_window_and_words_the_sentence() {
        let said = "5-hour window 67% used · resets 2026-09-06T02:10:00.000Z · 7-day window 53% used · resets 2026-09-07T07:00:00.000Z";
        let before = at("2026-09-06T01:00:00.000Z");
        assert_eq!(usage_percent(said, before), Some(67));
        let sentence = usage_window_sentence(said, before);
        assert!(matches(r"^67% of the 5-hour window used, resets .+ · 53% of the 7-day window, resets .+$", &sentence), "{sentence}");
        assert_eq!(usage_percent("nothing", before), None);
    }

    #[test]
    fn does_not_count_a_window_whose_reset_has_passed_and_says_it_has_reset() {
        let said = "7-day window 66% used · resets 2026-09-28T07:00:00.000Z · 5-hour window 20% used · resets 2026-09-27T05:30:00.000Z · from a run at 2026-09-27T01:23:19.233Z";
        let later = at("2026-09-27T15:30:00.000Z");
        assert_eq!(usage_percent(said, later), Some(66));
        let sentence = usage_window_sentence(said, later);
        assert!(matches(r"^66% of the 7-day window used, resets .+ · the 5-hour window has reset since \(as of Locust's last run .+\)$", &sentence), "{sentence}");
        assert!(!sentence.contains("20%"));
    }

    #[test]
    fn says_where_a_reading_came_from_the_last_run_in_locust_or_the_account() {
        assert_eq!(usage_read_of("5-hour window 20% used · from a run at 2026-09-27T01:23:19.233Z"), Some(ReadOf::Run("2026-09-27T01:23:19.233Z".into())));
        assert_eq!(usage_read_of("primary 10% used · as of 2026-09-27T15:00:00.000Z"), Some(ReadOf::Account("2026-09-27T15:00:00.000Z".into())));
        assert_eq!(usage_read_of("5-hour window 20% used"), None);
        let now = at("2026-09-28T00:00:00.000Z");
        assert!(matches(r"^From Locust's last run on it, .+\. Use outside Locust since then isn't counted\.$", &usage_read_line("5-hour window 20% used · from a run at 2026-09-27T01:23:19.233Z", now).unwrap()));
        assert!(matches(r"^Read from your account at .+\.$", &usage_read_line("primary 10% used · as of 2026-09-27T15:00:00.000Z", now).unwrap()));
    }

    #[test]
    fn a_window_of_remaining_quota_says_left_and_counts_its_use() {
        let windows = usage_windows_of("week window 15% left", Utc::now());
        assert_eq!(windows, vec![UsageWindow { name: "week window".into(), percent: 85, remaining: Some(15), resets: None, expired: false }]);
        assert_eq!(usage_window_sentence("week window 15% left", Utc::now()), "15% of the week window left");
    }
}
