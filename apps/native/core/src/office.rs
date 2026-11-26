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

/// Which reader a document came from (Electron `OfficeDocument.kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Word,
    Slides,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OfficeDocument {
    pub kind: DocumentKind,
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
pub(crate) fn looks_numeric(text: &str) -> bool {
    static NUMERIC: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^[-+(]?[$€£¥]?\s?-?(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?\)?%?$").unwrap()
    });
    NUMERIC.is_match(text.trim())
}

/// RFC 4180 CSV (`parseCsv`): quoted fields, doubled quotes inside them, line breaks inside quotes, CRLF or LF. Stops
/// once it has `limit_rows` rows, so a huge file costs what the viewer shows.
pub(crate) fn parse_csv(text: &str, delimiter: char, limit_rows: usize) -> Vec<Vec<String>> {
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
        // Electron's `replace(/\n{3,}/g, '\n\n')`: at most one blank line between blocks.
        if line.is_empty() {
            blank += 1;
            if blank <= 1 {
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

/// Electron `shared/office-document.ts` `MAX_DOCUMENT_BLOCKS`: at most this many blocks are drawn; the rest is said.
pub const MAX_DOCUMENT_BLOCKS: usize = 4_000;
/// Electron `MAX_DOCUMENT_TABLE_ROWS` and `MAX_DOCUMENT_TABLE_COLUMNS`: a table keeps at most this many of each.
pub const MAX_DOCUMENT_TABLE_ROWS: usize = 200;
pub const MAX_DOCUMENT_TABLE_COLUMNS: usize = 20;
/// Electron `MAX_DOCUMENT_TEXT`: a block's text past this is cut, and says so.
pub const MAX_DOCUMENT_TEXT: usize = 4_000;

const WORD: &str = "Word document";
const SLIDES: &str = "presentation";

fn regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("a pattern of this file")
}

macro_rules! pattern {
    ($text:expr) => {{
        static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex($text));
        &*PATTERN
    }};
}

/// A block's text, bounded (Electron `documentText`): control characters out, long text keeps its start and says it
/// was cut. JavaScript counts UTF-16 units, so the bound does too.
fn document_text(text: &str) -> String {
    let flat: String = text.chars().filter(|c| !matches!(*c as u32, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f)).collect();
    if flat.encode_utf16().count() <= MAX_DOCUMENT_TEXT {
        return flat;
    }
    let mut kept = String::new();
    let mut units = 0;
    for c in flat.chars() {
        units += c.len_utf16();
        if units > MAX_DOCUMENT_TEXT {
            break;
        }
        kept.push(c);
    }
    format!("{kept}… (cut here)")
}

/// The five XML entities and numeric references (Electron `decodeXml`). No DTD is ever read, so nothing expands.
fn decode_xml(text: &str) -> String {
    pattern!(r"&(#x[0-9a-fA-F]+|#\d+|lt|gt|amp|quot|apos);")
        .replace_all(text, |c: &regex::Captures| {
            let name = &c[1];
            match name {
                "lt" => "<".to_owned(),
                "gt" => ">".to_owned(),
                "amp" => "&".to_owned(),
                "quot" => "\"".to_owned(),
                "apos" => "'".to_owned(),
                _ => {
                    let code = match name.strip_prefix("#x") {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => name[1..].parse::<u32>().ok(),
                    };
                    code.filter(|c| *c > 0).and_then(char::from_u32).map_or_else(|| c[0].to_owned(), String::from)
                }
            }
        })
        .into_owned()
}

/// An attribute's value from an element's attribute text (Electron `attribute`), any namespace prefix allowed.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let found = regex(&format!(r#"(?:^|\s)(?:[\w-]+:)?{}="([^"]*)""#, regex::escape(name))).captures(attributes)?;
    Some(decode_xml(&found[1]))
}

/// A zip's parts, bounded the way Electron's `zipDirectory` bounds them. `noun` is what the person calls the file.
fn open_zip<'a>(bytes: &'a [u8], noun: &str) -> Result<zip::ZipArchive<Cursor<&'a [u8]>>, String> {
    let zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| format!("That file is not a {noun} Locust can read."))?;
    if zip.len() > MAX_ENTRIES {
        return Err(format!("That {noun} is too large to show here."));
    }
    Ok(zip)
}

