//! Electron's `shared/memory.test.ts` (16 cases): the memory block a reply carries (`memory_ops`, which reading a reply
//! calls; `memory_text`, `key`, `strip_memory_blocks`) and the memory section a turn's brief carries (`memory_section`,
//! with `briefed_rows` saying where an everywhere memory was written). Electron's numbers written out: 4 lines a reply,
//! 300 characters a memory.
use super::*;
use serde_json::json;

fn op(verb: &str, text: &str) -> (String, Option<String>, String) {
    (verb.to_owned(), None, text.to_owned())
}
/// Electron's `memorySection` input: rows as the store keeps them, Wren's brief in the folder "shop".
fn section_in(folder: &str, memories: &[Value], ask_first: bool, file: Option<&str>) -> String {
    memory_section(Some("Wren"), Some(folder), memories, ask_first, false, chrono::Utc::now(), file, None)
}
fn row(text: &str, scope: &str, by: &str, written_in: Option<&str>) -> Value {
    let mut m = json!({"text": text, "scope": scope, "by": by});
    if let Some(w) = written_in {
        m["where"] = json!(w);
    }
    m
}
fn many(count: usize, scope: &str) -> Vec<Value> {
    (0..count).map(|i| row(&format!("remembered thing number {i}, long enough to be a real note about the project"), scope, "Wren", None)).collect()
}
fn section(memories: &[Value]) -> String {
    section_in("app", memories, false, None)
}
fn beta() -> String {
    memory_section(
        Some("Beta"),
        Some("locust-sample"),
        &[row("notes.txt ends with BETA-TOUCHED", "workspace", "Beta", Some("locust-sample"))],
        false,
        false,
        chrono::Utc::now(),
        None,
        None,
    )
}

// case: reads remember, remember everywhere and forget, one per line, in order
#[test]
fn a_memory_block_reads_remember_everywhere_and_forget_in_order() {
    let reply = [
        "Done. Tests pass.",
        "<locust-memory>",
        "remember :: Tests run with pnpm test, never npm.",
        "REMEMBER EVERYWHERE :: Colin wants diffs, not prose.",
        "forget :: The API is on port 3000",
        "shout :: not a verb",
        "",
        "</locust-memory>",
    ]
    .join("\n");
    assert_eq!(
        memory_ops(&reply),
        vec![
            op("remember", "Tests run with pnpm test, never npm."),
            op("remember everywhere", "Colin wants diffs, not prose."),
            op("forget", "The API is on port 3000"),
        ]
    );
}

// case: caps the operations per reply, bounds the text, and drops control bytes
#[test]
fn a_memory_block_is_capped_bounded_and_without_control_bytes() {
    let lines: Vec<String> = (0..7).map(|i| format!("remember :: memory {i}")).collect();
    assert_eq!(memory_ops(&format!("<locust-memory>\n{}\n</locust-memory>", lines.join("\n"))).len(), 4);
    assert_eq!(memory_text(&"x".repeat(340)).encode_utf16().count(), 300);
    assert_eq!(memory_text("a\u{0007}b   c\n"), "ab c");
}

// case: matches memories case- and punctuation-blind
#[test]
fn memories_match_case_and_punctuation_blind() {
    assert_eq!(key("Tests run with pnpm test, never npm."), key("tests run with PNPM test never npm"));
    assert_ne!(key("port 3000"), key("port 3001"));
}

// case: strips its blocks from a reply and defangs the tag in quoted text
#[test]
fn memory_blocks_are_stripped_and_defanged_in_quoted_text() {
    let reply = "Done.\n\n<locust-memory>\nremember :: a\n</locust-memory>";
    assert_eq!(crate::memory_tidy::strip_memory_blocks(reply), "Done.");
    assert_eq!(memory_ops(&crate::protocol_tags::defang_protocol_blocks(reply)), vec![]);
    assert_eq!(memory_ops("no block here"), vec![]);
}

