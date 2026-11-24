//! reveal-file.test.ts through the window: a file a teammate handed over, pressed (its Copy, which asks the same
//! question as its reveal and its Save a copy, and then reads the file), in a conversation whose folder is the workspace.
//! The host decides, never the reply: `resolved` asks `decide_teammate_file` / `decide_reveal` / `contains`, and a read
//! asks `inside_on_disk` of `read_roots`. Expected words are Electron's (index.ts's text channel and reveal-file.ts).
use super::*;
use crate::file_menu::{contains, decide_reveal, inside_on_disk, RevealDecision};
use std::cell::RefCell;

const OUTSIDE_WORDS: &str =
    "That file is outside the folders your teammates work in, and no teammate wrote it or handed it over, so Locust will not open it.";
const NOT_THERE_WORDS: &str = "That file is not there. The teammate named it but did not write it.";
const LINKED_OUT_WORDS: &str =
    "That file is a link to somewhere outside the folder your teammates work in, so Locust will not open it.";

thread_local! {
    static FILES: RefCell<Vec<HandedFile>> = const { RefCell::new(Vec::new()) };
}

/// A folder of its own under the system's temporary folder, links resolved, gone when the test ends.
struct Base(PathBuf);
impl Base {
    fn new() -> Self {
        let raw = std::env::temp_dir().join(format!("locust-reveal-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
        std::fs::create_dir_all(&raw).unwrap();
        Base(raw.canonicalize().unwrap())
    }
    fn write(&self, rel: &str, words: &str) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, words).unwrap();
        path
    }
}
impl Drop for Base {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn shown(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Presses Copy on a reply's handed file `path`, in a conversation whose folder is `folder` (none: no mission has run),
/// with `references` added to the folders the host knows about. What the files say beside them after: the refusal, or
/// None when the file was read.
fn press_copy(folder: Option<&Path>, references: &[&Path], path: &str) -> Option<String> {
    let mut st = State::default();
    st.teammates.push(crate::Teammate {
        id: "tm".into(),
        name: "Wren".into(),
        role: "Custom".into(),
        model: None,
        runtime: Some("claude".into()),
        folder: folder.map(shown),
        runs_in: "ask".into(),
        account: None,
        busy: false,
    });
    st.conversations.insert("tm".into(), Default::default());
    st.reference_items = references
        .iter()
        .map(|r| locust_core::references::Item { id: shown(r), name: "ref".into(), path: shown(r), repository: None, refreshed_at: None, problem: None })
        .collect();
    FILES.with(|f| *f.borrow_mut() = vec![HandedFile { path: path.to_owned(), note: None }]);
    let mut rt = rune::Runtime::new(
        st,
        |st, _| col().w(900.0).child(FILES.with(|f| view(st, &f.borrow(), &[], "r1", &Palette::new(true))).unwrap()),
        (1000.0, 300.0),
        rune::Env::new(true, 1.0),
    );
    for _ in 0..6 {
        rt.frame(0.016);
    }
    let (x, y) = rt.find("handed-file-copy-r1-0").expect("Copy beside the file");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..6 {
        rt.frame(0.016);
    }
    rt.state.handed_file_refusal.as_ref().map(|(scope, why)| {
        assert_eq!(scope, "r1");
        why.clone()
    })
}

#[test]
fn says_yes_to_a_file_in_the_folder_and_in_a_folder_below_it() {
    // case: says yes to a file in the folder and in a folder below it
    let base = Base::new();
    let shop = base.0.join("shop");
    let notes = base.write("shop/notes.md", "notes");
    let deep = base.write("shop/src/deep/file.ts", "export {}");
    assert_eq!(press_copy(Some(&shop), &[], &shown(&notes)), None);
    assert_eq!(press_copy(Some(&shop), &[], &shown(&deep)), None);
    assert_eq!(press_copy(Some(&shop), &[], "src/deep/file.ts"), None, "a relative handover is the conversation's folder's");
}

#[test]
fn says_yes_to_the_folder_itself() {
    // case: says yes to the folder itself
    let base = Base::new();
    let shop = base.0.join("shop");
    std::fs::create_dir_all(&shop).unwrap();
    assert!(contains(&shop, &shop));
    assert_eq!(decide_reveal(&shown(&shop), std::slice::from_ref(&shop)), RevealDecision::Ok { path: shop.clone(), named: false });
    // Through the press: the folder is the host's to show, so what refuses its read is the read, never "outside".
    let said = press_copy(Some(&shop), &[], &shown(&shop)).expect("a folder has no words to copy");
    assert!(said == "Locust does not open that kind of file here." || said == "That is a folder, not a file.", "{said}");
}

#[test]
fn is_not_fooled_by_a_sibling_whose_name_starts_the_same_way() {
    // case: is not fooled by a sibling whose name starts the same way
    let base = Base::new();
    let shop = base.0.join("shop");
    std::fs::create_dir_all(&shop).unwrap();
    let keys = base.write("shop-secrets/keys.txt", "secret");
    assert_eq!(press_copy(Some(&shop), &[], &shown(&keys)).as_deref(), Some(OUTSIDE_WORDS));
    assert_eq!(press_copy(Some(&shop), &[], &shown(&base.0.join("shop-secrets"))).as_deref(), Some(OUTSIDE_WORDS));
}

#[test]
fn refuses_a_walk_back_up_and_out() {
    // case: refuses a walk back up and out
    let base = Base::new();
    let shop = base.0.join("shop");
    std::fs::create_dir_all(shop.join("sub")).unwrap();
    base.write("elsewhere.txt", "not the folder's");
    let up = format!("{}/../../Windows/System32", shown(&shop));
    let out = format!("{}/sub/../../elsewhere.txt", shown(&shop));
    assert_eq!(press_copy(Some(&shop), &[], &up).as_deref(), Some(OUTSIDE_WORDS));
    assert_eq!(press_copy(Some(&shop), &[], &out).as_deref(), Some(OUTSIDE_WORDS));
    assert_eq!(press_copy(Some(&shop), &[], "sub/../../elsewhere.txt").as_deref(), Some(OUTSIDE_WORDS));
}

#[test]
fn refuses_an_unrelated_folder() {
    // case: refuses an unrelated folder
    let base = Base::new();
    let shop = base.0.join("shop");
    std::fs::create_dir_all(&shop).unwrap();
    assert_eq!(press_copy(Some(&shop), &[], "/home/someone/.ssh/id_rsa").as_deref(), Some(OUTSIDE_WORDS));
}

#[test]
fn honours_a_file_in_a_workspace_the_host_knows_about() {
    // case: honours a file in a workspace the host knows about
    let base = Base::new();
    let shop = base.0.join("shop");
    let report = base.write("shop/docs/report.md", "the rollup");
    assert_eq!(decide_reveal(&shown(&report), std::slice::from_ref(&shop)), RevealDecision::Ok { path: report.clone(), named: false });
    assert_eq!(press_copy(Some(&shop), &[], &shown(&report)), None);
}

#[test]
fn refuses_everything_when_no_mission_has_run() {
    // case: refuses everything when no mission has run
    let base = Base::new();
    let report = base.write("shop/report.md", "the rollup");
    assert_eq!(decide_reveal(&shown(&report), &[]), RevealDecision::Outside { path: shown(&report) });
    assert_eq!(press_copy(None, &[], &shown(&report)).as_deref(), Some(OUTSIDE_WORDS), "no folder known: it refuses, not falls open");
}

#[test]
fn refuses_a_path_outside_every_known_workspace() {
    // case: refuses a path outside every known workspace
    let base = Base::new();
    let shop = base.0.join("shop");
    std::fs::create_dir_all(&shop).unwrap();
    let outside = "/home/someone/.aws/credentials";
    assert_eq!(decide_reveal(outside, std::slice::from_ref(&shop)), RevealDecision::Outside { path: outside.into() });
    assert_eq!(press_copy(Some(&shop), &[], outside).as_deref(), Some(OUTSIDE_WORDS));
}

#[test]
fn refuses_a_relative_path_rather_than_trying_each_root_until_one_exists() {
    // case: refuses a relative path rather than trying each root until one exists
    let base = Base::new();
    let shop = base.0.join("shop");
    std::fs::create_dir_all(&shop).unwrap();
    let other = base.0.join("other");
    base.write("other/report.md", "in another root");
    let roots = [shop.clone(), other.clone()];
    assert!(!matches!(decide_reveal("../../etc/passwd", &roots), RevealDecision::Ok { .. }));
    assert!(!matches!(decide_reveal("report.md", &roots), RevealDecision::Ok { .. }));
    // Through the press: a relative handover is read in the conversation's own folder only, never looked for in each
    // folder the host knows until one has it.
    let said = press_copy(Some(&shop), &[&other], "report.md").expect("not in the conversation's folder");
    assert_eq!(said, NOT_THERE_WORDS, "the other root's report.md was never looked for");
    assert_eq!(press_copy(Some(&shop), &[&other], "../../etc/passwd").as_deref(), Some(OUTSIDE_WORDS));
}

#[test]
fn refuses_anything_that_is_not_a_string_and_an_empty_one() {
    // case: refuses anything that is not a string, and an empty one
    // Rust's request is a typed &str, so only the empty and the blank are left to refuse; nothing is ever looked up.
    let roots = [PathBuf::from("/work/shop")];
    for nonsense in ["", "   "] {
        assert_eq!(decide_reveal(nonsense, &roots), RevealDecision::NoPath);
        assert_eq!(crate::file_menu::decide_teammate_file(nonsense, &roots, |_| panic!("never asked")), RevealDecision::NoPath);
    }
}

#[test]
fn honours_a_file_under_any_one_of_several_workspaces() {
    // case: honours a file under any one of several workspaces
    let base = Base::new();
    let shop = base.0.join("shop");
    std::fs::create_dir_all(&shop).unwrap();
    let other = base.0.join("other");
    let out = base.write("other/out.md", "out");
    assert!(matches!(decide_reveal(&shown(&out), &[shop.clone(), other.clone()]), RevealDecision::Ok { .. }));
    assert_eq!(press_copy(Some(&shop), &[&other], &shown(&out)), None);
}

#[cfg(unix)]
#[test]
fn refuses_a_link_that_leads_out_keeps_the_folders_own_files_and_a_project_reached_through_a_junction() {
    // case: refuses a link that leads out; keeps the folder’s own files, and a project reached through a junction
    let base = Base::new();
    let project = base.0.join("project");
    let outside = base.0.join("outside");
    base.write("project/own.txt", "mine");
    base.write("outside/secret.txt", "not the folder's");
    std::os::unix::fs::symlink(&outside, project.join("linked")).unwrap();
    std::os::unix::fs::symlink(&project, base.0.join("alias")).unwrap();
    let alias = base.0.join("alias");

    // By path arithmetic the linked file is inside; on disk it is not.
    assert!(contains(&project, &project.join("linked/secret.txt")));
    assert!(!inside_on_disk(&project.join("linked/secret.txt"), std::slice::from_ref(&project)));
    assert_eq!(press_copy(Some(&project), &[], "linked/secret.txt").as_deref(), Some(LINKED_OUT_WORDS));
    assert_eq!(press_copy(Some(&project), &[], &shown(&project.join("linked/secret.txt"))).as_deref(), Some(LINKED_OUT_WORDS));

    // The folder's own file, and the project reached through a link to it.
    assert_eq!(press_copy(Some(&project), &[], "own.txt"), None);
    assert!(inside_on_disk(&alias.join("own.txt"), std::slice::from_ref(&alias)));
    assert_eq!(press_copy(Some(&alias), &[], "own.txt"), None);

    // A file never written has no link to follow; the read itself says it is missing.
    assert!(inside_on_disk(&project.join("missing.txt"), std::slice::from_ref(&project)));
    let said = press_copy(Some(&project), &[], "missing.txt").expect("nothing to copy");
    assert_eq!(said, NOT_THERE_WORDS);
}
