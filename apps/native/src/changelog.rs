//! What's new / CHANGELOG parsing — port of `main/changelog.ts`.
//! The native app shows its own notes (not Electron's auto-updater).

use std::path::{Path, PathBuf};

pub const READING: &str = "Reading what changed…";
pub const EMPTY: &str = "This build shipped without its changelog.";
pub const SHOW_OLDER: &str = "Show older versions";
/// Curly apostrophe, matching Electron `WhatsNew.tsx`.
pub const SPLASH_TITLE: &str = "What’s new in Locust";
pub const SEE_EVERY: &str = "See every version";
pub const GOT_IT: &str = "Got it";
pub const WHATS_NEW_PAGE: usize = 12;
pub const WHATS_NEW_MORE: usize = 30;

/// Native release notes packaged with this binary (Electron's file stays read-only).
pub const NATIVE_CHANGELOG: &str = r#"# Changelog

## 0.1.0 - 2026-11-10
<!-- big -->

### New

- **Home, Board and Since you were away** on the native window.
- **What's new** from these notes, with a splash on big builds.

### Improved

- First-launch discovery reuses the native PATH finder for installed runtimes.
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangelogGroup {
    pub label: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangelogEntry {
    pub version: String,
    pub date: Option<String>,
    pub body: String,
    pub big: bool,
    pub groups: Vec<ChangelogGroup>,
}

fn is_big_line(line: &str) -> bool {
    regex::Regex::new(r"(?i)^\s*<!--\s*big\s*-->\s*$")
        .expect("big marker regex")
        .is_match(line)
}

fn parse_heading(line: &str) -> Option<(String, Option<String>)> {
    let re = regex::Regex::new(r"^##\s+([0-9]+\.[0-9]+\.[0-9]+)\s*(?:[-–—]\s*(\S+))?\s*$")
        .expect("release heading regex");
    let found = re.captures(line)?;
    Some((
        found.get(1)?.as_str().to_owned(),
        found.get(2).map(|m| m.as_str().to_owned()),
    ))
}

fn closed(version: String, date: Option<String>, body: Vec<String>) -> ChangelogEntry {
    let big = body.iter().any(|line| is_big_line(line));
    let kept: Vec<String> = body.into_iter().filter(|line| !is_big_line(line)).collect();
    let mut groups: Vec<(Option<String>, Vec<String>)> = vec![(None, Vec::new())];
    for line in &kept {
        if let Some(label) = line
            .strip_prefix("###")
            .filter(|rest| rest.starts_with(char::is_whitespace))
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            groups.push((Some(label.to_owned()), Vec::new()));
            continue;
        }
        if let Some((_, lines)) = groups.last_mut() {
            lines.push(line.clone());
        }
    }
    let groups: Vec<ChangelogGroup> = groups
        .into_iter()
        .filter_map(|(label, lines)| {
            let text = lines.join("\n").trim().to_owned();
            if text.is_empty() {
                None
            } else {
                Some(ChangelogGroup { label, text })
            }
        })
        .collect();
    ChangelogEntry {
        version,
        date,
        body: kept.join("\n").trim().to_owned(),
        big,
        groups,
    }
}

/// Every entry in the file, newest first (file order preserved).
pub fn entries(text: &str) -> Vec<ChangelogEntry> {
    let mut found = Vec::new();
    let mut open: Option<(String, Option<String>, Vec<String>)> = None;
    for line in text.split('\n') {
        if let Some((version, date)) = parse_heading(line) {
            if let Some((v, d, body)) = open.take() {
                found.push(closed(v, d, body));
            }
            open = Some((version, date, Vec::new()));
            continue;
        }
        if let Some((_, _, body)) = open.as_mut() {
            body.push(line.to_owned());
        }
    }
    if let Some((v, d, body)) = open {
        found.push(closed(v, d, body));
    }
    found
}

pub fn splash_entries<'a>(
    all: &'a [ChangelogEntry],
    seen: Option<&str>,
    current: &str,
) -> Vec<&'a ChangelogEntry> {
    let Some(seen) = seen else {
        return Vec::new();
    };
    if seen == current {
        return Vec::new();
    }
    let from = all.iter().position(|e| e.version == current);
    let to = all.iter().position(|e| e.version == seen);
    let (Some(from), Some(to)) = (from, to) else {
        return Vec::new();
    };
    if to <= from {
        return Vec::new();
    }
    all[from..to].iter().filter(|e| e.big).collect()
}

pub fn entry_for(text: &str, version: &str) -> Option<ChangelogEntry> {
    entries(text).into_iter().find(|e| e.version == version)
}

pub fn changelog_paths(resources: &Path, app: &Path) -> Vec<PathBuf> {
    vec![
        resources.join("CHANGELOG.md"),
        app.join("..").join("..").join("CHANGELOG.md"),
    ]
}

pub fn read_changelog(paths: &[PathBuf]) -> Option<String> {
    for path in paths {
        if let Ok(text) = std::fs::read_to_string(path) {
            return Some(text);
        }
    }
    None
}

/// Native notes: prefer a file beside the data folder / binary, else the embedded constant.
pub fn native_notes(extra: Option<&Path>) -> String {
    if let Some(path) = extra
        && let Ok(text) = std::fs::read_to_string(path)
    {
        return text;
    }
    NATIVE_CHANGELOG.to_owned()
}

