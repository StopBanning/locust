//! Electron `main/an-office-file-is-read-for-its-words.test.ts`, case by case. Expected words and values are Electron's
//! (`main/office-text.ts`, `shared/office-document.ts`), held to the same python-docx and python-pptx fixtures Electron
//! reads (`apps/desktop/test/documents/`).

use super::*;
use std::io::Write;

const REPORT: &[u8] = include_bytes!("../../../desktop/test/documents/report-python-docx.docx");
const DECK: &[u8] = include_bytes!("../../../desktop/test/documents/deck-python-pptx.pptx");

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

/// A zip of the given parts, deflated (Electron's `zipOf`).
fn zip_of(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, content) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    cursor.into_inner()
}

fn heading(level: usize, text: &str) -> DocumentBlock {
    DocumentBlock::Heading { level, text: text.into() }
}
fn paragraph(text: &str) -> DocumentBlock {
    DocumentBlock::Paragraph { text: text.into() }
}
fn item(depth: usize, text: &str) -> DocumentBlock {
    DocumentBlock::Item { depth, text: text.into() }
}
fn slide(number: usize, title: &str) -> DocumentBlock {
    DocumentBlock::Slide { number, title: Some(title.into()) }
}
fn table(rows: &[&[&str]]) -> DocumentBlock {
    DocumentBlock::Table { rows: rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect() }
}

fn docx(xml: &str) -> Vec<DocumentBlock> {
    read_docx(&zip_of(&[("word/document.xml", xml)])).expect("reads").blocks
}

// a Word document python-docx wrote

#[test]
fn has_its_title_headings_paragraphs_list_items_and_table_in_order() {
    let read = read_docx(REPORT).expect("the python-docx report reads");
    assert_eq!(read.kind, DocumentKind::Word);
    assert_eq!(
        read.blocks,
        [
            heading(1, "Quarterly Report"),
            paragraph("Revenue grew in every region. Costs held flat at $5 per unit; see <the table> & *notes*."),
            heading(1, "Highlights"),
            item(0, "New customers in the north"),
            item(0, "Two hires, both engineers"),
            item(1, "One of them in support"),
            heading(2, "By region"),
            table(&[&["Region", "Q2", "Q3"], &["North", "120", "150"], &["South", "90", "95"]]),
            paragraph("Prepared by the finance teammate."),
        ]
    );
    assert_eq!(read.more, 0);
}

#[test]
fn keeps_the_characters_a_model_might_write_exactly_as_written_nothing_is_markup() {
    let read = read_docx(REPORT).expect("reads");
    let first = read.blocks.iter().find_map(|b| match b {
        DocumentBlock::Paragraph { text } => Some(text.clone()),
        _ => None,
    });
    assert!(first.as_deref().is_some_and(|t| t.contains("<the table> & *notes*")), "{first:?}");
}

// a deck python-pptx wrote

#[test]
fn has_each_slide_numbered_under_its_title_with_its_text_and_table() {
    let read = read_pptx(DECK).expect("the python-pptx deck reads");
    assert_eq!(read.kind, DocumentKind::Slides);
    assert_eq!(
        read.blocks,
        [
            slide(1, "Launch Plan"),
            paragraph("October 2026"),
            slide(2, "What ships"),
            item(0, "Weekday routines"),
            item(1, "Monday to Friday by default"),
            item(0, "Word and PowerPoint previews"),
            slide(3, "Numbers"),
            table(&[&["Metric", "Value"], &["Users", "1,200"]]),
            paragraph("Source: the usage export"),
        ]
    );
}

// what is not drawn, and what is refused

#[test]
fn leaves_out_deleted_text_field_codes_and_empty_paragraphs_a_tab_stays_a_tab() {
    let xml = format!(
        r#"<w:document {W}><w:body>
      <w:p><w:r><w:t>Kept</w:t></w:r><w:del><w:r><w:delText>gone</w:delText></w:r></w:del><w:r><w:instrText>PAGE</w:instrText></w:r><w:r><w:tab/><w:t xml:space="preserve">after a tab</w:t></w:r></w:p>
      <w:p/>
      <w:p><w:pPr><w:tabs><w:tab w:val="left" w:pos="720"/></w:tabs></w:pPr><w:r><w:t>Tab stops are not tabs</w:t></w:r></w:p>
    </w:body></w:document>"#
    );
    assert_eq!(docx(&xml), [paragraph("Kept\tafter a tab"), paragraph("Tab stops are not tabs")]);
}

