//! A COMMIT MESSAGE WRITTEN FROM THE CHANGES: a port of Electron's shared/commit-draft.ts and folder-commit.ts
//! `pending`. "Write it from the changes" asks the conversation's teammate, in Ask mode, to write the message from the
//! diff: a subject line and a short body. It fills the box; nothing is committed until the person presses Commit.
use std::path::Path;

/// How much of the diff the model reads: a big change is described by its files and its first 40,000 characters.
pub const MAX_DRAFT_DIFF_CHARS: usize = 40_000;
pub const MAX_SUBJECT_CHARS: usize = 72;
/// New files shown whole, at most this many and this big each (shared/folder-diff.ts).
const MAX_NEW_FILES: usize = 200;
const MAX_NEW_FILE_BYTES: u64 = 512 * 1024;

/// What a commit would take, as the model reads it (folder-commit.ts `pending`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// The uncommitted changes against HEAD, new files whole, cut at the limit.
    pub diff: String,
    /// Every file changed, with its lines added and removed: whole even when the diff is cut.
    pub stat: String,
    pub cut: bool,
    /// The subjects of the folder's last commits, newest first.
    pub recent: Vec<String>,
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let cut: String = text.chars().take(max - 1).collect();
        format!("{}…", cut.trim_end())
    }
}

/// The prompt the teammate gets (commitDraftPrompt), word for word.
pub fn commit_draft_prompt(pending: &Pending, asks: &[String]) -> String {
    let asked: Vec<String> = asks
        .iter()
        .map(|a| a.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|a| !a.is_empty())
        .collect();
    let asked = &asked[asked.len().saturating_sub(5)..];
    let mut parts = vec![
        "Write the commit message for the changes below, as the person committing them would. Use only what is here: do not read files or run commands.".to_owned(),
        [
            format!("- The first line says what changed, in at most {MAX_SUBJECT_CHARS} characters, in plain words."),
            "- The body says what changed and why, in a few short lines or \"- \" points, leaving out what the diff already makes plain. A small change may have none.".to_owned(),
            "- Write it as this project writes its messages (its recent ones are below): no \"feat:\"-style prefix unless they use one.".to_owned(),
        ]
        .join("\n"),
    ];
    if !asked.is_empty() {
        parts.push(format!(
            "What the person asked for in the conversation that made these changes:\n{}",
            asked.iter().map(|a| format!("- {}", clip(a, 300))).collect::<Vec<_>>().join("\n")
        ));
    }
    if !pending.recent.is_empty() {
        parts.push(format!(
            "This project's recent commit messages:\n{}",
            pending.recent.iter().take(8).map(|s| format!("- {}", clip(s, 120))).collect::<Vec<_>>().join("\n")
        ));
    }
    parts.push(format!("The files changed:\n{}", pending.stat));
    parts.push(format!(
        "The changes{}:\n{}",
        if pending.cut { " (cut short; the list of files above is whole)" } else { "" },
        pending.diff
    ));
    parts.push("Answer in exactly this shape, and nothing else:\nSUBJECT: <the first line>\nBODY:\n<the body, or nothing>\nEND".to_owned());
    parts.join("\n\n")
}

/// A label line (`SUBJECT:`, `**BODY:**`...): what follows it on the line, or none when the line is not that label.
fn after_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let trimmed = line.trim_start().trim_start_matches(['*', '_']).trim_start();
    let rest = trimmed.get(..label.len()).filter(|head| head.eq_ignore_ascii_case(label)).map(|_| &trimmed[label.len()..])?;
    let rest = rest.trim_start_matches(['*', '_']).trim_start().strip_prefix(':')?;
    Some(rest.trim_start_matches(['*', '_']).trim_start())
}

fn is_end(line: &str) -> bool {
    line.trim().trim_matches(['*', '_']).trim().eq_ignore_ascii_case("END")
}

