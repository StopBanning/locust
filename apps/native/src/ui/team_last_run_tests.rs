//! Words 1, row 2: Team cards said "Runs 167 · Last run never" (Electron Screens.tsx:795, the same in Rust). Runs counts
//! every conversation the record gives a teammate; Last run was read only from the conversations the list shows, which
//! leaves out Scout's looks and side chats. Drawn through the Team screen with a real store: a teammate with runs shows
//! how long ago its newest record was written, "never" is said only with no runs, and runs whose records are gone say
//! "unknown", not "never".
use super::*;
use locust_core::store::Store;
use std::time::{Duration, SystemTime};

fn store(label: &str) -> (std::path::PathBuf, Store) {
    let root = std::env::temp_dir().join(format!("locust-team-last-run-{label}-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    (root, store)
}

/// A record for `id`, last written `ago` before now.
fn record(dir: &std::path::Path, id: &str, ago: Duration) {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{id}.jsonl"));
    std::fs::write(&path, "{}\n").unwrap();
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(SystemTime::now() - ago).unwrap();
}

fn team(owners: Value) -> Value {
    json!({
        "schemaVersion": 1,
        "teammates": [
            {"teammateId": "tm_scout", "name": "Scout", "hue": "lime", "role": "Scout", "createdAt": "2026-09-01T00:00:00.000Z"},
            {"teammateId": "tm_wren", "name": "Wren", "hue": "teal", "role": "Code & Migrations", "createdAt": "2026-09-01T00:00:00.000Z"},
            {"teammateId": "tm_newt", "name": "Newt", "hue": "rose", "role": "Custom", "createdAt": "2026-09-01T00:00:00.000Z"}
        ],
        "missionOwners": owners,
        "settings": {}
    })
}

/// The Team screen opened as a person opens it, over `store`.
fn team_screen(store: &Store, summaries: Vec<locust_core::ledger::Summary>) -> rune::Runtime<State> {
    let mut s = State::default();
    s.data = Some(store.clone());
    for m in summaries {
        s.ledger_summaries.insert(m.mission_id.clone(), m);
    }
    open(&mut s);
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1400.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// The texts drawn inside the element with this key, as a person reads it.
fn text_in(rt: &rune::Runtime<State>, key: &str) -> String {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[test]
fn a_teammate_whose_runs_are_not_listed_shows_how_long_ago_its_newest_record_was_written_not_never() {
    let (root, store) = store("scout");
    let ledger = store.ledger_dir().unwrap();
    record(&ledger, "look_1", Duration::from_secs(3 * 86_400));
    record(&ledger, "look_2", Duration::from_secs(2 * 3_600 + 60));
    // Scout's looks are never listed as conversations: nothing for them in the window's summaries.
    store.save_json("teammates.json", &team(json!({"look_1": "tm_scout", "look_2": "tm_scout"}))).unwrap();
    let rt = team_screen(&store, vec![]);
    let card = text_in(&rt, "team-card-tm_scout");
    assert!(card.contains("Runs | 2"), "{card}");
    assert!(card.contains("Last run | 2 hours ago"), "{card}");
    assert!(!card.contains("never"), "a card with runs said never: {card}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reads_electrons_records_too_and_a_listed_conversation_newer_than_the_records_wins() {
    let (root, store) = store("electron");
    record(&store.electron().join("mission-ledger"), "m_old", Duration::from_secs(5 * 86_400));
    store.save_json("teammates.json", &team(json!({"m_old": "tm_wren", "m_listed": "tm_scout"}))).unwrap();
    let listed = locust_core::ledger::Summary {
        mission_id: "m_listed".into(),
        updated: (chrono::Utc::now() - chrono::Duration::minutes(30)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        ..Default::default()
    };
    let rt = team_screen(&store, vec![listed]);
    assert!(text_in(&rt, "team-card-tm_wren").contains("Last run | 5 days ago"), "{}", text_in(&rt, "team-card-tm_wren"));
    assert!(text_in(&rt, "team-card-tm_scout").contains("Last run | 30 minutes ago"), "{}", text_in(&rt, "team-card-tm_scout"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn never_only_with_no_runs_and_runs_whose_records_are_gone_say_unknown() {
    let (root, store) = store("gone");
    store.save_json("teammates.json", &team(json!({"m_trashed": "tm_wren"}))).unwrap();
    let rt = team_screen(&store, vec![]);
    let wren = text_in(&rt, "team-card-tm_wren");
    assert!(wren.contains("Runs | 1") && wren.contains("Last run | unknown"), "{wren}");
    let newt = text_in(&rt, "team-card-tm_newt");
    assert!(newt.contains("Runs | 0") && newt.contains("Last run | never"), "{newt}");
    let _ = std::fs::remove_dir_all(root);
}
