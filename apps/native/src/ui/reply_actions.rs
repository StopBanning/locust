//! Read-only reply code and workspace file controls.
use super::*;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub struct FileTarget {
    pub path: String,
    pub start: usize,
    pub end: usize,
}

/// Electron CopyButton feedback: "Copied" / "Not copied", timed out the same way.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyFeedback {
    pub key: u64,
    pub ok: bool,
    pub until: Instant,
}

fn body_key(body: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    body.hash(&mut hasher);
    hasher.finish()
}

/// Syntax highlights for a reply code block; stays plain while live or past 60,000 UTF-16 units.
pub(crate) fn highlight_reply_code(
    live: bool,
    language: Option<&str>,
    code: &str,
) -> Vec<(std::ops::Range<usize>, rune_kit::syntax::Paint)> {
    if live || code.encode_utf16().count() > 60_000 {
        Vec::new()
    } else {
        rune_kit::syntax::highlight(language, code).to_vec()
    }
}

/// A file's language from its name (`codeColors.ts` `languageOfPath`): `cart.py` -> `Some("py")`,
/// `src/App.TSX` -> `Some("tsx")`, `deploy/Dockerfile` -> `Some("dockerfile")`; `None` for dotfiles
/// like `.env`, bare names like `README`, or unrecognised extensions like `notes.txt`.
pub(crate) fn language_of_path(path: &str) -> Option<String> {
    let name = path.replace('\\', "/").rsplit('/').next().unwrap_or("").to_ascii_lowercase();
    match name.as_str() {
        "dockerfile" => return Some("dockerfile".into()),
        "makefile" | ".bashrc" | ".zshrc" => return Some("shellscript".into()),
        ".gitconfig" | ".editorconfig" => return Some("ini".into()),
        _ => {}
    }
    let dot = name.rfind('.')?;
    if dot == 0 || dot + 1 == name.len() {
        return None;
    }
    let ext = &name[dot + 1..];
    let known = matches!(
        ext,
        "ts" | "mts" | "cts" | "typescript" | "tsx" | "js" | "mjs" | "cjs" | "javascript" | "jsx"
            | "json" | "jsonc" | "py" | "python" | "sh" | "bash" | "zsh" | "shell" | "console" | "shellscript"
            | "ps1" | "pwsh" | "ps" | "powershell" | "bat" | "cmd" | "batch"
            | "css" | "html" | "htm" | "markdown" | "md" | "yaml" | "yml" | "toml"
            | "rs" | "rust" | "go" | "golang" | "java" | "kotlin" | "kt" | "swift"
            | "c" | "h" | "cpp" | "c++" | "cc" | "hpp" | "csharp" | "cs" | "c#"
            | "ruby" | "rb" | "php" | "sql" | "diff" | "patch" | "xml" | "svg"
            | "dockerfile" | "docker" | "ini" | "cfg" | "conf" | "lua"
    );
    known.then(|| ext.to_owned())
}

/// Words and settle times from `CopyButton.tsx` (1_600 ms copied, 4_000 ms refused).
pub fn copy_said(ok: bool) -> (&'static str, Duration) {
    if ok {
        ("Copied", Duration::from_millis(1_600))
    } else {
        ("Not copied", Duration::from_millis(4_000))
    }
}

pub fn copy_label(feedback: Option<&CopyFeedback>, body: &str, now: Instant) -> &'static str {
    let key = body_key(body);
    match feedback {
        Some(f) if f.key == key && now < f.until => copy_said(f.ok).0,
        _ => "Copy",
    }
}

pub fn settle_copy_feedback(body: &str, ok: bool, now: Instant) -> CopyFeedback {
    let (_, hold) = copy_said(ok);
    CopyFeedback {
        key: body_key(body),
        ok,
        until: now + hold,
    }
}

pub(super) fn request_copy(tx: &mut Tx, body: String) {
    let expected = body.clone();
    tx.copy(body);
    // Write is flushed before reads in Rune's clipboard worker; confirm what landed.
    tx.read_clipboard(move |s: &mut State, content, tx| {
        let ok = content.text() == expected;
        s.copy_feedback = Some(settle_copy_feedback(&expected, ok, Instant::now()));
        let hold = copy_said(ok).1;
        tx.wake_after(hold);
    });
}
pub fn file_target(raw: &str) -> Option<FileTarget> {
    let raw = raw.trim().trim_start_matches("file://");
    if raw.contains("://") || raw.contains('\0') {
        return None;
    }
    // Plain path chips (no :line) still count when they look like file paths.
    if !raw.contains(':')
        && !raw.contains('#')
        && !crate::file_menu::looks_like_file_path(raw)
        && !raw.contains('/')
        && !raw.contains('.')
    {
        return None;
    }
    let re = crate::invariants::regex(r"^(.*?)(?::(\d+)(?:-(\d+))?|#L(\d+)(?:-L?(\d+))?)?$");
    let c = re.captures(raw)?;
    let path = c.get(1)?.as_str();
    if path.is_empty() || (!path.contains('/') && !path.contains('.')) {
        return None;
    }
    let start = c
        .get(2)
        .or_else(|| c.get(4))
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(1usize)
        .max(1);
    let end = c
        .get(3)
        .or_else(|| c.get(5))
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(start)
        .max(start);
    Some(FileTarget {
        path: path.into(),
        start,
        end,
    })
}

