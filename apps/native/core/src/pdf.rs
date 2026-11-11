//! PDF pages, thumbnails and text — port of `main/pdf-reading.ts`, `main/pdf-pages.ts`
//! and `shared/attachments.ts` `readablePdfNote`.
//!
//! On macOS, pages are read with Apple's PDFKit through the maintained `objc2-pdf-kit` 0.3
//! bindings (same objc2 0.6 line Rune already uses). Electron used a sandboxed pdf.js view;
//! native uses the system framework instead, still with no network and nothing executable.
//! Non-macOS builds refuse with Electron's "Locust could not read that PDF." words.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Larger than this is left to the runtime (`pdf-reading.ts`).
pub const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;

/// First N pages get pictures (`readablePdf` default).
pub const MAX_PICTURES: usize = 30;

/// Thumbnail long side in points (viewer card size).
const THUMB_SIDE: f64 = 720.0;

#[derive(Clone, Debug, PartialEq)]
pub struct PdfReading {
    pub pages: usize,
    pub text: Vec<String>,
    pub pictures: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadablePdf {
    pub pdf: String,
    pub text: Option<String>,
    pub pictures: Vec<String>,
    pub pages: usize,
}

pub fn is_pdf_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// `page-01.png`: two digits sort right for the thirty pages that are drawn.
pub fn picture_name(page: usize) -> String {
    format!("page-{page:02}.png")
}

/// Text file contents (`pdf-reading.ts` `readingText`), including the UTF-8 BOM.
pub fn reading_text(name: &str, reading: &PdfReading) -> String {
    let pages = reading
        .text
        .iter()
        .enumerate()
        .map(|(i, text)| format!("--- Page {} ---\n{}", i + 1, text.trim()))
        .collect::<Vec<_>>()
        .join("\n\n");
    let noun = if reading.pages == 1 { "page" } else { "pages" };
    let header = format!(
        "{name}, {} {noun}: the text Locust read out of it. Equations and tables can come out of order here; the page pictures show them as printed.",
        reading.pages
    );
    format!("\u{FEFF}{header}\n\n{pages}\n")
}

/// Agent note (`shared/attachments.ts` `readablePdfNote`).
pub fn readable_pdf_note(readables: &[ReadablePdf]) -> Option<String> {
    if readables.is_empty() {
        return None;
    }
    let lines: Vec<String> = readables
        .iter()
        .map(|readable| {
            let mut parts: Vec<String> = Vec::new();
            if let Some(text) = &readable.text {
                parts.push(format!("its text is in {text}"));
            } else {
                parts.push(
                    "it has no text in it (scanned pages), so read the pictures".into(),
                );
            }
            if !readable.pictures.is_empty() {
                let first = &readable.pictures[0];
                let last = readable.pictures.last().unwrap();
                let which = if readable.pictures.len() == readable.pages {
                    if readable.pages == 1 {
                        "its page is a picture".into()
                    } else {
                        format!("each of its {} pages is a picture", readable.pages)
                    }
                } else {
                    format!(
                        "its first {} of {} pages are pictures",
                        readable.pictures.len(),
                        readable.pages
                    )
                };
                if readable.pictures.len() == 1 {
                    parts.push(format!("{which}: {first}"));
                } else {
                    let last_name = last.rsplit('/').next().unwrap_or(last);
                    parts.push(format!("{which}, {first} to {last_name}"));
                }
            }
            format!("- {}: {}", readable.pdf, parts.join("; "))
        })
        .collect();
    let head = if readables.len() == 1 {
        "Locust has already opened the PDF for you, so no PDF tool or package is needed:".into()
    } else {
        format!(
            "Locust has already opened the {} PDFs for you, so no PDF tool or package is needed:",
            readables.len()
        )
    };
    let mut out = vec![head];
    out.extend(lines);
    out.push(
        "Read the text first. For equations, tables, charts and layout, look at the pictures: they show the page as printed, and the text may not.".into(),
    );
    Some(out.join("\n"))
}

/// Window refusal words from `pdf-pages.ts`.
pub fn pages_refusal(kind: &str) -> &'static str {
    match kind {
        "no-path" => "No path.",
        "no-workspace" => "No workspace.",
        "not-pdf" => "Not a PDF.",
        "not-local" => "Not a local file.",
        "outside" => "Outside the workspace.",
        "not-file" => "Not a file.",
        "too-large" => "Too large for Locust to draw.",
        _ => "Locust could not read that PDF.",
    }
}

/// Read a PDF's pages via PDFKit (macOS) into text + PNG thumbnails.
pub fn read_pdf_bytes(bytes: &[u8]) -> Result<PdfReading, String> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_PDF_BYTES {
        return Err(pages_refusal("could-not").into());
    }
    #[cfg(target_os = "macos")]
    {
        mac::read_pdf_bytes(bytes)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bytes;
        Err(pages_refusal("could-not").into())
    }
}

