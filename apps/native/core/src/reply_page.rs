//! A web page a model wrote into its reply, shown working (Electron `shared/reply-page.ts` and `main/reply-page.ts`,
//! 0.553). In Ask a model cannot write a file, so a page it makes arrives as a code block. A block that is a WHOLE page
//! (a doctype or `<html>` at the start, `</html>` at the end) runs as a page, served as a page a teammate wrote to disk
//! is (`page_preview`): its own folder, its own address, nothing of the app. A fragment, another language, or a page
//! still arriving stays code.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The largest page a reply may run: a game is tens of kilobytes.
pub const MAX_REPLY_PAGE_CHARS: usize = 2_000_000;

/// Where reply pages are written, under the app's own data folder: outside every project.
pub const REPLY_PAGES_FOLDER: &str = "reply-pages";

/// Whether a code block is a whole web page, not a fragment of one.
pub fn is_whole_page(code: &str, language: Option<&str>) -> bool {
    if let Some(language) = language
        && !matches!(language.trim().to_ascii_lowercase().as_str(), "html" | "htm" | "xhtml")
    {
        return false;
    }
    // Electron counts UTF-16 units.
    let length = code.encode_utf16().count();
    if length == 0 || length > MAX_REPLY_PAGE_CHARS {
        return false;
    }
    let start = code.trim_start().to_ascii_lowercase();
    let opens = start.strip_prefix("<!doctype").is_some_and(|rest| rest.trim_start().starts_with("html") && rest.starts_with(char::is_whitespace))
        || start.strip_prefix("<html").is_some_and(|rest| rest.starts_with(|c: char| c.is_whitespace() || c == '>'));
    opens && code.trim_end().to_ascii_lowercase().ends_with("</html>")
}

/// Writes a whole page where it is served: one folder per page, named by what it says, so a page's folder holds only
/// it and the same page is one file however often it is shown. None for anything that is not a whole page.
pub fn write_reply_page(html: &str, root: &Path) -> Option<PathBuf> {
    if !is_whole_page(html, None) {
        return None;
    }
    let digest = Sha256::digest(html.as_bytes());
    let name: String = digest.iter().map(|b| format!("{b:02x}")).collect::<String>()[..24].to_owned();
    let folder = root.join(name);
    std::fs::create_dir_all(&folder).ok()?;
    let path = folder.join("index.html");
    std::fs::write(&path, html).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "<!DOCTYPE html>\n<html><body><canvas></canvas></body></html>";

    // Electron's a-reply-page-is-a-whole-page.test.ts.
    #[test]
    fn a_doctype_or_html_at_the_start_and_the_end_tag_at_the_end() {
        assert!(is_whole_page(PAGE, Some("html")));
        assert!(is_whole_page("<html lang=\"en\"><body>hi</body></html>\n", Some("HTML")));
        assert!(is_whole_page(PAGE, None));
        assert!(is_whole_page("  <html>\n</html>", Some("xhtml")));
    }

    #[test]
    fn not_a_fragment_and_not_one_cut_off() {
        assert!(!is_whole_page("<div class=\"card\">hi</div>", Some("html")));
        assert!(!is_whole_page("<!DOCTYPE html>\n<html><body><canvas>", Some("html")));
        assert!(!is_whole_page("<htmlish></html>", Some("html")));
    }

    #[test]
    fn not_another_language_that_happens_to_hold_markup() {
        assert!(!is_whole_page(PAGE, Some("js")));
        assert!(!is_whole_page(PAGE, Some("markdown")));
    }

    #[test]
    fn not_past_the_size_a_page_in_a_reply_may_be() {
        let huge = format!("<!DOCTYPE html><html><body>{}</body></html>", "x".repeat(MAX_REPLY_PAGE_CHARS));
        assert!(!is_whole_page(&huge, Some("html")));
    }

    #[test]
    fn the_same_page_is_one_file_in_a_folder_of_its_own() {
        let root = std::env::temp_dir().join(format!("locust-reply-pages-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        let first = write_reply_page(PAGE, &root).unwrap();
        assert_eq!(first.file_name().unwrap(), "index.html");
        assert_eq!(std::fs::read_to_string(&first).unwrap(), PAGE);
        assert_eq!(write_reply_page(PAGE, &root).unwrap(), first);
        let other = write_reply_page("<html><body>2</body></html>", &root).unwrap();
        assert_ne!(other.parent(), first.parent());
        assert_eq!(std::fs::read_dir(first.parent().unwrap()).unwrap().count(), 1);
        assert_eq!(write_reply_page("<div>no</div>", &root), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
