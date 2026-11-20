//! Electron's `block-placement.test.ts` (4 cases): briefings never tell a model a block must be the last thing in its
//! reply; several blocks may follow one another, in any order, and every briefing that asks for one says so the same way.
use super::BLOCK_PLACEMENT;

/// Electron's four sources, as Rust has them: the workroom brief, the memory, the room's board, the relay.
const SOURCES: [&str; 4] = ["briefing.rs", "people.rs", "room_task.rs", "relay.rs"];

fn read(file: &str) -> String {
    let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file)).unwrap_or_default();
    // What the program says, not what its test modules check: the file up to its first test module.
    let cut = ["\n#[cfg(test)]\nmod ", "\n#[cfg(test)]\n#[path"].iter().filter_map(|m| text.find(m)).min().unwrap_or(text.len());
    text[..cut].to_owned()
}

#[test]
fn finds_the_briefings_so_a_pass_means_something() {
    for file in SOURCES {
        assert!(read(file).len() > 200, "{file}");
    }
}

#[test]
fn never_says_a_block_must_be_the_last_thing_in_the_reply() {
    for file in SOURCES {
        assert!(!read(file).to_lowercase().contains("nothing after it"), "{file}");
    }
}

#[test]
fn says_instead_that_several_blocks_may_follow_one_another() {
    assert!(BLOCK_PLACEMENT.to_lowercase().contains("one after another"));
    assert!(BLOCK_PLACEMENT.to_lowercase().contains("any order"));
}

#[test]
fn is_used_by_every_briefing_that_asks_for_a_block() {
    for file in ["briefing.rs", "people.rs", "room_task.rs"] {
        assert!(read(file).contains("BLOCK_PLACEMENT"), "{file}");
    }
}
