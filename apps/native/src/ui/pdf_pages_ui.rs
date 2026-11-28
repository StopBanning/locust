//! A PDF in the window (Electron 0.714: `PdfTile.tsx`, `PdfPages.tsx`, `pdfPages.ts`). Among the files going with the
//! next message a PDF is a card: a page marked PDF, its name and how many pages it has, never the path it sits at. Open
//! in the viewer it is its pages: the pictures Locust drew of it (`pdf::readable_pdf`), one under another, each
//! numbered, and the pages it did not draw said rather than left out. A picture of a page runs nothing, which is why
//! the viewer shows a PDF at all.
//!
//! The composer can reuse the first page after the viewer has read it. Until then, or if the picture cannot be
//! displayed, it keeps the PDF mark.
use super::*;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// "PDF · 2 pages", "PDF": what a PDF's card says under its name (`pdfMeta`).
pub(super) fn pdf_meta(pages: Option<usize>) -> String {
    match pages {
        None => "PDF".into(),
        Some(n) => format!("PDF · {n} {}", if n == 1 { "page" } else { "pages" }),
    }
}

/// What the viewer says under the pages it drew when the PDF has more, and nothing when it drew them all.
pub(super) fn more_said(drawn: usize, pages: usize) -> Option<String> {
    (pages > drawn).then(|| {
        format!("Locust drew the first {drawn} of its {pages} pages. Show it in the file manager to read the rest in your PDF app.")
    })
}

/// A file's size and time, so a PDF changed on disk is counted again.
type Seen = (u64, Option<std::time::SystemTime>);

/// Each PDF's page count, by path, as last seen: a card is drawn every frame, the file is read once.
static COUNTS: LazyLock<Mutex<HashMap<String, (Seen, Option<usize>)>>> = LazyLock::new(Default::default);

/// How many pages the PDF at `path` has, or `None` when it cannot be read.
fn page_count(path: &str) -> Option<usize> {
    let meta = std::fs::metadata(path).ok()?;
    let seen = (meta.len(), meta.modified().ok());
    let mut counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((was, count)) = counts.get(path)
        && *was == seen
    {
        return *count;
    }
    let count = if meta.is_file() && meta.len() > 0 && meta.len() <= crate::pdf::MAX_PDF_BYTES {
        std::fs::read(path).ok().and_then(|bytes| crate::pdf::page_count(&bytes))
    } else {
        None
    };
    counts.insert(path.to_owned(), (seen, count));
    count
}

/// A PDF going with the next message, as a card (`PdfTile`): its name and how many pages, never its path.
pub(super) fn card(s: &State, path: &str, p: &Palette) -> El {
    let folder = selected_conversation(s).and_then(|(t, _)| t.folder.clone());
    let reading = folder.as_deref().and_then(|folder| remembered_for_card(std::path::Path::new(folder), path));
    let pages = reading.as_ref().map(|read| read.pages)
        .or_else(|| page_count(&crate::file_menu::rooted_path(path, folder.as_deref())));
    let name = crate::file_menu::file_base_name(path);
    let thumb = reading.as_ref().and_then(|read| read.pictures.first())
        .and_then(|picture| crate::step_pictures::thread_picture(s, picture));
    let mut page = col()
        .key(format!("pdf-card/{path}/page"))
        .w(22.0)
        .h(28.0)
        .center()
        .radius(3.0)
        .fill(p.sidebar)
        .border(1.0, p.hairline);
    page = if let Some(image) = thumb {
        let (w, h) = crate::step_pictures::fitted(image.size().unwrap_or([22, 28]), 22.0, 28.0);
        page.child(el().key("pdf-card-thumbnail").size(w, h).pixels(image.pixels()).a11y_hidden())
    } else {
        page.child(text("PDF").font(type_::LABEL).color(p.muted))
    };
    row()
        .key(format!("pdf-card/{path}"))
        .gap(8.0)
        .center_y()
        .child(page)
        .child(
            col()
                .gap(1.0)
                .child(text(name).font(type_::LABEL).color(p.text))
                .child(text(pdf_meta(pages)).font(type_::LABEL).color(p.muted)),
        )
}

