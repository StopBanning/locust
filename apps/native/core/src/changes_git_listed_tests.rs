//! Electron's `renderer/src/a-file-git-shows-without-lines-is-listed.test.ts` (0.753, bug hunt 2026-10-12, CH1). git
//! writes an image added or changed, a file renamed and left as it was, a file made executable and a new empty file as a
//! `diff --git` header with no `---`/`+++`; each is still a file in the list. `changes::parse` draws the Changes panel,
//! Review changes and approval cards; `changes::read` (called from `src/ui/changes_panel.rs`) reads a real folder
//! through it, so each case is checked on Electron's text and on a folder git really changed.
use crate::changes::{self, git, parse};
use std::{fs, path::PathBuf};

const BRANCH: &str = "diff --git a/notes.txt b/notes.txt
index 3b18e51..a5c1966 100644
--- a/notes.txt
+++ b/notes.txt
@@ -1 +1 @@
-hello
+hello there
diff --git a/logo.png b/logo.png
index 1a2b3c4..5d6e7f8 100644
Binary files a/logo.png and b/logo.png differ
diff --git a/data.bin b/data.bin
new file mode 100644
index 0000000..9f86d08
Binary files /dev/null and b/data.bin differ
diff --git a/old.md b/new.md
similarity index 100%
rename from old.md
rename to new.md
diff --git a/run.sh b/run.sh
old mode 100644
new mode 100755
diff --git a/.keep b/.keep
new file mode 100644
index 0000000..e69de29
diff --git a/my notes/plan b.md b/my notes/plan b.md
new file mode 100644
diff --git a/gone.png b/gone.png
deleted file mode 100644
index 5d6e7f8..0000000
Binary files a/gone.png and /dev/null differ
diff --git a/last.txt b/last.txt
index 3b18e51..a5c1966 100644
--- a/last.txt
+++ b/last.txt
@@ -1 +1 @@
-a
+b
";

fn listed(files: &[changes::File]) -> Vec<(String, String, usize)> {
    files.iter().map(|f| (f.path.clone(), f.status.clone(), f.hunks.len())).collect()
}

fn row(path: &str, status: &str, hunks: usize) -> (String, String, usize) {
    (path.into(), status.into(), hunks)
}

/// A fresh git repository on `main` with one commit of `files`; removed when dropped.
struct Repo(PathBuf);
impl Repo {
    fn new(name: &str, files: &[(&str, &[u8])]) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = Repo(dir);
        repo.git(&["init", "-q", "-b", "main"]);
        for (path, bytes) in files {
            repo.write(path, bytes);
        }
        repo.git(&["add", "-A"]);
        repo.git(&["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-qm", "base"]);
        repo
    }
    fn git(&self, args: &[&str]) {
        git(&self.0, args).unwrap();
    }
    fn write(&self, path: &str, bytes: &[u8]) {
        let full = self.0.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, bytes).unwrap();
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// case: lists every one, with or without text lines, as git said it changed
#[test]
fn lists_every_one_with_or_without_text_lines_as_git_said_it_changed() {
    assert_eq!(
        listed(&parse(BRANCH)),
        [
            row("notes.txt", "MODIFIED", 1),
            row("logo.png", "MODIFIED", 0),
            row("data.bin", "ADDED", 0),
            row("new.md", "RENAMED", 0),
            row("run.sh", "MODIFIED", 0),
            row(".keep", "ADDED", 0),
            row("my notes/plan b.md", "ADDED", 0),
            row("gone.png", "DELETED", 0),
            row("last.txt", "MODIFIED", 1),
        ]
    );

    // The same kinds of change, made in a folder and read the way the Changes panel reads it.
    let repo = Repo::new(
        "git-listed",
        &[
            ("notes.txt", b"hello\n"),
            ("logo.png", b"\x89PNG\0one"),
            ("old.md", b"# Plan\nkept as it was\n"),
            ("run.sh", b"echo hi\n"),
            ("gone.png", b"\x89PNG\0gone"),
        ],
    );
    repo.write("notes.txt", b"hello there\n");
    repo.write("logo.png", b"\x89PNG\0two");
    repo.git(&["mv", "old.md", "new.md"]);
    // Made executable on disk, as an editor or `chmod +x` does (the panel reads the folder against HEAD, not the index);
    // Windows keeps no such bit on disk, so that change is only made and looked for on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(repo.0.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::remove_file(repo.0.join("gone.png")).unwrap();
    repo.write("data.bin", b"\0\x01\x02");
    repo.write(".keep", b"");
    repo.write("my notes/plan b.md", b"");
    let folder = changes::read(&repo.0, None).unwrap();
    let mut seen = listed(&folder.files);
    seen.sort();
    let mut want = vec![
        row("notes.txt", "MODIFIED", 1),
        row("logo.png", "MODIFIED", 0),
        row("new.md", "RENAMED", 0),
        row("gone.png", "DELETED", 0),
        row("data.bin", "ADDED", 0),
        row(".keep", "ADDED", 0),
        row("my notes/plan b.md", "ADDED", 0),
    ];
    if cfg!(unix) {
        want.push(row("run.sh", "MODIFIED", 0));
    }
    want.sort();
    assert_eq!(seen, want, "{}", folder.raw);
}

// case: still reads a file with lines after one without, and a diff of one header alone
#[test]
fn still_reads_a_file_with_lines_after_one_without_and_a_diff_of_one_header_alone() {
    assert_eq!(
        parse("diff --git a/big.log b/big.log\nnew file mode 100644\n"),
        [changes::File { path: "big.log".into(), status: "ADDED".into(), hunks: vec![] }]
    );
    // Diffs with no `diff --git` lines at all, as some runtimes send them, are read as before.
    let plain = parse("--- a/x.txt\n+++ b/x.txt\n@@ -1 +1 @@\n-a\n+b\n");
    assert_eq!(listed(&plain), [row("x.txt", "MODIFIED", 1)]);

    // A new empty file, then a new file with lines, in a real folder: the second keeps its lines.
    let repo = Repo::new("git-after-empty", &[]);
    repo.write(".keep", b"");
    repo.write("z.txt", b"one\ntwo\n");
    let folder = changes::read(&repo.0, None).unwrap();
    assert_eq!(listed(&folder.files), [row(".keep", "ADDED", 0), row("z.txt", "ADDED", 1)], "{}", folder.raw);
    assert_eq!(folder.files[1].counts(), (2, 0));
}
