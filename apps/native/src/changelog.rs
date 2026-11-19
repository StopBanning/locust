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

/// `September 23, 2026` from `2026-09-23` without Date parsing (Electron `changelogDate`): exactly four, two and two
/// digits with a real month read as words; anything else is said as it was written.
pub fn changelog_date(raw: &str) -> Option<String> {
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let b = raw.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(0..4) || !digits(5..7) || !digits(8..10) {
        return Some(raw.to_owned());
    }
    let month: usize = raw[5..7].parse().unwrap_or(0);
    let Some(name) = month.checked_sub(1).and_then(|m| MONTHS.get(m)) else {
        return Some(raw.to_owned());
    };
    let day: u32 = raw[8..10].parse().unwrap_or(0);
    Some(format!("{name} {day}, {}", &raw[0..4]))
}

pub fn version_heading(entry: &ChangelogEntry) -> String {
    match entry.date.as_deref().and_then(changelog_date) {
        Some(date) => format!("{} · {date}", entry.version),
        None => format!("Version {}", entry.version),
    }
}

#[cfg(test)]
#[path = "whats_new_tests.rs"]
mod whats_new_tests;

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
    fn keeps_its_groups_new_improved_fixed_as_claude_codes_whats_new_does() {
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
    fn written_before_the_groups_stays_as_it_was_written_one_group_no_label_guessed_for_it() {
        assert_eq!(
            entries(FILE)[1].groups,
            vec![ChangelogGroup {
                label: None,
                text: "- **Antigravity asks here.** Written before the groups.".into(),
            }]
        );
    }

    #[test]
    fn marked_big_says_so_and_the_mark_itself_is_nowhere_in_what_is_shown() {
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
    fn shows_the_big_builds_between_the_one_last_seen_and_the_one_running() {
        let all = entries(FILE);
        assert_eq!(
            splash_entries(&all, Some("0.276.0"), "0.281.0")
                .iter()
                .map(|e| e.version.as_str())
                .collect::<Vec<_>>(),
            vec!["0.277.0"]
        );
    }

    #[test]
    fn shows_nothing_when_nothing_big_came_since_on_the_same_version_or_on_a_first_install() {
        let all = entries(FILE);
        assert!(splash_entries(&all, Some("0.277.0"), "0.281.0").is_empty());
        assert!(splash_entries(&all, Some("0.281.0"), "0.281.0").is_empty());
        assert!(splash_entries(&all, None, "0.281.0").is_empty());
    }

    #[test]
    fn shows_nothing_when_the_version_last_seen_is_not_in_the_file_a_splash_about_the_wrong_builds_is_worse() {
        let all = entries(FILE);
        assert!(splash_entries(&all, Some("9.9.9"), "0.281.0").is_empty());
        assert!(splash_entries(&all, Some("0.280.0"), "0.276.0").is_empty());
    }

    /// The repository's own CHANGELOG.md, the file the home splash reads first (`changelog_paths`), read only.
    fn shipped() -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).expect("the repository's CHANGELOG.md")
    }

    #[test]
    fn is_read_whole_hundreds_of_builds_each_with_something_to_say() {
        let all = entries(&shipped());
        assert!(all.len() > 300, "{}", all.len());
        let empty: Vec<&str> = all.iter().filter(|e| e.groups.is_empty()).map(|e| e.version.as_str()).collect();
        assert!(empty.is_empty(), "entries with nothing to say: {empty:?}");
    }

    // case: gives someone arriving from 0.276 the bots, the machine, the design pass, their own model, a room that remembers, pages that run each runtime's own commands a monochrome Locust, hand-off chains, landing a teammate's branch, comparing models, comparing their work and building to compare, pets, talking with ChatGPT's voice, the voice using their connectors, talking to a teammate by voice, starting Talk from any app, Scout keeping watch, their own cloud computer, their own benchmark, Claude Code and Codex on their cloud computer, the work carried on when a plan runs out, Locust on their phone, and agents of their own; from 0.777 nothing
    #[test]
    fn gives_someone_arriving_from_0_276_every_big_build_since_and_from_the_last_big_one_nothing() {
        let all = entries(&shipped());
        let newest = all[0].version.clone();
        // Electron's list, newest first: every build the file marks big.
        let big: Vec<&str> = vec![
            "0.845.0", "0.844.0", "0.843.0", "0.842.0", "0.841.0", "0.840.0", "0.839.0", "0.838.0", "0.837.0", "0.836.0", "0.835.0", "0.834.0",
            "0.833.0", "0.832.0", "0.831.0", "0.830.0", "0.829.0", "0.828.0", "0.826.0", "0.806.0", "0.804.0", "0.799.0", "0.798.0", "0.797.0",
            "0.796.0", "0.794.0", "0.789.0", "0.777.0", "0.775.0", "0.774.0", "0.768.0", "0.763.0", "0.761.0", "0.756.0", "0.755.0", "0.746.0",
            "0.742.0", "0.737.0", "0.563.0", "0.506.0", "0.503.0", "0.501.0", "0.498.0", "0.491.0", "0.474.0", "0.462.0", "0.458.0", "0.448.0",
            "0.445.0", "0.441.0", "0.440.0", "0.435.0", "0.434.0", "0.426.0", "0.425.0", "0.370.0", "0.357.0", "0.350.0", "0.295.0", "0.277.0",
        ];
        let splash = |seen: &str| splash_entries(&all, Some(seen), &newest).iter().map(|e| e.version.clone()).collect::<Vec<_>>();
        assert_eq!(splash("0.276.0"), big);
        assert_eq!(splash("0.280.0"), big[..big.len() - 1]);
        // From each big build, the big builds after it and nothing older; from the newest big one, nothing.
        for (at, seen) in big.iter().enumerate() {
            assert_eq!(splash(seen), big[..at], "arriving from {seen}");
        }
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
