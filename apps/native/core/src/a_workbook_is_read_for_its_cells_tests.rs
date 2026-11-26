//! Electron `main/a-workbook-is-read-for-its-cells.test.ts`, case by case. Expected words and values are Electron's
//! (`main/xlsx.ts`, `shared/sheet.ts`), held to the same openpyxl fixture Electron reads
//! (`apps/desktop/test/workbooks/budget-openpyxl.xlsx`).

use super::*;
use std::io::Write;

const FIXTURE: &[u8] = include_bytes!("../../../desktop/test/workbooks/budget-openpyxl.xlsx");

const WORKBOOK: &str = r#"<workbook xmlns:r="r"><sheets><sheet name="Budget" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
const RELS: &str = r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;

/// A zip of the given parts, deflated (Electron's `zipOf`). `claim_size` writes that uncompressed size into every
/// header in place of the true one, for the cases no writer would produce.
fn zip_of(parts: &[(&str, &[u8])], claim_size: Option<u32>) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, content) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }
    let mut bytes = cursor.into_inner();
    if let Some(claim) = claim_size {
        let u32_at = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
        let u16_at = |b: &[u8], at: usize| u16::from_le_bytes(b[at..at + 2].try_into().unwrap()) as usize;
        let end = bytes.len() - 22;
        assert_eq!(u32_at(&bytes, end), 0x0605_4b50);
        let mut at = u32_at(&bytes, end + 16);
        for _ in 0..u16_at(&bytes, end + 10) {
            assert_eq!(u32_at(&bytes, at), 0x0201_4b50);
            let local = u32_at(&bytes, at + 42);
            bytes[at + 24..at + 28].copy_from_slice(&claim.to_le_bytes());
            bytes[local + 22..local + 26].copy_from_slice(&claim.to_le_bytes());
            at += 46 + u16_at(&bytes, at + 28) + u16_at(&bytes, at + 30) + u16_at(&bytes, at + 32);
        }
    }
    bytes
}

fn texts(book: &Workbook, sheet: usize) -> Vec<Vec<String>> {
    book.sheets[sheet].rows.iter().map(|row| row.iter().map(|cell| cell.text.clone()).collect()).collect()
}

fn book() -> Workbook {
    read_xlsx(FIXTURE).expect("the openpyxl workbook reads")
}

fn cell(text: &str, kind: SheetCellKind) -> SheetCell {
    SheetCell { text: text.into(), kind }
}

// a workbook openpyxl wrote

