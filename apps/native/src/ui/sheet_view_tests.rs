//! Electron's `renderer/src/a-spreadsheet-opens-as-a-grid.test.tsx` (0.364), its viewer cases, through the window: a
//! workbook kept as `read_document` keeps one (Electron's BUDGET, drawn as `SheetView` draws it), and real files opened
//! as the app opens them (`reply_actions::open_file`, then `viewer_on`).
use super::*;
use crate::office::{SheetCell, SheetCellKind, SheetTable};

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(name: &str) -> Temp {
    let dir = std::env::temp_dir().join(format!("locust-sheet-{name}-{}", locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    Temp(dir)
}

fn cell(text: &str, kind: SheetCellKind) -> SheetCell {
    SheetCell { text: text.into(), kind }
}

/// Electron's BUDGET workbook.
fn budget() -> Workbook {
    use SheetCellKind::{Formula, Number, Text};
    Workbook {
        sheets: vec![
            SheetTable {
                name: "Monthly Budget".into(),
                rows: vec![
                    vec![cell("Category", Text), cell("Budgeted", Text)],
                    vec![cell("Rent", Text), cell("$1,400.00", Number)],
                    vec![cell("Total", Text), cell("=SUM(B2:B2)", Formula)],
                ],
                more_rows: 0,
                more_columns: 0,
            },
            SheetTable { name: "Notes".into(), rows: vec![vec![cell("Yellow cells are yours.", Text)]], more_rows: 0, more_columns: 0 },
        ],
        more_sheets: 0,
    }
}

fn conversation_in(folder: &std::path::Path) -> State {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm".into(),
        name: "Wren".into(),
        role: String::new(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(folder.to_string_lossy().into_owned()),
    });
    s.conversations.insert("tm".into(), Default::default());
    s
}

fn drawn(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::reply_actions::viewer_on(s, &Palette::new(false), "darwin", &[]).unwrap(),
        (1600.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt
}

/// The viewer on BUDGET, kept for `path` as `read_document` keeps a workbook it read.
fn budget_in_viewer(dir: &Temp, path: &str) -> rune::Runtime<State> {
    keep(&dir.0, path, budget());
    let mut s = conversation_in(&dir.0);
    s.file_view = Some((crate::reply_actions::FileTarget { path: path.into(), start: 1, end: 1 }, Ok("words".into())));
    drawn(s)
}

/// `path` in `folder` opened as the app opens it.
fn opened(folder: &std::path::Path, path: &str) -> rune::Runtime<State> {
    let mut s = conversation_in(folder);
    crate::reply_actions::open_file(&mut s, crate::reply_actions::FileTarget { path: path.into(), start: 1, end: 1 });
    drawn(s)
}

fn texts(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

/// The text drawn under `key`.
fn at(rt: &rune::Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key)).and_then(|e| e.text)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for b in bytes {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A zip of `files`, stored (not compressed), as an .xlsx is.
fn stored_zip(files: &[(&str, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, body) in files {
        let (crc, len, at) = (crc32(body.as_bytes()), body.len() as u32, out.len() as u32);
        let head = |sig: u32, central: bool| {
            let mut h = sig.to_le_bytes().to_vec();
            if central {
                h.extend(20u16.to_le_bytes());
            }
            for v in [20u16, 0, 0, 0, 0] {
                h.extend(v.to_le_bytes());
            }
            h.extend(crc.to_le_bytes());
            h.extend(len.to_le_bytes());
            h.extend(len.to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes());
            if central {
                for v in [0u16, 0, 0] {
                    h.extend(v.to_le_bytes());
                }
                h.extend(0u32.to_le_bytes());
                h.extend(at.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(head(0x0403_4b50, false));
        out.extend(body.as_bytes());
        central.extend(head(0x0201_4b50, true));
    }
    let (cd_at, cd_len, n) = (out.len() as u32, central.len() as u32, files.len() as u16);
    out.extend(central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    for v in [0u16, 0, n, n] {
        out.extend(v.to_le_bytes());
    }
    out.extend(cd_len.to_le_bytes());
    out.extend(cd_at.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

/// A workbook of `sheets`, each a name and its cells as `<c>` XML in rows.
fn xlsx_of(sheets: &[(&str, &[&str])]) -> Vec<u8> {
    let mut files: Vec<(String, String)> = vec![
        ("[Content_Types].xml".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#.into()),
        ("_rels/.rels".into(), r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into()),
    ];
    let mut listed = String::new();
    let mut rels = String::new();
    for (i, (name, rows)) in sheets.iter().enumerate() {
        let n = i + 1;
        listed.push_str(&format!(r#"<sheet name="{name}" sheetId="{n}" r:id="rId{n}"/>"#));
        rels.push_str(&format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#));
        let rows: String = rows.iter().enumerate().map(|(r, cells)| format!(r#"<row r="{}">{cells}</row>"#, r + 1)).collect();
        files.push((format!("xl/worksheets/sheet{n}.xml"), format!(r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows}</sheetData></worksheet>"#)));
    }
    files.push(("xl/workbook.xml".into(), format!(r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{listed}</sheets></workbook>"#)));
    files.push(("xl/_rels/workbook.xml.rels".into(), format!(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#)));
    let files: Vec<(&str, String)> = files.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    stored_zip(&files)
}

/// A text cell at `r` holding `t`.
fn said(r: &str, t: &str) -> String {
    format!(r#"<c r="{r}" t="inlineStr"><is><t>{t}</t></is></c>"#)
}

/// Penny's budget as a real file: a formula with no stored result under a number.
fn budget_xlsx() -> Vec<u8> {
    xlsx_of(&[(
        "Monthly Budget",
        &[
            &format!("{}{}", said("A1", "Category"), said("B1", "Budgeted")),
            &format!(r#"{}<c r="B2"><v>1400</v></c>"#, said("A2", "Rent")),
            &format!(r#"{}<c r="B3"><f>SUM(B2:B2)</f></c>"#, said("A3", "Total")),
        ],
    )])
}

// case: is the grid a spreadsheet person reads: letters across, numbers down
#[test]
fn is_the_grid_a_spreadsheet_person_reads_letters_across_numbers_down() {
    let dir = temp("grid");
    let rt = budget_in_viewer(&dir, "grid.xlsx");
    let (a, b) = (rt.find("sheet/col/A").expect("A"), rt.find("sheet/col/B").expect("B"));
    assert!(a.0 < b.0 && (a.1 - b.1).abs() < 1.0, "A then B across: {a:?} {b:?}");
    assert_eq!(at(&rt, "sheet/col/A").as_deref(), Some("A"));
    assert_eq!(at(&rt, "sheet/row/3").as_deref(), Some("3"));
    assert!(rt.find("sheet/col/C").is_none() && rt.find("sheet/row/4").is_none());
    let (one, three) = (rt.find("sheet/row/1").unwrap(), rt.find("sheet/row/3").unwrap());
    assert!(one.1 < three.1, "numbers down");
    assert_eq!(at(&rt, "sheet/B2/number").as_deref(), Some("$1,400.00"));
    assert_eq!(at(&rt, "sheet/A2/text").as_deref(), Some("Rent"));
}

// case: shows a formula with no result as written, and says why once
#[test]
fn shows_a_formula_with_no_result_as_written_and_says_why_once() {
    let dir = temp("formula");
    let rt = budget_in_viewer(&dir, "formula.xlsx");
    assert_eq!(at(&rt, "sheet/B3/formula").as_deref(), Some("=SUM(B2:B2)"));
    assert_eq!(texts(&rt).iter().filter(|t| t.contains(FORMULA_SAID)).count(), 1, "{:?}", texts(&rt));
    // Read from a real file: the formula the file holds no result for, and the number it does hold.
    std::fs::write(dir.0.join("budget.xlsx"), budget_xlsx()).unwrap();
    let rt = opened(&dir.0, "budget.xlsx");
    assert_eq!(at(&rt, "sheet/B3/formula").as_deref(), Some("=SUM(B2:B2)"), "{:?}", texts(&rt));
    assert_eq!(at(&rt, "sheet/B2/number").as_deref(), Some("1400"), "{:?}", texts(&rt));
    assert_eq!(at(&rt, "sheet/note").as_deref(), Some(FORMULA_SAID));
}

// case: has a tab per sheet, below the grid as Excel keeps them
#[test]
fn has_a_tab_per_sheet_below_the_grid_as_excel_keeps_them() {
    let dir = temp("tabs");
    let mut rt = budget_in_viewer(&dir, "tabs.xlsx");
    let last_row = rt.find("sheet/row/3").unwrap();
    let notes = rt.find("sheet/tab/1").expect("a tab for Notes");
    assert!(notes.1 > last_row.1, "tabs below the grid: {notes:?} {last_row:?}");
    assert_eq!(at(&rt, "sheet/tab/0").as_deref(), Some("Monthly Budget"));
    assert_eq!(at(&rt, "sheet/tab/1").as_deref(), Some("Notes"));
    rt.pointer_down(notes.0, notes.1);
    rt.pointer_up(notes.0, notes.1);
    rt.frame(0.016);
    assert_eq!(at(&rt, "sheet/A1/text").as_deref(), Some("Yellow cells are yours."), "{:?}", texts(&rt));
    // A workbook of one sheet has no tabs.
    std::fs::write(dir.0.join("one.csv"), "a,b\n").unwrap();
    assert!(opened(&dir.0, "one.csv").find("sheet/tabs").is_none());
}

// case: says what it did not draw
#[test]
fn says_what_it_did_not_draw() {
    let dir = temp("big");
    let lines: Vec<String> = (0..510).map(|i| format!("row {},{i}", i + 1)).collect();
    std::fs::write(dir.0.join("big.csv"), lines.join("\n")).unwrap();
    let rt = opened(&dir.0, "big.csv");
    assert_eq!(at(&rt, "sheet/note").as_deref(), Some("10 more rows not shown here."), "{:?}", texts(&rt));
    assert!(rt.find("sheet/row/500").is_some() && rt.find("sheet/row/501").is_none());
    // A workbook: columns past AN and sheets past twelve, counted.
    let wide = format!("{}{}{}", said("A1", "first"), said("AN1", "fortieth"), said("AR1", "far"));
    let row: &[&str] = &[wide.as_str()];
    let names: Vec<String> = (1..=14).map(|n| format!("Sheet {n}")).collect();
    let sheets: Vec<(&str, &[&str])> = names.iter().map(|n| (n.as_str(), row)).collect();
    std::fs::write(dir.0.join("wide.xlsx"), xlsx_of(&sheets)).unwrap();
    let rt = opened(&dir.0, "wide.xlsx");
    assert_eq!(at(&rt, "sheet/note").as_deref(), Some("4 more columns · 2 more sheets not shown here."), "{:?}", texts(&rt));
    assert!(rt.find("sheet/col/AN").is_some() && rt.find("sheet/col/AO").is_none());
    assert!(rt.find("sheet/tab/11").is_some() && rt.find("sheet/tab/12").is_none());
}

// case: draws cell text as text, never markup
#[test]
fn draws_cell_text_as_text_never_markup() {
    let dir = temp("hostile");
    std::fs::write(dir.0.join("x.csv"), "a,<img src=x onerror=alert(1)>\n").unwrap();
    let rt = opened(&dir.0, "x.csv");
    assert_eq!(at(&rt, "sheet/B1/text").as_deref(), Some("<img src=x onerror=alert(1)>"), "{:?}", texts(&rt));
    // A quoted field keeps its comma, its doubled quote and its line break.
    std::fs::write(dir.0.join("q.csv"), "name,said\r\nRent,\"1,400\"\r\nPenny,\"she said \"\"hi\"\"\nthen left\"\r\n").unwrap();
    let rt = opened(&dir.0, "q.csv");
    assert_eq!(at(&rt, "sheet/B2/number").as_deref(), Some("1,400"), "{:?}", texts(&rt));
    assert_eq!(at(&rt, "sheet/B3/text").as_deref(), Some("she said \"hi\"\nthen left"), "{:?}", texts(&rt));
    // Never a control character, and a long cell bounded.
    let long = "x".repeat(400);
    let bounded = format!("{}…", "x".repeat(299));
    std::fs::write(dir.0.join("c.csv"), format!("be\u{7}ll,{long}\n")).unwrap();
    let rt = opened(&dir.0, "c.csv");
    assert_eq!(at(&rt, "sheet/A1/text").as_deref(), Some("bell"), "{:?}", texts(&rt));
    assert_eq!(at(&rt, "sheet/B1/text").as_deref(), Some(bounded.as_str()));
    std::fs::write(dir.0.join("long.xlsx"), xlsx_of(&[("Long", &[&said("A1", &long)])])).unwrap();
    assert_eq!(at(&opened(&dir.0, "long.xlsx"), "sheet/A1/text").as_deref(), Some(bounded.as_str()));
}

// case: is what the file viewer draws for a table
#[test]
fn is_what_the_file_viewer_draws_for_a_table() {
    let dir = temp("viewer");
    std::fs::write(dir.0.join("budget.xlsx"), budget_xlsx()).unwrap();
    let rt = opened(&dir.0, "budget.xlsx");
    assert!(rt.find("sheet/grid").is_some(), "{:?}", texts(&rt));
    assert!(texts(&rt).iter().any(|t| t.contains("budget.xlsx")), "{:?}", texts(&rt));
    // A table, not its words line by line.
    assert!(!texts(&rt).iter().any(|t| t.contains("| Category |")), "{:?}", texts(&rt));
}
