//! A spreadsheet in the viewer, drawn as one (Electron `SheetView.tsx`, 0.364): column letters across the top, row
//! numbers down the side, and a tab per sheet below the grid where Excel and Sheets keep theirs. Numbers sit right; a
//! formula the file holds no result for is shown as the formula, muted, with one line saying why (nothing here works a
//! formula out). Every bound the reader applied (`office::read_xlsx`, `office::read_csv`) is said under the grid. Cell
//! text is drawn as text: nothing in a cell is markup.
use super::*;
use crate::office::{SheetCellKind, SheetTable, Workbook};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Said once under a grid holding a formula with no stored result.
pub(super) const FORMULA_SAID: &str = "A formula the file holds no result for is shown as written; Excel or Sheets works it out.";

/// A cell's width in the grid, and the row numbers' column.
const CELL_W: f32 = 88.0;
const ROW_HEAD_W: f32 = 36.0;

type WorkbookKey = (std::path::PathBuf, String);

/// What `read_document` read of each workbook it opened, by folder and path, for the viewer to draw as a grid.
static READ: LazyLock<Mutex<HashMap<WorkbookKey, Workbook>>> = LazyLock::new(Default::default);

/// The sheet each open workbook shows, by folder and path: its tabs choose it.
static SHOWN: LazyLock<Mutex<HashMap<WorkbookKey, usize>>> = LazyLock::new(Default::default);

/// Keep the workbook read at `path` in `folder`, for [`grid`]; it opens on its first sheet.
pub(super) fn keep(folder: &std::path::Path, path: &str, book: Workbook) {
    let key = (folder.to_path_buf(), path.to_owned());
    READ.lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone(), book);
    SHOWN.lock().unwrap_or_else(|e| e.into_inner()).remove(&key);
}

/// What lay past the bounds and why a formula shows as written, in one line, or nothing when there is neither.
fn note(sheet: &SheetTable, more_sheets: usize) -> Option<String> {
    let count = |n: usize, one: &str, many: &str| (n > 0).then(|| format!("{n} more {}", if n == 1 { one } else { many }));
    let parts: Vec<String> = [
        count(sheet.more_rows, "row", "rows"),
        count(sheet.more_columns, "column", "columns"),
        count(more_sheets, "sheet", "sheets"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let formulas = sheet.rows.iter().flatten().any(|c| c.kind == SheetCellKind::Formula);
    let mut said = String::new();
    if !parts.is_empty() {
        said = format!("{} not shown here.", parts.join(" · "));
    }
    if formulas {
        if !said.is_empty() {
            said.push(' ');
        }
        said.push_str(FORMULA_SAID);
    }
    (!said.is_empty()).then_some(said)
}

/// The workbook at `path` in the viewer, as its grid (`SheetView`), when the viewer read one; `None` leaves its text.
pub(super) fn grid(s: &State, path: &str, p: &Palette) -> Option<El> {
    let folder = std::path::PathBuf::from(selected_conversation(s)?.0.folder.as_deref()?);
    let key = (folder, path.to_owned());
    let book = READ.lock().unwrap_or_else(|e| e.into_inner()).get(&key).cloned()?;
    let Some(last) = book.sheets.len().checked_sub(1) else {
        return Some(text("This workbook has no sheets.").font(type_::LABEL).color(p.muted));
    };
    let shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner()).get(&key).copied().unwrap_or(0).min(last);
    let sheet = &book.sheets[shown];
    let mut view = col().key("sheet").w_full().gap(8.0).grow(1.0).min_h(0.0);
    if sheet.rows.is_empty() {
        view = view.child(text(format!("{} is empty.", sheet.name)).font(type_::LABEL).color(p.muted));
    } else {
        let width = sheet.rows[0].len();
        let head = |words: String, key: String, w: f32| {
            text(words).key(key).font(type_::LABEL).color(p.muted).w(w).align(Align::Center).fill(p.raised)
        };
        let mut columns = row().child(el().w(ROW_HEAD_W).fill(p.raised));
        for c in 0..width {
            let name = crate::office::column_name(c);
            columns = columns.child(head(name.clone(), format!("sheet/col/{name}"), CELL_W));
        }
        let mut table = col().role("table").label(sheet.name.clone()).gap(1.0).child(columns);
        for (r, cells) in sheet.rows.iter().enumerate() {
            let mut line = row().gap(1.0).child(head((r + 1).to_string(), format!("sheet/row/{}", r + 1), ROW_HEAD_W));
            for (c, cell) in cells.iter().enumerate() {
                let kind = match cell.kind {
                    SheetCellKind::Text => "text",
                    SheetCellKind::Number => "number",
                    SheetCellKind::Formula => "formula",
                    SheetCellKind::Boolean => "boolean",
                    SheetCellKind::Error => "error",
                };
                let right = matches!(cell.kind, SheetCellKind::Number | SheetCellKind::Boolean);
                line = line.child(
                    text(cell.text.clone())
                        .key(format!("sheet/{}{}/{kind}", crate::office::column_name(c), r + 1))
                        .font(type_::LABEL)
                        .color(match cell.kind {
                            SheetCellKind::Formula => p.muted,
                            SheetCellKind::Error => p.red,
                            _ => p.text,
                        })
                        .w(CELL_W)
                        .align(if right { Align::End } else { Align::Start })
                        .px(4.0),
                );
            }
            table = table.child(line);
        }
        view = view.child(col().key("sheet/grid").grow(1.0).min_h(0.0).w_full().scroll_xy().child(table));
    }
    // Below the grid, as Excel keeps them; a workbook of one sheet has none.
    if book.sheets.len() > 1 {
        let mut tabs = row().key("sheet/tabs").role("tablist").label("Sheets").gap(4.0);
        for (i, one) in book.sheets.iter().enumerate() {
            let key = key.clone();
            tabs = tabs.child(
                text(one.name.clone())
                    .key(format!("sheet/tab/{i}"))
                    .role("tab")
                    .font(type_::LABEL)
                    .color(if i == shown { p.text } else { p.secondary })
                    .px(8.0)
                    .py(3.0)
                    .radius(4.0)
                    .fill(if i == shown { p.selected } else { p.raised })
                    .on_press(move |_s: &mut State| {
                        SHOWN.lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone(), i);
                    }),
            );
        }
        view = view.child(tabs);
    }
    if let Some(said) = note(sheet, book.more_sheets) {
        view = view.child(text(said).key("sheet/note").font(type_::LABEL).color(p.muted).wrap_anywhere());
    }
    Some(view)
}

#[cfg(test)]
#[path = "sheet_view_tests.rs"]
mod sheet_view_tests;