/// One part's text, or `None` when the file has no such part (Electron `partText`): a part that claims more than it may
/// inflate to is refused before it is inflated, and one that inflates past the cap is refused too.
fn part_text(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str, noun: &str) -> Result<Option<String>, String> {
    let Ok(mut part) = zip.by_name(name.trim_start_matches('/')) else {
        return Ok(None);
    };
    if part.size() > MAX_PART_BYTES {
        return Err(format!("That {noun} is too large to show here."));
    }
    let mut raw = Vec::new();
    (&mut part)
        .take(MAX_PART_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|_| format!("That {noun} is too large or damaged to show here."))?;
    if raw.len() as u64 > MAX_PART_BYTES {
        return Err(format!("That {noun} is too large or damaged to show here."));
    }
    Ok(Some(String::from_utf8_lossy(&raw).into_owned()))
}

/// Where an element that may nest (a table in a table) closes: the index after its end tag (Electron `closingAt`).
fn closing_at(xml: &str, tag: &str, from: usize) -> usize {
    let pattern = regex(&format!(r"<{tag}\b[^>]*?(/)?>|</{tag}>"));
    let mut depth = 1;
    let mut at = from;
    while let Some(found) = pattern.captures_at(xml, at) {
        let whole = found.get(0).expect("a match");
        at = whole.end();
        if whole.as_str().starts_with("</") {
            depth -= 1;
            if depth == 0 {
                return at;
            }
        } else if found.get(1).is_none() {
            depth += 1;
        }
    }
    xml.len()
}

/// The paragraphs (`true`) and tables (`false`) directly in a Word body or cell, in order (Electron `wordChildren`).
fn word_children(xml: &str) -> Vec<(bool, &str)> {
    let pattern = pattern!(r"<w:(tbl|p)\b[^>]*?(/)?>");
    let mut children = Vec::new();
    let mut at = 0;
    while let Some(found) = pattern.captures_at(xml, at) {
        let whole = found.get(0).expect("a match");
        at = whole.end();
        if found.get(2).is_some() {
            continue;
        }
        let kind = &found[1];
        let end = closing_at(xml, &format!("w:{kind}"), at);
        children.push((kind == "p", &xml[whole.start()..end]));
        at = end;
    }
    children
}

/// A Word paragraph's words (Electron `wordParagraphText`): its runs' text, tabs and breaks; deleted text and field
/// codes left out. The empty run `<w:t/>` is matched first, so it never swallows the XML up to the next `</w:t>`.
fn word_paragraph_text(paragraph: &str) -> String {
    let runs = pattern!(r"(?s)<w:pPr\b.*?</w:pPr>").replace_all(paragraph, "");
    let mut text = String::new();
    for found in pattern!(r"(?s)<w:t\b[^>]*/>|<w:t\b[^>]*>(.*?)</w:t>|<w:(tab|br|cr)\b[^>]*/>").captures_iter(&runs) {
        match found.get(2).map(|m| m.as_str()) {
            Some("tab") => text.push('\t'),
            Some(_) => text.push('\n'),
            None => text.push_str(&decode_xml(found.get(1).map_or("", |m| m.as_str()))),
        }
    }
    text.trim().to_owned()
}

fn word_table(table: &str) -> Vec<Vec<String>> {
    pattern!(r"(?s)<w:tr\b.*?</w:tr>")
        .find_iter(table)
        .map(|row| {
            pattern!(r"(?s)<w:tc\b.*?</w:tc>")
                .find_iter(row.as_str())
                .map(|cell| {
                    let words: Vec<String> = word_children(cell.as_str())
                        .into_iter()
                        .filter(|(paragraph, _)| *paragraph)
                        .map(|(_, xml)| word_paragraph_text(xml))
                        .filter(|t| !t.is_empty())
                        .collect();
                    document_text(&words.join(" "))
                })
                .collect()
        })
        .collect()
}

/// A table, bounded; a cut is said in the paragraph after it (Electron `tableBlocks`).
fn table_blocks(rows: Vec<Vec<String>>) -> Vec<DocumentBlock> {
    if rows.is_empty() {
        return Vec::new();
    }
    let total = rows.len();
    let widest = rows.iter().map(Vec::len).max().unwrap_or(0);
    let kept = rows
        .into_iter()
        .take(MAX_DOCUMENT_TABLE_ROWS)
        .map(|row| row.into_iter().take(MAX_DOCUMENT_TABLE_COLUMNS).collect())
        .collect();
    let mut blocks = vec![DocumentBlock::Table { rows: kept }];
    if total > MAX_DOCUMENT_TABLE_ROWS || widest > MAX_DOCUMENT_TABLE_COLUMNS {
        blocks.push(DocumentBlock::Paragraph {
            text: format!(
                "(This table has {total} rows and {widest} columns; the first {} rows and {} columns are shown.)",
                total.min(MAX_DOCUMENT_TABLE_ROWS),
                widest.min(MAX_DOCUMENT_TABLE_COLUMNS)
            ),
        });
    }
    blocks
}

