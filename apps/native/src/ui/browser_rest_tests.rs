//! Speed 1, row 2: a resting window does no disk reads for the browser, and an open browser still sees a changed folder.
use super::*;

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Dirs(std::path::PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dirs() -> Dirs {
    let root = std::env::temp_dir().join(format!(
        "locust-browser-rest-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    for sub in ["native", "electron", "one", "two"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    Dirs(root)
}

fn reads() -> usize {
    READS.with(|n| n.get())
}

/// A window with a teammate selected whose folder is `folder`, over the data in `d`.
fn window(d: &Dirs, folder: &str) -> State {
    let mut s = State::default();
    s.data = Some(crate::store::Store::new(d.0.join("native"), d.0.join("electron")).unwrap());
    s.teammates = vec![Teammate {
        id: "scout".into(),
        name: "Scout".into(),
        role: String::new(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: String::new(),
        folder: Some(d.0.join(folder).to_string_lossy().into_owned()),
    }];
    s.conversations.insert("scout".into(), Default::default());
    s
}

#[test]
fn a_closed_browser_reads_nothing_at_rest() {
    let d = dirs();
    let mut s = window(&d, "one");
    let before = reads();
    for _ in 0..100 {
        assert_eq!(keep_up(&mut s), RESTING, "closed with no tabs, it looks again only once a second");
    }
    assert_eq!(reads(), before, "a hundred resting polls read nothing from disk");
    assert_eq!(crate::invariants::lock(&s.browser.host).workspace, None);
}

#[test]
fn an_open_browser_reads_once_a_second_and_sees_a_changed_folder_at_once() {
    let d = dirs();
    let mut s = window(&d, "one");
    crate::invariants::lock(&s.browser.host).open = true;
    let before = reads();
    assert_eq!(keep_up(&mut s), BUSY);
    let one = d.0.join("one").canonicalize().unwrap();
    assert_eq!(crate::invariants::lock(&s.browser.host).workspace.as_deref(), Some(one.as_path()));
    let after_first = reads();
    assert!(after_first > before);
    for _ in 0..100 {
        keep_up(&mut s);
    }
    assert_eq!(reads(), after_first, "polls within the second read nothing more");
    // The person moves the teammate to another folder: the next poll looks it up.
    s.teammates[0].folder = Some(d.0.join("two").to_string_lossy().into_owned());
    keep_up(&mut s);
    let two = d.0.join("two").canonicalize().unwrap();
    assert_eq!(crate::invariants::lock(&s.browser.host).workspace.as_deref(), Some(two.as_path()));
}

#[test]
fn a_resting_window_s_poll_looks_at_the_browser_again_only_once_a_second() {
    let d = dirs();
    let mut s = window(&d, "one");
    TEST_ATTACHED.with(|a| a.set(true));
    let before = reads();
    poll(&mut s, &mut Tx::default());
    assert_eq!(s.browser.look_again, RESTING, "not every 100 ms while the browser is closed");
    assert_eq!(reads(), before);
    crate::invariants::lock(&s.browser.host).open = true;
    poll(&mut s, &mut Tx::default());
    assert_eq!(s.browser.look_again, BUSY, "an open browser follows its pages");
}
