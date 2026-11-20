use super::*;

fn entry(id: &str, name: &str, role: &str) -> PeerEntry {
    PeerEntry { teammate_id: id.into(), name: name.into(), role: role.into() }
}

fn temp_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "locust-delivered-once-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    ))
}

#[test]
fn the_same_block_twice_in_one_reply_one_message_one_record_one_report_one_relay() {
    let root = temp_root();
    let workroom = Workroom::new(&root);
    let me = entry("tm_wren", "Wren", "Code & Migrations");
    let others = vec![entry("tm_atlas", "Atlas", "Research & Briefs")];
    let block = "<locust-share to=\"Atlas\">The build runs through pnpm check.</locust-share>";
    let reply = format!("Done.\n{block}\n{block}");

    let (posted, relayed, said) = share_reply_for_relay(&workroom, &me, &others, "mission_1", &reply, None);
    assert_eq!(posted.len(), 1);
    assert_eq!(workroom.read().messages.len(), 1);
    assert_eq!(relayed.len(), 1);
    assert!(said.is_empty());

    let other_root = temp_root();
    let other_workroom = Workroom::new(&other_root);
    let (posted, said) = share_reply(&other_workroom, &me, &others, "mission_2", &reply, None);
    assert_eq!(posted.len(), 1);
    assert!(said.is_empty());

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&other_root);
}

#[test]
fn the_same_reply_read_again_delivers_nothing_new() {
    let root = temp_root();
    let workroom = Workroom::new(&root);
    let me = entry("tm_wren", "Wren", "Code & Migrations");
    let others = vec![entry("tm_atlas", "Atlas", "Research & Briefs")];
    let block = "<locust-share to=\"Atlas\">The build runs through pnpm check.</locust-share>";
    let reply = format!("Done.\n{block}");

    let (posted, relayed, said) = share_reply_for_relay(&workroom, &me, &others, "mission_1", &reply, None);
    assert_eq!(posted.len(), 1);
    assert_eq!(relayed.len(), 1);
    assert!(said.is_empty());

    let (posted, relayed, said) = share_reply_for_relay(&workroom, &me, &others, "mission_1", &reply, None);
    assert_eq!(posted.len(), 1);
    assert_eq!(relayed.len(), 1);
    assert!(said.is_empty());
    assert_eq!(workroom.read().messages.len(), 1);

    let _ = std::fs::remove_dir_all(&root);
}