fn bounded(kind: DocumentKind, mut blocks: Vec<DocumentBlock>) -> OfficeDocument {
    let more = blocks.len().saturating_sub(MAX_DOCUMENT_BLOCKS);
    blocks.truncate(MAX_DOCUMENT_BLOCKS);
    OfficeDocument { kind, blocks, more }
}

/// A Word document's headings, paragraphs, list items and tables (Electron `readDocx`), or a sentence for the person.
pub(crate) fn read_docx(bytes: &[u8]) -> Result<OfficeDocument, String> {
    let mut zip = open_zip(bytes, WORD)?;
    let Some(xml) = part_text(&mut zip, "word/document.xml", WORD)? else {
        return Err(format!("That file is not a {WORD} Locust can read."));
    };
    let body = pattern!(r"(?s)<w:body\b[^>]*>(.*)</w:body>").captures(&xml).map_or("", |c| c.get(1).map_or("", |m| m.as_str()));
    let mut blocks = Vec::new();
    for (paragraph, child) in word_children(body) {
        if blocks.len() > MAX_DOCUMENT_BLOCKS {
            break;
        }
        if !paragraph {
            blocks.extend(table_blocks(word_table(child)));
            continue;
        }
        let text = word_paragraph_text(child);
        if text.is_empty() {
            continue;
        }
        let properties = pattern!(r"(?s)<w:pPr\b.*?</w:pPr>").find(child).map_or("", |m| m.as_str());
        let value_of = |element: &regex::Regex| {
            element.captures(properties).and_then(|c| attribute(c.get(1).map_or("", |m| m.as_str()), "val"))
        };
        let style = value_of(pattern!(r"<w:pStyle\b([^>]*?)/?>")).unwrap_or_default();
        let outline = value_of(pattern!(r"<w:outlineLvl\b([^>]*?)/?>"));
        let heading_level: Option<f64> = if style.eq_ignore_ascii_case("title") {
            Some(1.0)
        } else if let Some(digit) = pattern!(r"(?i)^heading(\d)$").captures(&style) {
            digit[1].parse().ok()
        } else {
            outline.map(|o| o.trim().parse::<f64>().map_or(f64::NAN, |n| n + 1.0))
        };
        if let Some(level) = heading_level.filter(|l| l.is_finite()) {
            blocks.push(DocumentBlock::Heading { level: level.clamp(1.0, 3.0) as usize, text: document_text(&text) });
        } else if pattern!(r"<w:numPr\b").is_match(properties)
            || pattern!(r"(?i)^List(Bullet|Number|Paragraph|Continue)\d*$").is_match(&style)
        {
            // A list item by its own numbering, or by its style: python-docx's "List Bullet 2" carries its bullet in
            // the style, not the paragraph.
            let level = value_of(pattern!(r"<w:ilvl\b([^>]*?)/?>")).unwrap_or_else(|| {
                let last: f64 = pattern!(r"(\d)$").captures(&style).map_or(1.0, |c| c[1].parse().unwrap_or(1.0));
                (last - 1.0).max(0.0).to_string()
            });
            let depth = level.trim().parse::<f64>().ok().filter(|d| d.is_finite()).map_or(0, |d| d.clamp(0.0, 4.0) as usize);
            blocks.push(DocumentBlock::Item { depth, text: document_text(&text) });
        } else {
            blocks.push(DocumentBlock::Paragraph { text: document_text(&text) });
        }
    }
    Ok(bounded(DocumentKind::Word, blocks))
}

/// A PowerPoint paragraph's words: runs and line breaks (Electron `slideParagraphText`).
fn slide_paragraph_text(paragraph: &str) -> String {
    let mut text = String::new();
    for found in pattern!(r"(?s)<a:t\b[^>]*/>|<a:t\b[^>]*>(.*?)</a:t>|<a:br\b[^>]*/?>").captures_iter(paragraph) {
        if found[0].starts_with("<a:t") {
            text.push_str(&decode_xml(found.get(1).map_or("", |m| m.as_str())));
        } else {
            text.push(' ');
        }
    }
    pattern!(r"\s+").replace_all(&text, " ").trim().to_owned()
}