// case: reads an empty run as nothing, not as the XML after it (2026-10-10 sweep)
#[test]
fn reads_an_empty_run_as_nothing_not_as_the_xml_after_it() {
    let xml = format!(r#"<w:document {W}><w:body><w:p><w:r><w:t/></w:r><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#);
    assert_eq!(docx(&xml), [paragraph("Hello")]);
}

#[test]
fn reads_a_table_inside_a_table_cell_without_losing_the_paragraphs_after_it() {
    let cell = |text: &str| format!("<w:tc><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>");
    let inner = format!("<w:tbl><w:tr>{}</w:tr></w:tbl>", cell("inner"));
    let xml = format!(
        "<w:document {W}><w:body><w:tbl><w:tr>{}<w:tc>{inner}<w:p/></w:tc></w:tr></w:tbl><w:p><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>",
        cell("a")
    );
    assert_eq!(docx(&xml).last(), Some(&paragraph("After")));
}

#[test]
fn a_long_table_keeps_its_first_rows_and_says_how_many_it_has() {
    let rows: String = (0..MAX_DOCUMENT_TABLE_ROWS + 5).map(|i| format!("<w:tr><w:tc><w:p><w:r><w:t>{i}</w:t></w:r></w:p></w:tc></w:tr>")).collect();
    let blocks = docx(&format!("<w:document {W}><w:body><w:tbl>{rows}</w:tbl></w:body></w:document>"));
    let DocumentBlock::Table { rows } = &blocks[0] else { panic!("a table first: {blocks:?}") };
    assert_eq!(rows.len(), MAX_DOCUMENT_TABLE_ROWS);
    assert_eq!(
        blocks[1],
        paragraph(&format!(
            "(This table has {} rows and 1 columns; the first {} rows and 1 columns are shown.)",
            MAX_DOCUMENT_TABLE_ROWS + 5,
            MAX_DOCUMENT_TABLE_ROWS
        ))
    );
}

#[test]
fn a_file_that_is_not_a_zip_or_a_zip_with_no_document_in_it_is_refused_in_words() {
    assert_eq!(read_docx(b"not a zip at all"), Err("That file is not a Word document Locust can read.".into()));
    assert_eq!(read_pptx(&zip_of(&[("word/document.xml", "<w:document/>")])), Err("That file is not a presentation Locust can read.".into()));
    // A workbook handed in as a deck is not read as one.
    assert!(read_docx(&zip_of(&[("xl/workbook.xml", "<workbook/>")])).is_err());
}

// case: a slide's number, date and footer are left out
#[test]
fn a_slides_number_date_and_footer_are_left_out() {
    let presentation = r#"<p:presentation xmlns:p="p" xmlns:r="r"><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst></p:presentation>"#;
    let rels = r#"<Relationships><Relationship Id="rId2" Target="slides/slide1.xml"/></Relationships>"#;
    let shape = |kind: &str, text: &str| {
        format!(r#"<p:sp><p:nvSpPr><p:nvPr><p:ph type="{kind}"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#)
    };
    let slide_xml = format!(
        "<p:sld><p:cSld><p:spTree>{}{}{}{}</p:spTree></p:cSld></p:sld>",
        shape("title", "Only this"),
        shape("sldNum", "7"),
        shape("dt", "10/1/2026"),
        shape("ftr", "Confidential")
    );
    let read = read_pptx(&zip_of(&[
        ("ppt/presentation.xml", presentation),
        ("ppt/_rels/presentation.xml.rels", rels),
        ("ppt/slides/slide1.xml", &slide_xml),
    ]))
    .expect("reads");
    assert_eq!(read.blocks, [slide(1, "Only this")]);
}
