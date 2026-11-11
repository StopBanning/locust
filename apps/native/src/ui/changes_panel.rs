use super::*;
use std::{collections::HashSet, path::PathBuf};
#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub was_running: bool,
    pub folder: PathBuf,
    pub diff: Option<Result<changes::Folder, String>>,
    pub folded: HashSet<usize>,
    pub expanded: HashSet<usize>,
    /// Open context folds keyed by (file, hunk, first-hidden-row index), matching Electron per-segment folds.
    pub contexts: HashSet<(usize, usize, usize)>,
    pub files: bool,
    pub records: Vec<(String, serde_json::Value)>,
    pub asking: Option<String>,
    pub result: Option<String>,
}
fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}
fn root(s: &State) -> Option<PathBuf> {
    selected_conversation(s)
        .and_then(|(t, _)| t.folder.as_ref())
        .map(PathBuf::from)
}
pub(super) fn controls(s: &State, p: &Palette) -> El {
    let mut out = row().gap(6.0);
    if let Some(folder) = root(s) {
        if s.changes_repo
            .as_ref()
            .is_some_and(|(p, yes)| p == &folder && *yes)
        {
            out = out.child(button("Changes", p).on_press(|s: &mut State| open(s)))
        }
        out = out.child(button("Undo", p).on_press(|s: &mut State| open(s)));
    }
    out
}
fn open(s: &mut State) {
    if let Some(folder) = root(s) {
        s.changes = Panel {
            open: true,
            folder,
            files: true,
            ..Panel::default()
        };
        refresh(s, None)
    }
}
fn refresh(s: &mut State, sha: Option<&str>) {
    let folder = s.changes.folder.clone();
    s.changes.diff = None;
    let sha = sha.map(str::to_owned);
    let store = native::store();
    s.changes_job
        .request((folder, sha, store), |(folder, sha, store)| {
            let diff = changes::read(&folder, sha.as_deref());
            let records = store
                .map(|store| checkpoints::records(&store, &folder).map_err(|e| e.to_string()))
                .unwrap_or_else(|| Ok(vec![]));
            (folder, diff, records)
        });
    s.changes.folded.clear();
    s.changes.expanded.clear();
    s.changes.contexts.clear();
    s.changes.asking = None;
}
pub(super) type ReadResult = (
    PathBuf,
    Result<changes::Folder, String>,
    Result<Vec<(String, serde_json::Value)>, String>,
);
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    let folder = root(s);
    if folder != s.changes_repo_root {
        s.changes_repo_root = folder.clone();
        s.changes_repo = None;
        s.changes_repo_job.cancel();
        if let Some(ref folder) = folder {
            s.changes_repo_job.request(folder.clone(), |folder| {
                let yes = changes::repository(&folder);
                (folder, yes)
            });
        }
    }
    if let Some(answer) = s.changes_repo_job.take() {
        s.changes_repo = Some(answer.clone());
        // Electron hides Changes when the bound folder is not a repository.
        if s.changes.open && answer.0 == s.changes.folder && !answer.1 {
            s.changes.open = false;
        }
        changed = true;
    }
    if s.changes.open && folder.as_ref() != Some(&s.changes.folder) {
        s.changes.open = false;
        changed = true;
    }
    if let Some((folder, diff, records)) = s.changes_job.take()
        && s.changes.folder == folder
    {
        s.changes.diff = Some(diff);
        match records {
            Ok(r) => s.changes.records = r,
            Err(e) => s.changes.result = Some(e),
        }
        changed = true;
    }
    let running = turn_running(s);
    if s.changes.open && s.changes.was_running && !running {
        refresh(s, None);
        changed = true;
    }
    s.changes.was_running = running;
    changed
}