// case: lists the memories with who wrote them and where, and teaches the block
#[test]
fn the_memory_section_lists_who_wrote_each_and_where_and_teaches_the_block() {
    let text = section_in(
        "shop",
        &[row("Tests run with pnpm test.", "workspace", "Juno", None), row("Colin wants diffs, not prose.", "global", "Wren", Some("ledger"))],
        false,
        None,
    );
    assert!(text.contains("\"shop\""), "{text}");
    assert!(text.contains("- Tests run with pnpm test. (this folder, by Juno)"), "{text}");
    assert!(text.contains("- Colin wants diffs, not prose. (everywhere, by Wren in ledger)"), "{text}");
    assert!(text.contains("<locust-memory>"), "{text}");
    assert!(text.contains("kept at once"), "{text}");
    assert!(!text.contains("asked before"), "{text}");
    // The brief's rows say where an everywhere memory was written, when that was another folder, and only then.
    let root = std::env::temp_dir().join(format!("locust-memory-where-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let here = workspace_id(&root.join("shop"));
    let kept = |id: &str, text: &str, ws: &str, name: &str| json!({"memoryId": id, "text": text, "scope": "global", "workspaceId": ws, "workspaceName": name, "by": {"name": "Wren"}, "createdAt": "2026-09-01T00:00:00.000Z", "status": "kept", "enabled": true});
    std::fs::write(
        root.join("electron").join("memories.json"),
        json!({"schemaVersion": 1, "memories": [kept("mem_a", "Colin wants diffs, not prose.", "ws_ledger", "ledger"), kept("mem_b", "Short answers.", &here, "shop")]}).to_string(),
    )
    .unwrap();
    let rows = briefed_rows(&store, &root.join("shop"), None).unwrap();
    let said = section_in("shop", &rows, false, None);
    assert!(said.contains("- Colin wants diffs, not prose. (everywhere, by Wren in ledger"), "{said}");
    assert!(said.contains("- Short answers. (everywhere, by Wren,"), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}

// case: says when the person is asked first, and that nothing is remembered yet
#[test]
fn the_memory_section_says_when_the_person_is_asked_first_and_that_nothing_is_kept() {
    let text = section_in("shop", &[], true, None);
    assert!(text.contains("Nothing is remembered yet."), "{text}");
    let yours = section_in("shop", &[row("Port is 3001.", "workspace", "you", None)], false, None);
    assert!(yours.contains("- Port is 3001. (this folder, by the person)"), "{yours}");
    assert!(text.contains("asked before a memory is kept"), "{text}");
    assert!(!text.contains("kept at once"), "{text}");
}

// case: says not to raise one that has nothing to do with the question
#[test]
fn the_memory_section_says_not_to_raise_an_unrelated_memory() {
    assert!(beta().contains("nothing to do with what was asked"));
}

// case: names the specific thing that went wrong: reporting another teammate’s work
#[test]
fn the_memory_section_names_reporting_another_teammates_work() {
    assert!(beta().contains("another teammate's work"));
}

// case: still tells it to answer from a memory that does fit
#[test]
fn the_memory_section_still_says_to_answer_from_a_memory_that_fits() {
    assert!(beta().contains("answer from it and say it came from memory"));
}

// case: puts a file ahead of a note copied from it
#[test]
fn the_memory_section_puts_a_file_ahead_of_a_note_copied_from_it() {
    let text = beta();
    assert!(text.contains("the file wins"), "{text}");
    assert!(text.contains("a value a file holds (a status, a colour, a version in README.md) is the file's to say"), "{text}");
}

// case: stays bounded however many are kept
#[test]
fn the_memory_section_stays_bounded_however_many_are_kept() {
    let big = section(&many(400, "workspace"));
    assert!(big.encode_utf16().count() < 8000, "{}", big.len());
    assert!(big.len() > section(&[]).len());
}

// case: says how many it left out, rather than quietly shortening
#[test]
fn the_memory_section_says_how_many_it_left_out() {
    let said = section(&many(60, "workspace"));
    assert!(said.contains("older memories are kept but not in this brief"), "{said}");
    assert!(said.contains("Ask the person if you need one"), "{said}");
}

// case: pastes fewer and names the file for the rest, when the whole list is a file in the workspace
#[test]
fn the_memory_section_pastes_fewer_and_names_the_file_for_the_rest() {
    let said = section_in("app", &many(33, "workspace"), false, Some(".locust/memory.md"));
    assert!(said.contains("25 older memories are in .locust/memory.md"), "{said}");
    assert!(said.contains("Read that file when the task touches something remembered"), "{said}");
    assert!(said.contains("never edit it"), "{said}");
    assert!(!said.contains("Ask the person if you need one"), "{said}");
    assert!(said.len() < section(&many(33, "workspace")).len());
    assert!(!section_in("app", &many(3, "workspace"), false, Some(".locust/memory.md")).contains("older mem"));
}

// case: says nothing about dropping when nothing was dropped
#[test]
fn the_memory_section_says_nothing_of_dropping_when_nothing_was_dropped() {
    assert!(!section(&many(3, "workspace")).contains("not in this brief"));
    assert!(!section(&[]).contains("not in this brief"));
    assert!(section(&[]).contains("Nothing is remembered yet."));
}

// case: keeps this folder ahead of everywhere
#[test]
fn the_memory_section_keeps_this_folder_ahead_of_everywhere() {
    let mut rows = many(30, "global");
    rows.extend(many(2, "workspace"));
    let said = section(&rows);
    assert!(said.contains("remembered thing number 0, long enough"), "{said}");
    assert!(said.contains("this folder"), "{said}");
}

// case: keeps the newest of a long list, not the oldest
#[test]
fn the_memory_section_keeps_the_newest_of_a_long_list() {
    let said = section(&many(40, "workspace"));
    assert!(said.contains("number 39"), "{said}");
    assert!(!said.contains("number 0,"), "{said}");
}
