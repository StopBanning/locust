//! A PDF in the window (Electron 0.714: `PdfTile.tsx`, `PdfPages.tsx`, `pdfPages.ts`). Among the files going with the
//! next message a PDF is a card: a page marked PDF, its name and how many pages it has, never the path it sits at. Open
//! in the viewer it is its pages: the pictures Locust drew of it (`pdf::readable_pdf`), one under another, each
//! numbered, and the pages it did not draw said rather than left out. A picture of a page runs nothing, which is why
//! the viewer shows a PDF at all.
//!
//! Electron's card shows the first page's picture once it is drawn; the composer's card here keeps the PDF mark, since
//! a file is not read for its pictures until it is sent or opened.
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
    let pages = page_count(&crate::file_menu::rooted_path(path, folder.as_deref()));
    let name = crate::file_menu::file_base_name(path);
    row()
        .key(format!("pdf-card/{path}"))
        .gap(8.0)
        .center_y()
        .child(
            col()
                .key(format!("pdf-card/{path}/page"))
                .w(22.0)
                .h(28.0)
                .center()
                .radius(3.0)
                .fill(p.sidebar)
                .border(1.0, p.hairline)
                .child(text("PDF").font(type_::LABEL).color(p.muted)),
        )
        .child(
            col()
                .gap(1.0)
                .child(text(name).font(type_::LABEL).color(p.text))
                .child(text(pdf_meta(pages)).font(type_::LABEL).color(p.muted)),
        )
}

/// What `read_document` read of each PDF it opened (its page count and the pictures it drew), by folder and path, for
/// the viewer to draw: the viewer holds a file's words, and a PDF is shown as its pages.
static READ: LazyLock<Mutex<HashMap<(std::path::PathBuf, String), crate::pdf::ReadablePdf>>> = LazyLock::new(Default::default);

/// Keep what was read of the PDF at `path` in `folder`, for [`pages`].
pub(super) fn keep(folder: &std::path::Path, path: &str, read: crate::pdf::ReadablePdf) {
    READ.lock().unwrap_or_else(|e| e.into_inner()).insert((folder.to_path_buf(), path.to_owned()), read);
}

/// The PDF at `path` in the viewer, as its pages (`PdfPages`), when Locust drew any; `None` leaves its text.
pub(super) fn pages(s: &State, path: &str, p: &Palette) -> Option<El> {
    let folder = std::path::PathBuf::from(selected_conversation(s)?.0.folder.as_deref()?);
    let read = READ.lock().unwrap_or_else(|e| e.into_inner()).get(&(folder, path.to_owned())).cloned()?;
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
