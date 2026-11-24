//! A PDF page's text, in the order it is read (Electron `renderer/src/pdfText.ts`, 0.713).
//!
//! A PDF reader hands back a page's text as runs of characters, each with where it was drawn, in whatever order the
//! file drew them. Joined as given, a page set in TeX reads as one paragraph with the subscripts glued to the next word
//! and a matrix's rows run together. So the runs are put back on their lines: grouped by height on the page, read left
//! to right, a space where the gap is a word's and two where it is a column's (which keeps a matrix's entries apart). A
//! sub- or superscript sits a little above or below its line and joins it. Nothing is dropped; what cannot be told
//! apart stays in the order it came.
//!
//! Called from `pdf.rs`'s macOS reader (`mac::page_in_order`), which gathers PDFKit's placed characters into runs, as
//! Electron reads pdf.js's.

/// One run of text as the reader placed it: `transform` is `[a, b, c, d, x, y]`, `y` up from the page's bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub transform: [f64; 6],
    pub width: f64,
    pub height: f64,
}

struct Placed<'a> {
    text: &'a str,
    x: f64,
    y: f64,
    end: f64,
    size: f64,
}

struct Line<'a> {
    y: f64,
    size: f64,
    items: Vec<Placed<'a>>,
}

/// A number JavaScript's `||` would keep: not zero and not NaN.
fn truthy(n: f64) -> Option<f64> {
    (n != 0.0 && !n.is_nan()).then_some(n)
}

fn placed(run: &TextRun) -> Option<Placed<'_>> {
    if run.text.is_empty() {
        return None;
    }
    let [_, _, c, d, x, y] = run.transform;
    // The run's font size is the length of its vertical axis; a run without one borrows its height, then a reading size.
    let size = truthy(c.hypot(d)).or(truthy(run.height)).unwrap_or(10.0);
    if ![x, y, size].iter().all(|n| n.is_finite()) {
        return None;
    }
    let end = x + if run.width.is_finite() { run.width } else { 0.0 };
    Some(Placed { text: &run.text, x, y, end, size })
}

/// One page's runs, as lines of text, top to bottom.
pub fn page_text(runs: &[TextRun]) -> String {
    let mut items: Vec<Placed> = runs.iter().filter_map(placed).collect();
    if items.is_empty() {
        return String::new();
    }
    // Top first; on one height, left first (a stable sort, as JavaScript's is).
    items.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    let mut lines: Vec<Line> = Vec::new();
    for item in items {
        if let Some(line) = lines.last_mut()
            // Within six tenths of the larger size: a script beside its line joins it, the next line of a paragraph (a
            // whole line-height away) does not.
            && (line.y - item.y).abs() <= 0.6 * line.size.max(item.size)
        {
            // The line's height is its biggest run's: a superscript does not move it.
            if item.size > line.size {
                line.size = item.size;
                line.y = item.y;
            }
            line.items.push(item);
            continue;
        }
        lines.push(Line { y: item.y, size: item.size, items: vec![item] });
    }
    lines
        .into_iter()
        .map(|mut line| {
            line.items.sort_by(|a, b| a.x.total_cmp(&b.x));
            let mut text = String::new();
            let mut reached: Option<f64> = None;
            for item in &line.items {
                if let Some(reached) = reached {
                    let gap = item.x - reached;
                    let spaced = text.ends_with(char::is_whitespace) || item.text.starts_with(char::is_whitespace);
                    if !spaced && gap > line.size {
                        text.push_str("  ");
                    } else if !spaced && gap > 0.2 * line.size.min(item.size) {
                        text.push(' ');
                    }
                }
                text.push_str(item.text);
                reached = Some(reached.unwrap_or(item.end).max(item.end));
            }
            text.trim_end().to_owned()
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    //! Electron's `renderer/src/a-pdf-page-reads-in-order.test.ts`, its runs and words.
    use super::*;

    /// A run as pdf.js gives one: its size on the transform's axes, its width half a size a character unless given.
    fn run(text: &str, x: f64, y: f64, size: f64, width: Option<f64>) -> TextRun {
        let width = width.unwrap_or(text.chars().count() as f64 * size * 0.5);
        TextRun { text: text.into(), transform: [size, 0.0, 0.0, size, x, y], width, height: size }
    }

    // case: puts runs on their lines, top first, and keeps a matrix’s entries apart
    #[test]
    fn puts_runs_on_their_lines_top_first_and_keeps_a_matrixs_entries_apart() {
        let runs = [
            run("47", 150.0, 588.0, 12.0, None),
            run("2", 120.0, 588.0, 12.0, None),
            run("5", 90.0, 588.0, 12.0, None),
            run("Practice sheet", 72.0, 720.0, 18.0, None),
            run("3", 90.0, 610.0, 12.0, None),
            run("8", 120.0, 610.0, 12.0, None),
            run("1", 150.0, 610.0, 12.0, None),
        ];
        assert_eq!(page_text(&runs), "Practice sheet\n3  8  1\n5  2  47");
    }

    // case: a subscript joins its line, and the next line of a paragraph does not
    #[test]
    fn a_subscript_joins_its_line_and_the_next_line_of_a_paragraph_does_not() {
        // "x" with a subscript "1" a little below it, then the next line 12 points down.
        let runs = [
            run("x", 72.0, 700.0, 10.0, Some(5.0)),
            run("1", 77.5, 697.0, 7.0, Some(3.5)),
            run(" = 4", 81.0, 700.0, 10.0, Some(18.0)),
            run("next line", 72.0, 688.0, 10.0, None),
        ];
        assert_eq!(page_text(&runs), "x1 = 4\nnext line");
    }

    // case: an empty page is no text, not an error
    #[test]
    fn an_empty_page_is_no_text_not_an_error() {
        assert_eq!(page_text(&[]), "");
        assert_eq!(page_text(&[run("", 72.0, 700.0, 10.0, None)]), "");
    }
}
