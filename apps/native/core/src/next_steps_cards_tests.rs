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