/// The words of a file the viewer may show: held to the conversation's folder, then read as Electron's text channel
/// reads one (`locust_core::text_files`): only a kind of file a teammate writes to be read, nothing past a quarter of a
/// megabyte (refused whole, never cut), and bytes that are not UTF-8 shown as the replacement character.
pub fn read_file(root: &Path, target: &FileTarget) -> Result<String, String> {
    use locust_core::text_files;
    // The kind of file is judged first, by the name the message gave it: nothing on disk is touched for a file the
    // viewer will not open.
    if !text_files::is_viewable_text(&target.path) {
        return Err(text_files::NOT_OPENED.into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let Ok(path) = root.join(&target.path).canonicalize() else {
        return Err(text_files::NOT_THERE.into());
    };
    if !path.starts_with(&root) {
        return Err("This file is outside the conversation's folder.".into());
    }
    text_files::read_viewable_text(&target.path, &path)
}

/// A csv or tsv file's text, as Electron's text channel reads a sheet before it asks the allowlist (`SHEET_EXTENSIONS`,
/// `MAX_SHEET_FILE_BYTES`): held to the conversation's folder, refused whole past 8 MB, read as UTF-8 text.
fn read_sheet_text(root: &Path, target: &FileTarget) -> Result<String, String> {
    const MAX_SHEET_FILE_BYTES: u64 = 8 * 1024 * 1024;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let Ok(path) = root.join(&target.path).canonicalize() else {
        return Err(locust_core::text_files::NOT_THERE.into());
    };
    if !path.starts_with(&root) {
        return Err("This file is outside the conversation's folder.".into());
    }
    let Ok(measured) = std::fs::metadata(&path) else {
        return Err(locust_core::text_files::NOT_THERE.into());
    };
    if !measured.is_file() {
        return Err("That is a folder, not a file.".into());
    }
    if measured.len() > MAX_SHEET_FILE_BYTES {
        return Err(format!(
            "That spreadsheet is {}KB, too big to show here. Save a copy and open it in Excel or Sheets.",
            (measured.len() as f64 / 1024.0 + 0.5).floor()
        ));
    }
    match std::fs::read(&path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(_) => Err(locust_core::text_files::NOT_THERE.into()),
    }
}

/// CopyButton.tsx: the copy glyph, a check with "Copied" once it lands, "Not copied" in amber if the clipboard refused.
pub(super) fn copy_button(feedback: Option<&CopyFeedback>, body: String, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let label = copy_label(feedback, &body, Instant::now());
    let (glyph, said, tone) = match label {
        "Copied" => (&crate::look::icons::CHECK, Some(label), l.c("text-secondary")),
        "Not copied" => (&crate::look::icons::COPY, Some(label), l.c("amber-text")),
        _ => (&crate::look::icons::COPY, None, l.c("text-muted")),
    };
    let mut e = row()
        .gap(5.0)
        .py(4.0)
        .px(6.0)
        .center_y()
        .radius(l.t.px("radius-control"))
        .hover(l.c("hover-surface"))
        .child(crate::look::controls::glyph(&l, glyph, 12.0, Some(tone)));
    if let Some(said) = said {
        e = e.child(text(said).font(l.ty.mono).color(tone));
    }
    e.role("button")
        .label(if said == Some("Copied") { "Copied reply" } else { "Copy reply" })
        .on_press_tx(move |_s: &mut State, tx| request_copy(tx, body.clone()))
}

pub(super) fn open_file(s: &mut State, target: FileTarget) {
    let root = selected_conversation(s)
        .and_then(|(t, _)| t.folder.as_ref())
        .map(PathBuf::from);
    let body = root
        .ok_or_else(|| "This conversation has no folder.".into())
        // A picture is drawn by the viewer itself (step_pictures::thread_picture), never read as text.
        .and_then(|root| if locust_core::image_files::is_image_path(&target.path) { Ok(String::new()) } else { read_document(&root, &target) });
    s.file_view = Some((target, body));
    s.viewer_image_view = None;
}

/// UTF-8 text, or extracted PDF/office/sheet words when the path is one of those.
fn read_document(root: &Path, target: &FileTarget) -> Result<String, String> {
    let ext = crate::file_menu::file_extension(&target.path);
    if ext == "pdf" || crate::pdf::is_pdf_path(&target.path) {
        let name = crate::file_menu::file_base_name(&target.path);
        // Write its text and page PNGs into .locust/attachments (Electron readablePdf, as pdf-pages.ts does), then show the text.
        let readable = crate::pdf::readable_pdf(
            root,
            &target.path,
            &name,
            crate::pdf::MAX_PICTURES,
        )?;
        // Its pages, for the viewer to draw (pdf_pages_ui, Electron PdfPages).
        crate::pdf_pages_ui::keep(root, &target.path, readable);
        let abs = crate::pdf::decide_pdf_pages(&target.path, &root.to_string_lossy())
            .map_err(str::to_owned)?;
        if abs.metadata().map(|m| m.len()).unwrap_or(0) > crate::pdf::MAX_PDF_BYTES {
            return Err(crate::pdf::pages_refusal("too-large").into());
        }
        let bytes = std::fs::read(&abs).map_err(|e| e.to_string())?;
        let reading = crate::pdf::read_pdf_bytes(&bytes)?;
        return Ok(crate::pdf::reading_text(&name, &reading));
    }
    if ext == "docx" || ext == "pptx" {
        let path = root.join(&target.path);
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > crate::office::MAX_OFFICE_FILE_BYTES {
            return Err("That document is too large to show here.".into());
        }
        let doc = crate::office::read_office(&bytes, &ext)?;
        return Ok(crate::office::document_words(
            crate::file_menu::file_base_name(&target.path).as_str(),
            &doc,
        ));
    }
    if ext == "xlsx" {
        let path = root.join(&target.path);
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let book = crate::office::read_xlsx(&bytes)?;
        let words = format_workbook(&book);
        // Its grid, for the viewer to draw (sheet_view, Electron SheetView).
        crate::sheet_view::keep(root, &target.path, book);
        return Ok(words);
    }
    if ext == "csv" || ext == "tsv" {
        let text = read_sheet_text(root, target)?;
        // Named after the file, as Electron's host names it (`csvWorkbook(name, ...)`).
        let book = crate::office::read_csv(crate::file_menu::file_base_name(&target.path).as_str(), &text, ext == "tsv");
        let words = format_workbook(&book);
        crate::sheet_view::keep(root, &target.path, book);
        return Ok(words);
    }
    read_file(root, target)
}

fn format_workbook(book: &crate::office::Workbook) -> String {
    let mut blocks = Vec::new();
    for sheet in &book.sheets {
        blocks.push(crate::office::DocumentBlock::Heading {
            level: 2,
            text: sheet.name.clone(),
        });
        let table_rows: Vec<Vec<String>> = sheet
            .rows
            .iter()
            .map(|row| row.iter().map(|c| c.text.clone()).collect())
            .collect();
        if !table_rows.is_empty() {
            blocks.push(crate::office::DocumentBlock::Table { rows: table_rows });
        }
    }
    crate::office::document_words(
        "workbook",
        &crate::office::OfficeDocument { kind: crate::office::DocumentKind::Word, blocks, more: 0 },
    )
}

/// Lost bytes belong to the runtime's voice. Code samples and inline mentions stay prose/code.
pub(super) fn reply(s: &State, body: &str, live: bool, p: &Palette) -> El {
    reply_with_copy(s, body, live, true, p)
}

/// A reply, with Copy under it only when `copy` (the turn's last reply, `speech_runs::last_speech`). Its status chips
/// are only drawn: this is a reply with no box to write into (a file preview).
pub(super) fn reply_with_copy(s: &State, body: &str, live: bool, copy: bool, p: &Palette) -> El {
    reply_with(s, body, live, copy, false, p)
}

/// A reply in a conversation's thread, which has the box: a status chip in it can be changed (StatusChip.tsx, 0.733).
pub(super) fn reply_in_thread(s: &State, body: &str, live: bool, copy: bool, p: &Palette) -> El {
    reply_with(s, body, live, copy, true, p)
}

/// A status in a teammate's tracker, being changed: the reply it is in, its tone now, and the row it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StatusChange {
    pub reply: u64,
    pub tone: String,
    pub item: String,
}

/// A press on a status chip: where the thread has a box it opens the other statuses under that reply (a second press
/// closes them); anywhere else the chip is the plain chip it was.
pub(super) fn status_pressed(s: &mut State, url: &str, can_draft: bool, reply: u64) {
    let Some((tone, item)) = crate::markdown_prep::status_of_link(url) else { return };
    if !can_draft {
        return;
    }
    let change = StatusChange { reply, tone, item };
    s.status_change = if s.status_change.as_ref() == Some(&change) { None } else { Some(change) };
}

/// The other statuses, under the reply whose chip was pressed: picking one writes "Mark <the row> as <status>." into the
/// box for the person to send, never a change made behind the teammate's back.
fn status_strip(change: &StatusChange, p: &Palette) -> El {
    let mut strip = row()
        .key("status-change")
        .role("menu")
        .label(format!("Change the status of {}", change.item))
        .gap(6.0)
        .flex_wrap()
        .center_y()
        .child(text(format!("Change the status of {}", change.item)).font(type_::LABEL).color(p.muted));
    for (label, tone) in crate::markdown_prep::STATUS_CHOICES.iter().filter(|(_, tone)| *tone != change.tone) {
        let ask = crate::markdown_prep::status_ask(&change.item, label);
        let look = crate::markdown_prep::reply_link_look(&crate::markdown_prep::status_link(tone, &change.item), p.dark);
        let mut chip = text((*label).to_owned()).font(type_::LABEL).padding(4.0).radius(3.0);
        if let Some((span, _)) = look {
            if let Some(ink) = span.color {
                chip = chip.color(ink);
            }
            if let Some(tint) = span.bg {
                chip = chip.fill(tint);
            }
        }
        strip = strip.child(
            chip.key(format!("status-change-{}", tone))
                .role("menuitem")
                .label((*label).to_owned())
                .on_press(move |s: &mut State| {
                    s.status_change = None;
                    s.draft.replace_text(&ask);
                    s.draft.focused = true;
                }),
        );
    }
    strip
}

fn reply_with(s: &State, body: &str, live: bool, copy: bool, can_draft: bool, p: &Palette) -> El {
    let mut out = col().w_full().gap(8.0);
    let mut start = 0;
    let mut offset = 0;
    let mut fence: Option<(char, usize)> = None;
    for line in body.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\r', '\n']);
        let trimmed = bare.trim_start_matches(' ');
        let indent = bare.len() - trimmed.len();
        let marker = trimmed.chars().next().unwrap_or(' ');
        let count = trimmed.chars().take_while(|c| *c == marker).count();
        if indent <= 3 && (marker == '`' || marker == '~') && count >= 3 {
            match fence {
                Some((kind, size)) if kind == marker && count >= size
                    && trimmed[count..].trim().is_empty() => fence = None,
                None => fence = Some((marker, count)),
                _ => {}
            }
        } else if fence.is_none() && let Some(bytes) = bare
            .strip_prefix("<truncated ").and_then(|t| t.strip_suffix(" bytes>"))
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) {
            if start < offset {
                out = out.child(reply_markdown(s, &body[start..offset], live, false, can_draft, p));
            }
            out = out.child(text(format!("{bytes} bytes the runtime did not keep"))
                .key(format!("runtime-gap-{offset}"))
                .role("note").font(type_::LABEL).color(p.muted).w_full().wrap());
            start = offset + line.len();
        }
        offset += line.len();
    }
    if start == 0 {
        return reply_markdown(s, body, live, copy, can_draft, p);
    }
    if start < body.len() {
        out = out.child(reply_markdown(s, &body[start..], live, false, can_draft, p));
    }
    // A reply with a gap in it still copies whole, once.
    if copy {
        out = out.child(row().justify_end().child(copy_button(s.copy_feedback.as_ref(), body.into(), p)));
    }
    out
}

