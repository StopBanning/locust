use super::*;

const REPLY: &str = "Done.\n<locust-memory>\nabout you :: Prefers a short summary before any detail.\nremember :: Tests run with pnpm test.\n</locust-memory>";

fn with_store(mode: &str, check: impl FnOnce(&Store, &Path, &dyn Fn() -> Store)) {
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Temp(std::env::temp_dir().join(id("native-about-you-test").unwrap()));
    let open = || Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let folder = root.0.join("shop");
    std::fs::create_dir_all(&folder).unwrap();
    let store = open();
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [{"teammateId": "tm_ada", "name": "Ada", "role": "Custom"}],
                "missionOwners": {},
                "settings": {"memoryMode": mode, "aboutYou": "Keep it short."}
            }),
        )
        .unwrap();
    check(&store, &folder, &open);
}

fn assert_suggestion_after_reopening(store: &Store, expected: &str) {
    let roster = team(store).unwrap();
    assert_eq!(roster["settings"]["aboutYou"], "Keep it short.");
    let suggestions = roster["settings"]["aboutYouSuggestions"]
        .as_array()
        .unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0]["text"], expected);
    assert_eq!(suggestions[0]["by"], "Ada");
}

// case: puts the line to the person, names who suggested it, and keeps the memory as the mode says
#[test]
fn about_you_suggestion_waits_but_auto_memory_is_kept_after_reopening() {
    with_store("auto", |store, folder, open| {
        let (_, changes) =
            record_reply_changes(store, "tm_ada", folder, REPLY, Some("mission_1"), None).unwrap();
        assert_eq!(
            changes.about_you_suggested,
            ["Prefers a short summary before any detail."]
        );
        assert_eq!(changes.by, "Ada");
        let reopened = open();
        assert_suggestion_after_reopening(&reopened, "Prefers a short summary before any detail.");
        let file = memories(&reopened).unwrap();
        let rows = file["memories"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["text"], "Tests run with pnpm test.");
        assert_eq!(rows[0]["status"], "kept");
    });
}

// case: with memory OFF, still puts the line to the person, and keeps no memory
#[test]
fn about_you_suggestion_waits_with_memory_off_after_reopening() {
    with_store("off", |store, folder, open| {
        record_reply_changes(store, "tm_ada", folder, REPLY, Some("mission_1"), None).unwrap();
        let reopened = open();
        assert_suggestion_after_reopening(&reopened, "Prefers a short summary before any detail.");
        assert_eq!(memories(&reopened).unwrap()["memories"], json!([]));
    });
}

// case: with memory off and no suggestion, does nothing at all
#[test]
fn memory_off_without_an_about_you_line_neither_writes_nor_notifies() {
    with_store("off", |store, folder, open| {
        let reply =
            "Done.\n<locust-memory>\nremember :: Tests run with pnpm test.\n</locust-memory>";
        let roster_path = store.root().join("teammates.json");
        let before = std::fs::read(&roster_path).unwrap();
        let (notices, changes) =
            record_reply_changes(store, "tm_ada", folder, reply, Some("mission_1"), None).unwrap();
        assert!(notices.is_empty());
        assert!(changes.is_empty());
        assert_eq!(std::fs::read(&roster_path).unwrap(), before);
        assert!(!store.root().join("memories.json").exists());
        let reopened = open();
        let roster = team(&reopened).unwrap();
        assert_eq!(roster["settings"]["aboutYou"], "Keep it short.");
        assert!(roster["settings"].get("aboutYouSuggestions").is_none());
        assert_eq!(memories(&reopened).unwrap()["memories"], json!([]));
    });
}

// index.ts:4573 (suggestAboutYou, 0.424): a teammate's line waits for the person once -- not again while it is
// waiting (trimmed, any case), and not if the note already says it.
#[test]
fn an_about_you_line_waits_once_and_not_when_the_note_already_says_it() {
    with_store("auto", |store, folder, open| {
        let line = |text: &str| format!("Done.\n<locust-memory>\nabout you :: {text}\n</locust-memory>");
        let (_, first) = record_reply_changes(store, "tm_ada", folder, &line("Prefers a short summary before any detail."), Some("mission_1"), None).unwrap();
        assert_eq!(first.about_you_suggested, ["Prefers a short summary before any detail."]);
        // The same line again, in another case and with spaces around it: still waiting, so not asked again.
        let (_, again) = record_reply_changes(store, "tm_ada", folder, &line("  PREFERS a short summary before any detail.  "), Some("mission_2"), None).unwrap();
        assert!(again.about_you_suggested.is_empty());
        // The note already says "Keep it short.": a line it contains is never put to the person.
        let (_, said) = record_reply_changes(store, "tm_ada", folder, &line("keep it short"), Some("mission_3"), None).unwrap();
        assert!(said.about_you_suggested.is_empty());
        assert_suggestion_after_reopening(&open(), "Prefers a short summary before any detail.");
    });
}
