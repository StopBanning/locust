//! Carries cases 6-7 of `apps/desktop/src/main/a-teammate-signs-in-apart-in-the-browser.test.ts` (T256):
//! `ownBrowser` is recognized only as literal boolean `true`, and turning it off or removing the teammate
//! forgets the teammate's browser tabs while keeping shared tabs.
use super::*;
use serde_json::json;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(name: &str) -> Temp {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "locust-browser-panel-t256-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    Temp(root)
}

// case: is kept on the teammate’s record, only as a literal true, and an edit of its name keeps it
#[test]
fn own_browser_is_kept_only_as_literal_true_and_name_edit_keeps_it() {
    let t = temp("literal-true");
    let store = crate::store::Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    let mut s = State::default();
    s.data = Some(store.clone());
    crate::invariants::lock(&s.browser.host).open = true;

    // A non-boolean string "yes" is NOT accepted as ownBrowser: true.
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren", "ownBrowser": "yes"}]}),
        )
        .unwrap();
    s.browser.profile_refresh = None;
    keep_up(&mut s);
    assert!(crate::invariants::lock(&s.browser.host).profiles.is_empty());

    // Literal boolean true IS accepted, and renaming the teammate keeps it.
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren 2", "ownBrowser": true}]}),
        )
        .unwrap();
    s.browser.profile_refresh = None;
    keep_up(&mut s);
    assert_eq!(
        crate::invariants::lock(&s.browser.host).profiles,
        vec![("tm_wren".to_owned(), "Wren 2".to_owned())]
    );
}

// case: is a switch in the teammate dialog, cleared when turned off or the teammate is removed
#[test]
fn own_browser_switch_forgets_teammate_tabs_when_turned_off_or_removed() {
    let t = temp("forget-off");
    let store = crate::store::Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren", "ownBrowser": true}]}),
        )
        .unwrap();

    let mut s = State::default();
    s.data = Some(store.clone());
    s.teammates.push(Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.browser.menu = true;
    crate::invariants::lock(&s.browser.host).open = true;
    keep_up(&mut s);
    let shared_id = crate::invariants::lock(&s.browser.host).add(Profile(None)).unwrap();
    let _wren_id = crate::invariants::lock(&s.browser.host)
        .add(Profile(Some("tm_wren".into())))
        .unwrap();

    // The menu shows Wren's switch as "on".
    let mut rt = rune::Runtime::new(
        s,
        |s, _| browser_panel::view(s, &Palette::new(true)).unwrap_or_else(col),
        (600.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let texts: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(
        texts.iter().any(|line| line.contains("Wren · Own browser sign-ins: on")),
        "{texts:?}"
    );

    // Turning ownBrowser off in teammates.json causes keep_up to forget Wren's tabs and keep the shared tab.
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren"}]}),
        )
        .unwrap();
    rt.state.browser.profile_refresh = None;
    keep_up(&mut rt.state);
    let host = crate::invariants::lock(&rt.state.browser.host);
    assert_eq!(host.tabs.iter().map(|t| t.id).collect::<Vec<_>>(), vec![shared_id]);
    assert!(host.forgotten.contains(&Profile(Some("tm_wren".into()))));
}
