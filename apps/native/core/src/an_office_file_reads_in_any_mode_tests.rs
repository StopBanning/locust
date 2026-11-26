//! Electron `main/an-office-file-reads-in-any-mode.test.ts`, case by case. Expected words and values are Electron's
//! (`main/office-words.ts`), held to the same python-docx and python-pptx fixtures Electron copies in
//! (`apps/desktop/test/documents/`).

use super::*;
use std::path::PathBuf;

const REPORT: &[u8] = include_bytes!("../../../desktop/test/documents/report-python-docx.docx");
const DECK: &[u8] = include_bytes!("../../../desktop/test/documents/deck-python-pptx.pptx");

struct Project(PathBuf);
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn project() -> Project {
    let dir = std::env::temp_dir().join(format!("locust-office-words-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    Project(dir)
}

/// Set a file's modified time `seconds` from now, as Electron's `utimes` does.
fn touch(path: &Path, seconds: u64) {
    let at = std::time::SystemTime::now() + std::time::Duration::from_secs(seconds);
    std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
}

#[test]
fn names_nothing_in_a_folder_with_no_word_or_powerpoint_file() {
    let here = project();
    std::fs::write(here.0.join("notes.txt"), "plain").unwrap();
    assert_eq!(office_words_section(&here.0), None);
}

#[test]
fn keeps_each_documents_words_as_text_and_names_the_copy_in_the_brief() {
    let here = project();
    std::fs::write(here.0.join("report.docx"), REPORT).unwrap();
    std::fs::create_dir_all(here.0.join("decks")).unwrap();
    std::fs::write(here.0.join("decks").join("deck.pptx"), DECK).unwrap();
    let section = office_words_section(&here.0).expect("a section");
    assert!(section.contains(&format!("- report.docx: {OFFICE_WORDS_DIR}/report.docx.md")), "{section}");
    assert!(section.contains(&format!("- decks/deck.pptx: {OFFICE_WORDS_DIR}/decks/deck.pptx.md")), "{section}");
    assert!(section.contains("editing a copy changes nothing in the document"), "{section}");
    let words = std::fs::read_to_string(here.0.join(OFFICE_WORDS_DIR).join("report.docx.md")).unwrap();
    assert!(words.contains("# Quarterly Report"), "{words}");
    assert!(words.contains("| Region | Q2 | Q3 |"), "{words}");
    assert!(words.contains("| North | 120 | 150 |"), "{words}");
    assert!(words.contains("  - One of them in support"), "{words}");
    let slides = std::fs::read_to_string(here.0.join(OFFICE_WORDS_DIR).join("decks").join("deck.pptx.md")).unwrap();
    assert!(slides.contains("## Slide 1"), "{slides}");
}

#[test]
fn refreshes_a_copy_only_when_its_document_changed() {
    let here = project();
    std::fs::write(here.0.join("report.docx"), REPORT).unwrap();
    office_words_section(&here.0);
    let copy = here.0.join(OFFICE_WORDS_DIR).join("report.docx.md");
    std::fs::write(&copy, "stale words").unwrap();
    touch(&copy, 60);
    office_words_section(&here.0);
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "stale words");
    touch(&here.0.join("report.docx"), 120);
    office_words_section(&here.0);
    assert!(std::fs::read_to_string(&copy).unwrap().contains("# Quarterly Report"));
}

#[test]
fn leaves_out_a_damaged_file_words_lock_files_and_anything_past_the_limit() {
    let here = project();
    std::fs::write(here.0.join("broken.docx"), "not a zip").unwrap();
    std::fs::write(here.0.join("~$report.docx"), REPORT).unwrap();
    for at in 0..MAX_FILES + 3 {
        std::fs::write(here.0.join(format!("r{at:02}.docx")), REPORT).unwrap();
    }
    let section = office_words_section(&here.0).unwrap_or_default();
    assert!(!section.contains("broken.docx"), "{section}");
    assert!(!section.contains("~$report.docx"), "{section}");
    assert!(section.lines().filter(|l| l.starts_with("- ")).count() <= MAX_FILES, "{section}");
    assert!(!here.0.join(OFFICE_WORDS_DIR).join("broken.docx.md").exists());
}

#[test]
fn writes_a_table_cells_pipe_so_it_cannot_split_the_row() {
    let words = document_words(
        "a.docx",
        &OfficeDocument { kind: DocumentKind::Word, more: 0, blocks: vec![DocumentBlock::Table { rows: vec![vec!["a|b".into(), "c".into()]] }] },
    );
    assert!(words.contains("| a\\|b | c |"), "{words}");
}

// The brief a turn starts with names the copies (Electron `officeWordsSection`, called where the brief is built).
#[test]
fn a_turns_brief_names_each_documents_copy() {
    let here = project();
    let store = crate::store::Store::new(here.0.join("native"), here.0.join("electron")).unwrap();
    let folder = here.0.join("work");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("report.docx"), REPORT).unwrap();
    let brief = crate::people::brief(&store, "nobody", &folder, "Summarise the report.").unwrap();
    assert!(brief.contains("Word and PowerPoint files here are not plain text"), "{brief}");
    assert!(brief.contains(&format!("- report.docx: {OFFICE_WORDS_DIR}/report.docx.md")), "{brief}");
}

// Lane check: Electron's `office-words.ts` source, not its test: `SKIPPED` names `target` among the folders it never
// looks in, and `keepOutOfGit(folder)` keeps the copies out of a repository's commits.

#[test]
fn never_looks_in_a_build_folder_or_a_dot_folder_and_looks_one_level_down_in_any_other() {
    let here = project();
    for folder in ["target", "node_modules", "dist", ".hidden", "notes"] {
        std::fs::create_dir_all(here.0.join(folder)).unwrap();
        std::fs::write(here.0.join(folder).join("inside.docx"), REPORT).unwrap();
    }
    let section = office_words_section(&here.0).unwrap_or_default();
    assert!(section.contains("- notes/inside.docx: "), "{section}");
    for skipped in ["target/", "node_modules/", "dist/", ".hidden/"] {
        assert!(!section.contains(skipped), "{skipped} is not looked in: {section}");
    }
}

#[test]
fn keeps_its_copies_out_of_a_repositorys_commits() {
    let here = project();
    std::fs::create_dir_all(here.0.join(".git").join("info")).unwrap();
    std::fs::write(here.0.join("report.docx"), REPORT).unwrap();
    assert!(office_words_section(&here.0).is_some());
    let exclude = std::fs::read_to_string(here.0.join(".git").join("info").join("exclude")).unwrap_or_default();
    assert!(exclude.lines().any(|line| line.trim_end_matches('/').trim_start_matches('/') == ".locust"), "{exclude}");
}
