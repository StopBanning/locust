//! Read-only folder diffs. Commands and bounds ported from main/folder-commit.ts.
use std::{path::Path, process::Command};
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub kind: char,
    pub old: Option<usize>,
    pub new: Option<usize>,
    pub text: String,
}
#[derive(Clone, Debug, Default)]
pub struct Hunk {
    pub old: usize,
    pub new: usize,
    pub heading: String,
    pub rows: Vec<Row>,
}
#[derive(Clone, Debug, Default)]
pub struct File {
    pub path: String,
    pub status: String,
    pub hunks: Vec<Hunk>,
}
impl File {
    pub fn counts(&self) -> (usize, usize) {
        self.hunks
            .iter()
            .flat_map(|h| &h.rows)
            .fold((0, 0), |(a, d), r| {
                (
                    a + usize::from(r.kind == '+'),
                    d + usize::from(r.kind == '-'),
                )
            })
    }
    pub fn after(&self) -> Option<String> {
        if self.status == "DELETED" {
            return None;
        }
        let parts: Vec<_> = self
            .hunks
            .iter()
            .map(|h| {
                h.rows
                    .iter()
                    .filter(|r| r.kind != '-')
                    .map(|r| r.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|s| !s.is_empty())
            .collect();
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }
}
impl Hunk {
    pub fn range(&self) -> String {
        let old = self.rows.iter().filter(|r| r.kind != '+').count();
        let new = self.rows.iter().filter(|r| r.kind != '-').count();
        let span = |s, n| {
            if n == 1 {
                format!("{s}")
            } else {
                format!("{s},{n}")
            }
        };
        format!("@@ -{} +{} @@", span(self.old, old), span(self.new, new))
    }
    pub fn sides(&self) -> (String, String) {
        (
            self.rows
                .iter()
                .filter(|r| r.kind != '+')
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            self.rows
                .iter()
                .filter(|r| r.kind != '-')
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}
pub fn parse(text: &str) -> Vec<File> {
    let mut files = Vec::new();
    let mut file: Option<File> = None;
    let mut left = (0usize, 0usize);
    let mut numbers = (0, 0);
    for line in text.split('\n') {
        if (left.0 > 0 || left.1 > 0)
            && let Some(h) = file.as_mut().and_then(|f| f.hunks.last_mut())
        {
            let kind = line.chars().next().unwrap_or('\0');
            if (kind == '+' && left.1 > 0)
                || (kind == '-' && left.0 > 0)
                || (kind == ' ' && left.0 > 0 && left.1 > 0)
            {
                h.rows.push(Row {
                    kind,
                    old: (kind != '+').then_some(numbers.0),
                    new: (kind != '-').then_some(numbers.1),
                    text: line[1..].into(),
                });
                if kind != '+' {
                    numbers.0 += 1;
                    left.0 -= 1;
                }
                if kind != '-' {
                    numbers.1 += 1;
                    left.1 -= 1;
                }
                continue;
            }
        }
        if let Some(names) = line.strip_prefix("diff --git a/") {
            if let Some(f) = file.take() {
                files.push(f)
            }
            if let Some((old, new)) = names.rsplit_once(" b/") {
                file = Some(File {
                    path: new.into(),
                    status: if old == new { "MODIFIED" } else { "RENAMED" }.into(),
                    hunks: vec![],
                });
            }
            left = (0, 0);
            continue;
        }
        if let Some(old) = line.strip_prefix("--- ") {
            if file.as_ref().is_some_and(|f| !f.hunks.is_empty())
                && let Some(f) = file.take()
            {
                files.push(f)
            }
            let f = file.get_or_insert_with(File::default);
            if old == "/dev/null" {
                f.status = "ADDED".into()
            } else if f.path.is_empty() {
                f.path = old.strip_prefix("a/").unwrap_or(old).into();
                f.status = "MODIFIED".into()
            }
            continue;
        }
        let Some(f) = file.as_mut() else { continue };
        if let Some(new) = line.strip_prefix("+++ ") {
            if new == "/dev/null" {
                f.status = "DELETED".into()
            } else {
                f.path = new.strip_prefix("b/").unwrap_or(new).into()
            }
        }
        if line.starts_with("new file mode ") {
            f.status = "ADDED".into()
        }
        if line.starts_with("deleted file mode ") {
            f.status = "DELETED".into()
        }
        if let Some(p) = line.strip_prefix("rename to ") {
            f.path = p.into();
            f.status = "RENAMED".into()
        }
        if let Some(c) =
            crate::invariants::regex(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@ ?(.*)$")
                .captures(line)
        {
            let n = |i| {
                c.get(i)
                    .and_then(|m| m.as_str().parse::<usize>().ok())
                    .unwrap_or(1)
            };
            numbers = (n(1), n(3));
            left = (n(2), n(4));
            if f.hunks.is_empty() && numbers.0 == 0 && left.0 == 0 && left.1 > 0 {
                f.status = "ADDED".into()
            }
            f.hunks.push(Hunk {
                old: numbers.0,
                new: numbers.1,
                heading: c.get(5).map_or("", |m| m.as_str()).into(),
                rows: vec![],
            });
        }
    }
    if let Some(f) = file {
        files.push(f)
    }
    files
}
pub fn git(folder: &Path, args: &[&str]) -> Result<String, String> {
    use std::{
        io::Read,
        process::Stdio,
        time::{Duration, Instant},
    };
    let mut child = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(folder)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("No git output")?;
    let stderr = child.stderr.take().ok_or("No git error output")?;
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout
            .take(256 * 1024 * 1024)
            .read_to_end(&mut b)
            .map(|_| b)
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        stderr.take(1024 * 1024).read_to_end(&mut b).map(|_| b)
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Git took longer than 30 s and was stopped.".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let output = out
        .join()
        .map_err(|_| "Git output worker failed")?
        .map_err(|e| e.to_string())?;
    let error = err
        .join()
        .map_err(|_| "Git error worker failed")?
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(String::from_utf8_lossy(&output).into_owned())
    } else {
        // Electron's `printed()` keeps stdout when git exits non-zero (diff/show with changes).
        let out = String::from_utf8_lossy(&output).into_owned();
        if !out.is_empty() {
            Ok(out)
        } else {
            Err(String::from_utf8_lossy(&error).into_owned())
        }
    }
}

pub fn repository(folder: &Path) -> bool {
    git(folder, &["rev-parse", "--is-inside-work-tree"]).is_ok_and(|s| s.trim() == "true")
        && git(folder, &["rev-parse", "--show-toplevel"]).is_ok_and(|s| {
            s.trim().trim_end_matches('/').to_lowercase()
                == folder
                    .canonicalize()
                    .unwrap_or_else(|_| folder.to_owned())
                    .to_string_lossy()
                    .trim_end_matches('/')
                    .to_lowercase()
        })
}
pub fn new_file(path: &str, bytes: &[u8]) -> String {
    let head = format!("diff --git a/{path} b/{path}\nnew file mode 100644\n");
    if bytes.is_empty() {
        return head;
    }
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return format!("{head}Binary files /dev/null and b/{path} differ\n");
    }
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<_> = text
        .strip_suffix('\n')
        .unwrap_or(&text)
        .split('\n')
        .collect();
    format!(
        "{head}--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n{}\n",
        lines.len(),
        lines
            .iter()
            .map(|l| format!("+{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
#[derive(Clone, Default)]
pub struct Folder {
    pub base: String,
    pub branch: String,
    pub files: Vec<File>,
    pub truncated: bool,
    pub commits: Vec<(String, String)>,
}
pub fn read(folder: &Path, sha: Option<&str>) -> Result<Folder, String> {
    if !repository(folder) {
        return Err("This folder is not a git repository, so there are no changes to show.".into());
    }
    let run = |a: &[&str]| git(folder, a).unwrap_or_default();
    let branch = run(&["symbolic-ref", "--short", "-q", "HEAD"])
        .trim()
        .to_owned();
    let branch = if branch.is_empty() {
        "HEAD".into()
    } else {
        branch
    };
    let remote = run(&["remote", "get-url", "origin"]);
    let head = run(&["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"]);
    let head = head.trim().trim_start_matches("origin/");
    let known = if remote.trim().is_empty() {
        run(&["branch", "--format=%(refname:short)"])
    } else {
        run(&["branch", "-r", "--format=%(refname:short)"])
    };
    let default = if !head.is_empty() {
        head
    } else if known.lines().any(|l| l == "main" || l == "origin/main") {
        "main"
    } else if known.lines().any(|l| l == "master" || l == "origin/master") {
        "master"
    } else {
        "main"
    };
    let mut base = "HEAD".to_owned();
    let mut since = "HEAD".to_owned();
    if branch != default {
        for r in [
            format!("refs/remotes/origin/{default}"),
            format!("refs/heads/{default}"),
        ] {
            let found = run(&["merge-base", "HEAD", &r]);
            if matches!(found.trim().len(), 40 | 64) {
                base = default.into();
                since = found.trim().into();
                break;
            }
        }
    }
    let commits = if since == "HEAD" {
        vec![]
    } else {
        run(&[
            "log",
            "--max-count=100",
            "--format=%H%x1f%h%x1f%s%x1f%an%x1f%cI",
            &format!("{since}..HEAD"),
        ])
        .lines()
        .filter_map(|l| {
            let p: Vec<_> = l.split('\u{1f}').collect();
            (p.len() == 5).then(|| (p[0].into(), p[2].chars().take(200).collect()))
        })
        .collect::<Vec<_>>()
    };
    let one = sha.filter(|s| commits.iter().any(|c| c.0 == *s));
    let mut text = if let Some(s) = one {
        run(&[
            "-c",
            "core.quotePath=false",
            "show",
            "--format=",
            "--no-color",
            "--no-ext-diff",
            "-M",
            s,
            "--",
            ".",
            ":(exclude).locust",
        ])
    } else {
        run(&[
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "-M",
            &since,
            "--",
            ".",
            ":(exclude).locust",
        ])
    };
    let mut truncated = false;
    if one.is_none() {
        let listed = run(&[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            ".",
            ":(exclude).locust",
        ]);
        let paths: Vec<_> = listed.split('\0').filter(|p| !p.is_empty()).collect();
        truncated = paths.len() > 200;
        for p in paths.iter().take(200) {
            if text.len() >= 3 * 1024 * 1024 {
                truncated = true;
                break;
            }
            let full = folder.join(p);
            // Follow symlinks to files like Electron's stat/readFile; skip directories.
            if let Ok(info) = std::fs::metadata(&full) {
                if !info.is_file() {
                    continue;
                }
                if info.len() > 512 * 1024 {
                    text.push_str(&format!("diff --git a/{p} b/{p}\nnew file mode 100644\n"));
                } else if let Ok(bytes) = std::fs::read(&full) {
                    text.push_str(&new_file(p, &bytes));
                }
            }
        }
    }
    if text.len() > 3 * 1024 * 1024 {
        let mut at = 3 * 1024 * 1024;
        while !text.is_char_boundary(at) {
            at -= 1
        }
        let cut = text[..at]
            .rfind("\ndiff --git ")
            .map(|i| i + 1)
            .unwrap_or(at);
        text.truncate(cut);
        truncated = true;
    }
    Ok(Folder {
        base,
        branch,
        files: parse(&text),
        truncated,
        commits,
    })
}
fn hunk_word(count: usize) -> String {
    format!("{count} hunk{}", if count == 1 { "" } else { "s" })
}
pub fn completeness(f: &File, shown: usize, truncated: bool) -> String {
    let shown = shown.min(f.hunks.len());
    let rest = f.hunks.len() - shown;
    if rest > 0 {
        let n = f.hunks[shown..]
            .iter()
            .flat_map(|h| &h.rows)
            .filter(|r| r.kind != ' ')
            .count();
        format!(
            "Expand full file · {} more, {n} line{}",
            hunk_word(rest),
            if n == 1 { "" } else { "s" }
        )
    } else if truncated {
        format!(
            "The runtime reported more than fits here · {} rendered · the rest was not recorded inline",
            hunk_word(shown)
        )
    } else if shown == 1 {
        "The whole change is shown".into()
    } else {
        format!("All {shown} sections shown")
    }
}
/// Empty-diff copy matches Electron: clean branch vs a commit Locust cannot show.
pub fn empty_diff_words(commits: usize) -> &'static str {
    if commits == 0 {
        "Nothing has changed here since the branch left its base."
    } else {
        "This commit changes no files Locust can show."
    }
}
pub fn line_counts(added: usize, removed: usize) -> String {
    let mut parts = vec![];
    if added > 0 {
        parts.push(format!("+{added}"))
    }
    if removed > 0 {
        parts.push(format!("−{removed}"))
    }
    parts.join(" ")
}
pub fn kept_context(rows: &[Row]) -> Vec<bool> {
    rows.iter()
        .enumerate()
        .map(|(k, r)| {
            r.kind != ' '
                || rows
                    .iter()
                    .enumerate()
                    .any(|(q, t)| t.kind != ' ' && q.abs_diff(k) <= 3)
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    pub struct Temp(pub std::path::PathBuf);
    impl Temp {
        pub fn new() -> Self {
            let p = std::env::temp_dir().join(format!("locust-p7-{}-{}", std::process::id(), {
                static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            }));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn counts_only_say_what_changed() {
        assert_eq!(line_counts(299, 0), "+299");
        assert_eq!(line_counts(0, 12), "−12");
        assert_eq!(line_counts(5, 3), "+5 −3");
        assert_eq!(line_counts(0, 0), "");
    }
    #[test]
    fn context_folds_exactly_and_never_hides_changes() {
        let mut rows = vec![
            Row {
                kind: ' ',
                ..Row::default()
            };
            14
        ];
        rows[7].kind = '-';
        rows[8].kind = '+';
        let keep = kept_context(&rows);
        assert_eq!(keep[..4], [false; 4]);
        assert!(keep[4..12].iter().all(|b| *b));
        assert_eq!(keep[12..], [false; 2]);
    }
    #[test]
    fn a_diff_is_not_read_out_of_a_files_content() {
        let content = b"--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n";
        let f = parse(&new_file("run.sh", content));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, "run.sh");
        assert_eq!(f[0].counts(), (5, 0));
        assert_eq!(f[0].status, "ADDED");
    }
    #[test]
    fn header_like_rows_and_next_file() {
        let f = parse(
            "--- a/query.sql\n+++ b/query.sql\n@@ -1,2 +1,2 @@\n--- old comment\n+++ new line\n SELECT 1;\n--- a/two.txt\n+++ b/two.txt\n@@ -1 +1 @@\n-a\n+b\n",
        );
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].counts(), (1, 1));
        assert_eq!(f[0].hunks[0].rows[0].text, "-- old comment");
        assert_eq!(f[1].path, "two.txt");
    }
    #[test]
    fn added_cursor_file_counts_numbers_ranges_and_copy() {
        let f =
            parse("--- /tmp/a.rs\n+++ /tmp/a.rs\n@@ -0,0 +1,2 @@ symbol\n+one\n+two\n").remove(0);
        assert_eq!(f.status, "ADDED");
        assert_eq!(f.hunks[0].rows[1].new, Some(2));
        assert_eq!(f.hunks[0].range(), "@@ -0,0 +1,2 @@");
        assert_eq!(f.after().as_deref(), Some("one\ntwo"));
        assert_eq!(f.counts(), (2, 0));
    }
    #[test]
    fn copies_after_code_with_parts_apart_and_no_deleted_code() {
        let f = parse(
            "--- a/a\n+++ b/a\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n@@ -12 +12 @@\n-later\n+next\n",
        )
        .remove(0);
        assert_eq!(f.after().as_deref(), Some("keep\nnew\n\nnext"));
        let d = parse("--- a/a\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n");
        assert!(d[0].after().is_none());
    }
    #[test]
    fn ranges_derive_from_rows_and_completeness_never_silent() {
        let mut f = parse("--- a/a\n+++ b/a\n@@ -10,99 +20,99 @@\n-old\n+new\n").remove(0);
        assert_eq!(f.hunks[0].range(), "@@ -10 +20 @@");
        assert_eq!(completeness(&f, 1, false), "The whole change is shown");
        f.hunks.push(f.hunks[0].clone());
        assert_eq!(
            completeness(&f, 1, false),
            "Expand full file · 1 hunk more, 2 lines"
        );
        for n in 0..5 {
            for cut in [false, true] {
                assert!(!completeness(&f, n, cut).is_empty())
            }
        }
        assert_eq!(
            completeness(&f, 2, true),
            "The runtime reported more than fits here · 2 hunks rendered · the rest was not recorded inline"
        );
        assert_eq!(
            completeness(&f, 1, true),
            "Expand full file · 1 hunk more, 2 lines"
        );
    }
    #[test]
    fn empty_diff_words_split_clean_and_unshowable_commit() {
        assert_eq!(
            empty_diff_words(0),
            "Nothing has changed here since the branch left its base."
        );
        assert_eq!(
            empty_diff_words(1),
            "This commit changes no files Locust can show."
        );
    }
    #[test]
    fn binary_empty_rename_mode_and_unicode_paths_stay_listed() {
        let text = format!(
            "{}{}diff --git a/café b/café\nold mode 100644\nnew mode 100755\ndiff --git a/old b/new\nrename from old\nrename to new\n",
            new_file("empty", b""),
            new_file("picture", b"\0image")
        );
        let f = parse(&text);
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].status, "ADDED");
        assert_eq!(f[2].path, "café");
        assert_eq!(f[3].status, "RENAMED");
    }
    #[test]
    fn a_change_is_coloured_as_two_whole_sides() {
        let f=parse("--- a/cart.ts\n+++ b/cart.ts\n@@ -1,4 +1,4 @@\n /* cart,\n    priced */\n-export const total = 2\n+export const total = 1\n const label = \"done\"\n").remove(0);
        let h = &f.hunks[0];
        let (b, a) = h.sides();
        assert!(b.contains("total = 2"));
        assert!(a.contains("total = 1"));
        for code in [a, b] {
            let highlights = rune_kit::syntax::highlight(Some("ts"), &code);
            assert!(!highlights.is_empty());
            for (range, _) in highlights.iter() {
                assert!(code.get(range.clone()).is_some())
            }
        }
    }
    fn init(p: &Path, branch: &str) {
        git(p, &["init", "-q", "-b", branch]).unwrap();
        git(
            p,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "base",
            ],
        )
        .unwrap();
    }
    #[test]
    fn a_folder_without_git_offers_no_changes() {
        let t = Temp::new();
        assert!(!repository(&t.0));
        init(&t.0, "main");
        assert!(repository(&t.0));
        fs::create_dir(t.0.join("src")).unwrap();
        assert!(!repository(&t.0.join("src")));
    }
    #[test]
    fn reads_master_merge_base_commits_untracked_unicode_and_excludes_locust() {
        let t = Temp::new();
        init(&t.0, "master");
        git(&t.0, &["checkout", "-qb", "feature"]).unwrap();
        fs::write(t.0.join("café.txt"), "one\n").unwrap();
        git(&t.0, &["add", "."]).unwrap();
        git(
            &t.0,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                "first",
            ],
        )
        .unwrap();
        fs::write(t.0.join("new.rs"), "let x = 1;\n").unwrap();
        fs::create_dir(t.0.join(".locust")).unwrap();
        fs::write(t.0.join(".locust/private"), "secret").unwrap();
        let all = read(&t.0, None).unwrap();
        assert_eq!(all.base, "master");
        assert_eq!(all.commits.len(), 1);
        assert_eq!(all.files.len(), 2);
        assert!(all.files.iter().any(|f| f.path == "café.txt"));
        let one = read(&t.0, Some(&all.commits[0].0)).unwrap();
        assert_eq!(one.files.len(), 1);
        assert_eq!(read(&t.0, Some("deadbeef")).unwrap().files.len(), 2);
    }
    #[test]
    fn large_new_files_listed_and_new_file_limit_disclosed() {
        let t = Temp::new();
        init(&t.0, "main");
        fs::write(t.0.join("large"), vec![1; 512 * 1024 + 1]).unwrap();
        for i in 0..201 {
            fs::write(t.0.join(format!("z{i:03}")), "x").unwrap()
        }
        let d = read(&t.0, None).unwrap();
        assert!(d.truncated);
        assert_eq!(d.files.len(), 200);
        assert!(d.files[0].hunks.is_empty());
    }
    #[test]
    fn untracked_symlink_to_file_is_listed() {
        let t = Temp::new();
        init(&t.0, "main");
        fs::write(t.0.join("real.rs"), "let x = 1;\n").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("real.rs", t.0.join("alias.rs")).unwrap();
            let d = read(&t.0, None).unwrap();
            assert!(
                d.files.iter().any(|f| f.path == "alias.rs"),
                "symlink-to-file untracked entries stay listed like Electron"
            );
        }
    }
    #[test]
    fn git_keeps_stdout_when_diff_exits_nonzero() {
        let t = Temp::new();
        init(&t.0, "main");
        fs::write(t.0.join("a.txt"), "one\n").unwrap();
        git(&t.0, &["add", "a.txt"]).unwrap();
        git(
            &t.0,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                "add a",
            ],
        )
        .unwrap();
        fs::write(t.0.join("a.txt"), "two\n").unwrap();
        // `git diff --exit-code` exits 1 when there are changes but still prints the patch.
        let out = git(
            &t.0,
            &[
                "-c",
                "core.quotePath=false",
                "diff",
                "--exit-code",
                "--no-color",
                "--no-ext-diff",
                "HEAD",
                "--",
                ".",
            ],
        )
        .unwrap();
        assert!(out.contains("a.txt"), "stdout must survive a non-zero git exit: {out:?}");
    }
}
