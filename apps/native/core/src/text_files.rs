//! Reading a file a teammate wrote, to show it beside the conversation (Electron's `shared/text-files.ts`).
//!
//! A file in the workspace was written by a MODEL, so what the viewer may read is an allowlist of text by extension,
//! never a guess from the bytes (a wrong guess shows a megabyte of machine code as mojibake in a panel). And the bound
//! is a refusal, not a truncation: half a file shown as the whole one is a quiet lie. This module decides what may be
//! read and how much; the window decides how to draw it.

/// What the viewer will read, by extension. Every entry is something a teammate plausibly writes and a person
/// plausibly wants to read.
const TEXT_EXTENSIONS: &[&str] = &[
    "md", "markdown", "txt", "text", "log",
    "json", "jsonl", "yaml", "yml", "toml", "ini", "cfg", "conf", "env",
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "rb", "go", "rs", "java",
    "c", "h", "cpp", "hpp", "cs", "swift", "kt", "sh", "bash", "ps1", "bat",
    "css", "scss", "html", "htm", "xml", "svg", "sql", "graphql", "diff", "patch",
    "gitignore", "dockerignore", "editorconfig",
];

/// A quarter of a megabyte: enough for any report, brief or source file a teammate writes, and small enough that a
/// runaway log cannot be handed to the window whole.
pub const MAX_TEXT_BYTES: u64 = 256 * 1024;

/// How the window draws a file: as prose, or as code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerMode {
    Markdown,
    Code,
}

/// The extension, lowercased, or empty for a file that has none. Either path separator ends a folder's name, and a
/// dotfile with no second dot IS its extension (`.gitignore`, `.env`).
pub fn extension_of(path: &str) -> String {
    let normal = path.replace('\\', "/");
    let name = normal.rsplit('/').next().unwrap_or("");
    // JavaScript's `lastIndexOf('.')` and `slice` count UTF-16 units; the cuts here fall on a '.', so characters do.
    match name.rfind('.') {
        None => String::new(),
        Some(0) => name[1..].to_lowercase(),
        Some(dot) => name[dot + 1..].to_lowercase(),
    }
}

/// Whether the viewer will open this path at all.
pub fn is_viewable_text(path: &str) -> bool {
    TEXT_EXTENSIONS.contains(&extension_of(path).as_str())
}

/// How to draw it: Markdown through the renderer a reply uses, everything else as code (a `.ts` file drawn as
/// Markdown would eat its own asterisks).
pub fn viewer_mode(path: &str) -> ViewerMode {
    match extension_of(path).as_str() {
        "md" | "markdown" => ViewerMode::Markdown,
        _ => ViewerMode::Code,
    }
}

/// Why a file over the bound is not shown, in Electron's words (`main/index.ts`, the text channel).
pub fn too_big_to_show(bytes: u64) -> String {
    // `Math.round(size / 1024)`: halves round up.
    let kb = (bytes as f64 / 1024.0 + 0.5).floor();
    format!("That file is {kb}KB, too big to show here. Save a copy and open it in an editor.")
}

/// What the viewer says to a path it will not open.
pub const NOT_OPENED: &str = "Locust does not open that kind of file here.";

/// What the viewer says when the file is not there (or cannot be read).
pub const NOT_THERE: &str = "That file is not there. The teammate named it but did not write it.";

/// Read a file the viewer may show, in Electron's order and words (`main/index.ts`, the text channel): a path whose
/// extension is not on the list is refused before anything is touched; a folder is not a file; a file past the bound is
/// refused whole, never truncated; and the text is read the way Node's `readFile(path, 'utf8')` reads it, so bytes
/// that are not UTF-8 become U+FFFD instead of a refusal. `requested` is the path as the message named it (the
/// extension is judged by that), `file` the place on disk it resolved to.
pub fn read_viewable_text(requested: &str, file: &std::path::Path) -> Result<String, String> {
    if !is_viewable_text(requested) {
        return Err(NOT_OPENED.to_owned());
    }
    let Ok(measured) = std::fs::metadata(file) else {
        return Err(NOT_THERE.to_owned());
    };
    if !measured.is_file() {
        return Err("That is a folder, not a file.".to_owned());
    }
    if measured.len() > MAX_TEXT_BYTES {
        return Err(too_big_to_show(measured.len()));
    }
    match std::fs::read(file) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(_) => Err(NOT_THERE.to_owned()),
    }
}

/// What a handed-over file's Copy button says to a spreadsheet or a Word or PowerPoint file (`Thread.tsx`): the viewer
/// reads those for their cells and words, but there is no text to copy as it is, and saving a copy keeps it whole.
pub const ONLY_TEXT_CAN_BE_COPIED: &str = "Only a text file can be copied. Save a copy instead.";

/// A handed-over file's words for its Copy button, as Electron's `Thread.tsx` asks the text channel for them: the
/// kinds the viewer draws as a table or a document (`csv`, `tsv`, `xlsx`, `docx`, `pptx`) are refused in the Copy
/// sentence; everything else is read as the viewer reads it, so a kind it does not open (`pdf`, `doc`, `xls`, a
/// picture) says "Locust does not open that kind of file here."
pub fn read_for_copy(requested: &str, file: &std::path::Path) -> Result<String, String> {
    if matches!(extension_of(requested).as_str(), "csv" | "tsv" | "xlsx" | "docx" | "pptx") {
        return Err(ONLY_TEXT_CAN_BE_COPIED.to_owned());
    }
    read_viewable_text(requested, file)
}

#[cfg(test)]
#[path = "text_files_tests.rs"]
mod tests;
