//! Cloud view 1, row 1: what the last read of a cloud session brought, and when the person last opened its row, kept
//! in the session's own record, so its row says it without a new read. Records saved before these fields still read.

use std::sync::atomic::{AtomicU64, Ordering};

use super::{ClaudeCloud, ClaudeCloudReading, ClaudeCloudSession, CloudExchange, CloudPlatform, first_line};

const ID: &str = "cc_0123456789abcdef0123";
const SESSION: &str = "session_01G8RT9yXr7dbhVM7EUzpvEa";

fn store_file(label: &str) -> std::path::PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "locust-cloud-glance-{label}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("claude-cloud.json")
}

fn cloud(path: &std::path::Path) -> ClaudeCloud<'static> {
    ClaudeCloud::new(path, CloudPlatform::Linux, || None)
}

/// A record as 0.56x saved it: no reply, no opened time.
fn old_record(path: &std::path::Path) {
    let old = serde_json::json!({ "sessions": [{
        "id": ID, "startedAt": "2026-11-26T09:00:00.000Z", "prompt": "Fix the cart total", "folder": "/work",
        "sessionId": SESSION, "url": format!("https://claude.ai/code/{SESSION}"), "title": "Cart total"
    }] });
    std::fs::write(path, old.to_string()).unwrap();
}

fn reading(answers: &[(&str, Option<&str>, &str)], checked_at: &str) -> ClaudeCloudReading {
    ClaudeCloudReading::Ok {
        exchanges: answers
            .iter()
            .map(|(prompt, answer, at)| CloudExchange {
                prompt: (*prompt).into(),
                answer: answer.map(str::to_string),
                at: (*at).into(),
                model: None,
            })
            .collect(),
        checked_at: checked_at.into(),
        diff: None,
        change_too_big: false,
        note: None,
    }
}

#[test]
fn a_record_saved_before_the_glance_fields_still_reads_with_them_empty() {
    let path = store_file("old");
    old_record(&path);
    let sessions = cloud(&path).all();
    assert_eq!(sessions.len(), 1);
    let one = &sessions[0];
    assert_eq!(one.session_id.as_deref(), Some(SESSION));
    assert_eq!(one.title.as_deref(), Some("Cart total"));
    assert_eq!((one.last_reply.as_ref(), one.last_reply_at.as_ref(), one.news_at.as_ref(), one.opened_at.as_ref()), (None, None, None, None));
    assert!(!one.has_news());
}

#[test]
fn a_glance_field_that_does_not_read_is_dropped_alone_and_the_session_stays() {
    let path = store_file("bad");
    let saved = serde_json::json!({ "sessions": [{
        "id": ID, "startedAt": "2026-11-26T09:00:00.000Z", "prompt": "Fix the cart total", "folder": "/work",
        "lastReply": 42, "lastReplyAt": "yesterday", "newsAt": "2026-11-26T10:00:00.000Z", "openedAt": null
    }] });
    std::fs::write(&path, saved.to_string()).unwrap();
    let sessions = cloud(&path).all();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].last_reply, None);
    assert_eq!(sessions[0].last_reply_at, None);
    assert_eq!(sessions[0].news_at.as_deref(), Some("2026-11-26T10:00:00.000Z"));
}

#[test]
fn a_read_keeps_the_newest_replys_first_line_and_its_time_and_that_is_news() {
    let mut one = ClaudeCloudSession::default();
    one.take_reading(&reading(
        &[
            ("Fix the cart total", Some("Looking at cart.js."), "2026-11-26T10:30:00.000Z"),
            ("Also round", Some("\n  Fixed: the total now sums prices.\nThen I ran the tests."), "2026-11-26T10:42:00.000Z"),
            ("And the tax", None, "2026-11-26T10:50:00.000Z"),
        ],
        "2026-11-26T10:55:00.000Z",
    ));
    assert_eq!(one.last_reply.as_deref(), Some("Fixed: the total now sums prices."));
    assert_eq!(one.last_reply_at.as_deref(), Some("2026-11-26T10:42:00.000Z"));
    assert_eq!(one.news_at.as_deref(), Some("2026-11-26T10:55:00.000Z"));
    assert!(one.has_news());
    // Opened after: not new. The same reply read again later: still not new.
    one.opened_at = Some("2026-11-26T11:00:00.000Z".into());
    assert!(!one.has_news());
    one.take_reading(&reading(&[("Also round", Some("Fixed: the total now sums prices."), "2026-11-26T10:42:00.000Z")], "2026-11-26T11:30:00.000Z"));
    assert!(!one.has_news());
    // A different reply is.
    one.take_reading(&reading(&[("And the tax", Some("Tax added."), "")], "2026-11-26T11:40:00.000Z"));
    assert_eq!(one.last_reply.as_deref(), Some("Tax added."));
    assert_eq!(one.last_reply_at.as_deref(), Some("2026-11-26T11:40:00.000Z"), "no time in the transcript: the read's");
    assert!(one.has_news());
}

#[test]
fn a_failed_read_or_one_with_no_reply_in_words_keeps_what_was_kept() {
    let mut one = ClaudeCloudSession { last_reply: Some("Fixed.".into()), ..Default::default() };
    let before = one.clone();
    one.take_reading(&ClaudeCloudReading::Err { message: "no".into() });
    one.take_reading(&reading(&[("Fix it", None, "2026-11-26T10:42:00.000Z")], "2026-11-26T10:55:00.000Z"));
    assert_eq!(one, before);
}

#[test]
fn a_long_first_line_is_cut_and_says_so() {
    let long = "word ".repeat(60);
    let line = first_line(&long).unwrap();
    assert!(line.ends_with('\u{2026}') && line.chars().count() <= 160, "{line}");
    assert!(line.starts_with("word word"), "{line}");
    assert_eq!(first_line("   \n\n"), None);
}

#[test]
fn opening_the_row_is_kept_in_the_record_and_survives_a_new_read() {
    let path = store_file("opened");
    old_record(&path);
    let claude = cloud(&path);
    claude.mark_opened(ID, "2026-11-26T11:00:00.000Z");
    claude.mark_opened(ID, "not a time");
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("\"openedAt\": \"2026-11-26T11:00:00.000Z\""), "{saved}");
    let mut one = claude.all().remove(0);
    assert_eq!(one.opened_at.as_deref(), Some("2026-11-26T11:00:00.000Z"));
    one.take_reading(&reading(&[("Fix it", Some("Done."), "2026-11-26T11:05:00.000Z")], "2026-11-26T11:06:00.000Z"));
    assert!(one.has_news());
}
