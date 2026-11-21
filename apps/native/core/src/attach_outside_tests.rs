//! Electron's `main/attach-outside.test.ts` (11 cases): finding a home for a file from outside the workspace, keeping
//! that folder out of git, and refusing a source that is not really a file name. Rust ran the exclude rule already;
//! the name rule was `distinct_name`, which refused nothing (fixed 21 Nov, see pasted_attachment_tests.rs).
use super::*;
use std::collections::HashSet;

fn taken(names: &[&str]) -> HashSet<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

fn home(name: &str) -> Result<String, String> {
    attachment_destination(name, &HashSet::new())
}

const DOWNLOADS: &str = "/Users/c/Downloads";

// case: keeps the name it had
#[test]
fn keeps_the_name_it_had() {
    assert_eq!(home(&format!("{DOWNLOADS}/report.pdf")).as_deref(), Ok(".locust/attachments/report.pdf"));
    assert_eq!(home("/home/c/shot.png").as_deref(), Ok(".locust/attachments/shot.png"));
    // A trailing separator is dropped first, as Node's basename drops it.
    assert_eq!(home("/home/c/shot.png/").as_deref(), Ok(".locust/attachments/shot.png"));
}

// case: never overwrites a file already there
#[test]
fn never_overwrites_a_file_already_there() {
    assert_eq!(attachment_destination("/a/notes.md", &taken(&["notes.md"])).as_deref(), Ok(".locust/attachments/notes-2.md"));
    assert_eq!(attachment_destination("/b/notes.md", &taken(&["notes.md", "notes-2.md"])).as_deref(), Ok(".locust/attachments/notes-3.md"));
}

// case: handles a name with no extension, and one with several dots
#[test]
fn handles_a_name_with_no_extension_and_one_with_several_dots() {
    assert_eq!(attachment_destination("/a/README", &taken(&["README"])).as_deref(), Ok(".locust/attachments/README-2"));
    assert_eq!(attachment_destination("/a/archive.tar.gz", &taken(&["archive.tar.gz"])).as_deref(), Ok(".locust/attachments/archive.tar-2.gz"));
    // A leading dot is part of the name, not an extension.
    assert_eq!(attachment_destination("/a/.env", &taken(&[".env"])).as_deref(), Ok(".locust/attachments/.env-2"));
}

// case: adds the line to an exclude file that lacks it
#[test]
fn adds_the_line_to_an_exclude_file_that_lacks_it() {
    assert_eq!(exclude_with("").as_deref(), Some(".locust/\n"));
    assert_eq!(exclude_with("# comment\n*.log\n").as_deref(), Some("# comment\n*.log\n.locust/\n"));
}

// case: adds a newline first when the file does not end with one
#[test]
fn adds_a_newline_first_when_the_file_does_not_end_with_one() {
    assert_eq!(exclude_with("*.log").as_deref(), Some("*.log\n.locust/\n"));
}

// case: does nothing when the folder is already excluded
#[test]
fn does_nothing_when_the_folder_is_already_excluded() {
    assert_eq!(exclude_with(".locust/\n"), None);
    assert_eq!(exclude_with(".locust\n"), None);
    assert_eq!(exclude_with("*.log\n/.locust/\n"), None);
}

// case: refuses a name that would climb out of the folder
#[test]
fn refuses_a_name_that_would_climb_out_of_the_folder() {
    // On a Mac the backslash path is one name holding a backslash; on Windows its last segment is `..`. Both refused.
    assert!(home("C:\\a\\..").is_err_and(|e| e.contains("not a file name")));
    assert!(home("..").is_err_and(|e| e.contains("not a file name")));
    assert!(home("/a/..").is_err_and(|e| e.contains("not a file name")));
}

// case: refuses a name that would land on the folder itself
#[test]
fn refuses_a_name_that_would_land_on_the_folder_itself() {
    assert!(home("C:\\a\\.").is_err_and(|e| e.contains("not a file name")));
    assert!(home("C:\\").is_err_and(|e| e.contains("not a file name")));
    assert!(home("/a/.").is_err_and(|e| e.contains("not a file name")));
    assert!(home("/").is_err_and(|e| e.contains("not a file name")));
}

// case: refuses an alternate data stream
#[test]
fn refuses_an_alternate_data_stream() {
    assert!(home("C:\\a\\file.txt:stream").is_err_and(|e| e.contains("not a file name")));
    assert!(home("/a/file.txt:stream").is_err_and(|e| e.contains("not a file name")));
}

// case: still takes every ordinary name, including awkward ones
#[test]
fn still_takes_every_ordinary_name_including_awkward_ones() {
    for name in ["notes.md", "my report (final).pdf", "a b  c.txt", ".gitignore", "ünïcodé.md", "archive.tar.gz"] {
        assert_eq!(home(&format!("{DOWNLOADS}/{name}")), Ok(format!(".locust/attachments/{name}")));
    }
}

// case: a traversal INSIDE the source path is harmless, because only the name is used
#[test]
fn a_traversal_inside_the_source_path_is_harmless_because_only_the_name_is_used() {
    assert_eq!(home("/a/../../evil.txt").as_deref(), Ok(".locust/attachments/evil.txt"));
}
