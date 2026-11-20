//! Case 9 of Electron's `sols-review-is-answered.test.ts`: a conflict-marker check that fails is not "no markers", and
//! the receipt then never says the branch can land. `files_with_markers` runs real git here, so its three answers come
//! from git itself: a reference git cannot read (exit 128), a clean file (grep's exit 1), and a marked file.
use super::*;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(args: &[&str], cwd: &Path) {
    let out = std::process::Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn repository() -> Temp {
    let p = std::env::temp_dir().join(format!("locust-sol-markers-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&p).unwrap();
    let t = Temp(std::fs::canonicalize(&p).unwrap());
    git(&["init", "-q", "-b", "main"], &t.0);
    git(&["config", "user.email", "you@example.test"], &t.0);
    git(&["config", "user.name", "You"], &t.0);
    git(&["config", "commit.gpgsign", "false"], &t.0);
    std::fs::write(t.0.join("cart.py"), "x = 0\n").unwrap();
    std::fs::write(t.0.join("merged.py"), "<<<<<<< ours\nx = 1\n=======\nx = 2\n>>>>>>> theirs\n").unwrap();
    git(&["add", "."], &t.0);
    git(&["commit", "-q", "-m", "first"], &t.0);
    t
}

// case: 9. a marker check that fails is not "no markers", and the thread does not say it can land
#[test]
fn a_marker_check_that_fails_is_not_no_markers_and_the_thread_does_not_say_it_can_land() {
    let repo = repository();
    let m = Manager::new(&repo.0);
    let cart = vec!["cart.py".to_owned()];
    assert_eq!(m.files_with_markers(&repo.0, "no-such-ref", &cart), None, "git could not read the tree");
    assert_eq!(m.files_with_markers(&repo.0, "HEAD", &cart), Some(Vec::new()), "grep found nothing");
    let merged = vec!["merged.py".to_owned()];
    assert_eq!(m.files_with_markers(&repo.0, "HEAD", &merged), Some(merged.clone()), "grep found the markers");

    let said = checkpoint_sentence(&Ok(Checkpoint::Committed {
        sha: "abcdef1234567890".into(),
        branch: "locust/wren".into(),
        files: cart.clone(),
        skipped: Vec::new(),
        merge_finished: true,
        still_marked: None,
    }))
    .unwrap_or_default();
    assert!(!said.contains("can land now"), "{said}");
    assert!(said.contains("could not check it for conflict markers"), "{said}");
    // The control: a check that found nothing does say it can land.
    let clean = checkpoint_sentence(&Ok(Checkpoint::Committed {
        sha: "abcdef1234567890".into(),
        branch: "locust/wren".into(),
        files: cart,
        skipped: Vec::new(),
        merge_finished: true,
        still_marked: Some(Vec::new()),
    }))
    .unwrap_or_default();
    assert!(clean.contains("can land now"), "{clean}");
}
