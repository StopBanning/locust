//! Word/PowerPoint words, XLSX/CSV sheets — as far as Electron's `office-words.ts`,
//! `office-text.ts` and `xlsx.ts` go for reading (not editing, not macros).
//!
//! - `.xlsx`: `calamine` 0.38 (MIT/Apache) — small, read-only, no formula execution; pairs with zip 2.
//! - `.csv`/`.tsv`: split lines locally with the same bounds Electron uses.
//! - `.docx`/`.pptx`: zip + XML text extraction (same approach as Electron's own
//!   `office-text.ts`, no macro/OLE execution). Written copies land under native's
//!   `.locust/office-words/` only.

use std::io::{Cursor, Read};
use std::path::Path;

/// Electron `MAX_OFFICE_FILE_BYTES` / shared/office-document.ts (40 MiB).
pub const MAX_OFFICE_FILE_BYTES: u64 = 40 * 1024 * 1024;
pub const OFFICE_WORDS_DIR: &str = ".locust/office-words";
pub const MAX_FILES: usize = 12;
/// Electron `shared/sheet.ts` `MAX_SHEETS`.
pub const MAX_SHEETS: usize = 12;
pub const MAX_SHEET_ROWS: usize = 500;
pub const MAX_SHEET_COLUMNS: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentBlock {
    Heading { level: usize, text: String },
    Paragraph { text: String },
    Item { depth: usize, text: String },
    Slide { number: usize, title: Option<String> },
    Table { rows: Vec<Vec<String>> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct OfficeDocument {
    pub blocks: Vec<DocumentBlock>,
    pub more: usize,
}

/// Electron `shared/sheet.ts` `MAX_CELL_TEXT`.
pub const MAX_CELL_TEXT: usize = 300;

/// What a cell holds (`SheetCellKind`): numbers sit right, a formula with no stored result is drawn as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetCellKind {
    Text,
    Number,
    /// A formula the file holds no result for, shown as written.
    Formula,
    Boolean,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheetCell {
    pub text: String,
    pub kind: SheetCellKind,
}

impl SheetCell {
    fn empty() -> Self {
        SheetCell { text: String::new(), kind: SheetCellKind::Text }
    }
}

/// One sheet (`SheetTable`): its rows with trailing empty rows and columns trimmed, and what lay past the bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetTable {
    pub name: String,
    pub rows: Vec<Vec<SheetCell>>,
    /// Rows past the bound, counted and not drawn.
    pub more_rows: usize,
    /// Columns past the bound, counted and not drawn.
    pub more_columns: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Workbook {
    pub sheets: Vec<SheetTable>,
    /// Sheets past the bound.
    pub more_sheets: usize,
}

/// `A` for 0, `Z` for 25, `AA` for 26: a spreadsheet's own column names (`columnName`).
pub fn column_name(index: usize) -> String {
    let mut name = Vec::new();
    let mut rest = index + 1;
    while rest > 0 {
        name.push(b'A' + ((rest - 1) % 26) as u8);
        rest = (rest - 1) / 26;
    }
    name.reverse();
    String::from_utf8(name).unwrap_or_default()
}

/// A cell's text as it will be drawn: bounded, and never a control character (`cellText`).
fn cell_text(value: &str) -> String {
    let clean: String = value
        .chars()
        .filter(|c| !matches!(*c as u32, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f))
        .collect();
    // Electron counts UTF-16 units; a cell past the bound keeps MAX_CELL_TEXT - 1 of them and an ellipsis.
    if clean.encode_utf16().count() <= MAX_CELL_TEXT {
        return clean;
    }
    let mut kept = String::new();
    let mut units = 0;
    for c in clean.chars() {
        units += c.len_utf16();
        if units > MAX_CELL_TEXT - 1 {
            break;
        }
        kept.push(c);
    }
    kept.push('…');
    kept
}

/// Whether written text reads as a number -- `1,400`, `-3.5`, `$4,200.00`, `12%` -- so it sits right (`looksNumeric`).
fn looks_numeric(text: &str) -> bool {
    static NUMERIC: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^[-+(]?[$€£¥]?\s?-?(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?\)?%?$").unwrap()
    });
    NUMERIC.is_match(text.trim())
}

/// RFC 4180 CSV (`parseCsv`): quoted fields, doubled quotes inside them, line breaks inside quotes, CRLF or LF. Stops
/// once it has `limit_rows` rows, so a huge file costs what the viewer shows.
fn parse_csv(text: &str, delimiter: char, limit_rows: usize) -> Vec<Vec<String>> {
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        if c == '"' && field.is_empty() {
            quoted = true;
        } else if c == delimiter {
            row.push(std::mem::take(&mut field));
        } else if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
            if rows.len() >= limit_rows {
                return rows;
            }
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Trailing empty rows and columns off, bounds applied and counted (`boundedTable`). `known_rows` and `known_columns`
/// are how many rows and columns hold something in the whole source, when the grid is already cut to the bound.
fn bounded_table(name: &str, grid: Vec<Vec<SheetCell>>, known_rows: Option<usize>, known_columns: Option<usize>) -> SheetTable {
    let mut last = grid.len();
    while last > 0 && grid[last - 1].iter().all(|c| c.text.is_empty()) {
        last -= 1;
    }
    let kept = &grid[..last];
    let width = kept
        .iter()
        .map(|row| row.iter().rposition(|c| !c.text.is_empty()).map_or(0, |i| i + 1))
        .max()
        .unwrap_or(0);
    let drawn_width = width.min(MAX_SHEET_COLUMNS);
    let rows: Vec<Vec<SheetCell>> = kept
        .iter()
        .take(MAX_SHEET_ROWS)
        .map(|row| (0..drawn_width).map(|c| row.get(c).cloned().unwrap_or_else(SheetCell::empty)).collect())
        .collect();
    SheetTable {
        name: name.to_owned(),
        more_rows: known_rows.unwrap_or(kept.len()).saturating_sub(rows.len()),
        more_columns: width.max(known_columns.unwrap_or(0)).saturating_sub(drawn_width),
        rows,
    }
}

/// Electron `documentWords`.
pub fn document_words(name: &str, document: &OfficeDocument) -> String {
    let mut lines: Vec<String> = vec![
        format!(
            "Words of {name}, read by Locust. A copy: editing it changes nothing in the document."
        ),
        String::new(),
    ];
    for block in &document.blocks {
        match block {
            DocumentBlock::Heading { level, text } => {
                lines.push(format!("{} {text}", "#".repeat(*level)));
                lines.push(String::new());
            }
            DocumentBlock::Paragraph { text } => {
                lines.push(text.clone());
                lines.push(String::new());
            }
            DocumentBlock::Item { depth, text } => {
                lines.push(format!("{}- {text}", "  ".repeat(*depth)));
            }
            DocumentBlock::Slide { number, title } => {
                lines.push(String::new());
                match title {
                    Some(t) => lines.push(format!("## Slide {number}: {t}")),
                    None => lines.push(format!("## Slide {number}")),
                }
                lines.push(String::new());
            }
            DocumentBlock::Table { rows } => {
                for (at, row) in rows.iter().enumerate() {
                    let cell = |t: &str| t.replace('|', "\\|").split_whitespace().collect::<Vec<_>>().join(" ");
                    lines.push(format!(
                        "| {} |",
                        row.iter().map(|c| cell(c)).collect::<Vec<_>>().join(" | ")
                    ));
                    if at == 0 {
                        lines.push(format!(
                            "| {} |",
                            row.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
                        ));
                    }
                }
                lines.push(String::new());
            }
        }
    }
    if document.more > 0 {
        lines.push(format!(
            "({} more parts of the document were not read.)",
            document.more
        ));
    }
    let joined = lines.join("\n");
    let mut out = String::new();
    let mut blank = 0usize;
    for line in joined.lines() {
        if line.is_empty() {
            blank += 1;
            if blank <= 2 {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_owned() + "\n"
}

pub fn office_kind(name: &str) -> Option<&'static str> {
    if name.starts_with("~$") {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".docx") {
        Some("docx")
    } else if lower.ends_with(".pptx") {
        Some("pptx")
    } else {
        None
    }
}

/// Read a `.docx` or `.pptx` from bytes into structured blocks.
pub fn read_office(bytes: &[u8], kind: &str) -> Result<OfficeDocument, String> {
    if bytes.len() as u64 > MAX_OFFICE_FILE_BYTES {
        return Err("That document is too large to show here.".into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| format!("That file is not a {kind} Locust can read."))?;
    let mut blocks = Vec::new();
    let mut more = 0usize;
    match kind {
        "docx" => {
            let xml = zip_text(&mut zip, "word/document.xml")?;
            push_docx_blocks(&xml, &mut blocks, &mut more);
        }
        "pptx" => {
            let mut slide_names: Vec<String> = zip
                .file_names()
                .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
                .map(str::to_owned)
                .collect();
            slide_names.sort();
            for (i, name) in slide_names.iter().enumerate() {
                if blocks.len() > 400 {
                    more += 1;
                    continue;
                }
                let xml = zip_text(&mut zip, name)?;
                let title = first_text_run(&xml);
                blocks.push(DocumentBlock::Slide {
                    number: i + 1,
                    title: title.filter(|t| !t.is_empty()),
                });
                for para in text_runs(&xml) {
                    if !para.trim().is_empty() {
                        blocks.push(DocumentBlock::Paragraph { text: para });
                    }
                }
            }
        }
        _ => return Err("Not an office document Locust reads.".into()),
    }
    Ok(OfficeDocument { blocks, more })
}

fn zip_text(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, String> {
    let mut file = zip
        .by_name(name)
        .map_err(|_| "That document is damaged.".to_string())?;
    if file.size() > 24 * 1024 * 1024 {
        return Err("That document is too large to show here.".into());
    }
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .map_err(|_| "That document is damaged.".to_string())?;
    Ok(buf)
}

fn push_docx_blocks(xml: &str, blocks: &mut Vec<DocumentBlock>, more: &mut usize) {
    for para in xml.split("<w:p").skip(1) {
        if blocks.len() > 400 {
            *more += 1;
            continue;
        }
        let text = text_runs(&format!("<w:p{para}")).join("");
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if para.contains("w:pStyle") && para.contains("Heading") {
            let level = if para.contains("Heading1") {
                1
            } else if para.contains("Heading2") {
                2
            } else {
                3
            };
            blocks.push(DocumentBlock::Heading {
                level,
                text: trimmed.to_owned(),
            });
        } else if para.contains("w:numPr") {
            blocks.push(DocumentBlock::Item {
                depth: 0,
                text: trimmed.to_owned(),
            });
        } else {
            blocks.push(DocumentBlock::Paragraph {
                text: trimmed.to_owned(),
            });
        }
    }
}

fn text_runs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<w:t") {
        let after = &rest[start..];
        let Some(gt) = after.find('>') else { break };
        let content = &after[gt + 1..];
        let Some(end) = content.find("</w:t>") else { break };
        out.push(content[..end].to_owned());
        rest = &content[end + 6..];
    }
    // pptx uses a:t
    if out.is_empty() {
        rest = xml;
        while let Some(start) = rest.find("<a:t") {
            let after = &rest[start..];
            let Some(gt) = after.find('>') else { break };
            let content = &after[gt + 1..];
            let Some(end) = content.find("</a:t>") else { break };
            out.push(content[..end].to_owned());
            rest = &content[end + 6..];
        }
    }
    out
}

fn first_text_run(xml: &str) -> Option<String> {
    text_runs(xml).into_iter().find(|t| !t.trim().is_empty())
}

/// Read an `.xlsx` with calamine: the values the file holds, never a formula worked out. A formula the file holds no
/// result for is its formula, as written (`a-workbook-is-read-for-its-cells`).
pub fn read_xlsx(bytes: &[u8]) -> Result<Workbook, String> {
    if bytes.len() as u64 > MAX_OFFICE_FILE_BYTES {
        return Err("That workbook is too large to show here.".into());
    }
    let mut workbook =
        calamine::open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    use calamine::{Data, Reader};
    let names = workbook.sheet_names().to_vec();
    let more_sheets = names.len().saturating_sub(MAX_SHEETS);
    let mut sheets = Vec::new();
    for name in names.into_iter().take(MAX_SHEETS) {
        let Ok(range) = workbook.worksheet_range(&name) else {
            continue;
        };
        let formulas = workbook.worksheet_formula(&name).unwrap_or_default();
        // Where something is, in the sheet's own places (A1 is row 0, column 0), wherever the stored range starts.
        let mut known_rows = 0usize;
        let mut known_columns = 0usize;
        let mut mark = |(r, c): (u32, u32)| {
            known_rows = known_rows.max(r as usize + 1);
            known_columns = known_columns.max(c as usize + 1);
        };
        if let Some((r0, c0)) = range.start() {
            for (r, c, v) in range.used_cells() {
                if !matches!(v, Data::Empty) {
                    mark((r0 + r as u32, c0 + c as u32));
                }
            }
        }
        if let Some((r0, c0)) = formulas.start() {
            for (r, c, f) in formulas.used_cells() {
                if !f.is_empty() {
                    mark((r0 + r as u32, c0 + c as u32));
                }
            }
        }
        let grid: Vec<Vec<SheetCell>> = (0..known_rows.min(MAX_SHEET_ROWS))
            .map(|r| {
                (0..known_columns.min(MAX_SHEET_COLUMNS))
                    .map(|c| {
                        let at = (r as u32, c as u32);
                        let (text, kind) = match range.get_value(at) {
                            Some(Data::Empty) | None => match formulas.get_value(at).filter(|f| !f.is_empty()) {
                                Some(f) if f.starts_with('=') => (f.clone(), SheetCellKind::Formula),
                                Some(f) => (format!("={f}"), SheetCellKind::Formula),
                                None => (String::new(), SheetCellKind::Text),
                            },
                            Some(Data::String(t)) => (t.clone(), SheetCellKind::Text),
                            Some(Data::Bool(b)) => (if *b { "TRUE" } else { "FALSE" }.to_owned(), SheetCellKind::Boolean),
                            Some(Data::Error(e)) => (e.to_string(), SheetCellKind::Error),
                            Some(other) => (other.to_string(), SheetCellKind::Number),
                        };
                        SheetCell { text: cell_text(&text), kind }
                    })
                    .collect()
            })
            .collect();
        sheets.push(bounded_table(&name, grid, Some(known_rows), Some(known_columns)));
    }
    Ok(Workbook { sheets, more_sheets })
}

/// A CSV or TSV file as a one-sheet workbook (`csvWorkbook`): read up to one row past the bound, and the rows past
/// that counted from the line breaks left over (an estimate only when a quoted field holds a line break).
pub fn read_csv(text: &str, tsv: bool) -> Workbook {
    let parsed = parse_csv(text, if tsv { '\t' } else { ',' }, MAX_SHEET_ROWS + 1);
    let more_lines = if parsed.len() > MAX_SHEET_ROWS { text.split('\n').count().saturating_sub(parsed.len()) } else { 0 };
    let known = parsed.len() + more_lines;
    let grid = parsed
        .into_iter()
        .map(|row| {
            row.iter()
                .map(|value| {
                    let text = cell_text(value);
                    let kind = if looks_numeric(&text) { SheetCellKind::Number } else { SheetCellKind::Text };
                    SheetCell { text, kind }
                })
                .collect()
        })
        .collect();
    Workbook { sheets: vec![bounded_table("Sheet1", grid, Some(known), None)], more_sheets: 0 }
}

/// Write/refresh office-words copies under `folder` (native data only) and return the brief section.
pub fn office_words_section(folder: &Path) -> Option<String> {
    let files = office_files(folder);
    if files.is_empty() {
        return None;
    }
    let mut kept = Vec::new();
    for path in files {
        let source = folder.join(&path);
        let Ok(meta) = std::fs::metadata(&source) else {
            continue;
        };
        if meta.len() > MAX_OFFICE_FILE_BYTES {
            continue;
        }
        let copy = format!("{OFFICE_WORDS_DIR}/{path}.md");
        let copy_path = folder.join(&copy);
        let refresh = match std::fs::metadata(&copy_path) {
            Ok(held) => {
                let src_m = meta.modified().ok();
                let held_m = held.modified().ok();
                match (src_m, held_m) {
                    (Some(s), Some(h)) => s > h,
                    _ => true,
                }
            }
            Err(_) => true,
        };
        if refresh {
            let Ok(bytes) = std::fs::read(&source) else {
                continue;
            };
            let Some(kind) = office_kind(&path) else {
                continue;
            };
            let Ok(doc) = read_office(&bytes, kind) else {
                continue;
            };
            if let Some(parent) = copy_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&copy_path, document_words(&path, &doc));
        }
        kept.push(format!("- {path}: {copy}"));
    }
    if kept.is_empty() {
        return None;
    }
    Some(format!(
        "Word and PowerPoint files in this folder, as text Locust already read (a copy under {OFFICE_WORDS_DIR}/; editing a copy changes nothing in the document):\n{}",
        kept.join("\n")
    ))
}

fn office_files(folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    let mut names: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    names.sort_by_key(|e| e.file_name());
    for entry in names {
        if found.len() >= MAX_FILES {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_file() && office_kind(&name).is_some() {
            found.push(name);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_words_header_and_blocks() {
        let doc = OfficeDocument {
            blocks: vec![
                DocumentBlock::Heading {
                    level: 1,
                    text: "Title".into(),
                },
                DocumentBlock::Paragraph {
                    text: "Hello".into(),
                },
                DocumentBlock::Item {
                    depth: 0,
                    text: "Bullet".into(),
                },
                DocumentBlock::Slide {
                    number: 1,
                    title: Some("Intro".into()),
                },
                DocumentBlock::Table {
                    rows: vec![vec!["A".into(), "B".into()], vec!["1".into(), "2".into()]],
                },
            ],
            more: 2,
        };
        let words = document_words("Brief.docx", &doc);
        assert!(words.starts_with("Words of Brief.docx, read by Locust."));
        assert!(words.contains("A copy: editing it changes nothing"));
        assert!(words.contains("# Title"));
        assert!(words.contains("Hello"));
        assert!(words.contains("- Bullet"));
        assert!(words.contains("## Slide 1: Intro"));
        assert!(words.contains("| A | B |"));
        assert!(words.contains("2 more parts"));
    }

    #[test]
    fn csv_bounds_and_xlsx_empty_refuse() {
        let book = read_csv("a,b\n1,2\n", false);
        assert_eq!(book.sheets[0].name, "Sheet1");
        assert_eq!(book.sheets[0].rows[0][0].text, "a");
        assert!(read_xlsx(b"not a zip").is_err());
    }

    #[test]
    fn office_kind_skips_lock_files() {
        assert_eq!(office_kind("Notes.docx"), Some("docx"));
        assert_eq!(office_kind("Deck.pptx"), Some("pptx"));
        assert_eq!(office_kind("~$Notes.docx"), None);
    }

    #[test]
    fn bounds_match_electron() {
        assert_eq!(MAX_OFFICE_FILE_BYTES, 40 * 1024 * 1024);
        assert_eq!(MAX_SHEETS, 12);
        assert_eq!(MAX_SHEET_ROWS, 500);
        assert_eq!(MAX_SHEET_COLUMNS, 40);
        assert_eq!(MAX_FILES, 12);
    }

    #[test]
    fn reads_minimal_docx_and_writes_office_words() {
        // Minimal ZIP with word/document.xml (Electron office-text approach).
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("word/document.xml", opts).unwrap();
            use std::io::Write;
            zip.write_all(
                br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello office</w:t></w:r></w:p></w:body></w:document>"#,
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let bytes = cursor.into_inner();
        let doc = read_office(&bytes, "docx").expect("docx");
        assert!(
            doc.blocks.iter().any(|b| matches!(b, DocumentBlock::Paragraph { text } if text.contains("Hello office"))),
            "{doc:?}"
        );

        let dir = std::env::temp_dir().join(format!("p14-office-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Brief.docx"), &bytes).unwrap();
        let section = office_words_section(&dir).expect("section");
        assert!(section.contains("Brief.docx"));
        assert!(section.contains(OFFICE_WORDS_DIR));
        assert!(dir.join(OFFICE_WORDS_DIR).join("Brief.docx.md").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