/// A deck's slides, in the deck's order: each one's title, then its text and tables (Electron `readPptx`).
pub(crate) fn read_pptx(bytes: &[u8]) -> Result<OfficeDocument, String> {
    let mut zip = open_zip(bytes, SLIDES)?;
    let Some(presentation) = part_text(&mut zip, "ppt/presentation.xml", SLIDES)? else {
        return Err(format!("That file is not a {SLIDES} Locust can read."));
    };
    let rels = part_text(&mut zip, "ppt/_rels/presentation.xml.rels", SLIDES)?.unwrap_or_default();
    let mut targets = std::collections::HashMap::new();
    for found in pattern!(r"<(?:\w+:)?Relationship\b([^>]*?)/?>").captures_iter(&rels) {
        let attributes = found.get(1).map_or("", |m| m.as_str());
        if let (Some(id), Some(target)) = (attribute(attributes, "Id"), attribute(attributes, "Target")) {
            let target = match target.strip_prefix('/') {
                Some(rooted) => rooted.to_owned(),
                None => format!("ppt/{target}"),
            };
            targets.insert(id, target);
        }
    }
    let order: Vec<Option<String>> = pattern!(r"<p:sldId\b([^>]*?)/?>")
        .captures_iter(&presentation)
        .map(|found| pattern!(r#"\br:id="([^"]*)""#).captures(found.get(1).map_or("", |m| m.as_str())).map(|c| c[1].to_owned()))
        .collect();
    let mut blocks = Vec::new();
    for (index, id) in order.iter().enumerate() {
        if blocks.len() > MAX_DOCUMENT_BLOCKS {
            break;
        }
        let xml = match id.as_ref().and_then(|id| targets.get(id)) {
            Some(target) => part_text(&mut zip, target, SLIDES)?.unwrap_or_default(),
            None => String::new(),
        };
        let mut title: Option<String> = None;
        let mut inside = Vec::new();
        for shape in pattern!(r"(?s)<p:sp\b.*?</p:sp>|<p:graphicFrame\b.*?</p:graphicFrame>").find_iter(&xml) {
            let part = shape.as_str();
            let paragraphs_of = |xml: &str| pattern!(r"(?s)<a:p\b.*?</a:p>").find_iter(xml).map(|p| p.as_str().to_owned()).collect::<Vec<_>>();
            if part.starts_with("<p:graphicFrame") {
                let rows = pattern!(r"(?s)<a:tr\b.*?</a:tr>")
                    .find_iter(part)
                    .map(|row| {
                        pattern!(r"(?s)<a:tc\b.*?</a:tc>")
                            .find_iter(row.as_str())
                            .map(|cell| {
                                let words: Vec<String> = paragraphs_of(cell.as_str())
                                    .iter()
                                    .map(|p| slide_paragraph_text(p))
                                    .filter(|t| !t.is_empty())
                                    .collect();
                                document_text(&words.join(" "))
                            })
                            .collect()
                    })
                    .collect();
                inside.extend(table_blocks(rows));
                continue;
            }
            let kind = pattern!(r"<p:ph\b([^>]*?)/?>")
                .captures(part)
                .map(|ph| attribute(ph.get(1).map_or("", |m| m.as_str()), "type").unwrap_or_else(|| "body".to_owned()));
            // The slide number, date and footer say nothing a reader needs.
            if matches!(kind.as_deref(), Some("sldNum" | "dt" | "ftr")) {
                continue;
            }
            let paragraphs: Vec<(String, f64)> = paragraphs_of(part)
                .iter()
                .map(|p| {
                    let depth = pattern!(r"<a:pPr\b([^>]*?)/?>")
                        .captures(p)
                        .and_then(|c| attribute(c.get(1).map_or("", |m| m.as_str()), "lvl"))
                        .unwrap_or_else(|| "0".to_owned());
                    (slide_paragraph_text(p), depth.trim().parse::<f64>().unwrap_or(f64::NAN))
                })
                .filter(|(text, _)| !text.is_empty())
                .collect();
            if matches!(kind.as_deref(), Some("title" | "ctrTitle")) {
                if title.is_none() && !paragraphs.is_empty() {
                    title = Some(document_text(&paragraphs.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join(" ")));
                }
                continue;
            }
            for (text, depth) in paragraphs {
                inside.push(if matches!(kind.as_deref(), Some("body" | "obj")) {
                    let depth = if depth.is_finite() { depth.clamp(0.0, 4.0) as usize } else { 0 };
                    DocumentBlock::Item { depth, text: document_text(&text) }
                } else {
                    DocumentBlock::Paragraph { text: document_text(&text) }
                });
            }
        }
        blocks.push(DocumentBlock::Slide { number: index + 1, title });
        blocks.extend(inside);
    }
    Ok(bounded(DocumentKind::Slides, blocks))
}

/// Read a `.docx` or `.pptx` from bytes into structured blocks, the way Electron's `office-text.ts` does.
pub fn read_office(bytes: &[u8], kind: &str) -> Result<OfficeDocument, String> {
    if bytes.len() as u64 > MAX_OFFICE_FILE_BYTES {
        return Err("That document is too large to show here.".into());
    }
    match kind {
        "docx" => read_docx(bytes),
        "pptx" => read_pptx(bytes),
        _ => Err("Not an office document Locust reads.".into()),
    }
}

/// A zip part may inflate to at most this (Electron `xlsx.ts` `MAX_PART_BYTES`): a zip bomb stops here, not in memory.
const MAX_PART_BYTES: u64 = 24 * 1024 * 1024;
/// Electron `xlsx.ts` `MAX_ENTRIES`.
const MAX_ENTRIES: usize = 4_000;

/// The zip's bounds, checked before calamine reads a byte of it (Electron `zipDirectory` and `partText`): too many
/// entries, a part that claims more than it may inflate to, or one that inflates past the cap while claiming less, are
/// refused in words. `noun` is what the person calls the file.
fn checked_zip<'a>(bytes: &'a [u8], noun: &str) -> Result<zip::ZipArchive<Cursor<&'a [u8]>>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| format!("That file is not a {noun} Locust can read."))?;
    if zip.len() > MAX_ENTRIES {
        return Err(format!("That {noun} is too large to show here."));
    }
    for index in 0..zip.len() {
        let mut part = zip.by_index(index).map_err(|_| format!("That {noun} is damaged."))?;
        if part.size() > MAX_PART_BYTES {
            return Err(format!("That {noun} is too large to show here."));
        }
        let inflated = std::io::copy(&mut (&mut part).take(MAX_PART_BYTES + 1), &mut std::io::sink())
            .map_err(|_| format!("That {noun} is too large or damaged to show here."))?;
        if inflated > MAX_PART_BYTES {
            return Err(format!("That {noun} is too large or damaged to show here."));
        }
    }
    Ok(zip)
}

