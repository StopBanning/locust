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

#[derive(Clone, Debug, PartialEq)]
pub struct SheetCell {
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Workbook {
    pub sheets: Vec<(String, Vec<Vec<SheetCell>>)>,
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

/// Read an `.xlsx` with calamine (values only; formulas are not evaluated).
pub fn read_xlsx(bytes: &[u8]) -> Result<Workbook, String> {
    if bytes.len() as u64 > MAX_OFFICE_FILE_BYTES {
        return Err("That workbook is too large to show here.".into());
    }
    let mut workbook =
        calamine::open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    use calamine::Reader;
    let names = workbook.sheet_names().to_vec();
    if names.len() > MAX_SHEETS {
        return Err("That workbook is too large to show here.".into());
    }
    let mut sheets = Vec::new();
    for name in names {
        let Ok(range) = workbook.worksheet_range(&name) else {
            continue;
        };
        let mut rows = Vec::new();
        for (r_i, row) in range.rows().enumerate() {
            if r_i >= MAX_SHEET_ROWS {
                break;
            }
            let cells: Vec<SheetCell> = row
                .iter()
                .take(MAX_SHEET_COLUMNS)
                .map(|c| SheetCell {
                    text: match c {
                        calamine::Data::Empty => String::new(),
                        other => other.to_string(),
                    },
                })
                .collect();
            rows.push(cells);
        }
        sheets.push((name, rows));
    }
    Ok(Workbook { sheets })
}

/// Read CSV/TSV text into a single-sheet workbook.
pub fn read_csv(text: &str, tsv: bool) -> Workbook {
    let sep = if tsv { '\t' } else { ',' };
    let mut rows = Vec::new();
    for (r_i, line) in text.lines().enumerate() {
        if r_i >= MAX_SHEET_ROWS {
            break;
        }
        let cells: Vec<SheetCell> = line
            .split(sep)
            .take(MAX_SHEET_COLUMNS)
            .map(|c| SheetCell {
                text: c.trim().to_owned(),
            })
            .collect();
        rows.push(cells);
    }
    Workbook {
        sheets: vec![("Sheet1".into(), rows)],
    }
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
        assert_eq!(book.sheets[0].0, "Sheet1");
        assert_eq!(book.sheets[0].1[0][0].text, "a");
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