#[cfg(test)]
pub(super) fn reply_markdown_for_tests(s: &State, body: &str, p: &Palette) -> El {
    reply_markdown(s, body, false, true, true, p)
}

/// The reply as it is drawn while it is still arriving.
#[cfg(test)]
pub(super) fn live_reply_markdown_for_tests(s: &State, body: &str, p: &Palette) -> El {
    reply_markdown(s, body, true, true, true, p)
}

/// Sizes a rasterised math PNG (drawn at 2x in `math_tex::render_png`) to its point dimensions so
/// `Markdown`'s layout does not measure the image at full column width and push the reply off-screen (LIVE2 #1).
pub(super) fn math_image_size(px: [u32; 2], max_w: f32) -> Option<(f32, f32)> {
    let [pw, ph] = px;
    if pw == 0 || ph == 0 {
        return None;
    }
    let scale = 2.0_f32;
    let mut w = pw as f32 / scale;
    let mut h = ph as f32 / scale;
    if w > max_w {
        h *= max_w / w;
        w = max_w;
    }
    Some((w, h))
}

fn math_image_el(image: &rune::Image, alt: &str) -> Option<El> {
    let (w, h) = math_image_size(image.size()?, super::thread_view::THREAD_MAX)?;
    Some(
        el().role("image")
            .label(if alt.is_empty() { "Math" } else { alt })
            .size(w, h)
            .pixels(image.pixels()),
    )
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum InlinePiece {
    Text(String),
    Math {
        alt: String,
        src: String,
        baseline: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReplySegment {
    Markdown(String),
    InlineMathPara(Vec<InlinePiece>),
}

fn is_block_starter(line: &str) -> bool {
    let t = line.trim_start();
    if t.is_empty() {
        return true;
    }
    if t.starts_with('#')
        || t.starts_with('>')
        || t.starts_with('|')
        || t.starts_with("```")
        || t.starts_with("~~~")
    {
        return true;
    }
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    if matches!(t, "---" | "***" | "___")
        || (t.len() >= 3 && (t.bytes().all(|b| b == b'-') || t.bytes().all(|b| b == b'*') || t.bytes().all(|b| b == b'_')))
    {
        return true;
    }
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && digits <= 9
        && t[digits..].starts_with(['.', ')'])
        && t[digits + 1..].starts_with([' ', '\t'])
    {
        return true;
    }
    false
}

/// Parse a paragraph into prose and inline-math pieces when it contains at least one inline
/// `![alt](locust-math:...)` listed in `inline_math`.
fn parse_inline_math_pieces(
    para: &str,
    inline_math: &std::collections::HashMap<String, f32>,
) -> Option<Vec<InlinePiece>> {
    let marker = format!("]({}", crate::math_tex::MATH_SRC_PREFIX);
    let mut pieces = Vec::new();
    let mut pos = 0usize;
    let mut has_math = false;
    while let Some(rel_open) = para[pos..].find("![") {
        let open = pos + rel_open;
        let Some(rel_mid) = para[open + 2..].find(&marker) else {
            break;
        };
        let mid = open + 2 + rel_mid;
        let src_start = mid + 2;
        let Some(rel_close) = para[src_start..].find(')') else {
            break;
        };
        let close = src_start + rel_close;
        let src = &para[src_start..close];
        if let Some(&baseline) = inline_math.get(src) {
            if open > pos {
                pieces.push(InlinePiece::Text(para[pos..open].replace('\n', " ")));
            }
            pieces.push(InlinePiece::Math {
                alt: para[open + 2..mid].to_owned(),
                src: src.to_owned(),
                baseline,
            });
            has_math = true;
            pos = close + 1;
        } else {
            pos = close + 1;
        }
    }
    if !has_math {
        return None;
    }
    if pos < para.len() {
        pieces.push(InlinePiece::Text(para[pos..].replace('\n', " ")));
    }
    Some(pieces)
}

/// Split prepared Markdown into plain Markdown chunks and paragraphs containing inline math so
/// inline formulas draw inside their sentence instead of breaking it into blocks.
pub(super) fn split_inline_math_segments(
    markdown: &str,
    inline_math: &std::collections::HashMap<String, f32>,
) -> Vec<ReplySegment> {
    if inline_math.is_empty() {
        return vec![ReplySegment::Markdown(markdown.to_owned())];
    }
    let mut segments = Vec::new();
    let mut md_buf = String::new();
    let mut para_buf = String::new();
    let mut fence: Option<(u8, usize)> = None;

    let flush_md = |segments: &mut Vec<ReplySegment>, md_buf: &mut String| {
        if !md_buf.trim().is_empty() {
            segments.push(ReplySegment::Markdown(std::mem::take(md_buf)));
        } else {
            md_buf.clear();
        }
    };
    let flush_para = |segments: &mut Vec<ReplySegment>,
                      md_buf: &mut String,
                      para_buf: &mut String| {
        if para_buf.is_empty() {
            return;
        }
        let para = std::mem::take(para_buf);
        if let Some(pieces) = parse_inline_math_pieces(para.trim_end_matches(['\r', '\n']), inline_math) {
            flush_md(segments, md_buf);
            segments.push(ReplySegment::InlineMathPara(pieces));
        } else {
            md_buf.push_str(&para);
        }
    };

    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        let lead = trimmed.trim_start();
        if let Some((mark, n)) = fence {
            md_buf.push_str(line);
            let run = lead.bytes().take_while(|&b| b == mark).count();
            if run >= n && lead[run..].trim().is_empty() {
                fence = None;
            }
            continue;
        }
        if lead.starts_with("```") || lead.starts_with("~~~") {
            let mark = lead.as_bytes()[0];
            let run = lead.bytes().take_while(|&b| b == mark).count();
            flush_para(&mut segments, &mut md_buf, &mut para_buf);
            fence = Some((mark, run));
            md_buf.push_str(line);
            continue;
        }
        if trimmed.trim().is_empty() {
            flush_para(&mut segments, &mut md_buf, &mut para_buf);
            md_buf.push_str(line);
            continue;
        }
        let standalone_display_math = lead.starts_with("![")
            && lead.ends_with(')')
            && lead.contains(crate::math_tex::MATH_SRC_PREFIX)
            && parse_inline_math_pieces(lead, inline_math).is_none();
        if standalone_display_math || (para_buf.is_empty() && is_block_starter(trimmed)) {
            flush_para(&mut segments, &mut md_buf, &mut para_buf);
            md_buf.push_str(line);
            continue;
        }
        para_buf.push_str(line);
    }
    flush_para(&mut segments, &mut md_buf, &mut para_buf);
    flush_md(&mut segments, &mut md_buf);
    segments
}

/// Resolved single-line height and baseline (in points from box top) for `type_::BODY`.
pub(super) fn body_text_metrics() -> (f32, f32) {
    let (_, h) = rune::text::measure("H", &[], type_::BODY, None);
    let lh = if h > 0.0 { h } else { type_::BODY.line_px() };
    let base = rune::text::ink("H", type_::BODY)
        .map(|i| i.bounds[1] + i.bounds[3])
        .unwrap_or(lh * 0.78);
    (lh, base)
}

pub(super) fn inline_markdown_spans(
    inl: &rune_kit::markdown::Inline,
    p: &Palette,
) -> Vec<rune::Span> {
    let mut spans = Vec::new();
    for &(a, b, st) in &inl.spans {
        let mut sp = rune::Span::new(a, b);
        match st {
            rune_kit::markdown::Style::Strong => sp.weight = Some(600.0),
            rune_kit::markdown::Style::Em => sp.italic = true,
            rune_kit::markdown::Style::Code => {
                sp.mono = true;
                sp.weight = Some(400.0);
                sp.size = Some(type_::BODY.size * 0.9);
                sp = sp.bg(p.raised, 4.0);
            }
            rune_kit::markdown::Style::Link => {
                let url = inl
                    .links
                    .iter()
                    .find(|l| (l.0, l.1) == (a, b))
                    .map(|l| l.2.as_str());
                if let Some((own, _)) =
                    url.and_then(|u| crate::markdown_prep::reply_link_look(u, p.dark))
                {
                    sp = rune::Span {
                        start: a as u32,
                        end: b as u32,
                        ..own
                    };
                } else {
                    sp.color = Some(p.green);
                    sp.underline = Some(p.green);
                }
            }
            rune_kit::markdown::Style::CodePad | rune_kit::markdown::Style::Strike => continue,
        }
        spans.push(sp);
    }
    spans
}

/// Byte ranges of strikethrough runs in `inl`, matching `rune_kit::markdown::Renderer::select_inline`.
pub(super) fn inline_strike_ranges(inl: &rune_kit::markdown::Inline) -> Vec<(usize, usize)> {
    inl.spans
        .iter()
        .filter_map(|&(a, b, st)| match st {
            rune_kit::markdown::Style::Strike => Some((a, b)),
            _ => None,
        })
        .collect()
}

fn handle_reply_link(s: &mut State, url: &str, can_draft: bool, chip_reply: u64) {
    if crate::markdown_prep::status_of_link(url).is_some() {
        status_pressed(s, url, can_draft, chip_reply);
    } else if crate::markdown_prep::is_own_mark(url) {
        // A status chip or a colour swatch: drawn, never opened.
    } else if let Some(target) = file_target(url) {
        open_file(s, target);
    } else {
        // A web address opens in the person's browser; any other scheme is refused, and says so.
        crate::published_cards::open_web_link(s, url);
    }
}

fn wire_inline_links(
    mut item: El,
    links: &[(usize, usize, String)],
    p: &Palette,
    can_draft: bool,
    chip_reply: u64,
) -> El {
    let default_hover = rune::LinkPaint::new().color(p.green).underline(p.green);
    for (a, b, url) in links {
        let hover = crate::markdown_prep::reply_link_look(url, p.dark)
            .map_or(default_hover, |(_, h)| h);
        let mut link = rune::Link::new(*a..*b, url.clone()).hover(hover);
        if let Some(help) = crate::markdown_prep::link_host(url) {
            link = link.help(help);
        }
        item = item.link(link);
    }
    item.on_link(move |s: &mut State, url, _tx| handle_reply_link(s, url, can_draft, chip_reply))
}

fn prose_flex_items(
    raw: &str,
    p: &Palette,
    space_w: f32,
    trail_space: bool,
    can_draft: bool,
    chip_reply: u64,
) -> Vec<El> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let parsed = rune_kit::markdown::parse(trimmed);
    let inl = parsed.into_iter().find_map(|b| match b {
        rune_kit::markdown::Block::Para(i) => Some(i),
        _ => None,
    });
    let (plain, spans, strikes, links) = match inl {
        Some(i) => {
            let sp = inline_markdown_spans(&i, p);
            let st = inline_strike_ranges(&i);
            (i.text, sp, st, i.links)
        }
        None => (trimmed.to_owned(), Vec::new(), Vec::new(), Vec::new()),
    };
    if plain.is_empty() {
        return Vec::new();
    }
    // Keep short phrases intact so tests and selection see contiguous text, while splitting long
    // runs into words so wrapping flows smoothly around inline equations.
    if plain.chars().count() <= 40 {
        let mut item = rune::rich(plain, spans)
            .font(type_::BODY)
            .color(p.text)
            .exact_lines();
        for (a, b) in strikes {
            item = item.strike(a, b, None);
        }
        if !links.is_empty() {
            item = wire_inline_links(item, &links, p, can_draft, chip_reply);
        }
        if trail_space {
            item = item.mr(space_w);
        }
        return vec![item];
    }
    let mut words = Vec::new();
    let mut idx = 0usize;
    let bytes = plain.as_bytes();
    while idx < bytes.len() {
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        let start = idx;
        while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if start < idx {
            words.push((start, idx));
        }
    }
    let count = words.len();
    words
        .into_iter()
        .enumerate()
        .map(|(k, (start, end))| {
            let word = plain[start..end].to_owned();
            let word_spans: Vec<rune::Span> = spans
                .iter()
                .filter_map(|sp| {
                    let a = (sp.start as usize).max(start);
                    let b = (sp.end as usize).min(end);
                    (a < b).then(|| {
                        let mut s = sp.clone();
                        s.start = (a - start) as u32;
                        s.end = (b - start) as u32;
                        s
                    })
                })
                .collect();
            let mut item = rune::rich(word, word_spans)
                .font(type_::BODY)
                .color(p.text)
                .exact_lines();
            for &(sa, sb) in &strikes {
                let a = sa.max(start);
                let b = sb.min(end);
                if a < b {
                    item = item.strike(a - start, b - start, None);
                }
            }
            let word_links: Vec<(usize, usize, String)> = links
                .iter()
                .filter_map(|(la, lb, url)| {
                    let a = (*la).max(start);
                    let b = (*lb).min(end);
                    (a < b).then(|| (a - start, b - start, url.clone()))
                })
                .collect();
            if !word_links.is_empty() {
                item = wire_inline_links(item, &word_links, p, can_draft, chip_reply);
            }
            if k + 1 < count || trail_space {
                item = item.mr(space_w);
            }
            item
        })
        .collect()
}

fn render_inline_math_para(
    pieces: &[InlinePiece],
    math_pngs: &std::collections::HashMap<String, std::sync::Arc<[u8]>>,
    image_cache: Option<&std::sync::Arc<rune::ImageCache>>,
    p: &Palette,
    can_draft: bool,
    chip_reply: u64,
) -> El {
    let space_w = (rune::text::measure("x x", &[], type_::BODY, None).0
        - rune::text::measure("xx", &[], type_::BODY, None).0)
    .max(3.0);
    let (lh, text_base) = body_text_metrics();
    let mut items: Vec<El> = Vec::new();
    let mut i = 0usize;
    let mut pending_prefix: Option<String> = None;
    let mut consumed_prefix_of_next: usize = 0;

    while i < pieces.len() {
        match &pieces[i] {
            InlinePiece::Text(full) => {
                let slice = &full[consumed_prefix_of_next..];
                consumed_prefix_of_next = 0;
                let next_is_math = matches!(pieces.get(i + 1), Some(InlinePiece::Math { .. }));
                if next_is_math && !slice.is_empty() && !slice.ends_with(char::is_whitespace) {
                    if let Some(ws) = slice.rfind(char::is_whitespace) {
                        let body = &slice[..ws];
                        let attach = &slice[ws..].trim_start();
                        items.extend(prose_flex_items(body, p, space_w, true, can_draft, chip_reply));
                        pending_prefix = Some((*attach).to_owned());
                    } else {
                        pending_prefix = Some(slice.to_owned());
                    }
                } else {
                    let trail = slice.ends_with(char::is_whitespace) || next_is_math;
                    items.extend(prose_flex_items(slice, p, space_w, trail, can_draft, chip_reply));
                }
            }
            InlinePiece::Math { alt, src, baseline } => {
                let mut suffix: Option<String> = None;
                let mut after_space = false;
                if let Some(InlinePiece::Text(next_text)) = pieces.get(i + 1) {
                    if !next_text.is_empty() && !next_text.starts_with(char::is_whitespace) {
                        let end = next_text.find(char::is_whitespace).unwrap_or(next_text.len());
                        suffix = Some(next_text[..end].to_owned());
                        consumed_prefix_of_next = end;
                        after_space = end < next_text.len();
                    } else {
                        after_space = !next_text.trim().is_empty();
                    }
                } else if i + 1 < pieces.len() {
                    after_space = true;
                }
                let img_el = math_pngs
                    .get(src)
                    .and_then(|bytes| {
                        let cache = image_cache?;
                        let image = cache.load(bytes.clone());
                        let _ = image.wait();
                        let px = image.size()?;
                        let (w, h) = math_image_size(px, super::thread_view::THREAD_MAX)?;
                        let scale_y = h / (px[1] as f32 / 2.0);
                        let base = *baseline * scale_y;
                        let d: f32 =
                            (2.0_f32 * ((text_base - lh * 0.5) - (base - h * 0.5))).clamp(-12.0, 12.0);
                        let raw_img = el()
                            .role("image")
                            .label(if alt.is_empty() { "Math" } else { alt.as_str() })
                            .size(w, h)
                            .pixels(image.pixels());
                        Some(if d > 0.5 {
                            el().pt(d).child(raw_img)
                        } else if d < -0.5 {
                            el().pb(-d).child(raw_img)
                        } else {
                            raw_img
                        })
                    })
                    .unwrap_or_else(|| {
                        text(crate::math_tex::fallback_tex(alt, false))
                            .font(type_::BODY)
                            .color(p.text)
                            .exact_lines()
                    });
                let mut unit = row().items_center();
                if let Some(pre) = pending_prefix.take() {
                    unit = unit.child(text(pre).font(type_::BODY).color(p.text).exact_lines());
                }
                unit = unit.child(img_el);
                if let Some(suf) = suffix {
                    unit = unit.child(text(suf).font(type_::BODY).color(p.text).exact_lines());
                }
                if after_space {
                    unit = unit.mr(space_w);
                }
                items.push(unit);
            }
        }
        i += 1;
    }
    row().w_full().flex_wrap().items_center().children(items)
}

/// A live reply without the table header still arriving at its end (settledText.ts: "holds a header row back until the
/// rule under it has arrived"). rune-kit draws a `|` line of two cells as a table's head at once, so a line that turns
/// out to be prose would redraw from a table into a paragraph; held, it is drawn once its rule begins, as the table it
/// is. Only a header with nothing under it yet is held: not a row under another `|` line, nor a line inside a fence.
fn without_bare_header(body: &str) -> &str {
    let lines = body.strip_suffix('\n').unwrap_or(body);
    let at = lines.rfind('\n').map_or(0, |i| i + 1);
    let last = &lines[at..];
    let piped = |line: &str| line.trim_start().starts_with('|');
    let rule = |line: &str| line.trim().chars().all(|c| "|:- ".contains(c));
    if !piped(last) || rule(last) {
        return body;
    }
    let above = &lines[..at.saturating_sub(1)];
    let above_line = above.rsplit('\n').next().unwrap_or("");
    let in_fence = above.lines().filter(|l| l.trim_start().starts_with("```")).count() % 2 == 1;
    if (at > 0 && piped(above_line)) || in_fence { body } else { &body[..at] }
}

fn reply_markdown(s: &State, body: &str, live: bool, copy: bool, can_draft: bool, p: &Palette) -> El {
    let chip_reply = body_key(body);
    let feedback = s.copy_feedback.clone();
    let crate::markdown_prep::Prepared {
        markdown: prepared,
        undrawn,
        math: math_pngs,
        inline_math,
    } = crate::markdown_prep::prepare_reply(if live { without_bare_header(body) } else { body }, p.dark);
    let diagram_tabs = s.diagram_code_tab.clone();
    let image_cache = s.image_cache.clone();
    let pictures = crate::step_pictures::reply_pictures(s, &prepared);
    let framed: std::collections::HashSet<String> = pictures.keys().cloned().collect();
    let style = markdown_style(p);
    let dark = p.dark;

    let render_md_chunk = |chunk: String| -> El {
        let frame_palette = Palette::new(dark);
        let picture_palette = Palette::new(dark);
        let feedback = feedback.clone();
        let undrawn = undrawn.clone();
        let math_pngs = math_pngs.clone();
        let diagram_tabs = diagram_tabs.clone();
        let image_cache = image_cache.clone();
        let pictures = pictures.clone();
        let framed = framed.clone();
        Markdown::new(chunk)
            .streaming(live)
            .selectable(true)
            .on_link(move |s: &mut State, url| {
                if crate::markdown_prep::status_of_link(url).is_some() {
                    status_pressed(s, url, can_draft, chip_reply);
                } else if crate::markdown_prep::is_own_mark(url) {
                    // A status chip or a colour swatch: drawn, never opened.
                } else if let Some(target) = file_target(url) {
                    open_file(s, target);
                } else {
                    // A web address opens in the person's browser; any other scheme is refused, and says so.
                    crate::published_cards::open_web_link(s, url);
                }
            })
            // Where a web link really goes, under the pointer, so its label cannot lie about it (linkHost).
            .link_help(crate::markdown_prep::link_host)
            // A table's status as a chip, a colour code's swatch (statusChips.ts, hexColourOf).
            .link_style(move |url| crate::markdown_prep::reply_link_look(url, dark))
            .image_frame(move |shown, frame| {
                if frame.src.starts_with(crate::math_tex::MATH_SRC_PREFIX) {
                    return math_image_el(frame.image, frame.alt).unwrap_or(shown);
                }
                // A picture from the conversation's folder is a rounded card with its caption; math stays inline.
                if !framed.contains(frame.src) {
                    return shown;
                }
                crate::step_pictures::reply_picture_card(frame.image, frame.src, frame.alt, &picture_palette).unwrap_or(shown)
            })
            .images(move |src| {
                if let Some(picture) = pictures.get(src) {
                    return Some(picture.clone());
                }
                let bytes = math_pngs.get(src)?;
                let cache = image_cache.as_ref()?;
                let image = cache.load(bytes.clone());
                // Headless snaps and the first live frame need the decode done before paint.
                let _ = image.wait();
                Some(image)
            })
            .highlight(move |language, code| highlight_reply_code(live, language, code))
            .diagram_frame({
                let tabs = diagram_tabs.clone();
                let palette = Palette::new(dark);
                move |card, frame| {
                    let key = body_key(frame.source);
                    let code_on = tabs.get(&key).copied().unwrap_or(false);
                    let kind = crate::diagram::diagram_kind_words(crate::diagram::diagram_kind_of(
                        frame.source,
                    ));
                    let p = &palette;
                    let diagram_tab = el()
                        .key(format!("diagram-{}/tab-diagram", frame.index))
                        .padding(6.0)
                        .radius(4.0)
                        .fill(if !code_on { p.selected } else { p.raised })
                        .child(
                            text(crate::diagram::TAB_DIAGRAM)
                                .font(type_::LABEL)
                                .color(if !code_on { p.text } else { p.secondary }),
                        )
                        .on_press(move |s: &mut State| {
                            s.diagram_code_tab.insert(key, false);
                        });
                    let code_tab = el()
                        .key(format!("diagram-{}/tab-code", frame.index))
                        .padding(6.0)
                        .radius(4.0)
                        .fill(if code_on { p.selected } else { p.raised })
                        .child(
                            text(crate::diagram::TAB_CODE)
                                .font(type_::LABEL)
                                .color(if code_on { p.text } else { p.secondary }),
                        )
                        .on_press(move |s: &mut State| {
                            s.diagram_code_tab.insert(key, true);
                        });
                    let mut bar = row()
                        .w_full()
                        .gap(8.0)
                        .child(
                            row()
                                .gap(4.0)
                                .role("tablist")
                                .label(crate::diagram::TABS_ARIA)
                                .child(diagram_tab)
                                .child(code_tab),
                        )
                        .child(text(kind).font(type_::LABEL).color(p.muted));
                    if frame.open {
                        // Electron shows this while its WebView draws. An unclosed Mermaid fence reaches the
                        // reply as plain code (diagram::prepare_reply_markdown), so this is only for a fence
                        // rune-kit still counts as open.
                        bar = bar.child(
                            text(crate::diagram::WAITING)
                                .font(type_::LABEL)
                                .color(p.muted),
                        );
                    }
                    let body = if code_on {
                        // Plain source — do not re-enter a ```mermaid fence.
                        col()
                            .w_full()
                            .gap(4.0)
                            .child(text("mermaid").font(type_::LABEL).color(p.muted))
                            .child(
                                text(frame.source.to_owned())
                                    .font(type_::LABEL)
                                    .color(p.text)
                                    .wrap_anywhere(),
                            )
                    } else {
                        card
                    };
                    col()
                        .key(format!("diagram-{}", frame.index))
                        .w_full()
                        .gap(8.0)
                        .role("figure")
                        .label("A diagram in this reply")
                        .child(bar)
                        .child(body)
                }
            })
            .code_frame({
                let feedback = feedback.clone();
                let undrawn = undrawn.clone();
                move |card, frame| {
                    let p = &frame_palette;
                    if let Some(message) = undrawn.get(&frame.index) {
                        let caption = crate::diagram::not_drawn_caption(message);
                        return col()
                            .key(format!("diagram-text-{}", frame.index))
                            .w_full()
                            .gap(4.0)
                            .role("figure")
                            .child(card)
                            .child(
                                text(caption)
                                    .font(type_::LABEL)
                                    .color(p.muted)
                                    .wrap_anywhere(),
                            );
                    }
                    let mut controls = row().w_full().justify_end().gap(8.0);
                    if let Some(open_page) = crate::reply_page_ui::control(frame.source, frame.language, frame.open, frame.index, p) {
                        controls = controls.child(open_page);
                    }
                    if let Some(target) = frame.language.and_then(file_target) {
                        let label = target.path.clone();
                        controls = controls.child(
                            el().child(text(label).font(type_::LABEL).color(p.green))
                                .on_press(move |s: &mut State| open_file(s, target.clone())),
                        );
                    }
                    col()
                        .w_full()
                        .gap(4.0)
                        .child(controls.child(copy_button(
                            feedback.as_ref(),
                            frame.source.to_owned(),
                            p,
                        )))
                        .child(card)
                }
            })
            .render(&style)
    };

    let segments = split_inline_math_segments(&prepared, &inline_math);
    let body_el = if segments.len() == 1
        && matches!(segments.first(), Some(ReplySegment::Markdown(_)))
    {
        render_md_chunk(prepared)
    } else {
        let mut stack = col().w_full().gap(12.0);
        for seg in segments {
            match seg {
                ReplySegment::Markdown(md) => stack = stack.child(render_md_chunk(md)),
                ReplySegment::InlineMathPara(pieces) => {
                    stack = stack.child(render_inline_math_para(
                        &pieces,
                        &math_pngs,
                        image_cache.as_ref(),
                        p,
                        can_draft,
                        chip_reply,
                    ));
                }
            }
        }
        stack
    };
    let mut out = col().w_full().gap(8.0).child(body_el);
    if let Some(cards) = crate::published_cards::cards(s, body, p) {
        out = out.child(cards);
    }
    // Inline file spans are also available as explicit file controls.
    let paths = crate::invariants::regex(r"`([^`\n]+)`");
    let mut seen = std::collections::HashSet::new();
    for cap in paths.captures_iter(body) {
        if let Some(raw) = cap.get(1)
            && let Some(target) = file_target(raw.as_str())
            && seen.insert(raw.as_str())
        {
            out = out.child(
                el().child(
                    text(raw.as_str().to_owned())
                        .font(type_::LABEL)
                        .color(p.green),
                )
                .on_press(move |s: &mut State| open_file(s, target.clone())),
            );
        }
    }
    if can_draft && let Some(change) = s.status_change.as_ref().filter(|c| c.reply == chip_reply) {
        out = out.child(status_strip(change, p));
    }
    if !copy {
        return out;
    }
    out.child(
        row()
            .justify_end()
            .child(copy_button(s.copy_feedback.as_ref(), body.into(), p)),
    )
}

pub(super) fn viewer(s: &State, p: &Palette) -> Option<El> {
    // installed-editors.ts: only the editors whose command is on PATH are offered.
    let editors = locust_core::installed_editors::installed_editors();
    let editor_ids: Vec<&str> = editors.iter().map(|e| e.id.as_str()).collect();
    viewer_on(s, p, locust_core::installed_editors::this_platform(), &editor_ids)
}

/// The viewer on `platform` (Node's name) with `editor_ids` installed: its menu (file-menu.ts) and what each choice
/// copies, in that system's words and separator.
pub(super) fn viewer_on(s: &State, p: &Palette, platform: &'static str, editor_ids: &[&str]) -> Option<El> {
    let (target, body) = s.file_view.as_ref()?;
    let folder = selected_conversation(s)
        .and_then(|(t, _)| t.folder.clone());
    let absolute = crate::file_menu::rooted_path(&target.path, folder.as_deref());
    let menu = crate::file_menu::file_menu_choices(
        &absolute,
        folder.as_deref(),
        platform,
        editor_ids,
        false,
    );
    let mut menu_row = row().w_full().gap(6.0);
    for choice in menu {
        let label = choice.label;
        let id = choice.id;
        let absolute = absolute.clone();
        let folder = folder.clone();
        menu_row = menu_row.child(
            el().padding(4.0)
                .radius(4.0)
                .fill(p.raised)
                .key(format!("file-reader/menu/{id:?}"))
                .role("button")
                .label(label)
                .child(text(label).font(type_::LABEL).color(p.secondary))
                .on_press_tx(move |s: &mut State, tx| match id {
                    crate::file_menu::FileMenuId::CopyPath => {
                        request_copy(tx, crate::file_menu::system_path(&absolute, platform));
                    }
                    crate::file_menu::FileMenuId::CopyRelative => {
                        let rel = crate::file_menu::relative_to_folder(
                            &absolute,
                            folder.as_deref(),
                            platform,
                        );
                        // Written with the system's separator, as Copy path is (file-menu.ts `systemPath`).
                        request_copy(tx, crate::file_menu::system_path(&rel, platform));
                    }
                    crate::file_menu::FileMenuId::CopyName => {
                        request_copy(tx, crate::file_menu::file_base_name(&absolute));
                    }
                    crate::file_menu::FileMenuId::Reveal => {
                        let path = std::path::PathBuf::from(&absolute);
                        let roots: Vec<_> = folder
                            .as_ref()
                            .map(|f| vec![std::path::PathBuf::from(f)])
                            .unwrap_or_default();
                        if let crate::file_menu::RevealDecision::Ok { path, .. } =
                            crate::file_menu::decide_reveal(&path.to_string_lossy(), &roots)
                        {
                            let _ = crate::file_menu::reveal_path(&path);
                        }
                        let _ = s;
                    }
                    crate::file_menu::FileMenuId::Vscode | crate::file_menu::FileMenuId::Cursor => {
                        let want = if id == crate::file_menu::FileMenuId::Vscode { "vscode" } else { "cursor" };
                        if let Some(editor) = locust_core::installed_editors::installed_editors()
                            .iter()
                            .find(|e| e.id.as_str() == want)
                        {
                            let _ = locust_core::installed_editors::launch_editor(&editor.command, &absolute);
                        }
                    }
                    crate::file_menu::FileMenuId::Attach
                        if !s.attached.iter().any(|a| a == &absolute) =>
                    {
                        s.attached.push(absolute.clone());
                    }
                    _ => {}
                }),
        );
    }
    let picture = locust_core::image_files::is_image_path(&target.path);
    // FileViewer.tsx's head: the file by its own name, the whole path in its hover (a viewer that says only
    // `report.md` cannot tell you which); then Show it in the file manager and Save a copy. Nothing here hands the
    // file to the system to run.
    let name = target.path.replace('\\', "/").rsplit('/').next().unwrap_or(&target.path).to_owned();
    let mut head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text(name).color(p.text).wrap_anywhere().key("file-reader/name").label(target.path.clone()).help(target.path.clone()));
    if !picture && target.start > 0 {
        head = head.child(text(format!("lines {}\u{2013}{}", target.start, target.end)).font(type_::LABEL).color(p.muted));
    }
    let reveal_path = target.path.clone();
    let save_path = target.path.clone();
    head = head
        .child(rune::spacer())
        .child(
            el().padding(4.0)
                .key("file-reader/reveal")
                .role("button")
                .label("Show it in the file manager")
                .help("Show it in the file manager")
                .child(text("Show").font(type_::LABEL).color(p.secondary))
                .on_press(move |s: &mut State| crate::handed_files::reveal_handed_file(s, "file-reader", &reveal_path)),
        )
        .child(
            el().padding(4.0)
                .key("file-reader/save")
                .role("button")
                .label("Save a copy\u{2026}")
                .help("Save a copy\u{2026}")
                .child(text("Save a copy\u{2026}").font(type_::LABEL).color(p.secondary))
                .on_press_tx(move |s: &mut State, tx| crate::handed_files::save_handed_copy(s, tx, "file-reader", &save_path)),
        );
    let mut pane = col()
        .key("file-reader")
        .w(340.0)
        .h_full()
        .padding(16.0)
        .gap(12.0)
        .fill(p.sidebar)
        .child(head);
    if let Some((_, said)) = s.handed_file_refusal.as_ref().filter(|(scope, _)| scope == "file-reader") {
        pane = pane.child(text(said.clone()).font(type_::LABEL).color(p.amber).wrap_anywhere().role("status"));
    }
    pane = pane.child(menu_row)
        .child(
            el().child(text("Close").color(p.secondary))
                .on_press(|s: &mut State| {
                    s.file_view = None;
                    s.viewer_image_view = None;
                }),
        );
    // FileViewer.tsx (0.425): a web page runs, its source a tab away. Electron frames it in the viewer; here its Page
    // runs it at its own address in the browser beside the conversation, and the viewer stays its Source. Any other
    // file runs nothing.
    if body.is_ok() && matches!(locust_core::text_files::extension_of(&target.path).to_ascii_lowercase().as_str(), "html" | "htm") {
        pane = pane
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(crate::reply_page_ui::page_control("file-reader/page".into(), target.path.clone(), p))
                    .child(el().key("file-reader/source").role("tab").label("Source").child(text("Source").font(type_::LABEL).color(p.text))),
            )
            .child(text(crate::reply_page_ui::RUNS_APART).font(type_::LABEL).color(p.muted).wrap_anywhere());
    }
    // A PDF is shown as the pages Locust drew of it (PdfPages, 0.714); its words stay its text when none were drawn.
    let pdf_pages = if body.is_ok() && crate::pdf::is_pdf_path(&target.path) { crate::pdf_pages_ui::pages(s, &target.path, p) } else { None };
    // A spreadsheet is shown as its grid (SheetView, 0.364).
    let sheet = if body.is_ok() { crate::sheet_view::grid(s, &target.path, p) } else { None };
    match body {
        Ok(_) if pdf_pages.is_some() => pane = pane.children(pdf_pages),
        Ok(_) if sheet.is_some() => pane = pane.children(sheet),
        Ok(_) if picture => {
            // ThreadImage's press: the picture at the pane's width, with zoom and pan controls (ZoomableImage.tsx).
            pane = match crate::step_pictures::thread_picture(s, &target.path).and_then(|image| crate::step_pictures::viewer_picture(s, &image, &target.path, p)) {
                Some(shown) => pane.child(shown),
                None => pane.child(text("This picture cannot be shown here.").color(p.muted).wrap_anywhere()),
            };
        }
        // Markdown is rendered as prose, through the renderer every reply uses: drawn, never interpreted.
        Ok(body) if locust_core::text_files::viewer_mode(&target.path) == locust_core::text_files::ViewerMode::Markdown => {
            pane = pane.child(
                col()
                    .key("file-reader/prose")
                    .grow(1.0)
                    .min_h(0.0)
                    .w_full()
                    .scroll(ScrollSpec { stick_bottom: false })
                    .child(reply_markdown(s, body, false, false, false, p)),
            );
        }
        // Anything else verbatim, in its own scroll box.
        Ok(body) => {
            let mut lines = col().w_full().gap(2.0);
            for (i, line) in body
                .lines()
                .enumerate()
                .skip(target.start.saturating_sub(4))
            {
                lines = lines.child(
                    text(format!("{:4}  {line}", i + 1))
                        .font(type_::LABEL)
                        .color(p.text)
                        .wrap_anywhere()
                        .fill(if (target.start..=target.end).contains(&(i + 1)) {
                            p.selected
                        } else {
                            p.sidebar
                        }),
                );
            }
            pane = pane
                .child(copy_button(s.copy_feedback.as_ref(), body.clone(), p))
                .child(
                    col()
                        .key("file-reader/code")
                        .grow(1.0)
                        .min_h(0.0)
                        .w_full()
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .child(lines),
                );
        }
        Err(error) => pane = pane.child(text(error.clone()).color(p.red).wrap_anywhere()),
    }
    Some(pane.child(text(VIEWER_REGISTER).font(type_::LABEL).color(p.muted).wrap_anywhere().key("file-reader/register")))
}

