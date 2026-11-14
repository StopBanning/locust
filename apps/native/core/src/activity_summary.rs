//! The collapsed activity counts files, commands and calls independently of ledger events.
use crate::ledger::{Item, Status};
pub fn is_edit(item: &Item) -> bool {
    matches!(item, Item::Tool { name, detail, .. } if name == "file_change" || crate::mission_export::edit_tool_name(name) || (crate::command_outcomes::is_shell(name) && crate::mission_export::is_edit_command(&detail.arguments)))
}
fn target(item: &Item) -> &str {
    match item {
        Item::Tool {
            name,
            summary,
            detail,
            ..
        } => {
            if !detail.arguments.is_empty() && !detail.arguments.starts_with(['{', '[']) {
                &detail.arguments
            } else if !summary.is_empty() {
                summary
            } else {
                name
            }
        }
        _ => "",
    }
}
/// No diff was reported: the runtime named each file, so each gets a row.
pub fn unreported_paths(item: &Item) -> Vec<String> {
    if !is_edit(item) {
        return Vec::new();
    }
    let name = target(item);
    if !name.contains('\n') {
        return Vec::new();
    }
    name.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
pub fn activity_summary(items: &[Item]) -> String {
    let (mut commands, mut helpers, mut other) = (0, 0, 0);
    let mut paths: Vec<String> = Vec::new();
    for item in items {
        if let Item::Tool {
            name,
            status,
            detail,
            ..
        } = item
        {
            if is_edit(item) && *status != Status::Failed {
                for path in target(item)
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    let flat = path
                        .replace('\\', "/")
                        .trim_start_matches("./")
                        .to_lowercase();
                    let same = if flat.contains(['.', '/']) {
                        paths.iter().position(|p| {
                            p == &flat
                                || p.ends_with(&format!("/{flat}"))
                                || flat.ends_with(&format!("/{p}"))
                        })
                    } else {
                        None
                    };
                    if let Some(at) = same {
                        if flat.len() > paths[at].len() {
                            paths[at] = flat;
                        }
                    } else {
                        paths.push(flat);
                    }
                }
            } else if crate::command_outcomes::is_shell(name) && !is_edit(item) {
                commands += 1;
            } else if detail.helper {
                helpers += 1;
            } else {
                other += 1;
            }
        }
    }
    let mut parts = Vec::new();
    if !paths.is_empty() {
        parts.push(format!(
            "Edited {} file{}",
            paths.len(),
            if paths.len() == 1 { "" } else { "s" }
        ));
    }
    if commands > 0 {
        parts.push(format!(
            "ran {commands} command{}",
            if commands == 1 { "" } else { "s" }
        ));
    }
    if helpers > 0 {
        parts.push(format!(
            "asked {helpers} subagent{}",
            if helpers == 1 { "" } else { "s" }
        ));
    }
    if other > 0 {
        parts.push(format!(
            "{other} tool call{}",
            if other == 1 { "" } else { "s" }
        ));
    }
    if parts.is_empty() {
        "No tool activity".into()
    } else {
        parts.join(" · ")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::ToolDetail;
    fn tool(name: &str, command: &str, failed: bool) -> Item {
        Item::Tool {
            name: name.into(),
            summary: command.into(),
            status: if failed { Status::Failed } else { Status::Done },
            detail: Box::new(ToolDetail {
                arguments: command.into(),
                ..Default::default()
            }),
        }
    }
    fn edit(name: &str) -> Item {
        tool("apply_patch", name, false)
    }
    #[test]
    fn counts_edits_and_commands_separately_from_their_tool_events() {
        assert_eq!(
            activity_summary(&[
                edit("apply_patch"),
                edit("apply_patch"),
                tool("shell", "pnpm test", false)
            ]),
            "Edited 2 files · ran 1 command"
        );
    }
    #[test]
    fn counts_files_not_edit_calls_one_codex_file_change_can_name_several() {
        assert_eq!(
            activity_summary(&[edit("C:/w/README.md\nC:/w/src/prices.ts")]),
            "Edited 2 files"
        );
    }
    #[test]
    fn draws_one_row_per_file_a_runtime_named_without_a_diff_never_one_row_named_after_the_tool() {
        assert_eq!(
            unreported_paths(&tool(
                "file_change",
                "C:/w/README.md\nC:/w/src/prices.ts",
                false
            )),
            ["C:/w/README.md", "C:/w/src/prices.ts"]
        );
    }
    #[test]
    fn singularizes_honestly() {
        assert_eq!(activity_summary(&[edit("x")]), "Edited 1 file");
        assert_eq!(
            activity_summary(&[tool("write", "C:/Users/x/.claude/plans/p.md", true)]),
            "1 tool call"
        );
    }
    #[test]
    fn counts_sed_as_an_edit_only_in_place_and_tee_only_with_a_file() {
        assert_eq!(
            activity_summary(&[tool("shell", "sed -n '1,40p' src/app.ts", false)]),
            "ran 1 command"
        );
        assert_eq!(
            activity_summary(&[tool("shell", "sed -i 's/a/b/' src/app.ts", false)]),
            "Edited 1 file"
        );
        for s in [
            "sed -i.bak 's/a/b/' x",
            "sed --in-place 's/a/b/' x",
            "tee out.log",
            "tee -a out.log",
        ] {
            assert!(crate::mission_export::is_edit_command(s));
        }
        for s in ["sed -E 's/a/b/g' x", "tee"] {
            assert!(!crate::mission_export::is_edit_command(s));
        }
    }
    #[test]
    fn says_so_when_there_was_no_tool_activity() {
        assert_eq!(activity_summary(&[]), "No tool activity");
    }
    #[test]
    fn classifies_a_patch_command_as_an_edit_and_a_test_run_as_a_command() {
        assert_eq!(
            activity_summary(&[
                tool("shell", "apply_patch <<EOF", false),
                tool("shell", "pnpm test", false)
            ]),
            "Edited 1 file · ran 1 command"
        );
    }
}
