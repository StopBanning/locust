//! LIVE2 #10: the folder chip and a door-started turn name the same folder. The window's startup folder (a launch
//! folder or `--workspace`, which only the window resolves) becomes the one Locust has open, which the engine and the
//! door read; Electron has one folder.
use crate::*;
use rune::{Env, Runtime};
use std::path::PathBuf;

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_folder_the_window_opens_at_startup_is_where_a_door_turn_runs() {
    let root = std::env::temp_dir().join(format!("locust-startup-folder-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let _dirs = Dirs(root.clone());
    let launch = root.join("launched-here");
    std::fs::create_dir_all(&launch).unwrap();
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let _hook = native::hooks::install(store.clone(), None);
    assert_eq!(native::open_folder(), None, "nothing is open before the window starts");

    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    apply_team(
        &mut s,
        vec![Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Code & Migrations".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        }],
    );
    // The window resolves its launch folder, as resolve_startup_workspace does for a project launch.
    let launched = launch.to_string_lossy().into_owned();
    let seen = std::cell::RefCell::new(None);
    open_startup_folder(&mut s, |remembered| {
        *seen.borrow_mut() = Some(remembered.map(str::to_owned));
        Some(launched.clone())
    });
    assert_eq!(seen.into_inner(), Some(None), "the remembered folder is what the resolver is asked about");
    assert_eq!(s.home.folder_path.as_deref(), Some(launched.as_str()));

    // The window's chip names it.
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let ([x, y, w, h], _) = rt.presented("folder-chip").expect("the folder chip is on screen");
    let words: Vec<String> = rt
        .elements()
        .into_iter()
        .filter(|e| e.rect[0] + e.rect[2] / 2.0 >= x && e.rect[0] + e.rect[2] / 2.0 <= x + w && e.rect[1] + e.rect[3] / 2.0 >= y && e.rect[1] + e.rect[3] / 2.0 <= y + h)
        .filter_map(|e| e.text)
        .collect();
    assert!(words.iter().any(|t| t == "launched-here"), "the folder chip names the launch folder: {words:?}");

    // The engine and the door read the same folder: a door turn with no teammate runs there.
    let canonical = launch.canonicalize().unwrap();
    assert_eq!(native::open_folder().and_then(|p| p.canonicalize().ok()), Some(canonical.clone()));
    let door_turn = native::folder_for(&native::nobody(Some("codex".into()), None), store.electron()).unwrap();
    assert_eq!(door_turn, canonical, "a door-started turn runs where the window's chip says");

    // The next start remembers it, and a start that resolves to what is remembered changes nothing.
    let mut again = State::default();
    again.data = Some(store.clone());
    open_startup_folder(&mut again, |remembered| remembered.map(str::to_owned));
    assert_eq!(again.home.folder_path.map(PathBuf::from).and_then(|p| p.canonicalize().ok()), Some(canonical));
}
