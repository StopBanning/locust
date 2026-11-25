//! Electron's `main/a-pdf-arrives-readable.test.ts` (0.713): an attached PDF is opened for the agent, the same way for
//! every runtime -- its text and a picture of each page in Locust's own folder, named in the message between the file
//! list and the person's words. The reader is a fake here, as in Electron, except in the last test, which sends a real
//! PDF through the native send path (`native::send_from` -> `attachments_for_run`) to a fake Codex that keeps what it
//! was sent, and PDFKit reads it.
//!
//! Not carried: "Send waits for the reading the attach button started, rather than starting a second". The native
//! composer starts no reading when a file is attached; the PDF is read once, when the message is sent, and a second
//! send of the same bytes is answered from what the first wrote ("reads the same PDF once" below).
use crate::attachments::{split_attachments, with_attachments, with_attachments_note};
use crate::attachments_for_run::attachments_for_run_with;
use crate::pdf::{PdfReading, ReadablePdf, readable_pdf_note, readable_pdf_with};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const PNG: &[u8] = &[0x89, 0x50, 0x4e, 0x47];
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("locust-pdf-readable-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".locust/attachments")).unwrap();
        Self(root)
    }
    fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn two_pages() -> PdfReading {
    PdfReading { pages: 2, text: vec!["Page one words".into(), "Page two words".into()], pictures: vec![PNG.to_vec(), PNG.to_vec()] }
}

/// The fake reader, as a run's PDF opener: what it is given is read as `reading`.
fn opener(reading: PdfReading) -> impl FnMut(&Path, &str, &str) -> Option<ReadablePdf> {
    move |folder, pdf, name| readable_pdf_with(folder, pdf, name, 30, |_| Ok(reading.clone())).ok()
}

fn paths(of: &[&str]) -> Vec<String> {
    of.iter().map(|p| (*p).to_owned()).collect()
}

// case: names the text and the pictures, and says no tool is needed
#[test]
fn names_the_text_and_the_pictures_and_says_no_tool_is_needed() {
    let note = readable_pdf_note(&[ReadablePdf {
        pdf: ".locust/attachments/Homework 5.pdf".into(),
        text: Some(".locust/attachments/Homework 5.pdf.txt".into()),
        pictures: paths(&[".locust/attachments/Homework 5.pdf-pages/page-01.png", ".locust/attachments/Homework 5.pdf-pages/page-02.png"]),
        pages: 2,
    }])
    .unwrap();
    assert!(note.contains("no PDF tool or package is needed"), "{note}");
    assert!(note.contains("its text is in .locust/attachments/Homework 5.pdf.txt"), "{note}");
    assert!(note.contains("each of its 2 pages is a picture, .locust/attachments/Homework 5.pdf-pages/page-01.png to page-02.png"), "{note}");
    assert!(note.contains("For equations, tables, charts and layout, look at the pictures"), "{note}");
}

// case: says when only the first pages are pictures, and when there is no text at all
#[test]
fn says_when_only_the_first_pages_are_pictures_and_when_there_is_no_text_at_all() {
    let pictures: Vec<String> = (1..=30).map(|i| format!(".locust/attachments/Book.pdf-pages/page-{i:02}.png")).collect();
    let book = readable_pdf_note(&[ReadablePdf { pdf: "Book.pdf".into(), text: Some("Book.pdf.txt".into()), pictures, pages: 120 }]).unwrap();
    assert!(book.contains("its first 30 of 120 pages are pictures"), "{book}");
    let scan = readable_pdf_note(&[ReadablePdf { pdf: "Scan.pdf".into(), text: None, pictures: paths(&["Scan.pdf-pages/page-01.png"]), pages: 1 }]).unwrap();
    assert!(
        scan.contains("it has no text in it (scanned pages), so read the pictures; its page is a picture: Scan.pdf-pages/page-01.png"),
        "{scan}"
    );
    assert!(readable_pdf_note(&[]).is_none());
}

// case: sits between the file list and the person’s words, and only in what is sent
#[test]
fn sits_between_the_file_list_and_the_persons_words_and_only_in_what_is_sent() {
    let files = paths(&[".locust/attachments/Homework 5.pdf"]);
    assert_eq!(
        with_attachments_note("Explain problem 3.", &files, Some("NOTE")),
        "Read this file in the workspace before you answer:\n- .locust/attachments/Homework 5.pdf\n\nNOTE\n\nExplain problem 3."
    );
    // The recorded prompt carries no note, so the bubble reads as it always did.
    assert_eq!(split_attachments(&with_attachments("Explain problem 3.", &files)), ("Explain problem 3.".to_owned(), files));
}

