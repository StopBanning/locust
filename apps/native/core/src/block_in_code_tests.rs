// Carries apps/desktop/src/shared/a-block-in-code-is-an-example.test.ts (Electron 4ce2f37a): a block inside code is an
// example, not an action. Electron's parseMemoryBlocks is people::memory_ops here.
use crate::{
    peer_share::parse_share_blocks,
    people::memory_ops,
    protocol_tags::{parse_decision, parse_file_blocks},
    room_task::parse_task_blocks,
};

fn fenced(block: &str) -> String {
    ["Here is how a teammate writes one:", "```", block, "```", "That is the whole protocol."].join("\n")
}

#[test]
fn a_share_block_inside_code_sends_no_message() {
    assert!(parse_share_blocks(&fenced(r#"<locust-share to="Pip">Please rerun the tests.</locust-share>"#)).is_empty());
    assert!(parse_share_blocks(r#"Write `<locust-share to="Pip">hi</locust-share>` at the end."#).is_empty());
}

#[test]
fn a_memory_block_inside_code_keeps_no_memory() {
    assert!(memory_ops(&fenced("<locust-memory>\nremember :: Tests run with pnpm test.\n</locust-memory>")).is_empty());
}

#[test]
fn a_task_ask_or_file_block_inside_code_moves_asks_and_hands_nothing() {
    assert!(parse_task_blocks(&fenced("<locust-task>\nclaim :: Write the release notes\n</locust-task>")).is_empty());
    assert!(parse_decision(&fenced("<locust-ask>\nWhich way?\n- Rewrite it :: slower\n- Patch it :: quicker\n</locust-ask>")).is_none());
    assert!(parse_file_blocks(&fenced("<locust-file>\ndocs/report.md :: the rollup\n</locust-file>")).is_empty());
}

#[test]
fn a_real_block_still_acts_with_the_code_it_quotes_left_whole() {
    let shares = parse_share_blocks("Done.\n<locust-share to=\"Pip\">Run `pnpm test` in apps/desktop, then tell me.</locust-share>");
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].text, "Run `pnpm test` in apps/desktop, then tell me.");
    assert_eq!(
        memory_ops("Done.\n<locust-memory>\nremember :: Tests run with `pnpm test`, never npm.\n</locust-memory>"),
        [("remember".to_owned(), None, "Tests run with `pnpm test`, never npm.".to_owned())]
    );
}

#[test]
fn a_real_block_acts_after_an_example_in_the_same_reply() {
    let reply = [fenced(r#"<locust-share to="Pip">An example.</locust-share>"#), r#"<locust-share to="Gem">The real one.</locust-share>"#.to_owned()].join("\n");
    assert_eq!(parse_share_blocks(&reply).iter().map(|s| s.to.as_str()).collect::<Vec<_>>(), ["Gem"]);
}