/// FileViewer.tsx's register: why there is no open button, said as what it does (0.515).
const VIEWER_REGISTER: &str = "Shown here only: Locust never runs a file a teammate made, or opens it in another app.";

#[cfg(test)]
#[path = "the_file_viewer_tests.rs"]
mod the_file_viewer_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_ranges_and_urls() {
        for value in ["src/main.rs:12-18", "src/main.rs#L12-L18"] {
            let t = file_target(value).unwrap();
            assert_eq!((t.start, t.end), (12, 18));
            assert_eq!(t.path, "src/main.rs");
        }
        assert!(file_target("https://example.com/a.rs").is_none());
        assert!(file_target("hello").is_none());
    }
    #[test]
    fn code_preserves_unicode_and_has_bounded_colouring() {
        let code = "const café = 'é'; // hi\n";
        let h = rune_kit::syntax::highlight(Some("ts"), code);
        assert!(!h.is_empty());
        for (r, _) in h.iter() {
            assert!(code.get(r.clone()).is_some());
        }
        assert!(rune_kit::syntax::highlight(Some("brainfudge"), "++++").is_empty());
    }
    #[test]
    fn files_are_read_only_and_confined() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            read_file(root, &file_target("Cargo.toml:1").unwrap())
                .unwrap()
                .contains("[package]")
        );
        assert!(read_file(root, &file_target("../native/../../outside.txt").unwrap()).is_err());
        // The kind of file is judged first, as Electron's text channel judges it: a file with no listed extension is
        // never opened, wherever it is.
        assert_eq!(
            read_file(root, &file_target("/etc/passwd").unwrap()).unwrap_err(),
            "Locust does not open that kind of file here."
        );
        // A listed kind that is not in the folder is held out of it.
        let outside = std::env::temp_dir().join(format!("locust-viewer-outside-{}.txt", locust_core::invariants::unique_nanos()));
        std::fs::write(&outside, "secret").unwrap();
        let said = read_file(root, &file_target(&outside.to_string_lossy()).unwrap());
        let _ = std::fs::remove_file(&outside);
        assert_eq!(said.unwrap_err(), "This file is outside the conversation's folder.");
        assert!(file_target("file://src/main.rs:3").is_some());
        assert!(file_target("src/main.rs\0:1").is_none());
    }

    #[test]
    fn copy_feedback_words_match_electron() {
        assert_eq!(copy_said(true), ("Copied", Duration::from_millis(1_600)));
        assert_eq!(
            copy_said(false),
            ("Not copied", Duration::from_millis(4_000))
        );
        let now = Instant::now();
        let body = "const total = 1;\n";
        let ok = settle_copy_feedback(body, true, now);
        assert_eq!(copy_label(Some(&ok), body, now), "Copied");
        assert_eq!(
            copy_label(Some(&ok), body, now + Duration::from_millis(1_601)),
            "Copy"
        );
        let bad = settle_copy_feedback(body, false, now);
        assert_eq!(copy_label(Some(&bad), body, now), "Not copied");
        assert_eq!(copy_label(Some(&bad), "other", now), "Copy");
    }
}

#[cfg(test)]
#[path = "file_menu_drawn_tests.rs"]
mod file_menu_drawn_tests;

#[cfg(test)]
#[path = "status_chip_tests.rs"]
mod status_chip_tests;

#[cfg(test)]
#[path = "live2_math_scene_tests.rs"]
mod live2_math_scene_tests;

#[cfg(test)]
#[path = "inline_math_tests.rs"]
mod inline_math_tests;

#[cfg(test)]
#[path = "settled_text_tests.rs"]
mod settled_text_tests;
