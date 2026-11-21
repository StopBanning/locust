//! Electron's `main/a-long-conversation-goes-by-file.test.ts` (0.513): a task too long for the brief is written whole
//! to `.locust/attachments/`, quoted from, and handed over as a file, never clipped or refused. Its values.
use super::*;
use serde_json::{Value, json};

fn checkpoint() -> Value {
    json!({
        "missionId": "mission_prior", "epoch": 2, "reason": "route-switch", "resumeSafety": "safe", "safetyReason": "settled",
        "createdAt": "2026-09-30T23:00:00.000Z", "unsettledActions": [], "settledActions": ["read README.md"],
        "assistantSummary": "I read the README and changed two files."
    })
}

/// 12,000 characters with a sentence at the very end that must not be lost.
fn long() -> String {
    format!("{}THE LAST LINE MATTERS.", "Build the habit tracker exactly as described. ".repeat(260))
}
const FILE: &str = ".locust/attachments/conversation-mission_prior.md";

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn project() -> Temp {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("locust-long-task-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    Temp(root)
}

// case: a long conversation goes to another model by file > a stopped run with a long task: refused before, handed over now
#[test]
fn a_stopped_run_with_a_long_task_refused_before_handed_over_now() {
    let task = long();
    assert!(task.len() > MAX_HANDOFF_PROMPT_LENGTH);
    assert!(compose_handoff_prompt(&task, &checkpoint(), "Codex CLI", None, &[], None).is_none());
    let briefing = compose_handoff_brief(&task, &checkpoint(), "Codex CLI", None, &[], None, &[], Some(FILE)).expect("a briefing");
    assert!(briefing.prompt.contains(&task[..200]));
    assert!(briefing.prompt.contains(&format!("The whole of it, as written, is in the file `{FILE}`")));
    assert!(!briefing.prompt.contains("did not fit here"));
    assert!(!briefing.prompt.contains("THE LAST LINE MATTERS."), "only its start is quoted");
}

// case: a long conversation goes to another model by file > beside a reply, the task is not clipped, and the reply is whole
#[test]
fn beside_a_reply_the_task_is_not_clipped_and_the_reply_is_whole() {
    let reply = "Now add a rename command. ".repeat(200);
    let briefing = compose_handoff_brief(&long(), &checkpoint(), "Codex CLI", Some(&reply), &[], None, &[], Some(FILE)).unwrap();
    assert!(briefing.prompt.contains(reply.trim()));
    assert!(!briefing.prompt.contains("did not fit here"));
    assert!(!briefing.task_clipped);
}

// case: a long conversation goes to another model by file > the brief and the line that hands over the file fit what a run will start with
#[test]
fn the_brief_and_the_line_that_hands_over_the_file_fit_what_a_run_will_start_with() {
    let reply = "x".repeat(6_000);
    let briefing = compose_handoff_brief(&long(), &checkpoint(), "Codex CLI", Some(&reply), &[], None, &[], Some(FILE)).expect("a briefing");
    let sent = crate::attachments::with_attachments(&briefing.prompt, &[FILE.to_owned()]);
    assert!(sent.chars().count() <= crate::native::MAX_PROMPT_LENGTH, "{}", sent.chars().count());
}

// case: a long conversation goes to another model by file > a short task stays inline, as it always was
#[test]
fn a_short_task_stays_inline_as_it_always_was() {
    let root = project();
    assert_eq!(long_task_file("Fix the typo in README.", &root.0, "mission_prior"), None);
    assert_eq!(long_task_file(&"x".repeat(TASK_INLINE_LIMIT), &root.0, "mission_prior"), None);
    assert_eq!(long_task_file_path(&"x".repeat(TASK_INLINE_LIMIT + 1), "mission_prior").as_deref(), Some(FILE));
    assert!(!root.0.join(".locust").exists());
}

// case: a long conversation goes to another model by file > a long task is written whole into .locust/attachments, kept out of git
#[test]
fn a_long_task_is_written_whole_into_locust_attachments_kept_out_of_git() {
    let root = project();
    std::fs::create_dir_all(root.0.join(".git").join("info")).unwrap();
    let task = long();
    assert_eq!(long_task_file(&task, &root.0, "mission_prior").as_deref(), Some(FILE));
    assert_eq!(std::fs::read_to_string(root.0.join(FILE)).unwrap(), format!("{task}\n"));
    assert!(std::fs::read_to_string(root.0.join(".git").join("info").join("exclude")).unwrap().contains(".locust/"));
}

// case: a long conversation goes to another model by file > never writes outside .locust/attachments for an id that is not one
#[test]
fn never_writes_outside_locust_attachments_for_an_id_that_is_not_one() {
    let root = project();
    assert_eq!(long_task_file(&long(), &root.0, "../escape"), None);
    assert_eq!(long_task_file_path(&long(), ""), None);
    assert_eq!(long_task_file_path(&long(), &"a".repeat(129)), None);
    assert!(long_task_file_path(&long(), &"a".repeat(128)).is_some());
    assert!(!root.0.join(".locust").exists());
}

#[test]
fn a_long_task_s_start_is_quoted_by_twelve_hundred_characters_then_named() {
    let briefing = compose_handoff_brief(&long(), &checkpoint(), "Codex CLI", Some("Go on."), &[], None, &[], Some(FILE)).unwrap();
    let start: String = long().chars().take(1_200).collect();
    assert!(briefing.prompt.contains(&format!("{}\u{2026}\n\n[That is only its start.", start.trim_end())));
    // Without a file the composer is as it was: beside a reply, the task gives up its end and says so.
    let clipped = compose_handoff_brief(&long(), &checkpoint(), "Codex CLI", Some("Go on."), &[], None, &[], None).unwrap();
    assert!(clipped.task_clipped && clipped.prompt.contains("did not fit here"));
}

#[test]
fn a_full_brief_still_leaves_room_for_the_line_that_hands_over_the_file() {
    // Whatever the reply's length, the brief and the file's line together fit what a run starts with, even when a
    // section is dropped and its notice said.
    let cp = json!({"unsettledActions": [], "settledActions": [], "assistantSummary": "s ".repeat(400)});
    for n in (5_600..7_000).step_by(3) {
        let reply = "y".repeat(n);
        let Some(briefing) = compose_handoff_brief(&long(), &cp, "Codex CLI", Some(&reply), &[], None, &[], Some(FILE)) else { continue };
        let sent = crate::attachments::with_attachments(&briefing.prompt, &[FILE.to_owned()]);
        assert!(sent.chars().count() <= crate::native::MAX_PROMPT_LENGTH, "reply of {n}: {}", sent.chars().count());
    }
}
