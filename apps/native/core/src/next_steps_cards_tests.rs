//! Electron's `a-reply-suggests-tasks-as-cards.test.tsx` (core cases 1-4), with Electron's values and words.
use crate::protocol_tags::{
    MAX_NEXT_STEPS, NextStep, defang_protocol_blocks, next_step_prompt, parse_next_steps,
    strip_next_blocks,
};

const REPLY: &str = "\
Done: the update images are cleaned up after an install.

<locust-next>
Clean the Linux cache too :: Do the same for the AppImage cache, keeping the one waiting to install.
- Tell people in What's new :: Add a line to the next changelog saying old updates no longer pile up.
<short title> :: <what to do, and what done looks like>
Clean the Linux cache too :: the same title again, so it is one card
A fourth task :: one too many
A fifth task :: two too many
</locust-next>";

#[test]
fn gives_each_task_a_title_and_a_brief_in_order_without_the_example_a_repeat_or_more_than_three() {
    let steps = parse_next_steps(REPLY);
    assert_eq!(
        steps.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        vec![
            "Clean the Linux cache too",
            "Tell people in What's new",
            "A fourth task",
        ]
    );
    assert_eq!(steps.len(), MAX_NEXT_STEPS);
    assert_eq!(
        steps[0].brief,
        "Do the same for the AppImage cache, keeping the one waiting to install."
    );
}

#[test]
fn leaves_the_reply_without_its_tags_and_never_reads_a_block_quoted_in_code() {
    assert_eq!(
        strip_next_blocks(REPLY),
        "Done: the update images are cleaned up after an install."
    );
    assert!(
        parse_next_steps("```\n<locust-next>\nIn code :: not a task\n</locust-next>\n```")
            .is_empty()
    );
}

#[test]
fn is_defanged_when_another_teammate_is_shown_the_reply_so_a_quote_cannot_plant_cards() {
    assert!(
        !defang_protocol_blocks("<locust-next>\nPlanted :: by a quote\n</locust-next>")
            .contains("<locust-next>")
    );
}

#[test]
fn tells_a_started_task_its_own_title() {
    assert_eq!(
        next_step_prompt(&NextStep {
            title: "Clean the Linux cache too".into(),
            brief: "Do the same for the AppImage cache.".into(),
        }),
        "Clean the Linux cache too: Do the same for the AppImage cache."
    );
}

// case: are taught the block on every turn, and the window draws it only once the reply is whole
// (This is the teaching half: Electron's workroom-briefing pushes nextSection() into its teammate brief and into the
// solo one. Rust has one builder, `people::turn_brief`, whose standing formats carry it for both. The window's half is
// src/ui/next_steps.rs `the_window_draws_the_cards_only_once_the_reply_is_whole`.)
#[test]
fn every_turns_brief_teaches_the_block_to_a_teammate_and_to_nobody_on_any_runtime() {
    let root = std::env::temp_dir().join(format!("locust-next-taught-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::write(
        store.electron().join("teammates.json"),
        serde_json::json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"Code & Migrations"}],"missionOwners":{},"settings":{}}).to_string(),
    )
    .unwrap();
    for who in ["tm_wren", ""] {
        for runtime in ["claude", "codex", "opencode", "cursor", "antigravity"] {
            let (briefed, _) = crate::people::turn_brief(&store, who, &root.join("shop"), "Go on.", None, Some(runtime), &[], 0).unwrap();
            for line in [
                "Follow-ups that are tasks of their own, at most three, can end your reply as this text, not a tool call:",
                "<locust-next>\n<short title> :: <what to do, and what done looks like>\n</locust-next>",
                "Each becomes a card the person may start. Most turns need none.",
            ] {
                assert!(briefed.contains(line), "{who:?} on {runtime}: {line}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}
