//! Electron's `renderer/src/a-pdf-is-a-card.test.tsx` (0.714), through the window: a PDF going with the next message is
//! a card in the composer (`thread_view`), and a PDF opened in the viewer (`reply_actions::open_file`, `viewer_on`) is
//! its pages. Electron draws the card in the sent message, from the copy Locust made in `.locust/attachments`; the
//! window sends a file's own path and draws the card where the file is attached.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(name: &str) -> Temp {
    let dir = std::env::temp_dir().join(format!("locust-pdf-card-{name}-{}", locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    Temp(dir)
}

/// A PDF of `pages` pages (200 x 200, a word on each), its cross-reference table counted.
fn pdf_of(pages: usize) -> Vec<u8> {
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{}] /Count {pages} >>", kids.join(" ")),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    for i in 0..pages {
        let content = format!("BT /F1 24 Tf 50 100 Td (Page {}) Tj ET", i + 1);
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>",
            5 + 2 * i
        ));
        objects.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len() + 1));
    }
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

fn texts(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

fn keys(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.key).collect()
}

/// The composer with `attached` going with the next message, drawn through the conversation view.
fn composer_with(attached: Vec<String>) -> rune::Runtime<State> {
    let mut s = State::default();
    crate::scenes::composer_base_scene(&mut s, false);
    s.attached = attached;
    let mut rt = rune::Runtime::new(s, |s, _| crate::thread_view::thread_view(s, &Palette::new(false)), (1200.0, 1000.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    rt
}

/// A conversation whose folder is `folder`, with `path` opened in the viewer as the app opens it.
fn viewer_on_file(folder: &std::path::Path, path: &str) -> rune::Runtime<State> {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm".into(),
        name: "Wren".into(),
        role: String::new(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(folder.to_string_lossy().into_owned()),
    });
    s.conversations.insert("tm".into(), Default::default());
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    crate::reply_actions::open_file(&mut s, crate::reply_actions::FileTarget { path: path.into(), start: 1, end: 1 });
    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::reply_actions::viewer_on(s, &Palette::new(false), "darwin", &[]).unwrap(),
        (1200.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn pages_drawn(rt: &rune::Runtime<State>) -> usize {
    keys(rt).iter().filter(|k| k.starts_with("pdf-pages/page-")).count()
}

// case: is its name and its page count, never the path it was copied to
#[test]
fn is_its_name_and_its_page_count_never_the_path_it_was_copied_to() {
    let dir = temp("name");
    std::fs::create_dir_all(dir.0.join(".locust/attachments")).unwrap();
    // Before its pages can be counted it says only what it knows.
    let unread = dir.0.join(".locust/attachments/Homework 5.pdf");
    std::fs::write(&unread, b"not a pdf yet").unwrap();
    let path = unread.to_string_lossy().into_owned();
    let rt = composer_with(vec![path.clone()]);
    let said = texts(&rt);
    assert!(said.iter().any(|t| t == "Homework 5.pdf"), "{said:?}");
    assert!(!said.iter().any(|t| t.contains(".locust/attachments")), "{said:?}");
    assert!(keys(&rt).contains(&format!("pdf-card/{path}/page")), "{:?}", keys(&rt));
    assert_eq!(said.iter().filter(|t| *t == "PDF").count(), 2, "the page's mark and the line under its name: {said:?}");
    // A PDF it can read is counted.
    if cfg!(target_os = "macos") {
        let read = dir.0.join(".locust/attachments/Report.pdf");
        std::fs::write(&read, pdf_of(3)).unwrap();
        let rt = composer_with(vec![read.to_string_lossy().into_owned()]);
        let said = texts(&rt);
        assert!(said.iter().any(|t| t == "Report.pdf") && said.iter().any(|t| t == "PDF · 3 pages"), "{said:?}");
    }
    // Any other file is still its name.
    let rt = composer_with(vec!["/work/notes.md".into()]);
    assert!(texts(&rt).iter().any(|t| t == "notes.md") && !keys(&rt).iter().any(|k| k.starts_with("pdf-card/")));
}

// case: says how many pages, in one word or two
#[test]
fn says_how_many_pages_in_one_word_or_two() {
    assert_eq!(pdf_meta(None), "PDF");
    assert_eq!(pdf_meta(Some(1)), "PDF · 1 page");
    assert_eq!(pdf_meta(Some(12)), "PDF · 12 pages");
    assert!(crate::pdf::is_pdf_path("notes/Report.PDF"));
    assert!(!crate::pdf::is_pdf_path("notes/report.pdf.txt"));
}

// case: a page each, numbered, and the pages it did not draw said rather than left out
#[test]
fn a_page_each_numbered_and_the_pages_it_did_not_draw_said_rather_than_left_out() {
    // Electron's numbers: two pictures of a forty-page book.
    let dir = temp("book");
    keep(
        &dir.0,
        "Book.pdf",
        crate::pdf::ReadablePdf {
            pdf: "Book.pdf".into(),
            text: None,
            pictures: vec!["Book.pdf-pages/page-01.png".into(), "Book.pdf-pages/page-02.png".into()],
            pages: 40,
        },
    );
    let mut s = State::default();
    s.teammates.push(Teammate { id: "tm".into(), name: "Wren".into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: Some(dir.0.to_string_lossy().into_owned()) });
    s.conversations.insert("tm".into(), Default::default());
    let mut rt = rune::Runtime::new(s, |s, _| pages(s, "Book.pdf", &Palette::new(false)).unwrap(), (800.0, 2000.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    assert_eq!(pages_drawn(&rt), 2);
    let said = texts(&rt);
    for words in ["1 of 40", "2 of 40", "Locust drew the first 2 of its 40 pages. Show it in the file manager to read the rest in your PDF app."] {
        assert!(said.iter().any(|t| t == words), "{words}: {said:?}");
    }
    // Opened as the app opens it: the pages Locust drew, numbered, and the rest said.
    if cfg!(target_os = "macos") {
        std::fs::write(dir.0.join("Long.pdf"), pdf_of(crate::pdf::MAX_PICTURES + 1)).unwrap();
        let rt = viewer_on_file(&dir.0, "Long.pdf");
        assert_eq!(pages_drawn(&rt), crate::pdf::MAX_PICTURES, "{:?}", texts(&rt));
        let said = texts(&rt);
        assert!(!said.iter().any(|t| t == "31 of 31") && said.iter().any(|t| t == "30 of 31"), "{said:?}");
        assert!(said.iter().any(|t| t.starts_with("Locust drew the first 30 of its 31 pages.")), "{said:?}");
    }
}

// case: a PDF drawn whole says nothing more
#[test]
fn a_pdf_drawn_whole_says_nothing_more() {
    assert_eq!(more_said(1, 1), None);
    if cfg!(target_os = "macos") {
        let dir = temp("sheet");
        std::fs::write(dir.0.join("Sheet.pdf"), pdf_of(1)).unwrap();
        let rt = viewer_on_file(&dir.0, "Sheet.pdf");
        assert_eq!(pages_drawn(&rt), 1, "{:?}", texts(&rt));
        assert!(texts(&rt).iter().any(|t| t == "1 of 1"), "{:?}", texts(&rt));
        assert!(!keys(&rt).iter().any(|k| k == "pdf-pages/more"), "{:?}", keys(&rt));
        // Shown as its page, not as its words.
        assert!(!texts(&rt).iter().any(|t| t.contains("Page 1")), "{:?}", texts(&rt));
    }
}

/// Electron's `readAttachedPdfs` (main/index.ts, 0.713): a PDF is opened for the agent the moment it is attached, in the
/// folder the turn will run in -- the teammate's own, not the one Locust has open -- so Send finds it read rather than
/// reading it a second time (`pdf_arrives_readable_tests`: "Send waits for the reading the attach button started").
#[cfg(target_os = "macos")]
#[test]
fn a_pdf_attached_in_the_composer_starts_being_opened_in_the_folder_the_turn_runs_in() {
    let own = temp("attach-own");
    let open = temp("attach-open");
    let pdf = own.0.join("Homework 5.pdf");
    std::fs::write(&pdf, pdf_of(2)).unwrap();
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm".into(),
        name: "Wren".into(),
        role: String::new(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(own.0.to_string_lossy().into_owned()),
    });
    s.home.folder_path = Some(open.0.to_string_lossy().into_owned());
    crate::composer_wiring::attach_dropped_paths(&mut s, &[pdf.clone()]);
    assert_eq!(s.attached, [pdf.to_string_lossy().into_owned()]);
    // Electron's layout: `<folder>/.locust/attachments/<name>.txt`, and the pictures with what was read kept beside them.
    let kept = own.0.join(".locust/attachments/Homework 5.pdf-pages/locust-reading.json");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !kept.is_file() {
        assert!(std::time::Instant::now() < deadline, "the attached PDF was never opened");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(own.0.join(".locust/attachments/Homework 5.pdf.txt").is_file());
    assert!(own.0.join(".locust/attachments/Homework 5.pdf-pages/page-02.png").is_file());
    assert!(!open.0.join(".locust").exists(), "nothing is written in a folder the turn does not run in");
}
