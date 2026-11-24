//! Electron's `main/memory-file.test.ts` (5 cases): team memory as a file in the folder, on `memory_file_text` and
//! `write_memory_file`, which a turn's brief calls before it names the file; on a real folder rather than Electron's
//! fake io.
use super::*;
use serde_json::json;

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-09-17T12:00:00.000Z").unwrap().with_timezone(&chrono::Utc)
}
fn line(text: &str, at: &str, scope: &str, by: &str) -> Value {
    json!({"text": text, "scope": scope, "by": by, "createdAt": at})
}
fn folder() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("locust-memory-file-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

// case: lists this folder and everywhere, newest last, with who and when, and says it is read-only
#[test]
fn the_memory_file_lists_this_folder_and_everywhere_with_who_and_when() {
    let text = memory_file_text(
        &[
            line("Deploys go out Tuesday.", "2026-08-01T00:00:00.000Z", "workspace", "Wembley"),
            line("Colin reads on his phone.", "2026-09-16T00:00:00.000Z", "global", "you"),
        ],
        now(),
    );
    assert!(text.contains("## This folder (1)"), "{text}");
    assert!(text.contains("- Deploys go out Tuesday. (by Wembley -- 2026-08-01 (6 weeks ago))"), "{text}");
    assert!(text.contains("## Everywhere (1)"), "{text}");
    assert!(text.contains("- Colin reads on his phone. (by the person -- 2026-09-16 (yesterday))"), "{text}");
    assert!(text.contains("Read-only: to change memory, use the <locust-memory> block"), "{text}");
}

// case: writes under .locust in the folder and returns the path the brief names
#[test]
fn the_memory_file_is_written_under_dot_locust_and_named_by_its_path() {
    let root = folder();
    let text = memory_file_text(&[line("A.", "2026-09-01T00:00:00.000Z", "workspace", "Wembley")], now());
    assert_eq!(write_memory_file(&root, &text), Some(".locust/memory.md"));
    assert_eq!(std::fs::read_to_string(root.join(".locust").join("memory.md")).unwrap(), text);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

// case: does not rewrite a file that already says the same
#[test]
fn the_memory_file_is_not_rewritten_when_it_already_says_the_same() {
    let root = folder();
    let text = memory_file_text(&[line("A.", "2026-09-01T00:00:00.000Z", "workspace", "Wembley")], now());
    std::fs::create_dir_all(root.join(".locust")).unwrap();
    let path = root.join(".locust").join("memory.md");
    std::fs::write(&path, &text).unwrap();
    let long_ago = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(long_ago).unwrap();
    assert_eq!(write_memory_file(&root, &text), Some(".locust/memory.md"));
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), long_ago, "the file was written again");
    let _ = std::fs::remove_dir_all(&root);
}

// case: keeps .locust out of git where there is a repository, and only there
#[test]
fn dot_locust_is_kept_out_of_git_only_where_there_is_a_repository() {
    let empty = memory_file_text(&[], now());
    let with_git = folder();
    std::fs::create_dir_all(with_git.join(".git")).unwrap();
    std::fs::write(with_git.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
    write_memory_file(&with_git, &empty);
    assert_eq!(std::fs::read_to_string(with_git.join(".git/info/exclude")).unwrap(), ".locust/\n");
    let already = folder();
    std::fs::create_dir_all(already.join(".git/info")).unwrap();
    std::fs::write(already.join(".git/HEAD"), "ref\n").unwrap();
    std::fs::write(already.join(".git/info/exclude"), ".locust/\n").unwrap();
    write_memory_file(&already, &empty);
    assert_eq!(std::fs::read_to_string(already.join(".git/info/exclude")).unwrap(), ".locust/\n");
    let no_git = folder();
    write_memory_file(&no_git, &empty);
    assert!(!no_git.join(".git").exists());
    for root in [with_git, already, no_git] {
        let _ = std::fs::remove_dir_all(&root);
    }
}

// case: returns nothing when the folder cannot be written, so the brief pastes as before
#[test]
fn the_memory_file_is_nothing_when_the_folder_cannot_be_written() {
    let root = folder();
    // `.locust` is a file here, so nothing can be written under it (a permission would not stop a test run as root).
    std::fs::write(root.join(".locust"), "not a folder").unwrap();
    let text = memory_file_text(&[line("A.", "2026-09-01T00:00:00.000Z", "workspace", "Wembley")], now());
    assert_eq!(write_memory_file(&root, &text), None);
    let _ = std::fs::remove_dir_all(&root);
}