/// The message in a draft's answer, or none when it is not in the shape asked for (draftedCommitMessage). What a model
/// says around it, a code fence, or quotes around the subject are left out; a long subject is cut.
pub fn drafted_commit_message(answer: &str) -> Option<String> {
    let normal = answer.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normal.split('\n').collect();
    let at = lines.iter().position(|l| after_label(l, "SUBJECT").is_some())?;
    let subject = after_label(lines[at], "SUBJECT")?
        .trim_end_matches(|c: char| c == '*' || c == '_' || c.is_whitespace())
        .trim()
        .trim_matches(['"', '\'', '`'])
        .trim();
    if subject.is_empty() {
        return None;
    }
    let mut body = String::new();
    if let Some(start) = lines.iter().enumerate().skip(at + 1).find(|(_, l)| after_label(l, "BODY").is_some()).map(|(i, _)| i) {
        let end = lines.iter().enumerate().skip(start + 1).find(|(_, l)| is_end(l)).map(|(i, _)| i).unwrap_or(lines.len());
        let mut kept: Vec<&str> = vec![after_label(lines[start], "BODY").unwrap_or("")];
        kept.extend(&lines[start + 1..end]);
        let joined = kept.into_iter().filter(|l| !l.trim_start().starts_with("```")).collect::<Vec<_>>().join("\n");
        let mut squeezed = joined.clone();
        while squeezed.contains("\n\n\n") {
            squeezed = squeezed.replace("\n\n\n", "\n\n");
        }
        body = squeezed.trim().to_owned();
    }
    let head = clip(&subject.split_whitespace().collect::<Vec<_>>().join(" "), MAX_SUBJECT_CHARS);
    Some(if body.is_empty() { head } else { format!("{head}\n\n{body}") })
}

/// What a commit would take, as text the model can read (folder-commit.ts `pending`): the uncommitted changes against
/// HEAD, new files whole, `.locust/` never; every file changed, even when the diff is cut; the last commits' subjects.
pub fn pending(folder: &Path) -> Option<Pending> {
    use crate::folder_commit::{FolderChanges, FolderCommits};
    if !matches!(FolderCommits::default().changes(folder), FolderChanges::Changes { .. }) {
        return None;
    }
    let git = |args: &[&str]| crate::changes::git(folder, args).unwrap_or_default();
    const NOT_LOCUST: [&str; 3] = ["--", ".", ":(exclude).locust"];
    let plain = ["-c", "core.quotePath=false"];
    let born = crate::changes::git(folder, &["rev-parse", "--verify", "-q", "HEAD"]).is_ok();
    let mut diff = String::new();
    let mut counted = String::new();
    if born {
        let mut args = plain.to_vec();
        args.extend(["diff", "--no-color", "--no-ext-diff", "-M", "HEAD"]);
        args.extend(NOT_LOCUST);
        diff = git(&args);
        let mut stat = plain.to_vec();
        stat.extend(["diff", "--stat=100", "--no-color", "-M", "HEAD"]);
        stat.extend(NOT_LOCUST);
        counted = git(&stat).trim_end().to_owned();
    }
    let mut ls = vec!["ls-files", "--others", "--exclude-standard", "-z"];
    ls.extend(NOT_LOCUST);
    let others: Vec<String> = git(&ls).split('\0').filter(|p| !p.is_empty()).map(str::to_owned).collect();
    for path in others.iter().take(MAX_NEW_FILES) {
        if diff.len() >= MAX_DRAFT_DIFF_CHARS {
            break;
        }
        let full = folder.join(path);
        if std::fs::metadata(&full).map(|m| m.len()).unwrap_or(u64::MAX) > MAX_NEW_FILE_BYTES {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&full) {
            diff.push_str(&crate::changes::new_file(path, &bytes));
        }
    }
    let cut = diff.len() > MAX_DRAFT_DIFF_CHARS || others.len() > MAX_NEW_FILES;
    if diff.len() > MAX_DRAFT_DIFF_CHARS {
        let mut end = MAX_DRAFT_DIFF_CHARS;
        while !diff.is_char_boundary(end) {
            end -= 1;
        }
        diff.truncate(end);
    }
    let recent: Vec<String> = git(&["log", "--max-count=8", "--format=%s"])
        .lines()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let fresh = others.iter().map(|p| format!(" {p} (new)"));
    let stat = std::iter::once(counted).chain(fresh).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    Some(Pending { diff, stat, cut, recent })
}

