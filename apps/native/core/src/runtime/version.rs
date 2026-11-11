//! An agent CLI's version: read out of what `--version` prints, and asked for with a bounded probe.
//!
//! `parse` follows runtime-adapters/src/version.ts (parseRuntimeVersion): the first `major.minor.patch`, with an
//! optional pre-release and build, that stands alone in the text. "codex-cli 0.151.0-alpha.7.2", "2.1.251 (Claude
//! Code)" and Copilot's "GitHub Copilot CLI 1.0.82." all read.

use super::process::Process;
use regex::Regex;
use std::io;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// `major.minor.patch`, with `-pre` and `+build` when the CLI printed them.
    pub version: String,
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

fn pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    // version.ts's SEMVER_PATTERN without its trailing look-ahead, which the regex crate does not have; `stands_alone`
    // checks it instead, trying the shorter forms the JavaScript engine would fall back to.
    P.get_or_init(|| {
        Regex::new(r"(?:^|[^0-9A-Za-z])v?(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?")
            .unwrap()
    })
}

/// version.ts's `(?=$|\.(?![0-9])|[^0-9A-Za-z.+-])`: nothing follows, or a full stop that ends a sentence, or a
/// character that cannot continue a version.
fn stands_alone(rest: &str) -> bool {
    let mut chars = rest.chars();
    match chars.next() {
        None => true,
        Some('.') => !chars.next().is_some_and(|c| c.is_ascii_digit()),
        Some(c) => !(c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-')),
    }
}

pub fn parse(output: &str) -> Option<Version> {
    let text = output.trim();
    let mut from = 0;
    while from <= text.len() {
        let caps = pattern().captures_at(text, from)?;
        let whole = caps.get(0).unwrap();
        let number = |i: usize| caps.get(i).unwrap().as_str();
        let core_end = caps.get(3).unwrap().end();
        let pre = caps.get(4);
        let build = caps.get(5);
        // Longest first, as the JavaScript engine backtracks: with build, without it, without the pre-release too.
        let mut ends = vec![(whole.end(), pre, build)];
        if build.is_some() {
            ends.push((pre.map_or(core_end, |p| p.end()), pre, None));
        }
        if pre.is_some() {
            ends.push((core_end, None, None));
        }
        for (end, pre, build) in ends {
            if stands_alone(&text[end..]) {
                let (major, minor, patch) = (number(1), number(2), number(3));
                return Some(Version {
                    version: format!(
                        "{major}.{minor}.{patch}{}{}",
                        pre.map_or("", |m| m.as_str()),
                        build.map_or("", |m| m.as_str())
                    ),
                    major: major.parse().ok()?,
                    minor: minor.parse().ok()?,
                    patch: patch.parse().ok()?,
                });
            }
        }
        // The next match may start inside this one, as a JavaScript regex would retry from the next character.
        from = whole.start() + text[whole.start()..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// commands.ts's versionAtLeast: `major.minor.patch` compared as numbers, pre-release ignored; anything unreadable is
/// not at least anything.
pub fn at_least(version: &str, floor: &str) -> bool {
    let read = |t: &str| -> Option<[u64; 3]> {
        let caps = Regex::new(r"^v?(\d+)\.(\d+)\.(\d+)").unwrap().captures(t.trim())?;
        Some([caps[1].parse().ok()?, caps[2].parse().ok()?, caps[3].parse().ok()?])
    };
    match (read(version), read(floor)) {
        (Some(have), Some(need)) => have >= need,
        _ => false,
    }
}

/// Runs `<path> --version` for at most five seconds and 4 KiB of output; `name` is for the error messages.
pub fn probe(path: &Path, name: &str) -> io::Result<String> {
    let mut command = Command::new(path);
    command.arg("--version");
    let mut process = Process::spawn(&mut command)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = String::new();
    let take = |process: &mut Process, output: &mut String| -> io::Result<()> {
        while let Ok(s) = process.lines.try_recv() {
            if output.len() + s.len() > 4096 {
                let _ = process.stop();
                return Err(io::Error::other(format!("{name}'s version output exceeded its bound")));
            }
            output.push_str(&s);
        }
        Ok(())
    };
    loop {
        take(&mut process, &mut output)?;
        if let Some(code) = process.exited()? {
            take(&mut process, &mut output)?;
            if code != 0 {
                return Err(io::Error::other(format!("{name}'s version command failed")));
            }
            return Ok(output.trim().to_owned());
        }
        if Instant::now() >= deadline {
            let _ = process.stop();
            return Err(io::Error::other(format!("{name} did not report its version within five seconds")));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // runtime-adapters.test.ts, "parses runtime versions"
    #[test]
    fn reads_versions_as_electron_does() {
        assert_eq!(parse("codex-cli 0.151.0-alpha.7.2").unwrap().version, "0.151.0-alpha.7.2");
        assert_eq!(parse("2.1.251 (Claude Code)").unwrap().version, "2.1.251");
        assert_eq!(parse("GitHub Copilot CLI 1.0.82.").unwrap().version, "1.0.82");
        assert_eq!(parse("1.18.27").unwrap().version, "1.18.27");
        assert_eq!(parse("codex-cli 0.167.1").unwrap().major, 0);
        assert_eq!(parse("v2.0.1+build.5").unwrap().version, "2.0.1+build.5");
        assert!(parse("version 1.2").is_none());
        // Edge cases checked against version.ts with node on 11 Nov 2026.
        assert_eq!(parse("1.2.3.4").unwrap().version, "2.3.4");
        assert!(parse("abc1.2.3").is_none());
        assert_eq!(parse("abc1.2.3 then 4.5.6").unwrap().version, "4.5.6");
        assert_eq!(parse("1.2.3-rc.1.").unwrap().version, "1.2.3-rc.1");
        assert_eq!(parse("1.2.3-rc+b.x").unwrap().version, "1.2.3-rc+b.x");
        assert!(parse("x 1.2.3-").is_none());
    }

    #[test]
    fn compares_as_commands_ts_does() {
        assert!(at_least("0.153.0", "0.153.0"));
        assert!(at_least("0.167.1", "0.153.0"));
        assert!(at_least("1.0.0", "0.153.0"));
        assert!(!at_least("0.152.9", "0.153.0"));
        assert!(at_least("0.153.0-alpha.1", "0.153.0"));
        assert!(!at_least("codex-cli 0.167.1", "0.153.0"));
        assert!(!at_least("", "0.153.0"));
    }
}
