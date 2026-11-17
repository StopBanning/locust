// Carries apps/desktop/src/shared/the-example-file-is-never-a-file.test.ts (Electron 4ce2f37a).
use crate::protocol_tags::{FILE_BLOCK_EXAMPLE_PATH, parse_file_blocks};

#[test]
fn a_file_block_ignores_the_briefing_example_and_keeps_a_real_file_beside_it() {
    let reply = format!("Done.\n<locust-file>\n{FILE_BLOCK_EXAMPLE_PATH} :: what it is, in a few words\nindex.html :: the todo page\n</locust-file>");
    assert_eq!(parse_file_blocks(&reply).iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["index.html"]);
}