/// "Write it from the changes" (index.ts COMMIT_DRAFT_CHANNEL): the teammate writes the message from the diff in a
/// read-only run; with its trailer added, as the drafted message says who made the changes. Electron's words when not.
pub fn write(folder: &Path, asks: &[String], teammate: &crate::engine::Teammate) -> Result<String, String> {
    let kept = "The message you had is still in the box.";
    let no = |why: &str| format!("{why} {kept}");
    let pending = pending(folder).ok_or_else(|| no("There is nothing here to commit now."))?;
    let answer = crate::native::run_once(teammate, folder, &commit_draft_prompt(&pending, asks)).map_err(|why| no(&why))?;
    let message = drafted_commit_message(&answer).ok_or_else(|| no("It did not answer with a message."))?;
    Ok(format!("{message}\n\nLocust-Teammate: {}", teammate.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending_example() -> Pending {
        Pending {
            diff: "diff --git a/cart.py b/cart.py\n-    return a - b\n+    return a + b\n".into(),
            stat: " cart.py | 2 +-\n 1 file changed, 1 insertion(+), 1 deletion(-)".into(),
            cut: false,
            recent: vec!["Totals round to the cent".into(), "Invoices list their lines in order".into()],
        }
    }

    #[test]
    fn the_teammate_gets_the_diff_the_files_the_asks_and_the_recent_messages_with_the_shape_to_answer_in() {
        let p = pending_example();
        let prompt = commit_draft_prompt(&p, &["Fix the total".into(), "   ".into(), "Then add a test".into()]);
        assert!(prompt.contains("do not read files or run commands"));
        assert!(prompt.contains("- Fix the total\n- Then add a test"));
        assert!(prompt.contains("This project's recent commit messages:\n- Totals round to the cent\n- Invoices list their lines in order"));
        assert!(prompt.contains(&format!("The files changed:\n{}", p.stat)));
        assert!(prompt.contains(&format!("The changes:\n{}", p.diff)));
        assert!(prompt.ends_with("SUBJECT: <the first line>\nBODY:\n<the body, or nothing>\nEND"));
        // A diff cut short says so, and a folder with no history yet has no "recent" section.
        let cut = commit_draft_prompt(&Pending { cut: true, recent: vec![], ..p }, &[]);
        assert!(cut.contains("(cut short; the list of files above is whole)"));
        assert!(!cut.contains("recent commit messages") && !cut.contains("What the person asked for"));
    }

    #[test]
    fn the_message_is_the_subject_and_the_body_and_nothing_said_around_them() {
        assert_eq!(
            drafted_commit_message("Here is the message:\n\nSUBJECT: Total adds its two amounts\nBODY:\nIt subtracted them, so every invoice\nwas short by twice the second line.\nEND\n\nLet me know!").as_deref(),
            Some("Total adds its two amounts\n\nIt subtracted them, so every invoice\nwas short by twice the second line.")
        );
        assert_eq!(drafted_commit_message("SUBJECT: Total adds its two amounts\nBODY:\nEND").as_deref(), Some("Total adds its two amounts"));
        assert_eq!(
            drafted_commit_message("**SUBJECT:** \"Total adds its two amounts\"\n**BODY:**\n```\n- Fixes the sign\n```\n**END**").as_deref(),
            Some("Total adds its two amounts\n\n- Fixes the sign")
        );
        // No END: the rest is the body.
        assert_eq!(drafted_commit_message("SUBJECT: Fix\nBODY: one line\nand another").as_deref(), Some("Fix\n\none line\nand another"));
    }

    #[test]
    fn a_long_subject_is_cut_and_an_answer_not_in_the_shape_asked_for_is_none() {
        let long = drafted_commit_message(&format!("SUBJECT: {}\nBODY:\nEND", "word ".repeat(30))).expect("a subject");
        assert_eq!(long.chars().count(), MAX_SUBJECT_CHARS);
        assert!(long.ends_with('…'));
        assert_eq!(drafted_commit_message("Total adds its two amounts"), None);
        assert_eq!(drafted_commit_message("SUBJECT:\nBODY:\nsomething\nEND"), None);
    }

    #[test]
    fn pending_reads_the_changes_against_head_new_files_whole_and_never_dot_locust() {
        let dir = std::env::temp_dir().join(format!("locust-draft-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&dir).expect("dir");
        let dir = dir.canonicalize().expect("canonical");
        let sh = |args: &[&str]| {
            let out = std::process::Command::new("git").args(args).current_dir(&dir).output().expect("git");
            assert!(out.status.success(), "{args:?}");
        };
        sh(&["init", "-q", "-b", "main"]);
        sh(&["config", "user.email", "you@example.test"]);
        sh(&["config", "user.name", "You"]);
        sh(&["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("cart.py"), "a - b\n").expect("write");
        sh(&["add", "."]);
        sh(&["commit", "-q", "-m", "Totals round to the cent"]);
        std::fs::write(dir.join("cart.py"), "a + b\n").expect("write");
        std::fs::write(dir.join("notes.md"), "new\n").expect("write");
        std::fs::create_dir_all(dir.join(".locust")).expect("dir");
        std::fs::write(dir.join(".locust/secret.txt"), "never").expect("write");
        let p = pending(&dir).expect("pending");
        assert!(p.diff.contains("+a + b") && p.diff.contains("notes.md") && !p.diff.contains("never"));
        assert!(p.stat.contains("cart.py") && p.stat.contains(" notes.md (new)"));
        assert_eq!(p.recent, vec!["Totals round to the cent".to_owned()]);
        assert!(!p.cut);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Real OpenCode (free model), real git, throwaway folders: the teammate writes a message from a real diff.
    /// `LOCUST_REAL_OPENCODE=1 cargo test --release real_opencode_writes_a_commit_message -- --ignored`
    #[test]
    #[ignore = "real OpenCode turn; requires LOCUST_REAL_OPENCODE=1"]
    fn real_opencode_writes_a_commit_message() {
        assert_eq!(std::env::var("LOCUST_REAL_OPENCODE").ok().as_deref(), Some("1"), "set LOCUST_REAL_OPENCODE=1 to run this real test");
        let root = std::env::temp_dir().join(format!("locust-draft-real-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        let work = root.join("work");
        std::fs::create_dir_all(&work).expect("dir");
        let work = work.canonicalize().expect("canonical");
        let store = crate::store::Store::new(root.join("native"), root.join("electron")).expect("store");
        std::fs::create_dir_all(store.electron()).expect("electron dir");
        let _hook = crate::native::hooks::install(store, None);
        let sh = |args: &[&str]| {
            let out = std::process::Command::new("git").args(args).current_dir(&work).output().expect("git");
            assert!(out.status.success(), "{args:?}");
        };
        sh(&["init", "-q", "-b", "main"]);
        sh(&["config", "user.email", "you@example.test"]);
        sh(&["config", "user.name", "You"]);
        sh(&["config", "commit.gpgsign", "false"]);
        std::fs::write(work.join("cart.py"), "def total(a, b):\n    return a - b\n").expect("write");
        sh(&["add", "."]);
        sh(&["commit", "-q", "-m", "Add the cart total"]);
        std::fs::write(work.join("cart.py"), "def total(a, b):\n    return a + b\n").expect("write");
        let teammate = crate::engine::Teammate {
            id: "tm_real".into(),
            name: "Scout".into(),
            role: String::new(),
            runtime: Some("opencode".into()),
            model: None,
            busy: false,
            runs_in: "ask".into(),
            folder: Some(work.to_string_lossy().into()),
        };
        let message = write(&work, &["Fix the cart total".into()], &teammate).expect("a drafted message");
        eprintln!("DRAFTED<<{message}>>");
        let subject = message.lines().next().unwrap_or("");
        assert!(!subject.is_empty() && subject.chars().count() <= MAX_SUBJECT_CHARS, "{message}");
        assert!(message.ends_with("Locust-Teammate: Scout"), "{message}");
        // Nothing was committed: the box is filled, the commit is the person's.
        let count = std::process::Command::new("git").args(["rev-list", "--count", "HEAD"]).current_dir(&work).output().expect("git");
        assert_eq!(String::from_utf8_lossy(&count.stdout).trim(), "1");
        let _ = std::fs::remove_dir_all(&root);
    }
}