/// The relationships calamine needs and Electron never asks for, added where a file lacks them, so calamine reads what
/// Electron reads: calamine finds the workbook through the package's own `_rels/.rels` (Electron reads
/// `xl/workbook.xml` by name), and takes a sheet only from a relationship typed as one (Electron takes every sheet's
/// target as its part). A file that already has both is read as it is; otherwise the other parts are copied as they
/// are, never inflated again.
fn with_package_relationships<'a>(
    bytes: &'a [u8],
    zip: &mut zip::ZipArchive<Cursor<&'a [u8]>>,
) -> Result<std::borrow::Cow<'a, [u8]>, String> {
    const WORKBOOK_RELS: &str = "xl/_rels/workbook.xml.rels";
    const WORKSHEET: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
    static UNTYPED: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"<((?:\w+:)?Relationship)\b([^>]*?)(/?)>").unwrap());
    let damaged = |_| "That workbook is damaged.".to_string();
    let package = zip.index_for_name("_rels/.rels").is_some();
    let rels = match zip.by_name(WORKBOOK_RELS) {
        Ok(mut part) => {
            let mut text = String::new();
            part.read_to_string(&mut text).map_err(|_| "That workbook is damaged.".to_string())?;
            Some(text)
        }
        Err(_) => None,
    };
    let typed = rels.as_deref().map(|text| {
        UNTYPED
            .replace_all(text, |c: &regex::Captures| {
                if attribute_present(&c[2], "Type") {
                    c[0].to_owned()
                } else {
                    format!(r#"<{}{} Type="{WORKSHEET}"{}>"#, &c[1], &c[2], &c[3])
                }
            })
            .into_owned()
    });
    if package && typed == rels {
        return Ok(std::borrow::Cow::Borrowed(bytes));
    }
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..zip.len() {
        let part = zip.by_index_raw(index).map_err(damaged)?;
        if typed != rels && part.name() == WORKBOOK_RELS {
            continue;
        }
        out.raw_copy_file(part).map_err(damaged)?;
    }
    let mut add = |name: &str, text: &[u8]| -> Result<(), String> {
        out.start_file(name, zip::write::SimpleFileOptions::default()).map_err(damaged)?;
        std::io::Write::write_all(&mut out, text).map_err(|_| "That workbook is damaged.".to_string())
    };
    if let (Some(text), true) = (&typed, typed != rels) {
        add(WORKBOOK_RELS, text.as_bytes())?;
    }
    if !package {
        add(
            "_rels/.rels",
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        )?;
    }
    Ok(std::borrow::Cow::Owned(out.finish().map_err(damaged)?.into_inner()))
}

/// Whether an element's attribute text names this attribute, with or without a namespace prefix.
fn attribute_present(attributes: &str, name: &str) -> bool {
    attributes.split_whitespace().any(|a| a.split('=').next().is_some_and(|key| key.rsplit(':').next() == Some(name)))
}

/// Electron `BUILTIN_DATE_FORMATS`: the built-in number formats that are dates.
const BUILTIN_DATE_FORMATS: [u32; 12] = [14, 15, 16, 17, 18, 19, 20, 21, 22, 45, 46, 47];

