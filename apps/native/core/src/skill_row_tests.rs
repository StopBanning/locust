//! renderer/src/a-skill-row-names-its-skill.test.ts, case for case: the activity row (`tool_fold::shown_name`, what the
//! thread draws for a tool), the steps line, the live line (`thread_view::live_action`) and the rail
//! (`signal_rail::tool_name`) all name a Skill call by its skill, without the prefix Locust added.
use crate::ledger::{Item, Status, ToolDetail};
use crate::path_words::relative_path;
use crate::signal_rail::tool_name;
use crate::steps_line::steps_line;
use crate::thread_view::live_action;
use crate::tool_fold::shown_name;
use serde_json::json;

fn tool(tool: &str, name: &str) -> Item {
    Item::Tool { name: tool.into(), status: Status::Done, summary: name.into(), detail: Box::new(ToolDetail { arguments: name.into(), ..Default::default() }) }
}

#[test]
fn reads_as_the_skill_used_without_the_prefix_locust_added() {
    assert_eq!(shown_name("Skill", "project:review", ""), "Used the review skill");
    assert_eq!(shown_name("Skill", "personal:shipcheck", ""), "Used the shipcheck skill");
    assert_eq!(shown_name("Skill", "small-business:ad-manager", ""), "Used the small-business:ad-manager skill");
}

#[test]
fn one_with_no_name_still_reads_as_a_skill_never_as_the_tool_id() {
    assert_eq!(shown_name("Skill", "Skill", ""), "Used a skill");
    assert_eq!(shown_name("Skill", "", ""), "Used a skill");
}

#[test]
fn the_steps_line_says_which_skill_and_its_short_form_says_a_skill_never_a_tool() {
    let copy = "C:/Users/me/AppData/Roaming/@teammate/desktop/claude-skills/0f8a6c1e-2b3d-4e5f-8a9b-0c1d2e3f4a5b/project/skills/locust-probe/second-word.md";
    let line = steps_line(&[tool("Skill", "project:locust-probe"), tool("Read", copy)], true, Some("C:/work/pebble"), false);
    assert!(line.title.starts_with("Used the locust-probe skill, read "), "{}", line.title);
    assert!(line.title.contains("second-word.md"), "{}", line.title);
    assert!(!line.shorter.join(" | ").contains("a tool"), "{:?}", line.shorter);
    assert!(line.shorter.iter().any(|f| f.starts_with("Used a skill")), "{:?}", line.shorter);
}

#[test]
fn a_skills_own_file_reads_as_the_file_it_is_a_copy_of_not_the_runs_copy() {
    let root = "C:/Users/me/AppData/Roaming/@teammate/desktop/claude-skills/0f8a6c1e-2b3d-4e5f-8a9b-0c1d2e3f4a5b";
    assert_eq!(relative_path(&format!("{root}/project/skills/locust-probe/second-word.md"), Some("C:/work/pebble")), ".claude/skills/locust-probe/second-word.md");
    // Windows separators, as Claude Code reports them.
    let windows = format!("{root}/personal/skills/shipcheck/SKILL.md").replace('/', "\\");
    assert_eq!(relative_path(&windows, Some("C:/work/pebble")), "~/.claude/skills/shipcheck/SKILL.md");
    // Native names its run's copy by `<pid>-<nanos>` (skills::fresh), not a uuid: it reads the same way.
    let native = "/Users/me/Library/Application Support/Locust/claude-skills/4242-1763500000123456789/project/skills/review/checklist.md";
    assert_eq!(relative_path(native, Some("/work/pebble")), ".claude/skills/review/checklist.md");
}

#[test]
fn the_live_line_and_the_rail_say_the_same() {
    assert_eq!(live_action(None, "tool_use", "Skill", Some("project:review")).as_deref(), Some("Using the review skill"));
    assert_eq!(tool_name(&json!({"name": "Skill", "toolKind": "tool_use", "command": "project:review"})), "Used the review skill");
}