/// A regular source file's identity and change information, without rereading its bytes.
#[derive(PartialEq, Eq)]
struct SourceStamp {
    path: std::path::PathBuf,
    len: u64,
    modified: std::time::SystemTime,
}

fn source_stamp(folder: &std::path::Path, path: &str) -> Option<SourceStamp> {
    let rooted = crate::file_menu::rooted_path(path, folder.to_str());
    let path = std::fs::canonicalize(rooted).ok()?;
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() {
        return None;
    }
    Some(SourceStamp { path, len: meta.len(), modified: meta.modified().ok()? })
}

struct RememberedPdf {
    reading: crate::pdf::ReadablePdf,
    source: Option<SourceStamp>,
}

/// An opened viewer holds its original reading. An attachment card only reuses
/// it while the file it represents is still the file that was read.
static READ: LazyLock<Mutex<HashMap<(std::path::PathBuf, String), RememberedPdf>>> = LazyLock::new(Default::default);

pub(super) fn keep(folder: &std::path::Path, path: &str, reading: crate::pdf::ReadablePdf) {
    let source = source_stamp(folder, path);
    READ.lock().unwrap_or_else(|e| e.into_inner()).insert(
        (folder.to_path_buf(), path.to_owned()),
        RememberedPdf { reading, source },
    );
}

fn remembered(folder: &std::path::Path, path: &str) -> Option<crate::pdf::ReadablePdf> {
    let cache = READ.lock().unwrap_or_else(|e| e.into_inner());
    Some(cache.get(&(folder.to_path_buf(), path.to_owned()))?.reading.clone())
}

fn remembered_for_card(folder: &std::path::Path, path: &str) -> Option<crate::pdf::ReadablePdf> {
    let stamp = source_stamp(folder, path)?;
    let cache = READ.lock().unwrap_or_else(|e| e.into_inner());
    let entry = cache.get(&(folder.to_path_buf(), path.to_owned()))?;
    (entry.source.as_ref() == Some(&stamp)).then(|| entry.reading.clone())
}

/// The PDF at `path` in the viewer, as its pages (`PdfPages`), when Locust drew any; `None` leaves its text.
pub(super) fn pages(s: &State, path: &str, p: &Palette) -> Option<El> {
    let folder = std::path::PathBuf::from(selected_conversation(s)?.0.folder.as_deref()?);
    let read = remembered(&folder, path)?;
    if read.pictures.is_empty() {
        return None;
    }
    let name = crate::file_menu::file_base_name(path);
    let mut list = col().key("pdf-pages").w_full().gap(12.0);
    for (i, picture) in read.pictures.iter().enumerate() {
        let n = i + 1;
        let shown = match crate::step_pictures::thread_picture(s, picture) {
            Some(image) => {
                let fit = image.size().map(|[w, h]| 300.0 * h.max(1) as f32 / w.max(1) as f32).unwrap_or(400.0);
                el().size(300.0, fit).pixels(image.pixels()).border(1.0, p.hairline).role("image").label(format!("{name}, page {n}"))
            }
            None => el().w(300.0).h(400.0).fill(p.raised).label(format!("Page {n}, being drawn")),
        };
        list = list.child(
            col()
                .key(format!("pdf-pages/page-{n}"))
                .gap(4.0)
                .child(shown)
                .child(text(format!("{n} of {}", read.pages)).font(type_::LABEL).color(p.muted)),
        );
    }
    if let Some(more) = more_said(read.pictures.len(), read.pages) {
        list = list.child(text(more).key("pdf-pages/more").font(type_::LABEL).color(p.muted).wrap_anywhere());
    }
    Some(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(list))
}

#[cfg(test)]
#[path = "pdf_pages_ui_tests.rs"]
mod pdf_pages_ui_tests;

#[cfg(test)]
#[path = "pdf_card_fallback_tests.rs"]
mod pdf_card_fallback_tests;