/// Electron `BUILTIN_FORMATS`: the built-in number formats it draws by their code; any other built-in is `General`.
fn builtin_format(id: u32) -> Option<&'static str> {
    match id {
        1 => Some("0"),
        2 => Some("0.00"),
        3 => Some("#,##0"),
        4 => Some("#,##0.00"),
        9 => Some("0%"),
        10 => Some("0.00%"),
        _ => None,
    }
}

/// A format code with its quoted text, `[colour]` sections and escaped characters taken out.
fn bare_format(code: &str) -> String {
    static QUOTED: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r#""[^"]*"|\[[^\]]*\]|\\."#).unwrap());
    QUOTED.replace_all(code, "").into_owned()
}

/// How a style shows a number (Electron `stylesOf`): as a date, or with the format code it names. calamine gives the
/// code it knows for every built-in; Electron knows six, and draws any other built-in as `General`.
fn number_style(format: Option<&calamine::NumberFormat>) -> (bool, Option<String>) {
    let Some(format) = format else { return (false, None) };
    let id = format.format_id.unwrap_or(0);
    let code = if id >= 164 { Some(format.format_code.clone()) } else { builtin_format(id).map(str::to_owned) };
    let bare = bare_format(code.as_deref().unwrap_or(""));
    let lower = bare.to_ascii_lowercase();
    let date = BUILTIN_DATE_FORMATS.contains(&id)
        || lower.contains('y')
        || lower.contains('d')
        || (lower.contains('m') && !lower.contains('h') && !lower.contains('s'));
    (date, code)
}

/// An Excel date serial as a date a person reads (Electron `excelDate`): 2026-09-26, with the time when it has one.
fn excel_date(serial: f64, from_1904: bool) -> String {
    let epoch = if from_1904 {
        chrono::NaiveDate::from_ymd_opt(1904, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
    }
    .and_then(|d| d.and_hms_opt(0, 0, 0));
    let Some(at) = epoch.and_then(|e| e.checked_add_signed(chrono::TimeDelta::milliseconds((serial * 86_400_000.0).round() as i64)))
    else {
        return js_number(serial);
    };
    let day = at.format("%Y-%m-%d").to_string();
    let time = at.format("%H:%M").to_string();
    if serial.fract() == 0.0 || time == "00:00" { day } else { format!("{day} {time}") }
}

/// A number as JavaScript writes `String(Number(value.toPrecision(12)))`: twelve significant digits, no trailing zeros.
fn js_number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let rounded: f64 = format!("{value:.11e}").parse().unwrap_or(value);
    if rounded == 0.0 {
        return "0".into();
    }
    let magnitude = rounded.abs();
    if (1e-7..1e21).contains(&magnitude) {
        return rounded.to_string();
    }
    let text = format!("{rounded:e}");
    match text.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') => format!("{mantissa}e+{exponent}"),
        _ => text,
    }
}

/// `x` to `decimals` places the way `Intl.NumberFormat` writes it (Electron `formatNumber`): from the shortest decimal
/// that reads back as `x`, half rounded up. Rust's own `{:.n}` rounds the exact binary value, so it writes 0.285 as
/// 0.28 and 12.5 as 12 where Electron writes 0.29 and 13.
fn fixed_half_up(x: f64, decimals: usize) -> String {
    if !x.is_finite() {
        return format!("{x:.decimals$}");
    }
    let shortest = x.to_string();
    let (whole, fraction) = shortest.split_once('.').unwrap_or((shortest.as_str(), ""));
    let fraction = format!("{fraction:0<width$}", width = decimals + 1);
    let mut digits: Vec<u8> = whole.bytes().chain(fraction.bytes().take(decimals)).collect();
    if fraction.as_bytes()[decimals] >= b'5' {
        let mut at = digits.len();
        loop {
            if at == 0 {
                digits.insert(0, b'1');
                break;
            }
            at -= 1;
            if digits[at] == b'9' {
                digits[at] = b'0';
            } else {
                digits[at] += 1;
                break;
            }
        }
    }
    let point = digits.len() - decimals;
    let (whole, fraction) = digits.split_at(point);
    let whole = String::from_utf8_lossy(whole);
    if decimals == 0 { whole.into_owned() } else { format!("{whole}.{}", String::from_utf8_lossy(fraction)) }
}