// case: writes the text with each page under its number, and one picture a page
#[test]
fn writes_the_text_with_each_page_under_its_number_and_one_picture_a_page() {
    let f = Folder::new();
    f.write(".locust/attachments/Homework 5.pdf", b"%PDF-1.5 fake");
    let readable = readable_pdf_with(&f.0, ".locust/attachments/Homework 5.pdf", "Homework 5.pdf", 30, |_| Ok(two_pages())).unwrap();
    assert_eq!(
        readable,
        ReadablePdf {
            pdf: ".locust/attachments/Homework 5.pdf".into(),
            text: Some(".locust/attachments/Homework 5.pdf.txt".into()),
            pictures: paths(&[".locust/attachments/Homework 5.pdf-pages/page-01.png", ".locust/attachments/Homework 5.pdf-pages/page-02.png"]),
            pages: 2,
        }
    );
    let text = std::fs::read_to_string(f.0.join(".locust/attachments/Homework 5.pdf.txt")).unwrap();
    // A byte-order mark, so Windows PowerShell reads it as UTF-8.
    assert!(text.starts_with("\u{feff}Homework 5.pdf, 2 pages: the text Locust read out of it."), "{text}");
    assert!(text.contains("--- Page 1 ---\nPage one words\n\n--- Page 2 ---\nPage two words"), "{text}");
    let mut pngs: Vec<String> = std::fs::read_dir(f.0.join(".locust/attachments/Homework 5.pdf-pages"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".png"))
        .collect();
    pngs.sort();
    assert_eq!(pngs, ["page-01.png", "page-02.png"]);
}

// case: reads the same PDF once, and again when it changes
#[test]
fn reads_the_same_pdf_once_and_again_when_it_changes() {
    let f = Folder::new();
    let pdf = ".locust/attachments/notes.pdf";
    f.write(pdf, b"%PDF one");
    let calls = Cell::new(0);
    let read = |_: &[u8]| {
        calls.set(calls.get() + 1);
        Ok(two_pages())
    };
    readable_pdf_with(&f.0, pdf, "notes.pdf", 30, read).unwrap();
    readable_pdf_with(&f.0, pdf, "notes.pdf", 30, read).unwrap();
    assert_eq!(calls.get(), 1);
    f.write(pdf, b"%PDF two");
    readable_pdf_with(&f.0, pdf, "notes.pdf", 30, read).unwrap();
    assert_eq!(calls.get(), 2);
    // Its files gone, it is read again rather than named where nothing is.
    std::fs::remove_file(f.0.join(".locust/attachments/notes.pdf.txt")).unwrap();
    readable_pdf_with(&f.0, pdf, "notes.pdf", 30, read).unwrap();
    assert_eq!(calls.get(), 3);
}

// case: a scan has pictures and no text file
#[test]
fn a_scan_has_pictures_and_no_text_file() {
    let f = Folder::new();
    f.write(".locust/attachments/scan.pdf", b"%PDF scan");
    let scan = PdfReading { pages: 1, text: vec!["  ".into()], pictures: vec![PNG.to_vec()] };
    let readable = readable_pdf_with(&f.0, ".locust/attachments/scan.pdf", "scan.pdf", 30, |_| Ok(scan)).unwrap();
    assert_eq!(readable.text, None);
    assert_eq!(readable.pictures, [".locust/attachments/scan.pdf-pages/page-01.png"]);
    assert!(!f.0.join(".locust/attachments/scan.pdf.txt").exists());
}

// case: a run in the project folder is told where the PDF’s text and pictures are
#[test]
fn a_run_in_the_project_folder_is_told_where_the_pdfs_text_and_pictures_are() {
    let f = Folder::new();
    f.write(".locust/attachments/Homework 5.pdf", b"%PDF hw");
    let prompt = with_attachments("Explain problem 3.", &paths(&[".locust/attachments/Homework 5.pdf"]));
    let sent = attachments_for_run_with(&prompt, &f.0, &f.0, opener(two_pages()));
    assert!(
        sent.starts_with("Read this file in the workspace before you answer:\n- .locust/attachments/Homework 5.pdf\n\nLocust has already opened the PDF for you"),
        "{sent}"
    );
    assert!(sent.contains(".locust/attachments/Homework 5.pdf.txt"), "{sent}");
    assert!(sent.ends_with("\n\nExplain problem 3."), "{sent}");
}

// case: a project PDF a worktree is sent is opened in the worktree’s own folder
#[test]
fn a_project_pdf_a_worktree_is_sent_is_opened_in_the_worktrees_own_folder() {
    let f = Folder::new();
    let tree = f.0.join(".locust/worktrees/tm_wren");
    std::fs::create_dir_all(&tree).unwrap();
    f.write("docs/spec.pdf", b"%PDF spec");
    let sent = attachments_for_run_with(&with_attachments("Summarize.", &paths(&["docs/spec.pdf"])), &f.0, &tree, opener(two_pages()));
    assert!(sent.contains("- .locust/attachments/spec.pdf\n"), "{sent}");
    assert!(sent.contains("its text is in .locust/attachments/spec.pdf.txt"), "{sent}");
    assert!(std::fs::read_to_string(tree.join(".locust/attachments/spec.pdf.txt")).unwrap().contains("Page one words"));
    // Nothing is written beside the project's own file.
    assert!(!f.0.join("docs/spec.pdf.txt").exists());
}

