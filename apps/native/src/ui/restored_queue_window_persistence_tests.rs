use super::*;
use rune::{Env, Runtime};

struct QueueRoot(std::path::PathBuf);

impl Drop for QueueRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn queue_window() -> (QueueRoot, State) {
    let root = QueueRoot(std::env::temp_dir().join(format!("locust-queue-disk-{}", crate::invariants::unique_nanos())));
    let data = store::Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let mut s = State::default();
    s.data = Some(data);
    s.engine = Some(Kind::Native);
    s.teammates.push(Teammate {
        id: "tm_wren".into(), name: "Wren".into(), role: "Code".into(),
        runtime: Some("codex".into()), model: Some("gpt-5.6-luna".into()),
        account: None, busy: false, runs_in: "ask".into(), folder: Some(root.0.to_string_lossy().into_owned()),
    });
    s.conversations.insert("tm_wren".into(), Conversation {
        conversation_id: Some("run_one".into()), ..Default::default()
    });
    (root, s)
}

fn row(id: &str, key: &str, text: &str, origin: composer::Origin) -> composer::QueuedRow {
    composer::QueuedRow {
        id: id.into(), key: key.into(), text: text.into(), origin,
        attachments: Vec::new(), issue: None, restored: false, held: None, retry_at: None, tries: None,
    }
}

// case: A failed write keeps the previous queue and reports that nothing changed.
#[test]
fn failed_queue_discard_keeps_the_two_messages_and_says_not_saved() {
    let (_root, mut s) = queue_window();
    s.queue_rows = vec![
        row("q_one", "run_one", "Check it.", composer::Origin::Person),
        row("q_other", "run_other", "Keep me.", composer::Origin::Person),
    ];
    assert!(persist_queue(&mut s));
    let path = composer::queue_store::queue_path(s.data.as_ref().unwrap()).unwrap();
    let backup = path.with_extension("backup");
    std::fs::rename(&path, &backup).unwrap();
    std::fs::create_dir(&path).unwrap();
    fn draw(s: &State, _: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, draw, (1000., 900.), Env::new(true, 1.));
    rt.frame(0.);
    let button = rt.elements().into_iter().find(|e| e.path.contains("/queued") && e.text.as_deref() == Some("Discard"))
        .expect("the live queue's Discard button");
    let [x, y, w, h] = button.rect;
    rt.pointer_down(x + w / 2., y + h / 2.);
    rt.pointer_up(x + w / 2., y + h / 2.);
    rt.frame(0.016);
    assert_eq!(rt.state.queue_rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(), ["Check it.", "Keep me."]);
    assert!(rt.state.queue_error.as_deref().is_some_and(|e| e.contains("not saved")));
    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(backup, path).unwrap();
    let saved = composer::read_queue(rt.state.data.as_ref().unwrap()).unwrap();
    assert_eq!(saved.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(), ["Check it.", "Keep me."]);
}

// case: Only person-written messages are persisted rather than host scheduling intents.
#[test]
fn a_host_intent_stays_in_the_window_but_is_not_written_with_a_person_message() {
    let (_root, mut s) = queue_window();
    s.queue_rows.push(row("q_host", "run_other", "Host only.", composer::Origin::Host));
    assert!(enqueue_follow_up(&mut s, "Check it.".into(), Vec::new()));
    assert_eq!(s.queue_rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(), ["Host only.", "Check it."]);
    let saved = composer::read_queue(s.data.as_ref().unwrap()).unwrap();
    assert_eq!(saved.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(), ["Check it."]);
}