/// A number the way its style asks, within reason (Electron `formatNumber`): decimals, thousands, percent, a currency
/// sign.
fn format_number(value: f64, format: Option<&str>) -> String {
    let Some(format) = format.filter(|f| *f != "General") else {
        return js_number(value);
    };
    let section = format.split(';').next().unwrap_or("");
    let bare = bare_format(section);
    let percent = bare.contains('%');
    let decimals = bare
        .find('.')
        .map(|at| bare[at + 1..].chars().take_while(|c| *c == '0').count())
        .unwrap_or(0);
    let grouped = bare.contains(',');
    let sign = section.chars().find(|c| matches!(c, '$' | '€' | '£' | '¥'));
    let shown = if percent { value * 100.0 } else { value };
    let fixed = fixed_half_up(shown.abs(), decimals);
    let (whole, fraction) = fixed.split_once('.').map_or((fixed.as_str(), None), |(w, f)| (w, Some(f)));
    let mut digits = String::new();
    for (at, c) in whole.chars().enumerate() {
        if grouped && at > 0 && (whole.len() - at) % 3 == 0 {
            digits.push(',');
        }
        digits.push(c);
    }
    if let Some(fraction) = fraction {
        digits.push('.');
        digits.push_str(fraction);
    }
    let body = format!("{}{digits}{}", sign.map(String::from).unwrap_or_default(), if percent { "%" } else { "" });
    if shown < 0.0 { format!("-{body}") } else { body }
}

/// Read an `.xlsx` with calamine: the values the file holds, never a formula worked out. A formula the file holds no
/// result for is its formula, as written; a number is drawn in its style's format (`a-workbook-is-read-for-its-cells`).
/// The zip's bounds are Electron's and are checked first; a file with no workbook in it is refused in words.
pub fn read_xlsx(bytes: &[u8]) -> Result<Workbook, String> {
    if bytes.len() as u64 > MAX_OFFICE_FILE_BYTES {
        return Err("That workbook is too large to show here.".into());
    }
    let mut zip = checked_zip(bytes, "workbook")?;
    if zip.index_for_name("xl/workbook.xml").is_none() {
        return Err("That file is not a workbook Locust can read.".into());
    }
    let bytes = with_package_relationships(bytes, &mut zip)?;
    use calamine::{Data, Reader};
    let mut workbook: calamine::Xlsx<_> =
        calamine::Xlsx::new(Cursor::new(bytes.as_ref())).map_err(|_| "That workbook is damaged.".to_string())?;
    let from_1904 = workbook.has_1904_epoch();
    let names = workbook.sheet_names().to_vec();
    let more_sheets = names.len().saturating_sub(MAX_SHEETS);
    let mut sheets = Vec::new();
    for name in names.into_iter().take(MAX_SHEETS) {
        let Ok(range) = workbook.worksheet_range(&name) else {
            sheets.push(bounded_table(&cell_text(&name), Vec::new(), None, None));
            continue;
        };
        let formulas = workbook.worksheet_formula(&name).unwrap_or_default();
        let styles = workbook.worksheet_style(&name).ok();
        let holds = |v: &Data| !matches!(v, Data::Empty) && !matches!(v, Data::String(s) if s.is_empty());
        // Where something is, in the sheet's own places (A1 is row 0, column 0), wherever the stored range starts.
        let mut known_rows = 0usize;
        let mut known_columns = 0usize;
        let mut mark = |(r, c): (u32, u32)| {
            known_rows = known_rows.max(r as usize + 1);
            known_columns = known_columns.max(c as usize + 1);
        };
        if let Some((r0, c0)) = range.start() {
            for (r, c, v) in range.used_cells() {
                if holds(v) {
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
                        let number = |value: f64| {
                            let (date, code) =
                                number_style(styles.as_ref().and_then(|s| s.get(at)).and_then(|s| s.number_format.as_ref()));
                            let text = if date { excel_date(value, from_1904) } else { format_number(value, code.as_deref()) };
                            (text, SheetCellKind::Number)
                        };
                        let (text, kind) = match range.get_value(at).filter(|v| holds(v)) {
                            None => match formulas.get_value(at).filter(|f| !f.is_empty()) {
                                Some(f) if f.starts_with('=') => (f.clone(), SheetCellKind::Formula),
                                Some(f) => (format!("={f}"), SheetCellKind::Formula),
                                None => (String::new(), SheetCellKind::Text),
                            },
                            Some(Data::String(t)) => (t.clone(), SheetCellKind::Text),
                            Some(Data::Bool(b)) => (if *b { "TRUE" } else { "FALSE" }.to_owned(), SheetCellKind::Boolean),
                            Some(Data::Error(e)) => (e.to_string(), SheetCellKind::Error),
                            Some(Data::Float(f)) => number(*f),
                            Some(Data::Int(i)) => number(*i as f64),
                            Some(Data::DateTime(d)) => number(d.as_f64()),
                            Some(other) => (other.to_string(), SheetCellKind::Text),
                        };
                        SheetCell { text: cell_text(&text), kind }
                    })
                    .collect()
            })
            .collect();
        sheets.push(bounded_table(&cell_text(&name), grid, Some(known_rows), Some(known_columns)));
    }
    Ok(Workbook { sheets, more_sheets })
}

