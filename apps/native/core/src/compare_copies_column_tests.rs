//! Electron's `a-column-answers-in-a-copy.test.ts` (6 cases): a column that answers in a copy works in the folder as it
//! is, uncommitted work included, without what is rebuilt or fetched; a copy is reused for follow-ups, removed with its
//! comparison, and never named outside its root. Who answers in a copy: nobody now; Antigravity's app cannot join.
use super::*;
use crate::compare::{compare_needs_copy, compare_refusal_of};
use crate::routine_copy::MAX_COPY_BYTES;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn made(prefix: &str) -> Temp {
    let d = std::env::temp_dir().join(format!(
        "locust-{prefix}-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    ));
    fs::create_dir_all(&d).unwrap();
    Temp(d)
}
fn names(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

// case: holds the folder as it is, without what is rebuilt or fetched, and is reused for follow-ups
#[test]
fn holds_the_folder_as_it_is_without_what_is_rebuilt_or_fetched_and_is_reused_for_follow_ups() {
    let (folder, root) = (made("copy-src"), made("copy-root"));
    fs::create_dir_all(folder.0.join("src")).unwrap();
    fs::write(folder.0.join("src/cart.py"), "unsaved work\n").unwrap();
    for left in [".git", "node_modules", ".locust"] {
        fs::create_dir_all(folder.0.join(left)).unwrap();
        fs::write(folder.0.join(left).join("x"), "x").unwrap();
    }
    let copy = make_compare_copy(&folder.0, "cmp_1", CompareSlotId::B, &root.0, None).unwrap();
    assert_eq!(
        fs::read_to_string(copy.join("src/cart.py")).unwrap(),
        "unsaved work\n"
    );
    for left in [".git", "node_modules", ".locust"] {
        assert!(
            fs::symlink_metadata(copy.join(left)).is_err(),
            "{left} is left out of the copy"
        );
    }

    // A follow-up reuses it: what the column did there is still there.
    fs::write(copy.join("notes.md"), "its own\n").unwrap();
    fs::write(copy.join("src/cart.py"), "the column's fix\n").unwrap();
    fs::write(folder.0.join("src/cart.py"), "the person kept working\n").unwrap();
    assert_eq!(
        make_compare_copy(&folder.0, "cmp_1", CompareSlotId::B, &root.0, None).unwrap(),
        copy
    );
    assert_eq!(
        fs::read_to_string(copy.join("notes.md")).unwrap(),
        "its own\n"
    );
    assert_eq!(
        fs::read_to_string(copy.join("src/cart.py")).unwrap(),
        "the column's fix\n",
        "reused, never copied over again"
    );
    // Its record is still the first copy's, so what the column changed is still counted as its own.
    assert_eq!(
        copy_changes("cmp_1", CompareSlotId::B, &root.0)
            .unwrap()
            .changed,
        ["notes.md", "src/cart.py"]
    );
    // And the folder itself never saw it.
    assert!(!folder.0.join("notes.md").exists());
}

// case: says so, and makes nothing, when the folder is too big to copy
#[test]
fn says_so_and_makes_nothing_when_the_folder_is_too_big_to_copy() {
    let (folder, root) = (made("copy-big"), made("copy-root"));
    for index in 0..6 {
        fs::write(folder.0.join(format!("f{index}.txt")), "").unwrap();
    }
    let refused = make_compare_copy(
        &folder.0,
        "cmp_2",
        CompareSlotId::A,
        &root.0,
        Some(Limits {
            files: 5,
            bytes: MAX_COPY_BYTES,
        }),
    )
    .unwrap_err();
    assert!(
        refused.contains("too big to copy (more than 5 files"),
        "{refused}"
    );
    assert_eq!(names(&root.0), Vec::<String>::new());
    // Within the limit, the same folder is copied.
    assert!(
        make_compare_copy(
            &folder.0,
            "cmp_2",
            CompareSlotId::A,
            &root.0,
            Some(Limits {
                files: 6,
                bytes: MAX_COPY_BYTES
            })
        )
        .is_ok()
    );
}

// case: leaves a file too big to copy out, and copies the rest
#[test]
fn leaves_a_file_too_big_to_copy_out_and_copies_the_rest() {
    let (folder, root) = (made("copy-bigfile"), made("copy-root"));
    fs::write(folder.0.join("index.html"), "<p>x</p>").unwrap();
    fs::File::create(folder.0.join("clip.mp4"))
        .unwrap()
        .set_len(MAX_COPIED_FILE_BYTES + 1)
        .unwrap();
    assert_eq!(
        copy_refusal(
            &folder.0,
            Some(Limits {
                files: 1,
                bytes: 1024
            })
        ),
        None
    );
    let copy = make_compare_copy(&folder.0, "cmp_9", CompareSlotId::A, &root.0, None).unwrap();
    assert!(copy.join("index.html").exists());
    assert!(!copy.join("clip.mp4").exists());
}

// case: removes the comparison's copies and nothing else
#[test]
fn removes_the_comparisons_copies_and_nothing_else() {
    let (folder, root) = (made("copy-src"), made("copy-root"));
    fs::write(folder.0.join("a.txt"), "a").unwrap();
    make_compare_copy(&folder.0, "cmp_3", CompareSlotId::A, &root.0, None).unwrap();
    make_compare_copy(&folder.0, "cmp_3", CompareSlotId::B, &root.0, None).unwrap();
    make_compare_copy(&folder.0, "cmp_4", CompareSlotId::A, &root.0, None).unwrap();
    assert_eq!(
        names(&root.0).len(),
        6,
        "each copy has its manifest beside it"
    );
    remove_compare_copies("cmp_3", &root.0).unwrap();
    // Each copy has its manifest beside it (0.448); both go together.
    assert_eq!(names(&root.0), ["cmp_4-a", "cmp_4-a.manifest.json"]);
    // A name that is not a comparison's removes nothing.
    remove_compare_copies("../x", &root.0).unwrap();
    assert_eq!(names(&root.0), ["cmp_4-a", "cmp_4-a.manifest.json"]);
}

// case: refuses an id that could name a path outside its root
#[test]
fn refuses_an_id_that_could_name_a_path_outside_its_root() {
    let (folder, root) = (made("copy-src"), made("copy-root"));
    let copies = root.0.join("copies");
    fs::create_dir_all(&copies).unwrap();
    let refused = make_compare_copy(&folder.0, "../escape", CompareSlotId::A, &copies, None).unwrap_err();
    assert!(refused.contains("cannot name a copy"), "{refused}");
    assert!(!root.0.join("escape-a").exists());
    assert_eq!(names(&copies), Vec::<String>::new());
}

// case: is nobody now; Antigravity cannot join at all; everyone else answers in the folder
// Electron asks per platform (`compareNeedsCopy(runtime, platform)`); the Rust app runs on one platform and asks only
// the runtime, and the answer is no for every runtime on every platform Electron names.
#[test]
fn is_nobody_now_antigravity_cannot_join_at_all_everyone_else_answers_in_the_folder() {
    for runtime in ["cursor", "claude", "codex", "opencode", "antigravity"] {
        assert!(
            !compare_needs_copy(runtime),
            "{runtime} answers in the folder"
        );
    }
    assert!(
        compare_refusal_of("antigravity", false)
            .is_some_and(|why| why.contains("only in the folder it has open"))
    );
    // Through Antigravity CLI it answers in any folder, so it may join (0.543).
    assert_eq!(compare_refusal_of("antigravity", true), None);
    assert_eq!(compare_refusal_of("cursor", false), None);
    assert_eq!(compare_refusal_of("claude", false), None);
}
