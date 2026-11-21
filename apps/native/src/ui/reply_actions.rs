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
}

/// UTF-8 text, or extracted PDF/office/sheet words when the path is one of those.
fn read_document(root: &Path, target: &FileTarget) -> Result<String, String> {
    let ext = crate::file_menu::file_extension(&target.path);
    if ext == "pdf" || crate::pdf::is_pdf_path(&target.path) {
        let name = crate::file_menu::file_base_name(&target.path);
        // Write text + page PNGs beside the file (Electron readablePdf), then show the text.
        let readable = crate::pdf::readable_pdf(
            root,
            &target.path,
            &name,
            crate::pdf::MAX_PICTURES,
        )?;
        let _ = crate::pdf::readable_pdf_note(std::slice::from_ref(&readable));
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
        return Ok(format_workbook(&book));
    }
    if ext == "csv" || ext == "tsv" {
        let text = read_sheet_text(root, target)?;
        let book = crate::office::read_csv(&text, ext == "tsv");
        return Ok(format_workbook(&book));
    }
    read_file(root, target)
}

fn format_workbook(book: &crate::office::Workbook) -> String {
    let mut blocks = Vec::new();
    for (name, rows) in &book.sheets {
        blocks.push(crate::office::DocumentBlock::Heading {
            level: 2,
            text: name.clone(),
        });
        let table_rows: Vec<Vec<String>> = rows
            .iter()
            .map(|row| row.iter().map(|c| c.text.clone()).collect())
            .collect();
        if !table_rows.is_empty() {
            blocks.push(crate::office::DocumentBlock::Table { rows: table_rows });
        }
    }
    crate::office::document_words(
        "workbook",
        &crate::office::OfficeDocument { blocks, more: 0 },
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

fn reply_markdown(s: &State, body: &str, live: bool, copy: bool, can_draft: bool, p: &Palette) -> El {
    let chip_reply = body_key(body);
    let frame_palette = Palette::new(p.dark);
    let feedback = s.copy_feedback.clone();
    let crate::markdown_prep::Prepared { markdown: prepared, undrawn, math: math_pngs } =
        crate::markdown_prep::prepare_reply(body, p.dark);
    let diagram_tabs = s.diagram_code_tab.clone();
    let image_cache = s.image_cache.clone();
    let pictures = crate::step_pictures::reply_pictures(s, &prepared);
    let picture_palette = Palette::new(p.dark);
    let framed: std::collections::HashSet<String> = pictures.keys().cloned().collect();
    let mut out = col().w_full().gap(8.0).child(
        Markdown::new(prepared)
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
            .link_style({
                let dark = p.dark;
                move |url| crate::markdown_prep::reply_link_look(url, dark)
            })
            .image_frame(move |shown, frame| {
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
                let palette = Palette::new(p.dark);
                move |card, frame| {
                    let key = body_key(frame.source);
                    let code_on = tabs.get(&key).copied().unwrap_or(false);
                    let kind = crate::diagram::diagram_kind_words(crate::diagram::diagram_kind_of(
                        frame.source,
                    ));
                    let p = &palette;
                    let diagram_tab = el()
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
                        // Electron shows this while its WebView draws; native shows it only
                        // while the fence is still arriving.
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
            .render(&markdown_style(p)),
    );
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
    let (target, body) = s.file_view.as_ref()?;
    let folder = selected_conversation(s)
        .and_then(|(t, _)| t.folder.clone());
    let absolute = crate::file_menu::rooted_path(&target.path, folder.as_deref());
    // installed-editors.ts: only the editors whose command is on PATH are offered.
    let editors = locust_core::installed_editors::installed_editors();
    let editor_ids: Vec<&str> = editors.iter().map(|e| e.id.as_str()).collect();
    let menu = crate::file_menu::file_menu_choices(
        &absolute,
        folder.as_deref(),
        locust_core::installed_editors::this_platform(),
        &editor_ids,
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
                .child(text(label).font(type_::LABEL).color(p.secondary))
                .on_press_tx(move |s: &mut State, tx| match id {
                    crate::file_menu::FileMenuId::CopyPath => {
                        request_copy(tx, crate::file_menu::system_path(&absolute, locust_core::installed_editors::this_platform()));
                    }
                    crate::file_menu::FileMenuId::CopyRelative => {
                        let rel = crate::file_menu::relative_to_folder(
                            &absolute,
                            folder.as_deref(),
                            locust_core::installed_editors::this_platform(),
                        );
                        request_copy(tx, rel);
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
    let mut pane = col()
        .key("file-reader")
        .w(340.0)
        .h_full()
        .padding(16.0)
        .gap(12.0)
        .fill(p.sidebar)
        .child(
            text(if picture { target.path.clone() } else { format!("{}:{}-{}", target.path, target.start, target.end) })
                .color(p.text)
                .wrap_anywhere(),
        )
        .child(menu_row)
        .child(
            el().child(text("Close").color(p.secondary))
                .on_press(|s: &mut State| s.file_view = None),
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
    match body {
        Ok(_) if picture => {
            // ThreadImage's press: the picture at the pane's width, its own shape, never enlarged.
            pane = match crate::step_pictures::thread_picture(s, &target.path).and_then(|image| crate::step_pictures::viewer_picture(&image, &target.path, p)) {
                Some(shown) => pane.child(shown),
                None => pane.child(text("This picture cannot be shown here.").color(p.muted).wrap_anywhere()),
            };
        }
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
    Some(pane.child(text("Shown here only: Locust never runs a file a teammate made, or opens it in another app.").font(type_::LABEL).color(p.muted).wrap_anywhere()))
}

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
#[path = "status_chip_tests.rs"]
mod status_chip_tests;