fn line_count_marks(added: usize, removed: usize, p: &Palette) -> El {
    if added > 0 && removed == 0 {
        return text(changes::line_counts(added, 0)).color(p.green);
    }
    if removed > 0 && added == 0 {
        return text(changes::line_counts(0, removed)).color(p.red);
    }
    if added > 0 && removed > 0 {
        // Same zero-free words as `line_counts`, split so + is green and − is red.
        return row()
            .gap(0.0)
            .child(text(format!("+{added}")).color(p.green))
            .child(text(" "))
            .child(text(format!("−{removed}")).color(p.red));
    }
    row()
}
fn code_row(
    r: &changes::Row,
    line: &str,
    offset: usize,
    highlights: &[rune_kit::syntax::Highlight],
    p: &Palette,
) -> El {
    use rune_kit::syntax::{Paint, Syntax};
    let row_fill = if r.kind == '+' {
        p.green
    } else if r.kind == '-' {
        p.red
    } else {
        p.secondary
    };
    let sign = match r.kind {
        '+' => "+",
        '-' => "−",
        _ => "",
    };
    let sign_tone = match r.kind {
        '+' => p.green,
        '-' => p.red,
        _ => p.secondary,
    };
    let mut spans = row().gap(0.0);
    let mut cursor = 0;
    for (range, paint) in highlights {
        let a = range
            .start
            .max(offset)
            .saturating_sub(offset)
            .min(line.len());
        let b = range.end.saturating_sub(offset).min(line.len());
        if a >= b || a < cursor || !line.is_char_boundary(a) || !line.is_char_boundary(b) {
            continue;
        }
        if cursor < a {
            spans = spans.child(text(line[cursor..a].to_owned()).color(p.text))
        }
        let c = match paint {
            Paint::Color(c) => *c,
            Paint::Kind(Syntax::Comment) => p.muted,
            Paint::Kind(Syntax::String) => p.green,
            Paint::Kind(Syntax::Number) => p.amber,
            Paint::Kind(Syntax::Keyword | Syntax::Type | Syntax::Function) => {
                if p.dark {
                    color("#b6a1e8")
                } else {
                    color("#6c42a8")
                }
            }
            _ => p.text,
        };
        spans = spans.child(text(line[a..b].to_owned()).color(c));
        cursor = b;
    }
    if cursor < line.len() {
        spans = spans.child(text(line[cursor..].to_owned()).color(p.text))
    }
    row()
        .w_full()
        .gap(8.0)
        .px(6.0)
        .py(2.0)
        .fill(if r.kind == ' ' {
            p.bg
        } else {
            row_fill.alpha(if p.dark { 0.12 } else { 0.09 })
        })
        .child(
            text(format!(
                "{:>4} {:>4}",
                r.old.map(|n| n.to_string()).unwrap_or_default(),
                r.new.map(|n| n.to_string()).unwrap_or_default(),
            ))
            .color(p.muted),
        )
        .child(text(sign.to_owned()).color(sign_tone))
        .child(spans)
}
pub(super) fn view(s: &State, p: &Palette) -> El {
    let panel = &s.changes;
    let mut head = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(text("Changes").font(type_::TITLE).color(p.text))
        .child(
            button(
                if panel.files {
                    "Hide files"
                } else {
                    "Show files"
                },
                p,
            )
            .on_press(|s: &mut State| s.changes.files = !s.changes.files),
        )
        .child(button("Refresh", p).on_press(|s: &mut State| refresh(s, None)))
        .child(button("Close", p).on_press(|s: &mut State| s.changes.open = false));
    if let Some(Ok(diff)) = &panel.diff {
        head = head.child(
            text(format!("{} → {}", diff.base, diff.branch))
                .color(p.muted)
                .wrap(),
        );
    }
    let mut body = col().w_full().gap(8.0);
    if let Some(result) = &panel.result {
        body = body.child(text(result.clone()).color(p.amber).wrap())
    }
    for (id, record) in &panel.records {
        if record["shared"] == true {
            continue;
        }
        if let Some(receipt) = record.get("undone") {
            let n = receipt["restored"].as_array().map_or(0, Vec::len);
            body = body.child(
                text(if n == 0 {
                    "Nothing was put back.".into()
                } else if n == 1 {
                    "Undone: 1 file put back as it was before this turn.".into()
                } else {
                    format!("Undone: {n} files put back as they were before this turn.")
                })
                .color(p.muted)
                .wrap(),
            );
            continue;
        }
        let count = checkpoints::changed(record).len();
        if count == 0 {
            continue;
        }
        let run = id.clone();
        let mut undo = col()
            .w_full()
            .gap(6.0)
            .padding(8.0)
            .fill(p.raised)
            .child(text(format!("Turn {id}")).color(p.muted).wrap());
        if panel.asking.as_deref() == Some(id) {
            let not = record["notKept"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            undo=undo.child(text(format!("Put back the {} this turn changed, as they were before it? A file changed since is left as it is.{}",if count==1{"1 file".into()}else{format!("{count} files")},if not.is_empty(){String::new()}else{format!(" Too large to have been kept: {not}.")})).color(p.text).wrap())
            .child(row().child(button("Undo",p).on_press(move |s:&mut State|{
                let answer=native::store().ok_or_else(||"No native store".into()).and_then(|store|checkpoints::undo(&store,&s.changes.folder,&run).map_err(|e|e.to_string()));
                s.changes.result=Some(match answer{Ok(v)=>{let n=v["restored"].as_array().map_or(0,Vec::len);let mut said=if n==0{"Nothing was put back.".into()}else if n==1{"Undone: 1 file put back as it was before this turn.".into()}else{format!("Undone: {n} files put back as they were before this turn.")};if let Some(left)=v["leftAlone"].as_array(){for f in left{said.push_str(&format!("\n{} left as it is: {}",f["path"].as_str().unwrap_or(""),f["why"].as_str().unwrap_or("")))}}said},Err(e)=>e});s.changes.asking=None;
            })).child(button("Keep them",p).on_press(|s:&mut State|s.changes.asking=None)));
        } else {
            undo = undo.child(
                button("Undo these changes", p)
                    .on_press(move |s: &mut State| s.changes.asking = Some(run.clone())),
            )
        }
        body = body.child(undo);
    }
    match &panel.diff {
        Some(Err(e)) => body = body.child(text(e.clone()).color(p.muted).wrap()),
        Some(Ok(diff)) => {
            if diff.files.is_empty() {
                body = body.child(
                    text(changes::empty_diff_words(diff.commits.len()).to_owned())
                        .color(p.muted)
                        .wrap(),
                )
            }
            if !diff.commits.is_empty() {
                body =
                    body.child(button("All changes", p).on_press(|s: &mut State| refresh(s, None)));
                for (sha, subject) in &diff.commits {
                    let sha = sha.clone();
                    body = body.child(
                        button(subject.clone(), p)
                            .on_press(move |s: &mut State| refresh(s, Some(&sha))),
                    )
                }
            }
            if panel.files {
                for (i, f) in diff.files.iter().enumerate() {
                    let (a, d) = f.counts();
                    let badge = kind(&f.path);
                    body = body.child(
                        row()
                            .gap(8.0)
                            .center_y()
                            .child(button(format!("{badge}  {}", f.path), p).on_press(
                                move |s: &mut State| {
                                    s.changes.folded.remove(&i);
                                },
                            ))
                            .child(line_count_marks(a, d, p)),
                    );
                }
            }
            body = body.child(
                row()
                    .child(button("Collapse all files", p).on_press(|s: &mut State| {
                        if let Some(Ok(d)) = &s.changes.diff {
                            s.changes.folded = (0..d.files.len()).collect()
                        }
                    }))
                    .child(
                        button("Expand all files", p)
                            .on_press(|s: &mut State| s.changes.folded.clear()),
                    ),
            );
            for (i, f) in diff.files.iter().enumerate() {
                let (a, d) = f.counts();
                let open = !panel.folded.contains(&i);
                let mut file = col().w_full().gap(4.0).child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(
                            button(
                                format!(
                                    "{} {} · {}",
                                    if open { "⌄" } else { "›" },
                                    f.path,
                                    f.status,
                                ),
                                p,
                            )
                            .on_press(move |s: &mut State| {
                                if !s.changes.folded.remove(&i) {
                                    s.changes.folded.insert(i);
                                }
                            }),
                        )
                        .child(line_count_marks(a, d, p)),
                );
                if open {
                    if f.status != "DELETED" {
                        let folder = panel.folder.clone();
                        let path = f.path.clone();
                        file = file.child(button(format!("Copy {}", f.path), p).on_press_tx(
                            move |s: &mut State, tx| {
                                let target = reply_actions::FileTarget {
                                    path: path.clone(),
                                    start: 1,
                                    end: 1,
                                };
                                match reply_actions::read_file(&folder, &target) {
                                    Ok(body) => reply_actions::request_copy(tx, body),
                                    Err(e) => s.changes.result = Some(e),
                                }
                            },
                        ));
                    }
                    let shown = if panel.expanded.contains(&i) {
                        f.hunks.len()
                    } else {
                        2.min(f.hunks.len())
                    };
                    for (j, h) in f.hunks.iter().take(shown).enumerate() {
                        file =
                            file.child(text(format!("{} {}", h.range(), h.heading)).color(p.muted));
                        let (before, after) = h.sides();
                        let lang = std::path::Path::new(&f.path)
                            .extension()
                            .and_then(|p| p.to_str());
                        let bh = rune_kit::syntax::highlight(lang, &before);
                        let ah = rune_kit::syntax::highlight(lang, &after);
                        let mut offsets = (0, 0);
                        let mut hidden = 0;
                        let keep_mask = changes::kept_context(&h.rows);
                        for (k, r) in h.rows.iter().enumerate() {
                            let offset = if r.kind == '-' { offsets.0 } else { offsets.1 };
                            if r.kind != '+' {
                                offsets.0 += r.text.len() + 1
                            }
                            if r.kind != '-' {
                                offsets.1 += r.text.len() + 1
                            }
                            let run_start = {
                                let mut start = k;
                                while start > 0 && !keep_mask[start - 1] {
                                    start -= 1;
                                }
                                start
                            };
                            let keep = keep_mask[k] || panel.contexts.contains(&(i, j, run_start));
                            if !keep {
                                hidden += 1;
                                continue;
                            }
                            if hidden > 0 {
                                let start = k - hidden;
                                let label = if hidden == 1 {
                                    "1 unchanged line".into()
                                } else {
                                    format!("{hidden} unchanged lines")
                                };
                                file =
                                    file.child(button(label, p).on_press(move |s: &mut State| {
                                        s.changes.contexts.insert((i, j, start));
                                    }));
                                hidden = 0;
                            }
                            file = file.child(code_row(
                                r,
                                &r.text,
                                offset,
                                if r.kind == '-' { &bh } else { &ah },
                                p,
                            ));
                        }
                        if hidden > 0 {
                            let start = h.rows.len() - hidden;
                            let label = if hidden == 1 {
                                "1 unchanged line".into()
                            } else {
                                format!("{hidden} unchanged lines")
                            };
                            file = file.child(button(label, p).on_press(move |s: &mut State| {
                                s.changes.contexts.insert((i, j, start));
                            }));
                        }
                    }
                    if let Some(after) = f.after() {
                        file = file.child(reply_actions::copy_button(
                            s.copy_feedback.as_ref(),
                            after,
                            p,
                        ));
                    }
                    let statement =
                        changes::completeness(f, shown, diff.truncated && shown >= f.hunks.len());
                    if shown < f.hunks.len() {
                        file = file.child(button(statement, p).on_press(move |s: &mut State| {
                            s.changes.expanded.insert(i);
                        }));
                    } else {
                        file = file.child(text(statement).font(type_::LABEL).color(p.muted).wrap());
                    }
                }
                body = body.child(file);
            }
            if diff.truncated {
                body = body.child(
                    text("The rest is too large to show here. Your own git has all of it.")
                        .color(p.amber)
                        .wrap(),
                );
            }
        }
        None => body = body.child(text("Reading the folder’s changes…").color(p.muted)),
    }
    col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .padding(16.0)
        .gap(12.0)
        .child(head)
        .child(body.grow(1.0).min_h(0.0).scroll(ScrollSpec {
            stick_bottom: false,
        }))
}
fn kind(path: &str) -> &'static str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return "▫";
    };
    if stem.is_empty() {
        return "▫";
    }
    match ext.to_lowercase().as_str() {
        "js" => "JS",
        "mjs" => "JS",
        "cjs" => "JS",
        "jsx" => "JS",
        "ts" => "TS",
        "mts" => "TS",
        "cts" => "TS",
        "tsx" => "TS",
        "py" => "PY",
        "rs" => "RS",
        "go" => "GO",
        "java" => "JV",
        "kt" => "KT",
        "swift" => "SW",
        "rb" => "RB",
        "php" => "PH",
        "cs" => "C#",
        "c" => "C",
        "h" => "H",
        "cpp" => "C+",
        "hpp" => "H+",
        "html" => "<>",
        "htm" => "<>",
        "vue" => "V",
        "svelte" => "S",
        "css" => "#",
        "scss" => "#",
        "sass" => "#",
        "json" => "{}",
        "jsonl" => "{}",
        "yml" => "YM",
        "yaml" => "YM",
        "toml" => "TM",
        "sh" => "$",
        "bash" => "$",
        "ps1" => "$",
        "bat" => "$",
        "cmd" => "$",
        "sql" => "SQ",
        "md" | "mdx" | "txt" | "rst" | "csv" | "log" => "▤",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp" | "avif" => "▧",
        _ => "▫",
    }
}
pub(super) fn scene(s: &mut State) {
    team_scene(s);
    s.changes = Panel {
        open: true,
        files: true,
        diff: Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "locust/native-changes".into(),
            files: changes::parse(
                "diff --git a/src/cart.ts b/src/cart.ts\n--- a/src/cart.ts\n+++ b/src/cart.ts\n@@ -1,4 +1,4 @@\n /* The cart total. */\n-export const total = (items: number[]) => sum(items) * 2\n+export const total = (items: number[]) => sum(items)\n const label = \"done\"\n return total\n",
            ),
            ..changes::Folder::default()
        })),
        ..Panel::default()
    };
}
pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}

pub(super) fn undo_scene(s: &mut State) {
    scene(s);
    s.changes.records = vec![(
        "mission_example".into(),
        serde_json::json!({"before":{"src/cart.ts":"old","notes.md":"old"},"after":{"src/cart.ts":"new","notes.md":"new"},"notKept":["video.mp4"]}),
    )];
    s.changes.asking = Some("mission_example".into());
}
pub(super) fn undo_light_scene(s: &mut State) {
    undo_scene(s);
    s.appearance = Some(false);
}
