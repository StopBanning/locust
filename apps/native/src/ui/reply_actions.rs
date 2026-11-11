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

pub fn read_file(root: &Path, target: &FileTarget) -> Result<String, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = root
        .join(&target.path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(&root) {
        return Err("This file is outside the conversation's folder.".into());
    }
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    use std::io::Read;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1_048_576 {
        return Err("This file is too large to show here.".into());
    }
    String::from_utf8(bytes).map_err(|_| "This file is not UTF-8 text.".into())
}

pub(super) fn copy_button(feedback: Option<&CopyFeedback>, body: String, p: &Palette) -> El {
    let label = copy_label(feedback, &body, Instant::now());
    let tone = if label == "Not copied" {
        p.amber
    } else {
        p.secondary
    };
    el().padding(5.0)
        .radius(4.0)
        .fill(p.raised)
        .child(text(label).font(type_::LABEL).color(tone))
        .on_press_tx(move |_s: &mut State, tx| request_copy(tx, body.clone()))
}

pub(super) fn open_file(s: &mut State, target: FileTarget) {
    let root = selected_conversation(s)
        .and_then(|(t, _)| t.folder.as_ref())
        .map(PathBuf::from);
    let body = root
        .ok_or_else(|| "This conversation has no folder.".into())
        .and_then(|root| read_document(&root, &target));
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
        let text = read_file(root, target)?;
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

pub(super) fn reply(s: &State, body: &str, live: bool, p: &Palette) -> El {
    let frame_palette = Palette::new(p.dark);
    let feedback = s.copy_feedback.clone();
    let prepared = crate::diagram::prepare_reply_markdown(body);
    let (prepared, undrawn) = crate::diagram::take_undrawn_markers(&prepared);
    let (prepared, math_pngs) = crate::math_tex::prepare_math_markdown(&prepared, p.dark);
    let diagram_tabs = s.diagram_code_tab.clone();
    let image_cache = s.image_cache.clone();
    let mut out = col().w_full().gap(8.0).child(
        Markdown::new(prepared)
            .streaming(live)
            .selectable(true)
            .on_link(|s: &mut State, url| {
                if let Some(target) = file_target(url) {
                    open_file(s, target);
                }
            })
            .images(move |src| {
                let bytes = math_pngs.get(src)?;
                let cache = image_cache.as_ref()?;
                let image = cache.load(bytes.clone());
                // Headless snaps and the first live frame need the decode done before paint.
                let _ = image.wait();
                Some(image)
            })
            .highlight(move |language, code| {
                if live || code.encode_utf16().count() > 60_000 {
                    Vec::new()
                } else {
                    rune_kit::syntax::highlight(language, code).to_vec()
                }
            })
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
    let menu = crate::file_menu::file_menu_choices(
        &absolute,
        folder.as_deref(),
        if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "win32"
        },
        &["vscode", "cursor"],
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
                        request_copy(tx, crate::file_menu::system_path(&absolute, "darwin"));
                    }
                    crate::file_menu::FileMenuId::CopyRelative => {
                        let rel = crate::file_menu::relative_to_folder(
                            &absolute,
                            folder.as_deref(),
                            "darwin",
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
                    crate::file_menu::FileMenuId::Attach
                        if !s.attached.iter().any(|a| a == &absolute) =>
                    {
                        s.attached.push(absolute.clone());
                    }
                    _ => {}
                }),
        );
    }
    let mut pane = col()
        .key("file-reader")
        .w(340.0)
        .h_full()
        .padding(16.0)
        .gap(12.0)
        .fill(p.sidebar)
        .child(
            text(format!("{}:{}-{}", target.path, target.start, target.end))
                .color(p.text)
                .wrap_anywhere(),
        )
        .child(menu_row)
        .child(
            el().child(text("Close").color(p.secondary))
                .on_press(|s: &mut State| s.file_view = None),
        );
    match body {
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
        assert_eq!(
            read_file(root, &file_target("/etc/passwd").unwrap()).unwrap_err(),
            "This file is outside the conversation's folder."
        );
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