/// A CSV or TSV file as a one-sheet workbook named after the file (`csvWorkbook`): read up to one row past the bound,
/// and the rows past that counted from the line breaks left over (an estimate only when a quoted field holds a line
/// break).
pub fn read_csv(name: &str, text: &str, tsv: bool) -> Workbook {
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
    // Only a file cut at the row bound needs the counted lines; otherwise the table counts its own rows, after it drops
    // blank lines at the end (Electron counted those as "more rows not shown").
    let known = (more_lines > 0).then_some(known);
    Workbook { sheets: vec![bounded_table(name, grid, known, None)], more_sheets: 0 }
}

/// Folders `officeFiles` never looks in (Electron `office-words.ts` `SKIPPED`); a folder whose name starts with a dot is
/// skipped too.
const SKIPPED: [&str; 10] = [".git", ".locust", "node_modules", ".venv", "venv", "__pycache__", "dist", "build", ".next", "target"];

/// Writes or refreshes the text copy of each Word and PowerPoint file in `folder` and one level down, and returns the
/// brief's section naming them, or `None` when the folder holds none (Electron `officeWordsSection`). Never fails: a
/// file that will not read, or a copy that will not write, is left out, and the run goes ahead without it.
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
            Ok(held) => match (meta.modified(), held.modified()) {
                (Ok(source), Ok(held)) => held < source,
                _ => true,
            },
            Err(_) => true,
        };
        if refresh {
            let written = (|| {
                let bytes = std::fs::read(&source).ok()?;
                let document = read_office(&bytes, office_kind(&path)?).ok()?;
                std::fs::create_dir_all(copy_path.parent()?).ok()?;
                std::fs::write(&copy_path, document_words(&path, &document)).ok()
            })();
            if written.is_none() {
                // Not readable as a document (damaged, or not really one), or the copy would not write: left out.
                continue;
            }
        }
        kept.push(format!("- {path}: {copy}"));
    }
    if kept.is_empty() {
        return None;
    }
    crate::attachments_for_run::keep_out_of_git(folder);
    Some(format!(
        "Word and PowerPoint files here are not plain text, so Locust keeps a text copy of the words in each. Read the copy to know what a document says; it is refreshed when the document changes, and editing a copy changes nothing in the document:\n{}",
        kept.join("\n")
    ))
}

/// The order JavaScript's `localeCompare` gives names, near enough: symbols before digits before letters, case set
/// aside until the names are otherwise equal, and then lower case first.
fn locale_key(name: &str) -> (Vec<(u8, char)>, Vec<bool>) {
    let class = |c: char| if c.is_numeric() { 1 } else if c.is_alphabetic() { 2 } else { 0 };
    (
        name.chars().map(|c| (class(c), c.to_lowercase().next().unwrap_or(c))).collect(),
        name.chars().map(char::is_uppercase).collect(),
    )
}

/// The Word and PowerPoint files in a folder and one level down, project-relative with `/` (Electron `officeFiles`),
/// at most `MAX_FILES`, in name order at each level. Word's own lock files (`~$name.docx`) are not documents.
fn office_files(folder: &Path) -> Vec<String> {
    fn look(folder: &Path, relative: &str, depth: usize, found: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(folder.join(relative)) else {
            return;
        };
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_cached_key(|e| locale_key(&e.file_name().to_string_lossy()));
        for entry in entries {
            if found.len() >= MAX_FILES {
                return;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if relative.is_empty() { name.clone() } else { format!("{relative}/{name}") };
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_file() && office_kind(&name).is_some() {
                found.push(path);
            } else if kind.is_dir() && depth == 0 && !SKIPPED.contains(&name.as_str()) && !name.starts_with('.') {
                look(folder, &path, 1, found);
            }
        }
    }
    let mut found = Vec::new();
    look(folder, "", 0, &mut found);
    found
}

#[cfg(test)]
#[path = "a_workbook_is_read_for_its_cells_tests.rs"]
mod a_workbook_is_read_for_its_cells_tests;

#[cfg(test)]
#[path = "an_office_file_is_read_for_its_words_tests.rs"]
mod an_office_file_is_read_for_its_words_tests;

#[cfg(test)]
#[path = "an_office_file_reads_in_any_mode_tests.rs"]
mod an_office_file_reads_in_any_mode_tests;

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
            kind: DocumentKind::Word,
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
        let book = read_csv("a.csv", "a,b\n1,2\n", false);
        assert_eq!(book.sheets[0].name, "a.csv");
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
