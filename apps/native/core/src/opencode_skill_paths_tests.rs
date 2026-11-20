//! main/an-opencode-run-keeps-the-persons-skill-paths.test.ts, case for case. HOME, XDG and every OpenCode variable
//! are passed in: nothing here reads the person's own `~/.config/opencode`.
use super::*;
use crate::approval::Mode;
use crate::runtime::opencode_config::{legacy_config, with_skills};

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    cwd: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn fixture() -> Fixture {
    let base = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = base.join(format!("locust-skill-paths-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let (home, project) = (root.join("home"), root.join("project"));
    let cwd = project.join("child");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(project.join(".git"), "A worktree marker: never read.").unwrap();
    Fixture { root, home, project, cwd }
}
fn put(file: &Path, text: &str) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}
fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn keeps_the_last_config_that_names_them_in_opencodes_own_order_without_changing_any_file() {
    let f = fixture();
    let global = f.home.join(".config/opencode/opencode.jsonc");
    let explicit = f.root.join("custom.jsonc");
    let local = f.project.join("opencode.json");
    let text = "{ // do not mistake a URL for a comment\n \"skills\": {\"paths\": [\"/global\", \"https://example.com//folder\", \"/global\"],},\n \"provider\": {\"example\": {\"key\": \"{file:do-not-read-auth}\"}},\n}\n";
    put(&global, text);
    let read = |e: &[(&str, &str)]| opencode_skill_paths(&f.cwd, &f.home, &env(e)).unwrap();
    assert_eq!(read(&[]), ["/global", "https://example.com//folder"]);
    put(&explicit, "{/* custom */\"skills\":{\"paths\":[\"/custom\",],},}");
    let explicit_s = explicit.to_string_lossy().into_owned();
    assert_eq!(read(&[("OPENCODE_CONFIG", &explicit_s)]), ["/custom"]);
    put(&local, "{\"skills\":{\"paths\":[\"/project\"]}}");
    assert_eq!(read(&[("OPENCODE_CONFIG", &explicit_s)]), ["/project"]);
    put(&f.cwd.join("opencode.json"), "{\"skills\":{\"paths\":[\"/child\"]}}");
    assert_eq!(read(&[]), ["/child"]);
    // In one folder, .jsonc comes after .json; a config that names no paths leaves the last list alone.
    put(&f.cwd.join("opencode.jsonc"), "{\"skills\":{\"paths\":[\"/child-jsonc\"]}}");
    put(&f.cwd.join(".opencode/opencode.json"), "{\"skills\":{}}");
    assert_eq!(read(&[]), ["/child-jsonc"]);
    put(&f.cwd.join(".opencode/opencode.json"), "{\"skills\":{\"paths\":[\"/dot-opencode\"]}}");
    assert_eq!(read(&[]), ["/dot-opencode"]);
    let paths = read(&[("OPENCODE_CONFIG_CONTENT", "{\"skills\":{\"paths\":[\"/inline\"]}}")]);
    assert_eq!(paths, ["/inline"]);
    // The run's own config carries them, then its folder (createOpenCodeRunCommand's skillPaths).
    let config = with_skills(legacy_config(Mode::Ask, None), Some("/run/skills"), &paths);
    assert_eq!(config["skills"]["paths"], serde_json::json!(["/inline", "/run/skills"]));
    assert_eq!(std::fs::read_to_string(&global).unwrap(), text);
    assert_eq!(std::fs::read_to_string(&local).unwrap(), "{\"skills\":{\"paths\":[\"/project\"]}}");
}

#[test]
fn reads_the_global_folder_once_and_first_honors_xdg_and_the_config_directories_and_stops_at_the_worktree() {
    let f = fixture();
    let xdg = f.root.join("xdg");
    let over = f.root.join("override");
    let (xdg_s, over_s) = (xdg.to_string_lossy().into_owned(), over.to_string_lossy().into_owned());
    let read = |extra: &[(&str, &str)]| {
        let mut e = env(&[("XDG_CONFIG_HOME", &xdg_s)]);
        e.extend(env(extra));
        opencode_skill_paths(&f.cwd, &f.home, &e).unwrap()
    };
    put(&f.root.join("opencode.json"), "{\"skills\":{\"paths\":[\"/outside-worktree\"]}}");
    assert!(read(&[]).is_empty(), "a folder above the worktree's .git is not read");
    put(&xdg.join("opencode/opencode.json"), "{\"skills\":{\"paths\":[\"/global\"]}}");
    assert_eq!(read(&[]), ["/global"]);
    // Global then project gives the project's, as OpenCode does: the global folder is not read again after it.
    put(&f.project.join("opencode.json"), "{\"skills\":{\"paths\":[\"/project\"]}}");
    assert_eq!(read(&[]), ["/project"]);
    put(&f.home.join(".opencode/opencode.json"), "{\"skills\":{\"paths\":[\"/home-opencode\"]}}");
    assert_eq!(read(&[]), ["/home-opencode"]);
    put(&over.join("opencode.jsonc"), "{\"skills\":{\"paths\":[\"/override\"]}}");
    assert_eq!(read(&[("OPENCODE_CONFIG_DIR", &over_s)]), ["/override"]);
}

#[test]
fn does_not_load_disabled_project_config_and_names_a_file_it_cannot_read() {
    let f = fixture();
    let broken = f.project.join("opencode.jsonc");
    put(&broken, "{broken");
    for value in ["1", "true"] {
        assert!(opencode_skill_paths(&f.cwd, &f.home, &env(&[("OPENCODE_DISABLE_PROJECT_CONFIG", value)])).unwrap().is_empty());
    }
    assert_eq!(
        opencode_skill_paths(&f.cwd, &f.home, &env(&[])).unwrap_err(),
        format!("OpenCode’s settings in {} aren’t valid JSON, so nothing was started. Fix them, then send again.", broken.display())
    );
}

#[test]
fn keeps_an_empty_configuration_empty_and_returns_only_string_paths() {
    let f = fixture();
    assert!(opencode_skill_paths(&f.cwd, &f.home, &env(&[])).unwrap().is_empty());
    let content = "{\"skills\":{\"paths\":[false,1,null,\"/kept\",\"\"]}}";
    assert_eq!(opencode_skill_paths(&f.cwd, &f.home, &env(&[("OPENCODE_CONFIG_CONTENT", content)])).unwrap(), ["/kept"]);
}