#[test]
fn has_its_sheets_by_name_in_order() {
    let book = book();
    assert_eq!(book.sheets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Monthly Budget", "Notes"]);
    assert_eq!(book.more_sheets, 0);
}

#[test]
fn reads_text_numbers_in_their_formats_a_percent_and_a_date() {
    let rows = texts(&book(), 0);
    assert_eq!(rows[0][0], "Monthly Budget");
    assert_eq!(rows[2], ["Category", "Budgeted", "Actual"]);
    assert_eq!(rows[3], ["Salary", "$4,000.00", "4,000"]);
    assert_eq!(rows[6], ["Savings rate", "12.5%", ""]);
    assert_eq!(rows[7], ["Paid on", "2026-09-26", ""]);
    // Entities decoded, and nothing in them becomes markup.
    assert_eq!(rows[8][0], "Rent & utilities <shared>");
}

#[test]
fn shows_a_formula_with_no_stored_result_as_the_formula_never_a_number_it_did_not_hold() {
    assert_eq!(book().sheets[0].rows[5][1], cell("=SUM(B4:B5)", SheetCellKind::Formula));
}

#[test]
fn does_not_count_a_styled_empty_cell_as_content() {
    // Z20 is filled yellow and empty: no column Z, no row 20.
    let book = book();
    assert_eq!(book.sheets[0].rows.len(), 9);
    assert_eq!(book.sheets[0].rows[0].len(), 3);
    assert_eq!(book.sheets[0].more_columns, 0);
    assert_eq!(book.sheets[0].more_rows, 0);
}

#[test]
fn reads_a_boolean_on_the_second_sheet() {
    assert_eq!(texts(&book(), 1)[1], ["", "TRUE"]);
}

// a workbook this does not trust

#[test]
fn shows_a_stored_result_where_the_file_holds_one() {
    let sheet = r#"<worksheet><sheetData><row r="1"><c r="A1"><f>1+1</f><v>2</v></c></row></sheetData></worksheet>"#;
    let book = read_xlsx(&zip_of(
        &[("xl/workbook.xml", WORKBOOK.as_bytes()), ("xl/_rels/workbook.xml.rels", RELS.as_bytes()), ("xl/worksheets/sheet1.xml", sheet.as_bytes())],
        None,
    ))
    .expect("reads");
    assert_eq!(book.sheets[0].rows[0][0], cell("2", SheetCellKind::Number));
}

#[test]
fn refuses_a_part_that_claims_more_than_it_may_inflate_to_before_inflating_it() {
    let bomb = zip_of(&[("xl/workbook.xml", WORKBOOK.as_bytes())], Some(64 * 1024 * 1024));
    assert!(read_xlsx(&bomb).is_err());
    // And one that lies about its size is still stopped by the inflate cap.
    let huge = format!("<workbook>{}</workbook>", "x".repeat(30 * 1024 * 1024));
    let liar = zip_of(&[("xl/workbook.xml", huge.as_bytes())], Some(10));
    let refused = read_xlsx(&liar).expect_err("refused");
    assert!(refused.contains("too large or damaged"), "{refused}");
}

#[test]
fn says_in_words_what_it_cannot_read() {
    let refused = read_xlsx(b"not a zip at all").expect_err("refused");
    assert!(refused.contains("not a workbook"), "{refused}");
    let refused = read_xlsx(&zip_of(&[("word/document.xml", b"<w/>")], None)).expect_err("refused");
    assert!(refused.contains("not a workbook"), "{refused}");
}

#[test]
fn draws_at_most_the_bound_and_counts_the_rest() {
    let rows: String = (1..=MAX_SHEET_ROWS + 25)
        .map(|n| format!(r#"<row r="{n}"><c r="A{n}" t="inlineStr"><is><t>r{n}</t></is></c></row>"#))
        .collect();
    let sheet = format!("<worksheet><sheetData>{rows}</sheetData></worksheet>");
    let book = read_xlsx(&zip_of(
        &[("xl/workbook.xml", WORKBOOK.as_bytes()), ("xl/_rels/workbook.xml.rels", RELS.as_bytes()), ("xl/worksheets/sheet1.xml", sheet.as_bytes())],
        None,
    ))
    .expect("reads");
    assert_eq!(book.sheets[0].rows.len(), MAX_SHEET_ROWS);
    assert_eq!(book.sheets[0].more_rows, 25);
}

// a CSV

#[test]
fn reads_quoted_fields_doubled_quotes_and_line_breaks_inside_them() {
    assert_eq!(
        parse_csv("a,\"b, c\",\"say \"\"hi\"\"\"\r\n1,\"two\nlines\",3\n", ',', MAX_SHEET_ROWS + 1),
        vec![vec!["a", "b, c", "say \"hi\""], vec!["1", "two\nlines", "3"]]
    );
}

#[test]
fn becomes_a_one_sheet_workbook_with_its_numbers_set_right() {
    let book = read_csv("budget.csv", "Category,Budgeted\nRent,\"1,400\"\nFood,$500.00\n", false);
    assert_eq!(book.sheets[0].name, "budget.csv");
    assert_eq!(book.sheets[0].rows[1], [cell("Rent", SheetCellKind::Text), cell("1,400", SheetCellKind::Number)]);
    assert_eq!(book.sheets[0].rows[2][1].kind, SheetCellKind::Number);
}

#[test]
fn reads_a_tsv_the_same_way() {
    let book = read_csv("a.tsv", "x\ty\n1\t2", true);
    assert_eq!(book.sheets[0].rows[1].iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["1", "2"]);
}

// a spreadsheet's own words

#[test]
fn names_columns_the_way_a_spreadsheet_does() {
    assert_eq!([0, 25, 26, 27, 701, 702].map(column_name), ["A", "Z", "AA", "AB", "ZZ", "AAA"]);
}

#[test]
fn knows_a_number_when_it_sees_one_written() {
    for text in ["1", "-3.5", "1,400", "$4,200.00", "12%", "(250)"] {
        assert!(looks_numeric(text), "{text}");
    }
    for text in ["Rent", "1,40", "12 apples", "2026-09-26", ""] {
        assert!(!looks_numeric(text), "{text}");
    }
}

// Lane check: the bounds sit where Electron's `xlsx.ts` puts them (`MAX_ENTRIES = 4_000`, `MAX_PART_BYTES = 24 MiB`),
// and the built-in formats are the ones it names (`BUILTIN_DATE_FORMATS`, `BUILTIN_FORMATS`).

fn a_workbook_with(extra_entries: usize, claim_size: Option<u32>) -> Vec<u8> {
    let sheet = r#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>hi</t></is></c></row></sheetData></worksheet>"#;
    let names: Vec<String> = (0..extra_entries).map(|n| format!("pad/{n}.txt")).collect();
    let mut parts: Vec<(&str, &[u8])> =
        vec![("xl/workbook.xml", WORKBOOK.as_bytes()), ("xl/_rels/workbook.xml.rels", RELS.as_bytes()), ("xl/worksheets/sheet1.xml", sheet.as_bytes())];
    parts.extend(names.iter().map(|name| (name.as_str(), &b"x"[..])));
    zip_of(&parts, claim_size)
}

#[test]
fn a_zip_of_four_thousand_entries_reads_and_one_more_is_too_large() {
    // Electron: `entries.length > MAX_ENTRIES` is refused, so 4,000 is the last that reads.
    let book = read_xlsx(&a_workbook_with(4_000 - 3, None)).expect("4,000 entries read");
    assert_eq!(texts(&book, 0), [["hi"]]);
    assert_eq!(read_xlsx(&a_workbook_with(4_000 - 3 + 1, None)), Err("That workbook is too large to show here.".into()));
}

#[test]
fn a_part_that_claims_twenty_four_mebibytes_reads_and_one_byte_more_is_too_large() {
    // Electron: `size > MAX_PART_BYTES` is refused before inflating, so exactly 24 MiB is allowed.
    let book = read_xlsx(&a_workbook_with(0, Some(24 * 1024 * 1024))).expect("a claim of exactly 24 MiB reads");
    assert_eq!(texts(&book, 0), [["hi"]]);
    assert_eq!(read_xlsx(&a_workbook_with(0, Some(24 * 1024 * 1024 + 1))), Err("That workbook is too large to show here.".into()));
}

const STYLES_RELS: &str = r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

/// One cell, `value` drawn in the style whose `<numFmtId>` is `format_id` (a custom one is `164`, with `code`).
fn a_number_styled(value: &str, format_id: u32, code: &str) -> String {
    let styles = format!(
        r#"<styleSheet><numFmts count="1"><numFmt numFmtId="164" formatCode="{code}"/></numFmts><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="{format_id}"/></cellXfs></styleSheet>"#
    );
    let sheet = format!(r#"<worksheet><sheetData><row r="1"><c r="A1" s="1"><v>{value}</v></c></row></sheetData></worksheet>"#);
    let book = read_xlsx(&zip_of(
        &[
            ("xl/workbook.xml", WORKBOOK.as_bytes()),
            ("xl/_rels/workbook.xml.rels", STYLES_RELS.as_bytes()),
            ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
            ("xl/styles.xml", styles.as_bytes()),
        ],
        None,
    ))
    .expect("reads");
    book.sheets[0].rows[0][0].text.clone()
}

fn a_number_in_format(value: &str, format_id: u32) -> String {
    a_number_styled(value, format_id, "General")
}

#[test]
fn the_built_in_formats_are_the_ones_electron_names() {
    // BUILTIN_DATE_FORMATS holds 22 (`m/d/yyyy h:mm`): a serial in it is a date with its time.
    assert_eq!(a_number_in_format("46291.5", 22), "2026-09-26 12:00");
    assert_eq!(a_number_in_format("46291", 14), "2026-09-26");
    // BUILTIN_FORMATS: 9 is `0%`, so an eighth is 13%, not 12.5%; 10 is `0.00%`.
    assert_eq!(a_number_in_format("0.125", 9), "13%");
    assert_eq!(a_number_in_format("0.125", 10), "12.50%");
    // Any other built-in is General.
    assert_eq!(a_number_in_format("0.125", 11), "0.125");
}

// Electron draws a number with `toLocaleString('en-US', { minimumFractionDigits, maximumFractionDigits })`: ICU takes the
// shortest decimal that reads back as the number and rounds a tie up. The values below are what Node 24 writes.
#[test]
fn rounds_a_number_the_way_electrons_to_locale_string_does_a_tie_up_from_its_shortest_decimal() {
    for (value, code, drawn) in [
        ("0.285", "0.00", "0.29"),
        ("1.005", "0.00", "1.01"),
        ("0.125", "0.00", "0.13"),
        ("9.995", "0.00", "10.00"),
        ("0.995", "0.00", "1.00"),
        ("2.5", "0", "3"),
        ("0.5", "0", "1"),
        ("99.5", "0", "100"),
        ("999.5", "#,##0", "1,000"),
        ("1234.565", "#,##0.00", "1,234.57"),
        ("-2.5", "0", "-3"),
    ] {
        assert_eq!(a_number_styled(value, 164, code), drawn, "{value} as {code}");
    }
}