/// Open `pdf` relative to `folder`, write text + page PNGs beside the attachment under
/// native's own tree, and return the readable descriptor (Electron `readablePdf`).
pub fn readable_pdf(
    folder: &Path,
    pdf: &str,
    name: &str,
    max_pictures: usize,
) -> Result<ReadablePdf, String> {
    let path = folder.join(pdf);
    let meta = fs::metadata(&path).map_err(|_| pages_refusal("could-not").to_string())?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_PDF_BYTES {
        return Err(pages_refusal("could-not").into());
    }
    let bytes = fs::read(&path).map_err(|_| pages_refusal("could-not").to_string())?;
    let digest = hex_sha(&bytes);
    let cache = folder.join(".locust").join("pdf-cache").join(format!("{digest}.json"));
    if let Ok(raw) = fs::read_to_string(&cache)
        && let Ok(kept) = serde_json::from_str::<serde_json::Value>(&raw)
        && kept.get("digest").and_then(|v| v.as_str()) == Some(digest.as_str())
    {
        let pictures = kept
            .get("pictures")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let pages = kept.get("pages").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let text = kept
            .get("text")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        return Ok(ReadablePdf {
            pdf: pdf.to_owned(),
            text,
            pictures,
            pages,
        });
    }
    let reading = read_pdf_bytes(&bytes)?;
    let text_rel = format!("{pdf}.txt");
    let pictures_folder = format!("{pdf}-pages");
    let text_path = folder.join(&text_rel);
    if let Some(parent) = text_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let has_text = reading.text.iter().any(|t| !t.trim().is_empty());
    let text_field = if has_text {
        fs::write(&text_path, reading_text(name, &reading)).map_err(|e| e.to_string())?;
        Some(text_rel)
    } else {
        None
    };
    let pics_dir = folder.join(&pictures_folder);
    fs::create_dir_all(&pics_dir).map_err(|e| e.to_string())?;
    let mut pictures = Vec::new();
    for (i, png) in reading.pictures.iter().take(max_pictures).enumerate() {
        let rel = format!("{pictures_folder}/{}", picture_name(i + 1));
        fs::write(folder.join(&rel), png).map_err(|e| e.to_string())?;
        pictures.push(rel);
    }
    if let Some(parent) = cache.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        &cache,
        serde_json::json!({
            "digest": digest,
            "text": text_field,
            "pictures": pictures,
            "pages": reading.pages,
        })
        .to_string(),
    );
    Ok(ReadablePdf {
        pdf: pdf.to_owned(),
        text: text_field,
        pictures,
        pages: reading.pages,
    })
}