/// `September 23, 2026` from `2026-09-23` without Date parsing (Electron `changelogDate`).
pub fn changelog_date(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    let name = match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => return None,
    };
    if !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{name} {day}, {year}"))
}

pub fn version_heading(entry: &ChangelogEntry) -> String {
    match entry.date.as_deref().and_then(changelog_date) {
        Some(date) => format!("{} · {date}", entry.version),
        None => format!("Version {}", entry.version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "\
# Changelog

## 0.281.0 - 2026-09-23

### New

- **What's new, in Settings.** Every version.

### Improved

- **No banner.** Gone.

## 0.280.0 - 2026-09-23

- **Antigravity asks here.** Written before the groups.

## 0.277.0 - 2026-09-22
<!-- big -->

- **Every teammate is a bot.**

## 0.276.0 - 2026-09-22

- **Dialogs fit.**
";

    #[test]
    fn keeps_groups_as_claude_code_does() {
        let all = entries(FILE);
        let newest = &all[0];
        assert_eq!(
            newest.groups,
            vec![
                ChangelogGroup {
                    label: Some("New".into()),
                    text: "- **What's new, in Settings.** Every version.".into(),
                },
                ChangelogGroup {
                    label: Some("Improved".into()),
                    text: "- **No banner.** Gone.".into(),
                },
            ]
        );
    }

    #[test]
    fn pre_group_entries_stay_unlabeled() {
        assert_eq!(
            entries(FILE)[1].groups,
            vec![ChangelogGroup {
                label: None,
                text: "- **Antigravity asks here.** Written before the groups.".into(),
            }]
        );
    }

    #[test]
    fn big_mark_is_flagged_and_stripped() {
        let bot = entries(FILE)
            .into_iter()
            .find(|e| e.version == "0.277.0")
            .expect("bot");
        assert!(bot.big);
        assert!(!bot.body.contains("<!--"));
        assert!(!format!("{:?}", bot.groups).contains("<!--"));
        assert_eq!(
            entries(FILE)
                .into_iter()
                .filter(|e| e.big)
                .map(|e| e.version)
                .collect::<Vec<_>>(),
            vec!["0.277.0"]
        );
    }

    #[test]
    fn splash_shows_big_builds_between_seen_and_current() {
        let all = entries(FILE);
        assert_eq!(
            splash_entries(&all, Some("0.276.0"), "0.281.0")
                .iter()
                .map(|e| e.version.as_str())
                .collect::<Vec<_>>(),
            vec!["0.277.0"]
        );
        assert!(splash_entries(&all, Some("0.277.0"), "0.281.0").is_empty());
        assert!(splash_entries(&all, Some("0.281.0"), "0.281.0").is_empty());
        assert!(splash_entries(&all, None, "0.281.0").is_empty());
        assert!(splash_entries(&all, Some("9.9.9"), "0.281.0").is_empty());
        assert!(splash_entries(&all, Some("0.280.0"), "0.276.0").is_empty());
    }

    #[test]
    fn electron_words_exact() {
        assert_eq!(READING, "Reading what changed…");
        assert_eq!(EMPTY, "This build shipped without its changelog.");
        assert_eq!(SHOW_OLDER, "Show older versions");
        assert_eq!(SPLASH_TITLE, "What’s new in Locust");
        assert_eq!(SEE_EVERY, "See every version");
        assert_eq!(GOT_IT, "Got it");
        assert_eq!(
            changelog_date("2026-09-23").as_deref(),
            Some("September 23, 2026")
        );
        assert_eq!(
            version_heading(&ChangelogEntry {
                version: "0.281.0".into(),
                date: Some("2026-09-23".into()),
                body: String::new(),
                big: false,
                groups: vec![],
            }),
            "0.281.0 · September 23, 2026"
        );
    }

    #[test]
    fn native_notes_parse() {
        let all = entries(NATIVE_CHANGELOG);
        assert!(!all.is_empty());
        assert!(all.iter().any(|e| e.big));
        assert!(
            all[0]
                .groups
                .iter()
                .any(|g| g.label.as_deref() == Some("New"))
        );
    }

    #[test]
    fn en_dash_heading() {
        let text = "## 0.43.6 — 2026-09-07\n\n- **Old dash.**\n";
        let all = entries(text);
        assert_eq!(all[0].version, "0.43.6");
        assert_eq!(all[0].date.as_deref(), Some("2026-09-07"));
    }

    #[test]
    fn helpers_cover_paths_and_entry_for() {
        let paths = changelog_paths(Path::new("/res"), Path::new("/app/bin"));
        assert_eq!(paths.len(), 2);
        assert!(entry_for(FILE, "0.281.0").is_some());
        assert!(read_changelog(&[]).is_none());
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn only_electron_release_headings_and_big_markers_are_recognized() {
        let parsed = entries(
            "## 1.2.3 - 2026-11-10\n<!-- bigger -->\n###Not a group\ntext\n##1.2.4\n## 1..5\n## 1.2.6 - two words\n",
        );
        assert_eq!(parsed.len(), 1);
        assert!(!parsed[0].big);
        assert_eq!(parsed[0].groups.len(), 1);
        assert!(parsed[0].body.contains("<!-- bigger -->"));
        assert!(entries("## 1.2.3\n<!-- BIG -->\nnews")[0].big);
    }
}
