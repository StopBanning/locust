//! Electron's `main/a-pdf-shows-its-pages.test.ts` (0.714): the window asks the host for the
//! pictures Locust drew of an attached PDF — only a PDF inside the folder asked about, on disk
//! too, and only within a folder the host counts as its own.
use crate::pdf::{PdfPages, ReadablePdf, read_pdf_pages, read_pdf_pages_with};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("locust-pdf-pages-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".locust/attachments")).unwrap();
        std::fs::write(root.join(".locust/attachments/Homework 5.pdf"), b"%PDF-1.5 fake").unwrap();
        std::fs::write(root.join("notes.txt"), b"words").unwrap();
        Self(root)
    }

    fn path_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fake_prepare(prepared: &mut Vec<String>) -> impl FnMut(&Path, &str, &str) -> Option<ReadablePdf> + '_ {
    move |_where, pdf, name| {
        prepared.push(format!("{pdf}|{name}"));
        Some(ReadablePdf {
            pdf: pdf.to_owned(),
            text: Some(format!(".locust/attachments/{name}.txt")),
            pictures: vec![format!(".locust/attachments/{name}-pages/page-01.png")],
            pages: 2,
        })
    }
}

// case: reads a PDF in the folder, by its place in the folder, and answers its pictures
#[test]
fn reads_a_pdf_in_the_folder_by_its_place_in_the_folder_and_answers_its_pictures() {
    let root = Folder::new();
    let mut prepared = Vec::new();
    let answer = read_pdf_pages_with(
        ".locust/attachments/Homework 5.pdf",
        &root.path_str(),
        std::slice::from_ref(&root.0),
        fake_prepare(&mut prepared),
    )
    .expect("read pdf pages");
    assert_eq!(
        answer,
        PdfPages {
            pages: 2,
            pictures: vec![".locust/attachments/Homework 5.pdf-pages/page-01.png".into()],
            text: Some(".locust/attachments/Homework 5.pdf.txt".into()),
        }
    );
    assert_eq!(
        prepared.last().map(String::as_str),
        Some(".locust/attachments/Homework 5.pdf|Homework 5.pdf")
    );
    // The viewer passes the whole path; it is the same file.
    let whole = root.0.join(".locust/attachments/Homework 5.pdf");
    let answer_abs = read_pdf_pages_with(
        &whole.to_string_lossy(),
        &root.path_str(),
        std::slice::from_ref(&root.0),
        fake_prepare(&mut prepared),
    );
    assert!(answer_abs.is_ok(), "{answer_abs:?}");
    assert_eq!(
        prepared.last().map(String::as_str),
        Some(".locust/attachments/Homework 5.pdf|Homework 5.pdf")
    );
    #[cfg(target_os = "macos")]
    {
        let real_pdf = br"%PDF-1.4
1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj
2 0 obj<< /Type /Pages /Kids [3 0 R] /Count 1 >>endobj
3 0 obj<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources<< /Font<< /F1 5 0 R >> >> >>endobj
4 0 obj<< /Length 44 >>stream
BT /F1 24 Tf 50 100 Td (Hello) Tj ET
endstream
endobj
5 0 obj<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000266 00000 n 
0000000361 00000 n 
trailer<< /Size 6 /Root 1 0 R >>
startxref
445
%%EOF
";
        std::fs::write(root.0.join(".locust/attachments/Real.pdf"), real_pdf).unwrap();
        let real = read_pdf_pages(
            ".locust/attachments/Real.pdf",
            &root.path_str(),
            std::slice::from_ref(&root.0),
        )
        .expect("real pdf pages");
        assert_eq!(real.pages, 1);
        assert_eq!(real.pictures, vec![".locust/attachments/Real.pdf-pages/page-01.png"]);
        assert_eq!(real.text.as_deref(), Some(".locust/attachments/Real.pdf.txt"));
    }
}

// case: refuses what is not a PDF, what is outside the folder, and anything that is not a local path
#[test]
fn refuses_what_is_not_a_pdf_what_is_outside_the_folder_and_anything_that_is_not_a_local_path() {
    let root = Folder::new();
    let elsewhere = Folder::new();
    let mut prepared = Vec::new();

    assert_eq!(
        read_pdf_pages_with("notes.txt", &root.path_str(), std::slice::from_ref(&root.0), fake_prepare(&mut prepared)),
        Err("Not a PDF.".into())
    );
    assert!(
        read_pdf_pages_with("../outside.pdf", &root.path_str(), std::slice::from_ref(&root.0), fake_prepare(&mut prepared))
            .is_err()
    );
    let outside_pdf = elsewhere.0.join(".locust/attachments/Homework 5.pdf");
    assert!(
        read_pdf_pages_with(
            &outside_pdf.to_string_lossy(),
            &root.path_str(),
            std::slice::from_ref(&root.0),
            fake_prepare(&mut prepared),
        )
        .is_err()
    );
    assert_eq!(
        read_pdf_pages_with(
            "https://example.com/a.pdf",
            &root.path_str(),
            std::slice::from_ref(&root.0),
            fake_prepare(&mut prepared),
        ),
        Err("Not a local file.".into())
    );
    assert_eq!(
        read_pdf_pages_with(
            r"\\server\share\a.pdf",
            &root.path_str(),
            std::slice::from_ref(&root.0),
            fake_prepare(&mut prepared),
        ),
        Err("Not a local file.".into())
    );
    // A folder the host does not count as its own is no folder at all.
    assert!(
        read_pdf_pages_with(
            ".locust/attachments/Homework 5.pdf",
            &root.path_str(),
            std::slice::from_ref(&elsewhere.0),
            fake_prepare(&mut prepared),
        )
        .is_err()
    );
    assert!(prepared.is_empty(), "refused requests must not call prepare");
}

// case: says so when the PDF cannot be read, and when it is not there
#[test]
fn says_so_when_the_pdf_cannot_be_read_and_when_it_is_not_there() {
    let root = Folder::new();
    let mut prepared = Vec::new();
    assert_eq!(
        read_pdf_pages_with(
            ".locust/attachments/Homework 5.pdf",
            &root.path_str(),
            std::slice::from_ref(&root.0),
            |_, _, _| None,
        ),
        Err("Locust could not read that PDF.".into())
    );
    assert!(
        read_pdf_pages_with(
            ".locust/attachments/missing.pdf",
            &root.path_str(),
            std::slice::from_ref(&root.0),
            fake_prepare(&mut prepared),
        )
        .is_err()
    );
    // Real reader (read_pdf_pages) also refuses an unreadable/fake PDF with the same words.
    assert_eq!(
        read_pdf_pages(".locust/attachments/Homework 5.pdf", &root.path_str(), std::slice::from_ref(&root.0)),
        Err("Locust could not read that PDF.".into())
    );
}

// A PDF that is really there, but in another folder reached by climbing out, is refused as outside: being there is not
// being in the workspace (the older test's /etc/passwd.pdf was refused only because it does not exist).
#[test]
fn a_pdf_that_exists_outside_the_folder_by_a_climbing_path_is_refused() {
    let root = Folder::new();
    let elsewhere = Folder::new();
    let climb = format!("../{}/.locust/attachments/Homework 5.pdf", elsewhere.0.file_name().unwrap().to_string_lossy());
    assert!(root.0.join(&climb).exists(), "the PDF is there");
    assert_eq!(crate::pdf::decide_pdf_pages(&climb, &root.path_str()).unwrap_err(), "Outside the workspace.");
}