fn hex_sha(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Decide whether the window may show pages for `requested` under `folder`.
pub fn decide_pdf_pages(
    requested: &str,
    folder: &str,
) -> Result<PathBuf, &'static str> {
    if requested.is_empty() {
        return Err(pages_refusal("no-path"));
    }
    if folder.is_empty() || !Path::new(folder).is_absolute() {
        return Err(pages_refusal("no-workspace"));
    }
    if !is_pdf_path(requested) {
        return Err(pages_refusal("not-pdf"));
    }
    if requested.contains("://") || requested.starts_with("//") {
        return Err(pages_refusal("not-local"));
    }
    let path = Path::new(folder).join(requested);
    let Ok(canon_folder) = fs::canonicalize(folder) else {
        return Err(pages_refusal("outside"));
    };
    let Ok(canon_path) = fs::canonicalize(&path) else {
        return Err(pages_refusal("outside"));
    };
    if !canon_path.starts_with(&canon_folder) {
        return Err(pages_refusal("outside"));
    }
    let meta = fs::metadata(&canon_path).map_err(|_| pages_refusal("could-not"))?;
    if !meta.is_file() {
        return Err(pages_refusal("not-file"));
    }
    if meta.len() > MAX_PDF_BYTES {
        return Err(pages_refusal("too-large"));
    }
    Ok(canon_path)
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use objc2::AnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
    use objc2_foundation::{NSData, NSDictionary};
    use objc2_pdf_kit::{PDFDisplayBox, PDFDocument, PDFPage};

    pub(super) fn read_pdf_bytes(bytes: &[u8]) -> Result<PdfReading, String> {
        // PDFKit AppKit calls belong on the UI thread (Locust's view thread).
        let data = NSData::with_bytes(bytes);
        let doc = unsafe { PDFDocument::initWithData(PDFDocument::alloc(), &data) }
            .ok_or_else(|| pages_refusal("could-not").to_string())?;
        let count = unsafe { doc.pageCount() };
        if count == 0 {
            return Err(pages_refusal("could-not").into());
        }
        let mut text = Vec::with_capacity(count);
        let mut pictures = Vec::new();
        for i in 0..count {
            let page = unsafe { doc.pageAtIndex(i as _) }
                .ok_or_else(|| pages_refusal("could-not").to_string())?;
            let page_text = unsafe { page.string() }
                .map(|s| s.to_string())
                .unwrap_or_default();
            text.push(page_text);
            if pictures.len() < MAX_PICTURES
                && let Some(png) = page_png(&page)
            {
                pictures.push(png);
            }
        }
        Ok(PdfReading {
            pages: count,
            text,
            pictures,
        })
    }

    fn page_png(page: &PDFPage) -> Option<Vec<u8>> {
        let size = objc2_foundation::NSSize {
            width: THUMB_SIDE,
            height: THUMB_SIDE,
        };
        let image = unsafe { page.thumbnailOfSize_forBox(size, PDFDisplayBox::MediaBox) };
        let tiff = image.TIFFRepresentation()?;
        let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
        let props = NSDictionary::<objc2_foundation::NSString, objc2::runtime::AnyObject>::new();
        let png = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, props.as_ref())
        }?;
        Some(png.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_names_and_path_checks() {
        assert_eq!(picture_name(1), "page-01.png");
        assert_eq!(picture_name(12), "page-12.png");
        assert!(is_pdf_path("Homework 5.pdf"));
        assert!(!is_pdf_path("notes.txt"));
    }

    #[test]
    fn reading_text_has_bom_and_pages() {
        let reading = PdfReading {
            pages: 2,
            text: vec!["Page one words".into(), "Page two words".into()],
            pictures: vec![],
        };
        let t = reading_text("Homework 5.pdf", &reading);
        assert!(t.starts_with('\u{FEFF}'));
        assert!(t.contains("--- Page 1 ---"));
        assert!(t.contains("Page one words"));
        assert!(t.contains("2 pages"));
    }

    #[test]
    fn readable_note_matches_electron() {
        let note = readable_pdf_note(&[ReadablePdf {
            pdf: ".locust/attachments/Homework 5.pdf".into(),
            text: Some(".locust/attachments/Homework 5.pdf.txt".into()),
            pictures: vec![
                ".locust/attachments/Homework 5.pdf-pages/page-01.png".into(),
                ".locust/attachments/Homework 5.pdf-pages/page-02.png".into(),
            ],
            pages: 2,
        }])
        .unwrap();
        assert!(note.contains("no PDF tool or package is needed"));
        assert!(note.contains("its text is in .locust/attachments/Homework 5.pdf.txt"));
        assert!(note.contains(
            "each of its 2 pages is a picture, .locust/attachments/Homework 5.pdf-pages/page-01.png to page-02.png"
        ));
        assert!(note.contains("For equations, tables, charts and layout, look at the pictures"));

        let pictures: Vec<String> = (1..=30)
            .map(|i| format!(".locust/attachments/Book.pdf-pages/{}", picture_name(i)))
            .collect();
        let note = readable_pdf_note(&[ReadablePdf {
            pdf: "Book.pdf".into(),
            text: Some("Book.pdf.txt".into()),
            pictures,
            pages: 120,
        }])
        .unwrap();
        assert!(note.contains("its first 30 of 120 pages are pictures"));

        let note = readable_pdf_note(&[ReadablePdf {
            pdf: "Scan.pdf".into(),
            text: None,
            pictures: vec!["Scan.pdf-pages/page-01.png".into()],
            pages: 1,
        }])
        .unwrap();
        assert!(note.contains(
            "it has no text in it (scanned pages), so read the pictures; its page is a picture: Scan.pdf-pages/page-01.png"
        ));
        assert!(readable_pdf_note(&[]).is_none());
    }

    #[test]
    fn refusal_words_match_electron() {
        assert_eq!(pages_refusal("no-path"), "No path.");
        assert_eq!(pages_refusal("too-large"), "Too large for Locust to draw.");
        assert_eq!(pages_refusal("outside"), "Outside the workspace.");
    }

    #[test]
    fn decide_pdf_pages_refuses_outside() {
        let dir = std::env::temp_dir().join(format!("p14-pdf-dec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.pdf"), b"%PDF").unwrap();
        assert_eq!(
            decide_pdf_pages("/etc/passwd.pdf", &dir.to_string_lossy()).unwrap_err(),
            pages_refusal("outside")
        );
        assert_eq!(
            decide_pdf_pages("notes.txt", &dir.to_string_lossy()).unwrap_err(),
            pages_refusal("not-pdf")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn pdfkit_reads_text_and_png() {
        let bytes = br"%PDF-1.4
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
        let reading = read_pdf_bytes(bytes).expect("pdfkit");
        assert_eq!(reading.pages, 1);
        assert!(reading.text[0].contains("Hello"));
        assert!(!reading.pictures.is_empty());
        assert_eq!(&reading.pictures[0][..4], b"\x89PNG");

        let dir = std::env::temp_dir().join(format!("p14-pdf-rw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".locust/attachments")).unwrap();
        let rel = ".locust/attachments/Homework.pdf";
        std::fs::write(dir.join(rel), bytes).unwrap();
        let readable = readable_pdf(&dir, rel, "Homework.pdf", 30).expect("readable");
        assert_eq!(readable.pages, 1);
        assert!(readable.text.as_ref().is_some_and(|t| t.ends_with(".txt")));
        assert!(!readable.pictures.is_empty());
        // Second call hits the digest cache.
        let again = readable_pdf(&dir, rel, "Homework.pdf", 30).expect("cached");
        assert_eq!(again.pages, readable.pages);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