// case: a PDF that cannot be read is sent as it was, and other files are left alone
#[test]
fn a_pdf_that_cannot_be_read_is_sent_as_it_was_and_other_files_are_left_alone() {
    let f = Folder::new();
    f.write(".locust/attachments/broken.pdf", b"%PDF broken");
    f.write("NOTES.md", b"notes");
    let prompt = with_attachments("Read these.", &paths(&["NOTES.md", ".locust/attachments/broken.pdf"]));
    let failing = |folder: &Path, pdf: &str, name: &str| readable_pdf_with(folder, pdf, name, 30, |_| Err("Invalid PDF structure.".into())).ok();
    assert_eq!(attachments_for_run_with(&prompt, &f.0, &f.0, failing), prompt);
    let notes = with_attachments("Read it.", &paths(&["NOTES.md"]));
    assert_eq!(attachments_for_run_with(&notes, &f.0, &f.0, opener(two_pages())), notes);
}

/// Two PDFs of one name are opened under two names, so neither's text overwrites the other's.
#[test]
fn two_pdfs_of_one_name_are_opened_under_two_names() {
    let f = Folder::new();
    f.write("a/report.pdf", b"%PDF a");
    f.write("b/report.pdf", b"%PDF b");
    let sent = attachments_for_run_with(&with_attachments("Compare.", &paths(&["a/report.pdf", "b/report.pdf"])), &f.0, &f.0, opener(two_pages()));
    assert!(sent.contains("Locust has already opened the 2 PDFs for you"), "{sent}");
    assert!(sent.contains(".locust/attachments/report.pdf.txt") && sent.contains(".locust/attachments/report-2.pdf.txt"), "{sent}");
}

/// A one-page PDF saying `words` (the reader tests' own, in pdf.rs).
#[cfg(target_os = "macos")]
fn one_page_pdf(words: &str) -> Vec<u8> {
    let content = format!("BT /F1 24 Tf 72 700 Td ({words}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len() + 1),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj\n{body}\nendobj\n", i + 1));
    }
    let xref = out.len();
    out.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for at in offsets {
        out.push_str(&format!("{at:010} 00000 n \n"));
    }
    out.push_str(&format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1));
    out.into_bytes()
}

/// The real path: a message with a PDF, sent through the native host to a fake Codex in the project folder. What the
/// agent was sent names the PDF's text, which PDFKit read; the record keeps the bare prompt.
#[cfg(target_os = "macos")]
#[test]
fn a_pdf_sent_through_the_native_host_reaches_the_agent_readable() {
    use crate::native::{Origin, Send, hooks, send_from};
    let f = Folder::new();
    let home = f.0.join("home");
    let work = f.0.join("work");
    std::fs::create_dir_all(home.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(work.join(".locust/attachments")).unwrap();
    std::fs::write(work.join(".locust/attachments/Homework 5.pdf"), one_page_pdf("Problem three asks for x")).unwrap();
    let store = crate::store::Store::new(home.join("native"), home.join("electron")).unwrap();
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py");
    let _g = hooks::install(store, Some(("python3".into(), vec![fixture.into(), "cards-attached".into()])));
    let typed = with_attachments("Explain problem 3.", &paths(&[".locust/attachments/Homework 5.pdf"]));
    send_from(
        Send {
            teammate: crate::engine::Teammate {
                id: "tm_codec".into(),
                name: "Codec".into(),
                role: "Code & Migrations".into(),
                runtime: Some("codex".into()),
                model: Some("synthetic-model".into()),
                account: None,
                busy: false,
                runs_in: "accept-edits".into(),
                folder: Some(work.to_string_lossy().into()),
            },
            conversation: None,
            prompt: typed,
            model: Some("synthetic-model".into()),
            effort: None,
        },
        Origin::default(),
    )
    .expect("the turn starts");
    let sent_file = work.join("sent.txt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !sent_file.exists() {
        assert!(std::time::Instant::now() < deadline, "the agent was never sent the turn");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let sent = std::fs::read_to_string(&sent_file).unwrap();
    assert!(
        sent.contains("Read this file in the workspace before you answer:\n- .locust/attachments/Homework 5.pdf\n\nLocust has already opened the PDF for you"),
        "{sent}"
    );
    assert!(sent.contains("its text is in .locust/attachments/Homework 5.pdf.txt"), "{sent}");
    assert!(sent.ends_with("\n\nExplain problem 3."), "{sent}");
    let text = std::fs::read_to_string(work.join(".locust/attachments/Homework 5.pdf.txt")).unwrap();
    assert!(text.contains("Problem three asks for x"), "PDFKit read the page: {text}");
}

// A PDF named by a path that climbs out of the run's folder is never opened (or written about): only the folder's own files.
#[test]
fn a_pdf_outside_the_runs_folder_by_a_climbing_path_is_not_opened() {
    let f = Folder::new();
    f.write("secret.pdf", b"%PDF secret");
    let project = f.0.join("project");
    std::fs::create_dir_all(project.join(".locust/attachments")).unwrap();
    let mut opened = Vec::new();
    let sent = attachments_for_run_with(&with_attachments("Read it.", &paths(&["../secret.pdf"])), &project, &project, |_, pdf, _| {
        opened.push(pdf.to_owned());
        None
    });
    assert!(opened.is_empty(), "opened {opened:?}");
    assert!(!sent.contains("already opened"), "{sent}");
}
