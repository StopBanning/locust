//! Electron `main/workspace-brief.test.ts`: a folder's own instructions (`LOCUST.md`), bounded, and its sections for
//! one runtime (A4.2). Native reads the file for the Setup card (`read_workspace_brief`: its lines and whether it was
//! cut) and filters it per runtime (`brief_for_runtime`); it has no `briefSection` and gives the text to no run, so
//! those halves are not here.

use crate::teammate_setup::{brief_for_runtime, read_workspace_brief, LocustMdSetup, MAX_BRIEF_LINES, WORKSPACE_BRIEF};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static N: AtomicU64 = AtomicU64::new(0);

/// A folder holding `LOCUST.md` with `text`, removed when dropped.
struct Folder(PathBuf);
impl Folder {
    fn with(text: Option<&str>) -> Self {
        let dir = std::env::temp_dir().join(format!(".workspace-brief-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(text) = text {
            std::fs::write(dir.join(WORKSPACE_BRIEF), text).unwrap();
        }
        Self(dir)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn locust_md_is_read_from_the_folder_root_whole_its_line_endings_made_one() {
    let folder = Folder::with(Some("# shop\nRun tests with pnpm test.\r\nNever touch prices.csv.\n"));
    assert_eq!(read_workspace_brief(&folder.0, None), Some(LocustMdSetup { lines: 3, truncated: false }));
}

#[test]
fn none_or_empty_is_no_brief_at_all() {
    assert_eq!(read_workspace_brief(&Folder::with(None).0, None), None);
    assert_eq!(read_workspace_brief(&Folder::with(Some("   \n")).0, None), None);
}

#[test]
fn the_brief_is_bounded_by_lines_and_by_bytes_and_says_when_the_rest_was_cut() {
    let long: Vec<String> = (0..MAX_BRIEF_LINES + 5).map(|i| format!("line {i}")).collect();
    let by_lines = read_workspace_brief(&Folder::with(Some(&long.join("\n"))).0, None).unwrap();
    assert_eq!(by_lines, LocustMdSetup { lines: MAX_BRIEF_LINES, truncated: true });
    // Fifty lines of a thousand bytes: 24 lines (24,023 bytes) fit in 24 KiB, 25 do not.
    let fat = vec!["x".repeat(1000); 50].join("\n");
    let by_bytes = read_workspace_brief(&Folder::with(Some(&fat)).0, None).unwrap();
    assert!(by_bytes.truncated);
    assert!(by_bytes.lines < 50);
    assert_eq!(by_bytes.lines, 24);
}

const FILE: &str = "# Project\nRun the tests with npm test.\n<claude>\nUse the Task tool for long searches.\n</claude>\n<codex>\nPrefer apply_patch for edits.\n</codex>\nNever push to main.";

#[test]
fn each_runtime_gets_its_own_section_and_everyone_the_rest() {
    assert_eq!(brief_for_runtime(FILE, "claude"), "# Project\nRun the tests with npm test.\nUse the Task tool for long searches.\nNever push to main.");
    assert_eq!(brief_for_runtime(FILE, "codex"), "# Project\nRun the tests with npm test.\nPrefer apply_patch for edits.\nNever push to main.");
    assert_eq!(brief_for_runtime(FILE, "opencode"), "# Project\nRun the tests with npm test.\nNever push to main.");
}

#[test]
fn a_tag_that_names_no_runtime_a_tag_in_a_code_fence_and_a_tag_never_closed_are_left_alone() {
    let other = "<details>\nmore\n</details>";
    assert_eq!(brief_for_runtime(other, "claude"), other);
    let fenced = "Example:\n```\n<codex>\nsample\n</codex>\n```\nEnd.";
    assert_eq!(brief_for_runtime(fenced, "claude"), fenced);
    // An unclosed tag hides nothing: the rest of the file still arrives.
    let unclosed = "Top.\n<claude>\nFor Claude.\nEveryone again.";
    assert_eq!(brief_for_runtime(unclosed, "codex"), unclosed);
}

#[test]
fn a_runs_brief_is_bounded_after_the_other_runtimes_sections_are_gone() {
    let folder = Folder::with(Some(FILE));
    // Codex's: its four lines. With no runtime named, the file as written (the Setup count reads it so).
    assert_eq!(read_workspace_brief(&folder.0, Some("codex")).map(|b| b.lines), Some(4));
    assert_eq!(read_workspace_brief(&folder.0, None).map(|b| b.lines), Some(9));
    // A file that is only another runtime's section is no brief for this one.
    let only = Folder::with(Some("<claude>\nOnly Claude.\n</claude>"));
    assert_eq!(read_workspace_brief(&only.0, Some("codex")), None);
}
