use super::*;
use std::path::Path;
use std::time::{Duration, Instant};

fn settle(s: &mut State) {
    let stop = Instant::now() + Duration::from_secs(4);
    while s.issues_job.is_pending() {
        assert!(Instant::now() < stop, "issues job did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn issue_choose_ignores_unlisted_or_stale_picker_requests() {
    // Ensure a test gh stand-in is installed without overwriting another test's active fake.
    {
        let mut held = GH.lock().unwrap_or_else(|e| e.into_inner());
        if held.is_none() {
            *held = Some(Arc::new(|_args: &[&str], _at: &Path| {
                Ok(serde_json::json!({
                    "title": "Unlisted issue",
                    "url": "https://github.com/acme/hearth/issues/99",
                    "body": "Should never be opened.",
                    "labels": [],
                    "comments": []
                })
                .to_string())
            }));
        }
    }

    let mut s = State::default();
    crate::scenes::ledger_scene(&mut s, false, false);
    s.ledger.as_mut().unwrap().header.folder = Some("/tmp/hearth".into());
    s.ledger.as_mut().unwrap().conversation.conversation_id = Some("conv-a".into());
    let at = target(&s).expect("target");
    s.draft.replace_text("Keep my draft.");
    s.issues.open = true;
    s.issues.picker_target = Some(at);
    s.issues.issues = vec![ListedIssue {
        number: 5,
        title: "Crashes on start".into(),
        url: "https://github.com/acme/hearth/issues/5".into(),
        author: Some("dana".into()),
        created_at: "2026-10-12T09:00:00Z".into(),
        labels: vec![],
    }];

    // Choosing an issue number not in the current list must do nothing.
    choose(&mut s, 99);
    settle(&mut s);
    assert!(s.issues_job.take().is_none(), "an unlisted issue number must not start a gh read");
    assert_eq!(s.draft.text, "Keep my draft.");

    // Choosing a listed issue after the picker is no longer current (closed) must also do nothing.
    s.issues.open = false;
    choose(&mut s, 5);
    settle(&mut s);
    assert!(s.issues_job.take().is_none(), "a closed/stale picker must not start a gh read");
    assert_eq!(s.draft.text, "Keep my draft.");
}
