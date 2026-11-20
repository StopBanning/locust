//! main/a-claude-teammate-gets-its-skills.test.ts, case for case. Every folder is a temp one passed in: HOME is never
//! read, and nothing reaches the person's own `~/.claude/skills`.
use super::*;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("locust-claude-skills-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn dir(&self, name: &str) -> PathBuf {
        let d = self.0.join(name);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn skill(root: &Path, name: &str) {
    std::fs::create_dir_all(root.join(name)).unwrap();
    std::fs::write(root.join(name).join(SKILL_FILE), format!("---\nname: {name}\ndescription: test\n---\nSay hi.\n")).unwrap();
}
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

#[test]
fn the_folders_skills_always_as_the_plugin_project_named_the_way_a_run_calls_them() {
    let t = Temp::new();
    let (workspace, home, copies) = (t.dir("ws"), t.dir("home"), t.dir("copies"));
    skill(&workspace.join(".claude/skills"), "review");
    std::fs::write(workspace.join(".claude/skills/review/checklist.md"), "one\n").unwrap();
    skill(&home.join(".claude/skills"), "mine");
    let plugins = prepare_claude(&copies, &workspace, false, Some(&home), false, &Sources::default());
    assert_eq!(plugins.iter().map(|p| p.name).collect::<Vec<_>>(), ["project"]);
    assert_eq!(plugins[0].skills, ["project:review"]);
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(plugins[0].dir.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
    assert_eq!(manifest["name"], "project");
    // The whole skill folder, its supporting files with it.
    assert_eq!(std::fs::read_to_string(plugins[0].dir.join("skills/review/checklist.md")).unwrap(), "one\n");
    remove_copies(&copies_of(&plugins, None));
    assert!(listing(&copies).is_empty(), "the run's copies go when it ends: {:?}", listing(&copies));
    // The original is untouched.
    assert!(workspace.join(".claude/skills/review/SKILL.md").is_file());
}

#[test]
fn the_persons_own_only_when_settings_lends_them_as_the_plugin_personal() {
    let t = Temp::new();
    let (workspace, home, copies) = (t.dir("ws"), t.dir("home"), t.dir("copies"));
    skill(&home.join(".claude/skills"), "shipcheck");
    assert!(prepare_claude(&copies, &workspace, false, Some(&home), false, &Sources::default()).is_empty());
    let on = prepare_claude(&copies, &workspace, true, Some(&home), false, &Sources::default());
    assert_eq!(on.iter().map(|p| p.skills.clone()).collect::<Vec<_>>(), [vec!["personal:shipcheck".to_owned()]]);
    let copies_made = copies_of(&on, None);
    remove_copies(&copies_made);
    remove_copies(&copies_made);
    assert!(home.join(".claude/skills/shipcheck/SKILL.md").is_file());
}

#[test]
fn only_folders_holding_a_skill_md_with_a_name_a_skill_can_have_and_never_a_link() {
    let t = Temp::new();
    let root = t.dir("root");
    skill(&root, "good");
    std::fs::create_dir_all(root.join("notes")).unwrap();
    std::fs::write(root.join("notes/README.md"), "not a skill\n").unwrap();
    std::fs::write(root.join("loose.md"), "a file, not a folder\n").unwrap();
    skill(&root, ".hidden");
    skill(&root, "has space");
    let elsewhere = t.dir("elsewhere");
    skill(&elsewhere, "linked");
    std::os::unix::fs::symlink(elsewhere.join("linked"), root.join("linked")).unwrap();
    assert_eq!(skill_folders_in(&root), ["good"]);
}

#[test]
fn a_link_inside_a_skill_is_not_copied_so_removing_the_copy_cannot_reach_past_it() {
    let t = Temp::new();
    let (workspace, copies, precious) = (t.dir("ws"), t.dir("copies"), t.dir("precious"));
    std::fs::write(precious.join("keep.txt"), "keep\n").unwrap();
    let root = workspace.join(".claude/skills");
    skill(&root, "tricky");
    std::os::unix::fs::symlink(&precious, root.join("tricky/outside")).unwrap();
    let plugins = prepare_claude(&copies, &workspace, false, None, false, &Sources::default());
    assert_eq!(listing(&plugins[0].dir.join("skills/tricky")), ["SKILL.md"]);
    remove_copies(&copies_of(&plugins, None));
    // Electron also sweeps the whole copies folder at start (clearClaudeSkillCopies); a sweep follows no link either.
    let _ = std::fs::remove_dir_all(&copies);
    assert_eq!(std::fs::read_to_string(precious.join("keep.txt")).unwrap(), "keep\n");
}

#[test]
fn a_folder_with_no_skills_hands_over_nothing_and_leaves_nothing_behind() {
    let t = Temp::new();
    let (workspace, copies) = (t.dir("ws"), t.dir("copies"));
    assert!(prepare_claude(&copies, &workspace, false, None, false, &Sources::default()).is_empty());
    assert!(listing(&copies).is_empty(), "{:?}", listing(&copies));
}

#[test]
fn a_skill_folder_past_the_size_bound_is_left_out() {
    let t = Temp::new();
    let root = t.dir("root");
    skill(&root, "small");
    skill(&root, "huge");
    std::fs::write(root.join("huge/blob.bin"), vec![0u8; 11 * 1024 * 1024]).unwrap();
    assert_eq!(skill_folders_in(&root), ["small"]);
}
