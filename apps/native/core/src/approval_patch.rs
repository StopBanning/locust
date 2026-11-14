//! The diff behind a file-change approval (`main/approval-patch.ts` + `shared/approval-patch.ts`).

use serde_json::{json, Value};

pub const MAX_CHANGES_PER_ITEM: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileChangeKind {
    Add,
    Delete,
    Update,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChangeRecord {
    pub path: String,
    pub kind: FileChangeKind,
    pub move_path: Option<String>,
    pub diff: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPatch {
    pub text: String,
    pub added: usize,
    pub removed: usize,
    pub truncated: bool,
}

impl ToolPatch {
    pub fn to_json(&self) -> Value {
        json!({
            "text": self.text,
            "added": self.added,
            "removed": self.removed,
            "truncated": self.truncated,
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let text = obj.get("text")?.as_str()?.to_owned();
        let added = obj.get("added")?.as_u64()? as usize;
        let removed = obj.get("removed")?.as_u64()? as usize;
        let truncated = obj.get("truncated")?.as_bool()?;
        Some(Self {
            text,
            added,
            removed,
            truncated,
        })
    }
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn first_line(diff: &str) -> &str {
    diff.split('\n')
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
}

fn has_headers(diff: &str) -> bool {
    let first = first_line(diff);
    first.starts_with("--- ") || first.starts_with("+++ ")
}

fn has_hunks(diff: &str) -> bool {
    first_line(diff).starts_with("@@ ")
}

/// A path under the folder, as the folder sees it; anything else unchanged.
pub fn relative_to_folder(path: &str, workspace_path: Option<&str>) -> String {
    let Some(workspace) = workspace_path.filter(|s| !s.is_empty()) else {
        return path.to_owned();
    };
    let normal = |v: &str| v.replace('\\', "/").trim_end_matches('/').to_owned();
    let root = normal(workspace);
    let candidate = normal(path);
    let root_chars = root.chars().count();
    let cand_chars: Vec<(usize, char)> = candidate.char_indices().collect();
    if cand_chars.len() > root_chars
        && cand_chars[root_chars].1 == '/'
        && cand_chars[..root_chars]
            .iter()
            .flat_map(|(_, c)| c.to_lowercase())
            .eq(root.chars().flat_map(|c| c.to_lowercase()))
    {
        return candidate[cand_chars[root_chars].0 + 1..].to_owned();
    }
    path.to_owned()
}

/// Whole-file content as one hunk of added (or removed) lines.
fn content_hunk(content: &str, sign: char) -> String {
    let text = normalize_newlines(content);
    let text = text.strip_suffix('\n').unwrap_or(&text);
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() == 1 && lines[0].is_empty() {
        return String::new();
    }
    let range = lines.len();
    let header = if sign == '+' {
        format!("@@ -0,0 +1,{range} @@")
    } else {
        format!("@@ -1,{range} +0,0 @@")
    };
    let mut out = Vec::with_capacity(lines.len() + 1);
    out.push(header);
    for line in lines {
        out.push(format!("{sign}{line}"));
    }
    out.join("\n")
}

/// One unified diff for the whole item: a `---` / `+++` pair per file,
/// `/dev/null` for an add or a delete, the moved-to path for a move, and a
/// synthesised hunk when Codex sent whole-file content instead of one.
pub fn unified_patch_text(changes: &[FileChangeRecord], workspace_path: Option<&str>) -> String {
    let mut parts = Vec::new();
    for change in changes {
        let normalized = normalize_newlines(&change.diff);
        let body = normalized.trim_end();
        // An add or a delete is whole-file CONTENT, whatever it looks like (R32).
        let whole = matches!(change.kind, FileChangeKind::Add | FileChangeKind::Delete);
        if !whole && has_headers(body) {
            parts.push(body.to_owned());
            continue;
        }
        let path = relative_to_folder(&change.path, workspace_path);
        let moved = change
            .move_path
            .as_deref()
            .map(|m| relative_to_folder(m, workspace_path));
        let old_path = if change.kind == FileChangeKind::Add {
            "/dev/null".to_owned()
        } else {
            format!("a/{path}")
        };
        let new_path = if change.kind == FileChangeKind::Delete {
            "/dev/null".to_owned()
        } else {
            format!("b/{}", moved.as_deref().unwrap_or(&path))
        };
        let hunks = if !whole && has_hunks(body) {
            body.to_owned()
        } else if change.kind == FileChangeKind::Add {
            content_hunk(&change.diff, '+')
        } else if change.kind == FileChangeKind::Delete {
            content_hunk(&change.diff, '-')
        } else {
            body.to_owned()
        };
        if hunks.is_empty() {
            parts.push(format!("--- {old_path}\n+++ {new_path}"));
        } else {
            parts.push(format!("--- {old_path}\n+++ {new_path}\n{hunks}"));
        }
    }
    parts.join("\n")
}

/// The changes on a fileChange item, or None when the item is not one.
pub fn file_changes_of(item: &Value) -> Option<Vec<FileChangeRecord>> {
    let record = item.as_object()?;
    if record.get("type").and_then(Value::as_str) != Some("fileChange") {
        return None;
    }
    let changes = record.get("changes").and_then(Value::as_array)?;
    let mut out = Vec::new();
    for entry in changes {
        let Some(change) = entry.as_object() else {
            continue;
        };
        let Some(path) = change.get("path").and_then(Value::as_str) else {
            continue;
        };
        let kind_record = change.get("kind").and_then(Value::as_object);
        let kind = match kind_record
            .and_then(|k| k.get("type"))
            .and_then(Value::as_str)
        {
            Some("add") => FileChangeKind::Add,
            Some("delete") => FileChangeKind::Delete,
            _ => FileChangeKind::Update,
        };
        let move_path = kind_record
            .and_then(|k| k.get("move_path"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let diff = change
            .get("diff")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        out.push(FileChangeRecord {
            path: path.to_owned(),
            kind,
            move_path,
            diff,
        });
        if out.len() >= MAX_CHANGES_PER_ITEM {
            break;
        }
    }
    Some(out)
}

/// The item id a fileChange notification is about, when it carries an item.
pub fn item_of(params: &Value) -> Option<(String, Value)> {
    let item = params.get("item").filter(|v| v.is_object())?;
    let id = item.get("id").and_then(Value::as_str)?;
    Some((id.to_owned(), item.clone()))
}

pub fn tool_patch_from(unified: &str) -> Option<ToolPatch> {
    let v = crate::runtime::cursor_events::tool_patch_from(unified)?;
    ToolPatch::from_json(&v)
}

/// The patch for an approval, or None when the item is unknown or had no text.
pub fn approval_patch_from(
    changes: Option<&[FileChangeRecord]>,
    workspace_path: Option<&str>,
) -> Option<ToolPatch> {
    let changes = changes.filter(|c| !c.is_empty())?;
    tool_patch_from(&unified_patch_text(changes, workspace_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUNK: &str = "@@ -1,2 +1,2 @@\n-old line\n+new line\n context";
    const WS: &str = r"C:\Users\colin\shop";

    #[test]
    fn reads_the_changes_off_a_file_change_item_and_nothing_off_any_other_item() {
        let item = json!({
            "id": "item_1",
            "type": "fileChange",
            "changes": [
                { "path": "src/a.ts", "kind": { "type": "update" }, "diff": HUNK },
                { "path": "src/new.ts", "kind": { "type": "add" }, "diff": "hello\n" },
                { "path": "src/old.ts", "kind": { "type": "delete" }, "diff": "bye\n" },
                { "path": "src/moved.ts", "kind": { "type": "update", "move_path": "src/here.ts" }, "diff": HUNK },
                { "nonsense": true }
            ]
        });
        let mapped: Vec<_> = file_changes_of(&item)
            .unwrap()
            .into_iter()
            .map(|c| (c.path, c.kind, c.move_path))
            .collect();
        assert_eq!(
            mapped,
            vec![
                ("src/a.ts".into(), FileChangeKind::Update, None),
                ("src/new.ts".into(), FileChangeKind::Add, None),
                ("src/old.ts".into(), FileChangeKind::Delete, None),
                (
                    "src/moved.ts".into(),
                    FileChangeKind::Update,
                    Some("src/here.ts".into())
                ),
            ]
        );
        assert!(
            file_changes_of(&json!({ "id": "x", "type": "commandExecution", "command": "ls" }))
                .is_none()
        );
        assert_eq!(item_of(&json!({ "item": item })), Some(("item_1".into(), item)));
        assert!(item_of(&json!({ "item": { "type": "fileChange" } })).is_none());

        let many: Vec<_> = (0..(MAX_CHANGES_PER_ITEM + 10))
            .map(|i| json!({ "path": format!("f{i}.ts"), "kind": { "type": "add" }, "diff": "x\n" }))
            .collect();
        let capped = file_changes_of(&json!({ "id": "big", "type": "fileChange", "changes": many })).unwrap();
        assert_eq!(capped.len(), 200);
    }

    #[test]
    fn the_live_payload_an_add_carries_the_files_content_not_a_hunk_and_an_absolute_path_both_become_a_diff() {
        let live = file_changes_of(&json!({
            "id": "exec-7a33",
            "type": "fileChange",
            "changes": [{ "path": format!(r"{WS}\SMOKE.txt"), "kind": { "type": "add" }, "diff": "smoke ok.\n" }]
        }))
        .unwrap();
        assert_eq!(
            unified_patch_text(&live, Some(WS)),
            "--- /dev/null\n+++ b/SMOKE.txt\n@@ -0,0 +1,1 @@\n+smoke ok."
        );
        let patch = approval_patch_from(Some(&live), Some(WS)).unwrap();
        assert_eq!((patch.added, patch.removed, patch.truncated), (1, 0, false));
    }

    #[test]
    fn composes_a_header_pair_per_file_dev_null_for_add_and_delete_the_moved_to_path_for_a_move_hunks_kept_as_sent_for_an_update() {
        let changes = file_changes_of(&json!({
            "id": "i",
            "type": "fileChange",
            "changes": [
                { "path": "src/a.ts", "kind": { "type": "update" }, "diff": HUNK },
                { "path": "src/new.ts", "kind": { "type": "add" }, "diff": "one\ntwo\n" },
                { "path": "src/old.ts", "kind": { "type": "delete" }, "diff": "bye\n" },
                { "path": "src/moved.ts", "kind": { "type": "update", "move_path": "src/here.ts" }, "diff": HUNK }
            ]
        }))
        .unwrap();
        let text = unified_patch_text(&changes, None);
        let headers: Vec<_> = text
            .split('\n')
            .filter(|line| line.starts_with("--- ") || line.starts_with("+++ "))
            .collect();
        assert_eq!(
            headers,
            vec![
                "--- a/src/a.ts",
                "+++ b/src/a.ts",
                "--- /dev/null",
                "+++ b/src/new.ts",
                "--- a/src/old.ts",
                "+++ /dev/null",
                "--- a/src/moved.ts",
                "+++ b/src/here.ts",
            ]
        );
        assert!(text.contains("@@ -0,0 +1,2 @@\n+one\n+two"));
        assert!(text.contains("@@ -1,1 +0,0 @@\n-bye"));
        let headed = format!("--- a/x.ts\n+++ b/x.ts\n{HUNK}");
        assert_eq!(
            unified_patch_text(
                &[FileChangeRecord {
                    path: "x.ts".into(),
                    kind: FileChangeKind::Update,
                    move_path: None,
                    diff: headed.clone(),
                }],
                None
            ),
            headed
        );
    }

    #[test]
    fn paths_under_the_folder_are_shown_relative_to_it_whichever_slashes_and_case_others_are_left_alone() {
        assert_eq!(
            relative_to_folder(r"C:\Users\colin\shop\src\a.ts", Some(WS)),
            "src/a.ts"
        );
        assert_eq!(
            relative_to_folder("c:/users/colin/shop/src/a.ts", Some(WS)),
            "src/a.ts"
        );
        assert_eq!(
            relative_to_folder(r"D:\elsewhere\a.ts", Some(WS)),
            r"D:\elsewhere\a.ts"
        );
        assert_eq!(relative_to_folder("src/a.ts", None), "src/a.ts");
    }

    #[test]
    fn counts_the_lines_and_bounds_the_text_like_the_ledger_does_no_changes_is_no_patch() {
        let patch = approval_patch_from(
            Some(&[FileChangeRecord {
                path: "a.ts".into(),
                kind: FileChangeKind::Update,
                move_path: None,
                diff: HUNK.into(),
            }]),
            None,
        )
        .unwrap();
        assert_eq!((patch.added, patch.removed, patch.truncated), (1, 1, false));
        assert!(approval_patch_from(Some(&[]), None).is_none());
        assert!(approval_patch_from(None, None).is_none());
        let huge = approval_patch_from(
            Some(&[FileChangeRecord {
                path: "big.ts".into(),
                kind: FileChangeKind::Add,
                move_path: None,
                diff: "x\n".repeat(40_000),
            }]),
            None,
        )
        .unwrap();
        assert!(huge.truncated);
        assert_eq!(huge.added, 40_000);
    }
}
